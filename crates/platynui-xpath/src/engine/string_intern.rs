use string_cache::DefaultAtom;

/// Lightweight accessors that forward to the global `string_cache` interner.
/// The helpers exist to keep the previous crate-local API stable while
/// removing the bespoke caching layer.
pub fn intern_name(name: &str) -> DefaultAtom {
    DefaultAtom::from(name)
}

pub fn intern_function(name: &str) -> DefaultAtom {
    DefaultAtom::from(name)
}

pub fn intern_namespace(uri: &str) -> DefaultAtom {
    DefaultAtom::from(uri)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub name_cache_size: usize,
    pub function_cache_size: usize,
    pub namespace_cache_size: usize,
}

/// Placeholder stats – the underlying global interner does not expose counts.
pub fn cache_stats() -> CacheStats {
    CacheStats::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_returns_same_atom() {
        let a1 = intern_name("div");
        let a2 = intern_name("div");
        assert_eq!(a1, a2);
        assert_eq!(a1.as_ref(), "div");
    }

    #[test]
    fn namespace_interning_reuses_atom() {
        let a1 = intern_namespace("http://example.com/ns");
        let a2 = intern_namespace("http://example.com/ns");
        assert_eq!(a1, a2);
    }

    #[test]
    fn cache_stats_are_zero() {
        let stats = cache_stats();
        assert_eq!(stats.name_cache_size, 0);
        assert_eq!(stats.function_cache_size, 0);
        assert_eq!(stats.namespace_cache_size, 0);
    }
}
