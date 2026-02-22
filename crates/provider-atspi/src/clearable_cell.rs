//! Thread-safe cache cell that supports clearing.
//!
//! [`ClearableCell`] is a simple `Mutex<Option<T>>` wrapper that provides
//! first-writer-wins semantics and allows resetting the cached value so
//! subsequent reads re-resolve from the underlying data source.

use std::sync::Mutex;

/// Thread-safe cache cell that supports clearing via [`ClearableCell::clear`].
///
/// Unlike [`std::sync::OnceLock`], stored values can be reset so that
/// the next access re-queries the underlying data source.  Values are cloned
/// on read to avoid holding the internal lock across potentially long D-Bus
/// calls.
pub(crate) struct ClearableCell<T>(Mutex<Option<T>>);

impl<T> ClearableCell<T> {
    /// Creates an empty (unset) cell.
    pub(crate) fn new() -> Self {
        Self(Mutex::new(None))
    }

    /// Stores `value` if the cell is currently empty.  If the cell already
    /// holds a value the call is a no-op (first-writer-wins semantics).
    pub(crate) fn set(&self, value: T) {
        let mut guard = self.0.lock().expect("ClearableCell lock poisoned");
        if guard.is_none() {
            *guard = Some(value);
        }
    }

    /// Resets the cell to the empty state so that subsequent reads will
    /// re-resolve the value.
    pub(crate) fn clear(&self) {
        *self.0.lock().expect("ClearableCell lock poisoned") = None;
    }

    /// Returns `true` if the cell currently holds a value.
    pub(crate) fn is_set(&self) -> bool {
        self.0.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl<T: Clone> ClearableCell<T> {
    /// Returns a clone of the stored value, or `None` if the cell is empty.
    pub(crate) fn get(&self) -> Option<T> {
        self.0.lock().ok()?.clone()
    }

    /// Returns the stored value (cloned), initialising it with `f()` first
    /// if the cell is empty.  The initialiser runs without holding the lock
    /// so that long-running D-Bus calls do not block other threads.
    pub(crate) fn get_or_init(&self, f: impl FnOnce() -> T) -> T {
        // Fast path: already cached.
        {
            let guard = self.0.lock().expect("ClearableCell lock poisoned");
            if let Some(value) = guard.as_ref() {
                return value.clone();
            }
        }
        // Slow path: compute without holding the lock.
        let value = f();
        let mut guard = self.0.lock().expect("ClearableCell lock poisoned");
        if let Some(existing) = guard.as_ref() {
            // Another caller initialised it in the meantime.
            return existing.clone();
        }
        *guard = Some(value.clone());
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let cell: ClearableCell<i32> = ClearableCell::new();
        assert!(!cell.is_set());
        assert!(cell.get().is_none());
    }

    #[test]
    fn set_and_get() {
        let cell = ClearableCell::new();
        cell.set(42);
        assert!(cell.is_set());
        assert_eq!(cell.get(), Some(42));
    }

    #[test]
    fn set_is_first_writer_wins() {
        let cell = ClearableCell::new();
        cell.set(1);
        cell.set(2);
        assert_eq!(cell.get(), Some(1));
    }

    #[test]
    fn clear_resets() {
        let cell = ClearableCell::new();
        cell.set(42);
        assert!(cell.is_set());
        cell.clear();
        assert!(!cell.is_set());
        assert!(cell.get().is_none());
    }

    #[test]
    fn get_or_init_lazy() {
        let cell: ClearableCell<String> = ClearableCell::new();
        let value = cell.get_or_init(|| "hello".to_string());
        assert_eq!(value, "hello");
        // Second call returns cached value, not re-initialized.
        let value2 = cell.get_or_init(|| "world".to_string());
        assert_eq!(value2, "hello");
    }

    #[test]
    fn get_or_init_after_clear() {
        let cell: ClearableCell<i32> = ClearableCell::new();
        cell.set(1);
        cell.clear();
        let value = cell.get_or_init(|| 99);
        assert_eq!(value, 99);
    }

    #[test]
    fn threaded_set() {
        use std::sync::Arc;
        let cell = Arc::new(ClearableCell::new());
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let cell = cell.clone();
                std::thread::spawn(move || {
                    cell.set(i);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // Exactly one writer should have won.
        assert!(cell.is_set());
        let val = cell.get().unwrap();
        assert!((0..10).contains(&val));
    }
}
