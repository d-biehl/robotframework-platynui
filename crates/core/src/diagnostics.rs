//! Building blocks for diagnostics that do not depend on a logging framework.
//!
//! `platynui-core` stays free of `tracing` (see `dev-docs/error-handling.md`),
//! so these types return facts and leave the logging to the call site. The
//! rules they serve are in `dev-docs/logging.md`.

use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Mutex, PoisonError};

/// Where a failure stands in its subject's episode, as [`Transitions::failed`] reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Episode {
    /// The subject was not failing before: report the failure at warn or error.
    Started,
    /// The subject was already failing: record the failure at debug.
    Continuing,
}

/// Records which subjects are currently failing, so that a condition that can
/// recur on every call is reported once per episode.
///
/// The latch logs nothing. Its owner decides what a subject is (a provider, an
/// application instance, a process), logs on the facts it returns, and defines
/// what ends an episode: [`recovered`](Self::recovered) when the subject works
/// again, [`retain`](Self::retain) when the subject is gone. The end of an
/// episode is recorded once at debug.
#[derive(Debug)]
pub struct Transitions<K> {
    failing: Mutex<HashSet<K>>,
}

impl<K> Default for Transitions<K> {
    fn default() -> Self {
        Self { failing: Mutex::new(HashSet::new()) }
    }
}

impl<K: Eq + Hash> Transitions<K> {
    /// An empty latch: no subject is failing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a failure of `key`, and whether it starts an episode.
    ///
    /// Of several threads that report the first failure at the same time,
    /// exactly one gets [`Episode::Started`].
    pub fn failed<Q>(&self, key: &Q) -> Episode
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ToOwned<Owned = K> + ?Sized,
    {
        let mut failing = self.failing.lock().unwrap_or_else(PoisonError::into_inner);
        if failing.contains(key) {
            Episode::Continuing
        } else {
            failing.insert(key.to_owned());
            Episode::Started
        }
    }

    /// Ends the episode of `key`. Returns `true` when `key` was failing, so the
    /// owner records the recovery once.
    pub fn recovered<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.failing.lock().unwrap_or_else(PoisonError::into_inner).remove(key)
    }

    /// Forgets the failing subjects for which `keep` returns `false`, such as
    /// subjects that no longer exist, and returns them, so the owner records
    /// the end of their episodes.
    pub fn retain(&self, mut keep: impl FnMut(&K) -> bool) -> Vec<K> {
        let mut failing = self.failing.lock().unwrap_or_else(PoisonError::into_inner);
        let (kept, forgotten): (HashSet<K>, Vec<K>) =
            failing.drain().fold((HashSet::new(), Vec::new()), |(mut kept, mut forgotten), key| {
                if keep(&key) {
                    kept.insert(key);
                } else {
                    forgotten.push(key);
                }
                (kept, forgotten)
            });
        *failing = kept;
        forgotten
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn the_first_failure_starts_an_episode_and_later_ones_continue_it() {
        let latch = Transitions::<String>::new();
        assert_eq!(latch.failed("atspi"), Episode::Started);
        assert_eq!(latch.failed("atspi"), Episode::Continuing);
        assert_eq!(latch.failed("atspi"), Episode::Continuing);
    }

    #[test]
    fn recovery_rearms_and_is_false_for_a_key_that_never_failed() {
        let latch = Transitions::<String>::new();
        assert!(!latch.recovered("atspi"), "a key that never failed does not recover");
        latch.failed("atspi");
        assert!(latch.recovered("atspi"));
        assert!(!latch.recovered("atspi"), "a recovery is reported once");
        assert_eq!(latch.failed("atspi"), Episode::Started, "a failure after recovery starts a new episode");
    }

    #[test]
    fn keys_are_independent() {
        let latch = Transitions::<u32>::new();
        assert_eq!(latch.failed(&1), Episode::Started);
        assert_eq!(latch.failed(&2), Episode::Started);
        assert!(latch.recovered(&1));
        assert_eq!(latch.failed(&2), Episode::Continuing);
    }

    #[test]
    fn retain_forgets_keys_and_returns_them() {
        let latch = Transitions::<String>::new();
        latch.failed(":1.7");
        latch.failed(":1.8");
        let forgotten = latch.retain(|key| key != ":1.7");
        assert_eq!(forgotten, [":1.7".to_owned()]);
        assert_eq!(latch.failed(":1.7"), Episode::Started, "a forgotten key starts a new episode");
        assert_eq!(latch.failed(":1.8"), Episode::Continuing, "a kept key stays in its episode");
    }

    #[test]
    fn concurrent_failures_start_exactly_one_episode() {
        let latch = Arc::new(Transitions::<String>::new());
        let barrier = Arc::new(Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let latch = Arc::clone(&latch);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    latch.failed("atspi")
                })
            })
            .collect();
        let started = threads.into_iter().map(|t| t.join().unwrap()).filter(|e| *e == Episode::Started).count();
        assert_eq!(started, 1);
    }

    #[test]
    fn borrowed_keys_are_accepted() {
        let latch = Transitions::<String>::new();
        let name: &str = "atspi";
        assert_eq!(latch.failed(name), Episode::Started);
        assert!(latch.recovered(name));
    }
}
