//! Which query is running on this thread, so that a provider can share a value
//! between the nodes of one query without keeping it for the next.

use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static CURRENT: Cell<Option<Evaluation>> = const { Cell::new(None) };
}

/// One query of the UI tree. A value a provider caches under it is valid only
/// while the query runs, and only on the thread it runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evaluation(u64);

impl Evaluation {
    /// A query that no other query shares.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// The query running on this thread, if any.
    #[must_use]
    pub fn current() -> Option<Self> {
        CURRENT.with(Cell::get)
    }

    /// Marks this query as running on this thread until the guard drops.
    #[must_use]
    pub fn enter(self) -> EvaluationGuard {
        EvaluationGuard { previous: CURRENT.with(|current| current.replace(Some(self))), _thread: PhantomData }
    }
}

impl Default for Evaluation {
    fn default() -> Self {
        Self::new()
    }
}

/// Restores the query that ran before [`Evaluation::enter`].
pub struct EvaluationGuard {
    previous: Option<Evaluation>,
    _thread: PhantomData<*const ()>,
}

impl Drop for EvaluationGuard {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.previous));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_current_only_while_entered() {
        assert_eq!(Evaluation::current(), None);
        let outer = Evaluation::new();
        {
            let _outer = outer.enter();
            assert_eq!(Evaluation::current(), Some(outer));
            let inner = Evaluation::new();
            {
                let _inner = inner.enter();
                assert_eq!(Evaluation::current(), Some(inner));
            }
            assert_eq!(Evaluation::current(), Some(outer));
        }
        assert_eq!(Evaluation::current(), None);
    }

    #[test]
    fn every_query_is_new() {
        assert_ne!(Evaluation::new(), Evaluation::new());
    }
}
