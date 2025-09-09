use crate::xdm::{ExpandedName, XdmItem, XdmSequence};
use core::fmt;
use std::collections::HashMap;
use std::sync::Arc;

pub type Arity = usize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionKey {
    pub name: ExpandedName,
    pub arity: Arity,
}

pub struct FunctionSignature {
    pub name: ExpandedName,
    pub arity: Arity,
}

// Context passed into function implementations (M6)
pub struct CallCtx<'a, N> {
    pub dyn_ctx: &'a DynamicContext<N>,
    pub static_ctx: &'a StaticContext,
    // Resolved default collation according to resolution order (if available)
    pub default_collation: Option<Arc<dyn Collation>>,
    pub resolver: Option<Arc<dyn ResourceResolver>>,
    pub regex: Option<Arc<dyn RegexProvider>>,
}

pub type FunctionImpl<N> =
    Arc<dyn Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error> + Send + Sync>;

pub struct FunctionRegistry<N> {
    fns: HashMap<FunctionKey, FunctionImpl<N>>,
}

impl<N> Default for FunctionRegistry<N> {
    fn default() -> Self {
        Self {
            fns: HashMap::new(),
        }
    }
}

impl<N> FunctionRegistry<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: ExpandedName, arity: Arity, func: FunctionImpl<N>) {
        let key = FunctionKey { name, arity };
        self.fns.insert(key, func);
    }

    pub fn get(&self, name: &ExpandedName, arity: Arity) -> Option<&FunctionImpl<N>> {
        let key = FunctionKey {
            name: name.clone(),
            arity,
        };
        self.fns.get(&key)
    }
}

pub trait Collation: Send + Sync {
    fn uri(&self) -> &str;
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering;
    // Normalization/key: used for equality/substring semantics (contains/starts/ends)
    fn key(&self, s: &str) -> String {
        // Default: identity (codepoint)
        s.to_string()
    }
}

pub struct CodepointCollation;

impl Collation for CodepointCollation {
    fn uri(&self) -> &str {
        "http://www.w3.org/2005/xpath-functions/collation/codepoint"
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        a.cmp(b)
    }
}

// Simple case-insensitive collation
pub struct SimpleCaseCollation;

impl Collation for SimpleCaseCollation {
    fn uri(&self) -> &str {
        "urn:platynui:collation:simple-case"
    }
    fn compare(&self, a: &str, b: &str) -> core::cmp::Ordering {
        self.key(a).cmp(&self.key(b))
    }
    fn key(&self, s: &str) -> String {
        s.to_lowercase()
    }
}

// Simple accent-insensitive collation (NFD + remove combining marks)
pub struct SimpleAccentCollation;

impl Collation for SimpleAccentCollation {
    fn uri(&self) -> &str {
        "urn:platynui:collation:simple-accent"
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

// Simple case+accent-insensitive collation
pub struct SimpleCaseAccentCollation;

impl Collation for SimpleCaseAccentCollation {
    fn uri(&self) -> &str {
        "urn:platynui:collation:simple-case-accent"
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
        // Built-in simple collations (M7)
        reg.by_uri.insert(
            "urn:platynui:collation:simple-case".to_string(),
            Arc::new(SimpleCaseCollation),
        );
        reg.by_uri.insert(
            "urn:platynui:collation:simple-accent".to_string(),
            Arc::new(SimpleAccentCollation),
        );
        reg.by_uri.insert(
            "urn:platynui:collation:simple-case-accent".to_string(),
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

pub trait ResourceResolver: Send + Sync {
    fn doc(&self, _uri: &str) -> Result<Option<String>, Error> {
        Ok(None)
    }
    fn collection(&self, _uri: Option<&str>) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }
}

pub trait RegexProvider: Send + Sync {
    fn matches(&self, pattern: &str, flags: &str, text: &str) -> Result<bool, Error>;
    fn replace(
        &self,
        pattern: &str,
        flags: &str,
        text: &str,
        replacement: &str,
    ) -> Result<String, Error>;
    fn tokenize(&self, pattern: &str, flags: &str, text: &str) -> Result<Vec<String>, Error>;
}

pub struct RustRegexProvider;

impl RustRegexProvider {
    fn build_with_flags(pattern: &str, flags: &str) -> Result<regex::Regex, Error> {
        let mut builder = regex::RegexBuilder::new(pattern);
        for ch in flags.chars() {
            match ch {
                'i' => builder.case_insensitive(true),
                'm' => builder.multi_line(true),
                's' => builder.dot_matches_new_line(true),
                'x' => builder.ignore_whitespace(true),
                // Unsupported flags → FORX0002
                _ => {
                    return Err(Error::dynamic_err(
                        "err:FORX0002",
                        format!("unsupported regex flag: {}", ch),
                    ));
                }
            };
        }
        builder
            .build()
            .map_err(|_| Error::dynamic_err("err:FORX0002", "invalid regex pattern"))
    }
}

impl RegexProvider for RustRegexProvider {
    fn matches(&self, pattern: &str, flags: &str, text: &str) -> Result<bool, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        Ok(re.is_match(text))
    }
    fn replace(
        &self,
        pattern: &str,
        flags: &str,
        text: &str,
        replacement: &str,
    ) -> Result<String, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        Ok(re.replace_all(text, replacement).into_owned())
    }
    fn tokenize(&self, pattern: &str, flags: &str, text: &str) -> Result<Vec<String>, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        Ok(re
            .split(text)
            .map(|s| s.to_string())
            .collect::<Vec<String>>())
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Static,
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub code: String, // err:FOAR0001 etc.
    pub message: String,
}

impl Error {
    pub fn static_err(code: &str, msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Static,
            code: code.to_string(),
            message: msg.into(),
        }
    }
    pub fn dynamic_err(code: &str, msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Dynamic,
            code: code.to_string(),
            message: msg.into(),
        }
    }
    pub fn not_implemented(feature: &str) -> Self {
        Self::dynamic_err("err:NYI0000", format!("not implemented: {}", feature))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} ({})", self.kind_str(), self.message, self.code)
    }
}

