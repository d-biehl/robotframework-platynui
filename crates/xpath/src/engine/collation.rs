use crate::engine::runtime::{DynamicContext, Error, ErrorCode};
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

pub trait Collation {
    fn uri(&self) -> &str;
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering;
    fn key<'a>(&self, s: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(s)
    }
}

pub use crate::consts::CODEPOINT_URI;
pub use crate::consts::SIMPLE_ACCENT_URI;
pub use crate::consts::SIMPLE_CASE_ACCENT_URI;
pub use crate::consts::SIMPLE_CASE_URI;

#[derive(Clone)]
pub enum CollationKind {
    Codepoint(Rc<dyn Collation>),
    Other(Rc<dyn Collation>),
}

impl CollationKind {
    #[must_use]
    pub fn as_trait(&self) -> &dyn Collation {
        match self {
            CollationKind::Codepoint(c) | CollationKind::Other(c) => c.as_ref(),
        }
    }
}

/// Resolve the collation to use for an operation.
///
/// An explicit `uri` wins; otherwise `default_collation` is used, and failing
/// that the codepoint collation.
///
/// # Errors
///
/// Returns `FOCH0002` if `uri` is given but no collation with that URI is
/// registered in the dynamic context.
pub fn resolve_collation<N>(
    dyn_ctx: &DynamicContext<N>,
    default_collation: Option<&Rc<dyn Collation>>,
    uri: Option<&str>,
) -> Result<CollationKind, Error> {
    let arc = if let Some(u) = uri {
        if let Some(c) = dyn_ctx.collations.get(u) {
            c
        } else {
            return Err(Error::from_code(ErrorCode::FOCH0002, format!("unknown collation URI: {u}")));
        }
    } else if let Some(c) = default_collation {
        c.clone()
    } else {
        // Fallback to codepoint collation; registry should contain it, but don't panic if not.
        dyn_ctx.collations.get(CODEPOINT_URI).unwrap_or_else(|| Rc::new(CodepointCollation))
    };
    if arc.uri() == CODEPOINT_URI { Ok(CollationKind::Codepoint(arc)) } else { Ok(CollationKind::Other(arc)) }
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
    fn key<'a>(&self, s: &'a str) -> Cow<'a, str> {
        Cow::Owned(s.to_lowercase())
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
    fn key<'a>(&self, s: &'a str) -> Cow<'a, str> {
        use unicode_normalization::UnicodeNormalization;
        use unicode_normalization::char::canonical_combining_class as ccc;
        Cow::Owned(s.nfd().filter(|&ch| ccc(ch) == 0).collect())
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
    fn key<'a>(&self, s: &'a str) -> Cow<'a, str> {
        use unicode_normalization::UnicodeNormalization;
        use unicode_normalization::char::canonical_combining_class as ccc;
        let no_marks: String = s.nfd().filter(|&ch| ccc(ch) == 0).collect();
        Cow::Owned(no_marks.to_lowercase())
    }
}

/// Registry of available collations, keyed by their URI
pub struct CollationRegistry {
    by_uri: HashMap<String, Rc<dyn Collation>>,
}

impl Default for CollationRegistry {
    fn default() -> Self {
        let mut reg = Self { by_uri: HashMap::new() };
        let def: Rc<dyn Collation> = Rc::new(CodepointCollation);
        reg.by_uri.insert(def.uri().to_string(), def);
        // Built-in simple collations
        reg.by_uri.insert(SIMPLE_CASE_URI.to_string(), Rc::new(SimpleCaseCollation));
        reg.by_uri.insert(SIMPLE_ACCENT_URI.to_string(), Rc::new(SimpleAccentCollation));
        reg.by_uri.insert(SIMPLE_CASE_ACCENT_URI.to_string(), Rc::new(SimpleCaseAccentCollation));
        reg
    }
}

impl CollationRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn get(&self, uri: &str) -> Option<Rc<dyn Collation>> {
        self.by_uri.get(uri).cloned()
    }
    pub fn insert(&mut self, collation: Rc<dyn Collation>) {
        self.by_uri.insert(collation.uri().to_string(), collation);
    }
}
