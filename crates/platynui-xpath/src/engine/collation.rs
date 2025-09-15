use crate::engine::runtime::{DynamicContext, Error, ErrorCode};
use std::collections::HashMap;
use std::sync::Arc;

pub trait Collation: Send + Sync {
    fn uri(&self) -> &str;
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering;
    fn key(&self, s: &str) -> String {
        s.to_string()
    }
}

pub use crate::consts::CODEPOINT_URI;
pub use crate::consts::SIMPLE_ACCENT_URI;
pub use crate::consts::SIMPLE_CASE_ACCENT_URI;
pub use crate::consts::SIMPLE_CASE_URI;

#[derive(Clone)]
pub enum CollationKind {
    Codepoint(Arc<dyn Collation>),
    Other(Arc<dyn Collation>),
}

impl CollationKind {
    pub fn as_trait(&self) -> &dyn Collation {
        match self {
            CollationKind::Codepoint(c) => c.as_ref(),
            CollationKind::Other(c) => c.as_ref(),
        }
    }
}

pub fn resolve_collation<N>(
    dyn_ctx: &DynamicContext<N>,
    default_collation: Option<&Arc<dyn Collation>>,
    uri: Option<&str>,
) -> Result<CollationKind, Error> {
    let arc = if let Some(u) = uri {
        if let Some(c) = dyn_ctx.collations.get(u) {
            c
        } else {
            return Err(Error::from_code(
                ErrorCode::FOCH0002,
                format!("unknown collation URI: {u}"),
            ));
        }
    } else if let Some(c) = default_collation {
        c.clone()
    } else {
        dyn_ctx
            .collations
            .get(CODEPOINT_URI)
            .expect("codepoint collation registered")
    };
    if arc.uri() == CODEPOINT_URI {
        Ok(CollationKind::Codepoint(arc))
    } else {
        Ok(CollationKind::Other(arc))
    }
}

pub struct CodepointCollation;

impl Collation for CodepointCollation {
    fn uri(&self) -> &str {
        CODEPOINT_URI
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        a.cmp(b)
    }
}

/// Simple case-insensitive collation
pub struct SimpleCaseCollation;

impl Collation for SimpleCaseCollation {
    fn uri(&self) -> &str {
        SIMPLE_CASE_URI
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        self.key(a).cmp(&self.key(b))
    }
    fn key(&self, s: &str) -> String {
        s.to_lowercase()
    }
}

/// Simple accent-insensitive collation (NFD + remove combining marks)
pub struct SimpleAccentCollation;

impl Collation for SimpleAccentCollation {
    fn uri(&self) -> &str {
        SIMPLE_ACCENT_URI
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        self.key(a).cmp(&self.key(b))
    }
    fn key(&self, s: &str) -> String {
        use unicode_normalization::UnicodeNormalization;
        use unicode_normalization::char::canonical_combining_class as ccc;
        s.nfd().filter(|&ch| ccc(ch) == 0).collect()
    }
}

/// Simple case+accent-insensitive collation
pub struct SimpleCaseAccentCollation;

impl Collation for SimpleCaseAccentCollation {
    fn uri(&self) -> &str {
        SIMPLE_CASE_ACCENT_URI
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        self.key(a).cmp(&self.key(b))
    }
    fn key(&self, s: &str) -> String {
        use unicode_normalization::UnicodeNormalization;
        use unicode_normalization::char::canonical_combining_class as ccc;
        let no_marks: String = s.nfd().filter(|&ch| ccc(ch) == 0).collect();
        no_marks.to_lowercase()
    }
}

/// Registry of available collations, keyed by their URI
pub struct CollationRegistry {
    by_uri: HashMap<String, Arc<dyn Collation>>,
}

impl Default for CollationRegistry {
    fn default() -> Self {
        let mut reg = Self {
            by_uri: HashMap::new(),
        };
        let def: Arc<dyn Collation> = Arc::new(CodepointCollation);
        reg.by_uri.insert(def.uri().to_string(), def);
        // Built-in simple collations
        reg.by_uri
            .insert(SIMPLE_CASE_URI.to_string(), Arc::new(SimpleCaseCollation));
        reg.by_uri.insert(
            SIMPLE_ACCENT_URI.to_string(),
            Arc::new(SimpleAccentCollation),
        );
        reg.by_uri.insert(
            SIMPLE_CASE_ACCENT_URI.to_string(),
            Arc::new(SimpleCaseAccentCollation),
        );
        reg
    }
}

impl CollationRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, uri: &str) -> Option<Arc<dyn Collation>> {
        self.by_uri.get(uri).cloned()
    }
    pub fn insert(&mut self, collation: Arc<dyn Collation>) {
        self.by_uri.insert(collation.uri().to_string(), collation);
    }
}