impl core::error::Error for Error {}

impl Error {
    fn kind_str(&self) -> &str {
        match self.kind {
            ErrorKind::Static => "static",
            ErrorKind::Dynamic => "dynamic",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NamespaceBindings {
    pub by_prefix: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StaticContext {
    pub base_uri: Option<String>,
    pub default_function_namespace: Option<String>,
    pub default_collation: Option<String>,
    pub namespaces: NamespaceBindings,
}

impl Default for StaticContext {
    fn default() -> Self {
        Self {
            base_uri: None,
            default_function_namespace: Some("http://www.w3.org/2005/xpath-functions".to_string()),
            default_collation: Some(
                "http://www.w3.org/2005/xpath-functions/collation/codepoint".to_string(),
            ),
            namespaces: NamespaceBindings::default(),
        }
    }
}

#[derive(Clone)]
pub struct DynamicContext<N> {
    pub context_item: Option<XdmItem<N>>,
    pub variables: HashMap<ExpandedName, XdmSequence<N>>,
    pub default_collation: Option<String>,
    pub functions: Arc<FunctionRegistry<N>>,
    pub collations: Arc<CollationRegistry>,
    pub resolver: Option<Arc<dyn ResourceResolver>>,
    pub regex: Option<Arc<dyn RegexProvider>>,
    pub now: Option<chrono::DateTime<chrono::FixedOffset>>, // M8: current-*
    pub timezone_override: Option<chrono::FixedOffset>,     // M8: override zone
}

impl<N: 'static + Send + Sync + crate::model::XdmNode + Clone> Default for DynamicContext<N> {
    fn default() -> Self {
        Self {
            context_item: None,
            variables: HashMap::new(),
            default_collation: None,
            functions: Arc::new(crate::functions::default_function_registry()),
            collations: Arc::new(CollationRegistry::default()),
            resolver: None,
            regex: None,
            now: None,
            timezone_override: None,
        }
    }
}

pub struct DynamicContextBuilder<N> {
    ctx: DynamicContext<N>,
}

impl<N: 'static + Send + Sync + crate::model::XdmNode + Clone> Default
    for DynamicContextBuilder<N>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<N: 'static + Send + Sync + crate::model::XdmNode + Clone> DynamicContextBuilder<N> {
    pub fn new() -> Self {
        Self {
            ctx: DynamicContext::default(),
        }
    }

    pub fn with_context_item(mut self, item: impl Into<XdmItem<N>>) -> Self {
        self.ctx.context_item = Some(item.into());
        self
    }

    pub fn with_variable(mut self, name: ExpandedName, value: impl Into<XdmSequence<N>>) -> Self {
        self.ctx.variables.insert(name, value.into());
        self
    }

    pub fn with_default_collation(mut self, uri: impl Into<String>) -> Self {
        self.ctx.default_collation = Some(uri.into());
        self
    }

    pub fn with_functions(mut self, reg: Arc<FunctionRegistry<N>>) -> Self {
        self.ctx.functions = reg;
        self
    }

    pub fn with_collations(mut self, reg: Arc<CollationRegistry>) -> Self {
        self.ctx.collations = reg;
        self
    }

    pub fn with_resolver(mut self, res: Arc<dyn ResourceResolver>) -> Self {
        self.ctx.resolver = Some(res);
        self
    }

    pub fn with_regex(mut self, provider: Arc<dyn RegexProvider>) -> Self {
        self.ctx.regex = Some(provider);
        self
    }

    // M8: Set a fixed 'now' instant for deterministic date/time functions
    pub fn with_now(mut self, now: chrono::DateTime<chrono::FixedOffset>) -> Self {
        self.ctx.now = Some(now);
        self
    }

    // M8: Override timezone for current-* formatting (applied to 'now' if set)
    pub fn with_timezone(mut self, offset_minutes: i32) -> Self {
        let hours = offset_minutes / 60;
        let mins = offset_minutes % 60;
        if let Some(tz) = chrono::FixedOffset::east_opt(hours * 3600 + mins * 60) {
            self.ctx.timezone_override = Some(tz);
        }
        self
    }

    pub fn build(self) -> DynamicContext<N> {
        self.ctx
    }
}
