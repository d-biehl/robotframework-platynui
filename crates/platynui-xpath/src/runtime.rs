use crate::collation::{CODEPOINT_URI, Collation, CollationRegistry};
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

/// Error type returned by function resolution.
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// No function with the (possibly default-namespace resolved) name exists.
    Unknown(ExpandedName),
    /// Function exists, but not for the requested arity. Provides known arities.
    WrongArity {
        name: ExpandedName,
        available: Vec<Arity>,
    },
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
    // Range-based registrations keyed by name; each entry holds one or more
    // (min_arity, max_arity, impl) tuples. A call matches when argc >= min_arity
    // and (max_arity is None or argc <= max_arity). Variadic functions are
    // represented with max_arity = None. Exact-arity functions are stored with
    // min_arity == max_arity == arity.
    fns: HashMap<ExpandedName, Vec<(Arity, Option<Arity>, FunctionImpl<N>)>>,
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
        // Exact arity becomes a bounded range [arity, arity]
        self.register_range(name, arity, Some(arity), func);
    }

    /// Convenience: register a function by ExpandedName with a plain closure.
    /// This wraps the closure into the required Arc and stores it.
    pub fn register_fn<F>(&mut self, name: ExpandedName, arity: Arity, f: F)
    where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        self.register(name, arity, Arc::new(f));
    }

    /// Convenience: register a function in a namespace using ns URI and local name.
    pub fn register_ns<F>(&mut self, ns_uri: &str, local: &str, arity: Arity, f: F)
    where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: Some(ns_uri.to_string()),
            local: local.to_string(),
        };
        self.register_fn(name, arity, f);
    }

    /// Register a function by ExpandedName with an arity range.
    /// If `max_arity` is None, the function is variadic starting at `min_arity`.
    /// Overlapping ranges are allowed; the resolver will pick the most specific
    /// (highest min, then smallest max) that matches the requested arity.
    pub fn register_range(
        &mut self,
        name: ExpandedName,
        min_arity: Arity,
        max_arity: Option<Arity>,
        func: FunctionImpl<N>,
    ) {
        use std::collections::hash_map::Entry;
        match self.fns.entry(name) {
            Entry::Vacant(e) => {
                let mut v = vec![(min_arity, max_arity, func)];
                // ensure deterministic order even for single insert
                v.sort_by(|a, b| {
                    let min_ord = b.0.cmp(&a.0);
                    if min_ord != core::cmp::Ordering::Equal {
                        return min_ord;
                    }
                    match (&a.1, &b.1) {
                        (Some(amax), Some(bmax)) => amax.cmp(bmax),
                        (Some(_), None) => core::cmp::Ordering::Less,
                        (None, Some(_)) => core::cmp::Ordering::Greater,
                        (None, None) => core::cmp::Ordering::Equal,
                    }
                });
                e.insert(v);
            }
            Entry::Occupied(mut e) => {
                e.get_mut().push((min_arity, max_arity, func));
                // Keep deterministic order so the most specific wins if overlapping:
                // - higher min first
                // - for equal mins, smaller max first (None treated as infinity, thus last)
                e.get_mut().sort_by(|a, b| {
                    let min_ord = b.0.cmp(&a.0);
                    if min_ord != core::cmp::Ordering::Equal {
                        return min_ord;
                    }
                    match (&a.1, &b.1) {
                        (Some(amax), Some(bmax)) => amax.cmp(bmax),
                        (Some(_), None) => core::cmp::Ordering::Less,
                        (None, Some(_)) => core::cmp::Ordering::Greater,
                        (None, None) => core::cmp::Ordering::Equal,
                    }
                });
            }
        }
    }

    /// Register a variadic function by ExpandedName with a minimum arity.
    /// The function will be selected for any call with argc >= min_arity.
    pub fn register_variadic(
        &mut self,
        name: ExpandedName,
        min_arity: Arity,
        func: FunctionImpl<N>,
    ) {
        self.register_range(name, min_arity, None, func);
    }

    /// Convenience: register a variadic function in a namespace.
    pub fn register_ns_variadic<F>(&mut self, ns_uri: &str, local: &str, min_arity: Arity, f: F)
    where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: Some(ns_uri.to_string()),
            local: local.to_string(),
        };
        self.register_variadic(name, min_arity, Arc::new(f));
    }

    /// Convenience: register a variadic function without a namespace.
    pub fn register_local_variadic<F>(&mut self, local: &str, min_arity: Arity, f: F)
    where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: None,
            local: local.to_string(),
        };
        self.register_variadic(name, min_arity, Arc::new(f));
    }

    /// Convenience: register a function in a namespace with an arity range.
    pub fn register_ns_range<F>(
        &mut self,
        ns_uri: &str,
        local: &str,
        min_arity: Arity,
        max_arity: Option<Arity>,
        f: F,
    ) where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: Some(ns_uri.to_string()),
            local: local.to_string(),
        };
        self.register_range(name, min_arity, max_arity, Arc::new(f));
    }

    /// Convenience: register a function without a namespace with an arity range.
    pub fn register_local_range<F>(
        &mut self,
        local: &str,
        min_arity: Arity,
        max_arity: Option<Arity>,
        f: F,
    ) where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: None,
            local: local.to_string(),
        };
        self.register_range(name, min_arity, max_arity, Arc::new(f));
    }

    /// Convenience: register a function without a namespace.
    pub fn register_local<F>(&mut self, local: &str, arity: Arity, f: F)
    where
        F: 'static
            + Send
            + Sync
            + Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>,
    {
        let name = ExpandedName {
            ns_uri: None,
            local: local.to_string(),
        };
        self.register_fn(name, arity, f);
    }

    // Removed: get_exact helper. Exact-arity matching is handled inline in `resolve`.

    // Note: Previous Option-returning resolver was removed; use `resolve` for
    // both lookup and diagnostics (it returns Ok(func) when found, otherwise
    // returns a typed ResolveError to distinguish unknown from wrong-arity).

    /// Resolve a function by name/arity with optional default function namespace fallback.
    /// On success returns the function implementation; otherwise returns a typed
    /// error describing whether the function is unknown or known with different arities.
    pub fn resolve(
        &self,
        name: &ExpandedName,
        arity: Arity,
        default_ns: Option<&str>,
    ) -> Result<&FunctionImpl<N>, ResolveError> {
        // Determine the effective name reference for search/diagnostics.
        // Only allocate when default function namespace needs to be applied.
        let effective_buf: Option<ExpandedName> = if name.ns_uri.is_none() {
            default_ns.map(|ns| ExpandedName {
                ns_uri: Some(ns.to_string()),
                local: name.local.clone(),
            })
        } else {
            None
        };
        let effective: &ExpandedName = effective_buf.as_ref().unwrap_or(name);
        // First, support the legacy behavior: attempt an exact-arity match on the provided name
        // (useful for locally-registered, no-namespace functions) before applying default NS.
        if let Some(cands) = self.fns.get(name) {
            if let Some((_, _, f)) = cands
                .iter()
                .find(|(min, max, _)| *min == arity && matches!(max, Some(m) if *m == arity))
            {
                return Ok(f);
            }
        }
        // Single map access for both range resolution and diagnostics
        if let Some(cands) = self.fns.get(effective) {
            if let Some((_, _, f)) = cands
                .iter()
                .find(|(min, max, _)| arity >= *min && max.map_or(true, |m| arity <= m))
            {
                return Ok(f);
            }
            // Known function name but wrong arity: collect bounded arities for message
            let mut arities: Vec<Arity> = vec![];
            for (min, max, _) in cands.iter() {
                if let Some(m) = max {
                    arities.extend(*min..=*m);
                }
            }
            arities.sort_unstable();
            arities.dedup();
            return Err(ResolveError::WrongArity {
                name: effective.clone(),
                available: arities,
            });
        }
        // No registration under effective name at all
        Err(ResolveError::Unknown(effective.clone()))
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

/// Backreference-capable regex provider based on fancy-regex (backtracking engine).
pub struct FancyRegexProvider;

impl FancyRegexProvider {
    fn build_with_flags(pattern: &str, flags: &str) -> Result<fancy_regex::Regex, Error> {
        // Use RegexBuilder to configure flags instead of injecting inline flags.
        let mut builder = fancy_regex::RegexBuilder::new(pattern);
        for ch in flags.chars() {
            match ch {
                'i' => {
                    builder.case_insensitive(true);
                }
                'm' => {
                    builder.multi_line(true);
                }
                's' => {
                    builder.dot_matches_new_line(true);
                }
                'x' => {
                    builder.verbose_mode(true);
                }
                _ => {
                    // validate_regex_flags should have rejected already, but keep a guard
                    return Err(Error::dynamic(
                        ErrorCode::FORX0001,
                        format!("unsupported regex flag: {}", ch),
                    ));
                }
            }
        }
        builder
            .build()
            .map_err(|_| Error::dynamic(ErrorCode::FORX0002, "invalid regex pattern"))
    }
}

impl RegexProvider for FancyRegexProvider {
    fn matches(&self, pattern: &str, flags: &str, text: &str) -> Result<bool, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        re.is_match(text)
            .map_err(|_| Error::dynamic(ErrorCode::FORX0002, "regex evaluation error"))
    }
    fn replace(
        &self,
        pattern: &str,
        flags: &str,
        text: &str,
        replacement: &str,
    ) -> Result<String, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        // Pre-validate replacement template using fancy_regex::Expander to catch invalid references
        // and then enforce XPath-specific rule that $0 is invalid.
        if let Err(_e) = fancy_regex::Expander::default().check(replacement, &re) {
            // Map any template validation errors to FORX0004
            return Err(Error::dynamic(
                ErrorCode::FORX0004,
                "invalid replacement string",
            ));
        }
        // Explicitly reject $0 (group zero) as per XPath 2.0 rules.
        {
            let bytes = replacement.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'$' {
                    if i + 1 >= bytes.len() {
                        // dangling $ at end of replacement
                        return Err(Error::dynamic(
                            ErrorCode::FORX0004,
                            "dangling $ at end of replacement",
                        ));
                    }
                    match bytes[i + 1] {
                        b'$' => {
                            // literal $
                            i += 2;
                            continue;
                        }
                        b'{' => {
                            // ${name}
                            let mut j = i + 2;
                            while j < bytes.len() && bytes[j] != b'}' {
                                j += 1;
                            }
                            if j >= bytes.len() {
                                // unmatched '{' -> let Expander::check have caught this; keep FORX0004
                                return Err(Error::dynamic(
                                    ErrorCode::FORX0004,
                                    "invalid replacement string",
                                ));
                            }
                            let name = &replacement[(i + 2)..j];
                            if name == "0" {
                                return Err(Error::dynamic(
                                    ErrorCode::FORX0004,
                                    "invalid group $0",
                                ));
                            }
                            i = j + 1;
                            continue;
                        }
                        d if (d as char).is_ascii_digit() => {
                            // $n... : reject if the parsed number is 0 (i.e., exactly "$0")
                            if d == b'0' {
                                // "$0" (followed by non-digit or end) denotes group 0 which is invalid in XPath
                                // If there are more digits, this is "$0<d>" which is not a valid number in our syntax
                                // but Expander::check would have rejected invalid groups already; conservatively error here.
                                return Err(Error::dynamic(
                                    ErrorCode::FORX0004,
                                    "invalid group $0",
                                ));
                            }
                            // advance past the digits (Expander will handle actual expansion later)
                            let mut j = i + 2;
                            while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                                j += 1;
                            }
                            i = j;
                            continue;
                        }
                        _ => {
                            // Unsupported $-escape
                            return Err(Error::dynamic(
                                ErrorCode::FORX0004,
                                "invalid $-escape in replacement",
                            ));
                        }
                    }
                }
                // normal byte
                i += 1;
            }
        }
        // Run replacement by iterating matches to detect zero-length matches and to expand via Expander
        let mut out = String::new();
        let mut last = 0;
        for mc in re.captures_iter(text) {
            let cap =
                mc.map_err(|_| Error::dynamic(ErrorCode::FORX0002, "regex evaluation error"))?;
            let m = cap
                .get(0)
                .ok_or_else(|| Error::dynamic(ErrorCode::FORX0002, "no overall match"))?;
            // Append text before match
            out.push_str(&text[last..m.start()]);
            // Append expanded replacement using fancy-regex Expander (default $-syntax)
            fancy_regex::Expander::default().append_expansion(&mut out, replacement, &cap);
            last = m.end();
            if m.start() == m.end() {
                // zero-length match – per XPath 2.0 fn:replace this is an error (FORX0003)
                return Err(Error::dynamic(
                    ErrorCode::FORX0003,
                    "pattern matches zero-length in replace",
                ));
            }
        }
        out.push_str(&text[last..]);
        Ok(out)
    }
    fn tokenize(&self, pattern: &str, flags: &str, text: &str) -> Result<Vec<String>, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        // Use split iterator which already takes care of zero-length matches reasonably.
        let mut tokens = Vec::new();
        for part in re.split(text) {
            match part {
                Ok(s) => tokens.push(s.to_string()),
                Err(_e) => {
                    return Err(Error::dynamic(
                        ErrorCode::FORX0002,
                        "regex evaluation error",
                    ));
                }
            }
        }
        Ok(tokens)
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Static,
    Dynamic,
}

