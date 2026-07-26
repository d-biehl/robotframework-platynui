//! Process-wide registry of native top-level windows claimed by a provider.
//!
//! More than one provider can surface the same native window: a Java Swing
//! window is enumerated by the generic Windows UIA provider (as an empty
//! shell, since Swing implements no UIA provider) *and* by the Java provider
//! (with a full subtree). To show each window exactly once in the merged
//! desktop tree, the provider that genuinely represents the window registers a
//! claim on its native handle here, and generic providers consult the registry
//! during root streaming and skip windows claimed by someone else.
//!
//! The registry is process-wide (all runtimes in a process see the same
//! desktop reality) and reference-counted per `(window, provider)` pair, so
//! multiple runtime instances of the same provider can claim and release
//! independently without stealing each other's claims.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static CLAIMS: LazyLock<Mutex<HashMap<u64, HashMap<&'static str, usize>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn claims() -> std::sync::MutexGuard<'static, HashMap<u64, HashMap<&'static str, usize>>> {
    CLAIMS.lock().expect("window claims mutex poisoned")
}

/// Register a claim of `provider_id` on the native window `raw` (e.g. the
/// HWND value). Claims are counted: every `claim_window` needs a matching
/// [`release_window`].
pub fn claim_window(raw: u64, provider_id: &'static str) {
    let mut map = claims();
    *map.entry(raw).or_default().entry(provider_id).or_insert(0) += 1;
}

/// Release one claim of `provider_id` on `raw`. Releasing a window that was
/// never claimed is a no-op.
pub fn release_window(raw: u64, provider_id: &'static str) {
    let mut map = claims();
    if let Some(owners) = map.get_mut(&raw) {
        if let Some(count) = owners.get_mut(provider_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                owners.remove(provider_id);
            }
        }
        if owners.is_empty() {
            map.remove(&raw);
        }
    }
}

/// Whether `raw` is currently claimed by any provider other than
/// `provider_id`. This is the question a generic provider asks during root
/// streaming: "is someone else representing this window?"
pub fn is_claimed_by_other(raw: u64, provider_id: &str) -> bool {
    claims().get(&raw).is_some_and(|owners| owners.keys().any(|owner| *owner != provider_id))
}

/// Whether `raw` is claimed by anyone at all.
pub fn is_claimed(raw: u64) -> bool {
    claims().get(&raw).is_some_and(|owners| !owners.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The registry is process-global, so tests use distinct window values to
    // stay independent of each other and of test-ordering.

    #[test]
    fn unclaimed_window_is_free_for_everyone() {
        assert!(!is_claimed(0xA100));
        assert!(!is_claimed_by_other(0xA100, "uia"));
    }

    #[test]
    fn claim_is_visible_to_other_providers_only() {
        claim_window(0xA200, "java");
        assert!(is_claimed(0xA200));
        assert!(is_claimed_by_other(0xA200, "uia"), "another provider must see the claim");
        assert!(!is_claimed_by_other(0xA200, "java"), "the owner itself is not 'other'");
        release_window(0xA200, "java");
        assert!(!is_claimed(0xA200));
    }

    #[test]
    fn claims_are_reference_counted_per_provider() {
        claim_window(0xA300, "java");
        claim_window(0xA300, "java"); // second runtime instance
        release_window(0xA300, "java");
        assert!(is_claimed_by_other(0xA300, "uia"), "one instance still holds a claim");
        release_window(0xA300, "java");
        assert!(!is_claimed(0xA300));
    }

    #[test]
    fn releasing_never_claimed_windows_is_a_noop() {
        release_window(0xA400, "java");
        assert!(!is_claimed(0xA400));
    }

    #[test]
    fn distinct_windows_do_not_interfere() {
        claim_window(0xA500, "java");
        assert!(!is_claimed(0xA501));
        release_window(0xA500, "java");
    }
}