/// Canonicalized set of (initial) XPath/XQuery 2.0 error codes we currently emit.
/// This is intentionally small and will be expanded alongside feature coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Arithmetic
    FOAR0001, // divide by zero
    FOAR0002, // numeric overflow (currently rarely emitted; placeholder for strict mode)
    // Generic error (used by fn:error default and some adapters)
    FOER0000,
    // General function / argument errors
    FORG0001, // invalid lexical form / casting failure
    FORG0006, // requires single item
    FORG0004, // zero-or-one violated
    FORG0005, // exactly-one violated
    FOCA0001, // invalid value for cast / out-of-range
    FOCH0002, // collation does not exist
    FORX0001, // regex flags invalid
    FORX0002, // regex invalid pattern / bad backref
    FORX0003, // fn:replace zero-length match error
    FORX0004, // invalid replacement string
    XPTY0004, // type error (e.g. cast of multi-item sequence)
    XPST0003, // static type error (empty not allowed etc.)
    XPST0017, // unknown function
    NYI0000,  // project specific: not yet implemented
    // Fallback / unknown (kept last)
    Unknown,
}

/// ErrorCode notes:
/// - Only a subset of XPath/XQuery 2.0 codes currently emitted.
/// - Expansion strategy: introduce variants when first needed; keep Unknown as
///   safe fallback for forward compatibility with older compiled artifacts.
/// - Use `Error::code_enum()` for structured handling instead of matching raw strings.

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        use ErrorCode::*;
        match self {
            FOAR0001 => "err:FOAR0001",
            FOAR0002 => "err:FOAR0002",
            FOER0000 => "err:FOER0000",
            FORG0001 => "err:FORG0001",
            FORG0006 => "err:FORG0006",
            FORG0004 => "err:FORG0004",
            FORG0005 => "err:FORG0005",
            FOCA0001 => "err:FOCA0001",
            FOCH0002 => "err:FOCH0002",
            FORX0001 => "err:FORX0001",
            FORX0002 => "err:FORX0002",
            FORX0003 => "err:FORX0003",
            FORX0004 => "err:FORX0004",
            XPTY0004 => "err:XPTY0004",
            XPST0003 => "err:XPST0003",
            XPST0017 => "err:XPST0017",
            NYI0000 => "err:NYI0000",
            Unknown => "err:UNKNOWN",
        }
    }
    pub fn from_code(s: &str) -> Self {
        use ErrorCode::*;
        match s {
            "err:FOAR0001" => FOAR0001,
            "err:FOAR0002" => FOAR0002,
            "err:FOER0000" => FOER0000,
            "err:FORG0001" => FORG0001,
            "err:FORG0006" => FORG0006,
            "err:FORG0004" => FORG0004,
            "err:FORG0005" => FORG0005,
            "err:FOCA0001" => FOCA0001,
            "err:FOCH0002" => FOCH0002,
            "err:FORX0001" => FORX0001,
            "err:FORX0002" => FORX0002,
            "err:FORX0003" => FORX0003,
            "err:FORX0004" => FORX0004,
            "err:XPTY0004" => XPTY0004,
            "err:XPST0003" => XPST0003,
            "err:XPST0017" => XPST0017,
            "err:NYI0000" => NYI0000,
            _ => Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub code: String, // legacy storage; canonical accessor via error_code()
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
    pub fn code_enum(&self) -> ErrorCode {
        ErrorCode::from_code(&self.code)
    }
    pub fn not_implemented(feature: &str) -> Self {
        Self::dynamic_err(
            ErrorCode::NYI0000.as_str(),
            format!("not implemented: {}", feature),
        )
    }
    // New helpers using strongly typed ErrorCode
    pub fn dynamic(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self::dynamic_err(code.as_str(), msg)
    }
    pub fn static_code(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self::static_err(code.as_str(), msg)
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
        let mut ns = NamespaceBindings::default();
        // Ensure implicit xml namespace binding (cannot be overridden per spec)
        ns.by_prefix.insert(
            "xml".to_string(),
            "http://www.w3.org/XML/1998/namespace".to_string(),
        );
        Self {
            base_uri: None,
            default_function_namespace: Some("http://www.w3.org/2005/xpath-functions".to_string()),
            default_collation: Some(CODEPOINT_URI.to_string()),
            namespaces: ns,
        }
    }
}

/// Builder for `StaticContext` (Task 29 refinement): allows explicit namespace registrations
/// and default settings while preserving required implicit bindings.
pub struct StaticContextBuilder {
    ctx: StaticContext,
}

impl Default for StaticContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticContextBuilder {
    /// Create a new `StaticContextBuilder`.
    ///
    /// The resulting `StaticContext` is an immutable snapshot that is embedded into a
    /// compiled XPath expression at compile time via `compile_xpath_with_context`.
    /// After compilation, the evaluator only uses the captured copy; providing a different
    /// `StaticContext` at evaluation time has no effect. This mirrors XPath 2.0's separation
    /// of static and dynamic context (static parts fixed during static analysis / compilation).
    pub fn new() -> Self {
        Self {
            ctx: StaticContext::default(),
        }
    }

    pub fn with_base_uri(mut self, uri: impl Into<String>) -> Self {
        self.ctx.base_uri = Some(uri.into());
        self
    }

    pub fn with_default_function_namespace(mut self, uri: impl Into<String>) -> Self {
        self.ctx.default_function_namespace = Some(uri.into());
        self
    }

    pub fn with_default_collation(mut self, uri: impl Into<String>) -> Self {
        self.ctx.default_collation = Some(uri.into());
        self
    }

    /// Register a namespace prefix → URI mapping. Attempts to override the reserved `xml`
    /// prefix are ignored to keep spec conformance.
    pub fn with_namespace(mut self, prefix: impl Into<String>, uri: impl Into<String>) -> Self {
        let p = prefix.into();
        if p == "xml" {
            return self;
        }
        self.ctx.namespaces.by_prefix.insert(p, uri.into());
        self
    }

    pub fn build(self) -> StaticContext {
        self.ctx
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
