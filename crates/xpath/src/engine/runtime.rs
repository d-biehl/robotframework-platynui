use crate::compiler::ir;
use crate::engine::collation::{CODEPOINT_URI, Collation, CollationRegistry};
use crate::engine::functions::{
    parse_day_time_duration_secs, parse_duration_lexical, parse_qname_lexical, parse_year_month_duration_months,
};
use crate::model::{NodeKind, XdmNode};
use crate::xdm::{ExpandedName, XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use core::fmt;
use lru::LruCache;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct VariableBindings<N> {
    inner: Arc<VariableScope<N>>,
}

#[derive(Clone)]
struct VariableScope<N> {
    parent: Option<Arc<VariableScope<N>>>,
    bindings: HashMap<ExpandedName, XdmSequence<N>>,
}

impl<N> VariableScope<N> {
    fn empty() -> Arc<Self> {
        Arc::new(Self { parent: None, bindings: HashMap::new() })
    }

    fn with_binding(parent: Arc<Self>, name: ExpandedName, value: XdmSequence<N>) -> Arc<Self> {
        let mut bindings = HashMap::new();
        bindings.insert(name, value);
        Arc::new(Self { parent: Some(parent), bindings })
    }

    fn get(scope: &Arc<Self>, name: &ExpandedName) -> Option<XdmSequence<N>>
    where
        N: Clone,
    {
        let mut current: Option<&Arc<Self>> = Some(scope);
        while let Some(layer) = current {
            if let Some(value) = layer.bindings.get(name) {
                return Some(value.clone());
            }
            current = layer.parent.as_ref();
        }
        None
    }
}

impl<N> Default for VariableBindings<N> {
    fn default() -> Self {
        Self { inner: VariableScope::empty() }
    }
}

impl<N> VariableBindings<N> {
    pub fn with_binding(&self, name: ExpandedName, value: XdmSequence<N>) -> Self {
        Self { inner: VariableScope::with_binding(self.inner.clone(), name, value) }
    }

    pub fn insert(&mut self, name: ExpandedName, value: XdmSequence<N>)
    where
        N: Clone,
    {
        Arc::make_mut(&mut self.inner).bindings.insert(name, value);
    }

    pub fn get(&self, name: &ExpandedName) -> Option<XdmSequence<N>>
    where
        N: Clone,
    {
        VariableScope::get(&self.inner, name)
    }
}

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
    WrongArity { name: ExpandedName, available: Vec<Arity> },
}

pub struct CallCtx<'a, N> {
    pub dyn_ctx: &'a DynamicContext<N>,
    pub static_ctx: &'a StaticContext,
    // Resolved default collation according to resolution order (if available)
    pub default_collation: Option<Rc<dyn Collation>>,
    pub regex: Option<Rc<dyn RegexProvider>>,
    pub current_context_item: Option<XdmItem<N>>,
}

/// Stream-based function implementation for zero-copy streaming.
///
/// Functions using this signature work directly with lazy `XdmSequenceStream<N>`
/// arguments and return streaming results. This avoids materializing intermediate
/// sequences into `Vec`, enabling better performance for large result sets and
/// early termination scenarios.
pub type FunctionStreamImpl<N> =
    Arc<dyn Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>>;

// Type aliases to keep complex nested types readable
pub type FunctionStreamOverload<N> = (Arity, Option<Arity>, FunctionStreamImpl<N>);
pub type FunctionStreamOverloads<N> = Vec<FunctionStreamOverload<N>>;

pub(crate) const STATIC_CONTEXT_COMPILE_CACHE_CAPACITY: usize = 20;

pub struct FunctionImplementations<N> {
    stream_fns: HashMap<ExpandedName, FunctionStreamOverloads<N>>,
}

impl<N> Default for FunctionImplementations<N> {
    fn default() -> Self {
        Self { stream_fns: HashMap::new() }
    }
}

impl<N> FunctionImplementations<N> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a stream-based function with exact arity.
    pub fn register_stream(&mut self, name: ExpandedName, arity: Arity, func: FunctionStreamImpl<N>) {
        self.register_stream_range(name, arity, Some(arity), func);
    }

    /// Register a stream-based function with arity range.
    pub fn register_stream_range(
        &mut self,
        name: ExpandedName,
        min_arity: Arity,
        max_arity: Option<Arity>,
        func: FunctionStreamImpl<N>,
    ) {
        use std::collections::hash_map::Entry;
        match self.stream_fns.entry(name) {
            Entry::Vacant(e) => {
                let mut v: FunctionStreamOverloads<N> = vec![(min_arity, max_arity, func)];
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

    /// Convenience: register a stream function with closure.
    pub fn register_stream_fn<F>(&mut self, name: ExpandedName, arity: Arity, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        self.register_stream(name, arity, Arc::new(f));
    }

    /// Convenience: register a stream function in a namespace.
    pub fn register_stream_ns<F>(&mut self, ns_uri: &str, local: &str, arity: Arity, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: Some(ns_uri.to_string()), local: local.to_string() };
        self.register_stream_fn(name, arity, f);
    }

    /// Register a variadic stream function by ExpandedName with a minimum arity.
    /// The function will be selected for any call with argc >= min_arity.
    pub fn register_stream_variadic(&mut self, name: ExpandedName, min_arity: Arity, func: FunctionStreamImpl<N>) {
        self.register_stream_range(name, min_arity, None, func);
    }

    /// Convenience: register a variadic stream function in a namespace.
    pub fn register_stream_ns_variadic<F>(&mut self, ns_uri: &str, local: &str, min_arity: Arity, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: Some(ns_uri.to_string()), local: local.to_string() };
        self.register_stream_variadic(name, min_arity, Arc::new(f));
    }

    /// Convenience: register a variadic stream function without a namespace.
    pub fn register_stream_local_variadic<F>(&mut self, local: &str, min_arity: Arity, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: None, local: local.to_string() };
        self.register_stream_variadic(name, min_arity, Arc::new(f));
    }

    /// Convenience: register a stream function in a namespace with an arity range.
    pub fn register_stream_ns_range<F>(
        &mut self,
        ns_uri: &str,
        local: &str,
        min_arity: Arity,
        max_arity: Option<Arity>,
        f: F,
    ) where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: Some(ns_uri.to_string()), local: local.to_string() };
        self.register_stream_range(name, min_arity, max_arity, Arc::new(f));
    }

    /// Convenience: register a stream function without a namespace with an arity range.
    pub fn register_stream_local_range<F>(&mut self, local: &str, min_arity: Arity, max_arity: Option<Arity>, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: None, local: local.to_string() };
        self.register_stream_range(name, min_arity, max_arity, Arc::new(f));
    }

    /// Convenience: register a stream function without a namespace.
    pub fn register_stream_local<F>(&mut self, local: &str, arity: Arity, f: F)
    where
        F: 'static + Fn(&CallCtx<N>, &[XdmSequenceStream<N>]) -> Result<XdmSequenceStream<N>, Error>,
    {
        let name = ExpandedName { ns_uri: None, local: local.to_string() };
        self.register_stream_fn(name, arity, f);
    }

    /// Resolve a stream-based function by name/arity.
    ///
    /// Returns `Some` if a stream implementation exists for this function,
    /// `None` otherwise. The caller should fall back to `resolve()` for
    /// Vec-based implementations.
    pub fn resolve_stream(
        &self,
        name: &ExpandedName,
        arity: Arity,
        default_ns: Option<&str>,
    ) -> Option<&FunctionStreamImpl<N>> {
        let effective_buf: Option<ExpandedName> = if name.ns_uri.is_none() {
            default_ns.map(|ns| ExpandedName { ns_uri: Some(ns.to_string()), local: name.local.clone() })
        } else {
            None
        };
        let effective: &ExpandedName = effective_buf.as_ref().unwrap_or(name);

        // Try exact match on original name first (for no-namespace functions)
        if let Some(cands) = self.stream_fns.get(name)
            && let Some((_, _, f)) =
                cands.iter().find(|(min, max, _)| *min == arity && matches!(max, Some(m) if *m == arity))
        {
            return Some(f);
        }

        // Try effective name with range matching
        self.stream_fns.get(effective).and_then(|cands| {
            cands.iter().find(|(min, max, _)| arity >= *min && max.is_none_or(|m| arity <= m)).map(|(_, _, f)| f)
        })
    }
}

// Node-producing resolver for host adapters that can construct N directly
pub trait NodeResolver<N> {
    fn doc_node(&self, _uri: &str) -> Result<Option<N>, Error> {
        Ok(None)
    }
    fn collection_nodes(&self, _uri: Option<&str>) -> Result<Vec<N>, Error> {
        Ok(vec![])
    }
}

pub trait RegexProvider {
    fn matches(&self, pattern: &str, flags: &str, text: &str) -> Result<bool, Error>;
    fn replace(&self, pattern: &str, flags: &str, text: &str, replacement: &str) -> Result<String, Error>;
    fn tokenize(&self, pattern: &str, flags: &str, text: &str) -> Result<Vec<String>, Error>;
}

/// Backreference-capable regex provider based on fancy-regex (backtracking engine).
pub struct FancyRegexProvider;

impl FancyRegexProvider {
    fn build_with_flags(pattern: &str, flags: &str) -> Result<Rc<fancy_regex::Regex>, Error> {
        thread_local! {
            static REGEX_CACHE: std::cell::RefCell<HashMap<(String, String), Rc<fancy_regex::Regex>>> = Default::default();
        }
        let key = (pattern.to_string(), flags.to_string());
        if let Some(found) = REGEX_CACHE.with(|cell| cell.borrow().get(&key).cloned()) {
            return Ok(found);
        }

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
                    return Err(Error::from_code(ErrorCode::FORX0001, format!("unsupported regex flag: {}", ch)));
                }
            }
        }
        let compiled = builder.build().map_err(|e| {
            Error::from_code(ErrorCode::FORX0002, "invalid regex pattern")
                .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
        })?;
        let rc = Rc::new(compiled);
        REGEX_CACHE.with(|cell| cell.borrow_mut().insert(key, rc.clone()));
        Ok(rc)
    }
}

impl RegexProvider for FancyRegexProvider {
    fn matches(&self, pattern: &str, flags: &str, text: &str) -> Result<bool, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        re.is_match(text).map_err(|e| {
            Error::from_code(ErrorCode::FORX0002, "regex evaluation error")
                .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
        })
    }
    fn replace(&self, pattern: &str, flags: &str, text: &str, replacement: &str) -> Result<String, Error> {
        let re = Self::build_with_flags(pattern, flags)?;
        // Pre-validate replacement template using fancy_regex::Expander and enforce that $0 is invalid.
        if let Err(e) = fancy_regex::Expander::default().check(replacement, &re) {
            // Map any template validation errors to FORX0004
            return Err(Error::from_code(ErrorCode::FORX0004, "invalid replacement string")
                .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>)));
        }
        // Explicitly reject $0 (group zero) as per XPath 2.0 rules.
        {
            let bytes = replacement.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'$' {
                    if i + 1 >= bytes.len() {
                        // dangling $ at end of replacement
                        return Err(Error::from_code(ErrorCode::FORX0004, "dangling $ at end of replacement"));
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
                                return Err(Error::from_code(ErrorCode::FORX0004, "invalid replacement string"));
                            }
                            let name = &replacement[(i + 2)..j];
                            if name == "0" {
                                return Err(Error::from_code(ErrorCode::FORX0004, "invalid group $0"));
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
                                return Err(Error::from_code(ErrorCode::FORX0004, "invalid group $0"));
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
                            return Err(Error::from_code(ErrorCode::FORX0004, "invalid $-escape in replacement"));
                        }
                    }
                }
                // normal byte
                i += 1;
            }
        }
        // Run replacement by iterating matches to detect zero-length matches and expand via Expander
        let mut out = String::new();
        let mut last = 0;
        for mc in re.captures_iter(text) {
            let cap = mc.map_err(|e| {
                Error::from_code(ErrorCode::FORX0002, "regex evaluation error")
                    .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
            })?;
            let m = cap.get(0).ok_or_else(|| Error::from_code(ErrorCode::FORX0002, "no overall match"))?;
            // Append text before match
            out.push_str(&text[last..m.start()]);
            // Append expanded replacement using fancy-regex Expander
            fancy_regex::Expander::default().append_expansion(&mut out, replacement, &cap);
            last = m.end();
            if m.start() == m.end() {
                // zero-length match – per XPath 2.0 fn:replace this is an error (FORX0003)
                return Err(Error::from_code(ErrorCode::FORX0003, "pattern matches zero-length in replace"));
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
                Err(e) => {
                    return Err(Error::from_code(ErrorCode::FORX0002, "regex evaluation error")
                        .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>)));
                }
            }
        }
        Ok(tokens)
    }
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
    FOCH0003, // unsupported normalization form
    FODC0002, // default collection undefined
    FODC0004, // collection lookup failure
    FODC0005, // doc/document retrieval failure
    FONS0004, // unknown namespace prefix
    FONS0005, // base-uri unresolved
    FORX0001, // regex flags invalid
    FORX0002, // regex invalid pattern / bad backref
    FORX0003, // fn:replace zero-length match error
    FORX0004, // invalid replacement string
    XPTY0004, // type error (e.g. cast of multi-item sequence)
    XPDY0002, // context item undefined
    XPST0008, // undeclared variable / function
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
    /// Returns the QName (ExpandedName) for this spec-defined error code.
    /// Namespace: http://www.w3.org/2005/xqt-errors
    pub fn qname(&self) -> ExpandedName {
        ExpandedName {
            ns_uri: Some(ERR_NS.to_string()),
            local: match self {
                ErrorCode::FOAR0001 => "FOAR0001".to_string(),
                ErrorCode::FOAR0002 => "FOAR0002".to_string(),
                ErrorCode::FOER0000 => "FOER0000".to_string(),
                ErrorCode::FORG0001 => "FORG0001".to_string(),
                ErrorCode::FORG0006 => "FORG0006".to_string(),
                ErrorCode::FORG0004 => "FORG0004".to_string(),
                ErrorCode::FORG0005 => "FORG0005".to_string(),
                ErrorCode::FOCA0001 => "FOCA0001".to_string(),
                ErrorCode::FOCH0002 => "FOCH0002".to_string(),
                ErrorCode::FOCH0003 => "FOCH0003".to_string(),
                ErrorCode::FODC0002 => "FODC0002".to_string(),
                ErrorCode::FODC0004 => "FODC0004".to_string(),
                ErrorCode::FODC0005 => "FODC0005".to_string(),
                ErrorCode::FONS0004 => "FONS0004".to_string(),
                ErrorCode::FONS0005 => "FONS0005".to_string(),
                ErrorCode::FORX0001 => "FORX0001".to_string(),
                ErrorCode::FORX0002 => "FORX0002".to_string(),
                ErrorCode::FORX0003 => "FORX0003".to_string(),
                ErrorCode::FORX0004 => "FORX0004".to_string(),
                ErrorCode::XPTY0004 => "XPTY0004".to_string(),
                ErrorCode::XPDY0002 => "XPDY0002".to_string(),
                ErrorCode::XPST0008 => "XPST0008".to_string(),
                ErrorCode::XPST0003 => "XPST0003".to_string(),
                ErrorCode::XPST0017 => "XPST0017".to_string(),
                ErrorCode::NYI0000 => "NYI0000".to_string(),
                ErrorCode::Unknown => "UNKNOWN".to_string(),
            },
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
            "err:FOCH0003" => FOCH0003,
            "err:FODC0002" => FODC0002,
            "err:FODC0004" => FODC0004,
            "err:FODC0005" => FODC0005,
            "err:FONS0004" => FONS0004,
            "err:FONS0005" => FONS0005,
            "err:FORX0001" => FORX0001,
            "err:FORX0002" => FORX0002,
            "err:FORX0003" => FORX0003,
            "err:FORX0004" => FORX0004,
            "err:XPTY0004" => XPTY0004,
            "err:XPDY0002" => XPDY0002,
            "err:XPST0008" => XPST0008,
            "err:XPST0003" => XPST0003,
            "err:XPST0017" => XPST0017,
            "err:NYI0000" => NYI0000,
            _ => Unknown,
        }
    }
}

/// Namespace URI used for W3C-defined XPath/XQuery error codes (xqt-errors).
pub use crate::consts::ERR_NS;

#[derive(Debug, Clone, thiserror::Error)]
pub struct Error {
    pub code: ExpandedName,
    pub message: String,
    #[source]
    pub source: Option<Arc<dyn std::error::Error + Send + Sync>>, // optional chained cause
}

impl Error {
    /// New QName-centric constructor (preferred). Stores the QName directly.
    pub fn new_qname(code: ExpandedName, msg: impl Into<String>) -> Self {
        Self { code, message: msg.into(), source: None }
    }
    pub fn code_enum(&self) -> ErrorCode {
        // Only ERR_NS codes map to the enum; others are Unknown.
        if self.code.ns_uri.as_deref() == Some(ERR_NS) {
            let s = format!("err:{}", self.code.local);
            ErrorCode::from_code(&s)
        } else {
            ErrorCode::Unknown
        }
    }
    /// Attempt to reconstruct the QName from the stored string code.
    /// Always returns the stored QName.
    pub fn code_qname(&self) -> Option<ExpandedName> {
        Some(self.code.clone())
    }
    /// Format the code as a human-readable string (err:LOCAL or Q{ns}local).
    /// Owned formatter for human-readable code string.
    pub fn format_code(&self) -> String {
        if self.code.ns_uri.as_deref() == Some(ERR_NS) {
            format!("err:{}", self.code.local)
        } else if let Some(ns) = &self.code.ns_uri {
            format!("Q{{{}}}{}", ns, self.code.local)
        } else {
            self.code.local.clone()
        }
    }
    pub fn not_implemented(feature: &str) -> Self {
        Self::new_qname(ErrorCode::NYI0000.qname(), format!("not implemented: {}", feature))
    }
    // New helpers using strongly typed ErrorCode
    pub fn from_code(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self::new_qname(code.qname(), msg)
    }

    /// Compose an error with a source cause.
    pub fn with_source(mut self, source: impl Into<Option<Arc<dyn std::error::Error + Send + Sync>>>) -> Self {
        self.source = source.into();
        self
    }

    /// Public helper: parse a legacy error code string (e.g., "err:FOER0000" or "Q{ns}local")
    /// into an ExpandedName. Prefer using typed ErrorCode where possible.
    pub fn parse_code(s: &str) -> ExpandedName {
        if let Some(rest) = s.strip_prefix("err:") {
            return ExpandedName { ns_uri: Some(ERR_NS.to_string()), local: rest.to_string() };
        }
        if let Some(body) = s.strip_prefix('Q').and_then(|t| t.strip_prefix('{')).and_then(|t| t.split_once('}')) {
            let (ns, local) = body;
            return ExpandedName { ns_uri: Some(ns.to_string()), local: local.to_string() };
        }
        // Fallback: treat as unqualified local name
        ExpandedName { ns_uri: None, local: s.to_string() }
    }
}

// Convenience conversions to attach common source errors with domain codes
impl From<fancy_regex::Error> for Error {
    fn from(e: fancy_regex::Error) -> Self {
        Error::from_code(ErrorCode::FORX0002, "regex error")
            .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::from_code(ErrorCode::FODC0005, e.to_string())
            .with_source(Some(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.format_code())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NamespaceBindings {
    pub by_prefix: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArityRange {
    pub min: usize,
    pub max: Option<usize>,
}

impl ArityRange {
    pub fn contains(&self, value: usize) -> bool {
        if value < self.min {
            return false;
        }
        match self.max {
            Some(max) => value <= max,
            None => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Occurrence {
    #[default]
    ExactlyOne,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

impl Occurrence {
    pub fn allows_empty(self) -> bool {
        matches!(self, Occurrence::ZeroOrOne | Occurrence::ZeroOrMore)
    }

    pub fn allows_multiple(self) -> bool {
        matches!(self, Occurrence::ZeroOrMore | Occurrence::OneOrMore)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemTypeSpec {
    AnyItem,
    Node,
    Element,
    AnyAtomic,
    UntypedPromotable,
    Numeric,
    NumericOrDuration,
    Integer,
    String,
    Boolean,
    Double,
    Decimal,
    Float,
    AnyUri,
    Duration,
    YearMonthDuration,
    DayTimeDuration,
    DateTime,
    Date,
    Time,
    QName,
    SpecificAtomic(ExpandedName),
}

impl ItemTypeSpec {
    pub fn is_atomic(&self) -> bool {
        matches!(
            self,
            ItemTypeSpec::AnyAtomic
                | ItemTypeSpec::UntypedPromotable
                | ItemTypeSpec::Numeric
                | ItemTypeSpec::NumericOrDuration
                | ItemTypeSpec::Integer
                | ItemTypeSpec::String
                | ItemTypeSpec::Boolean
                | ItemTypeSpec::Double
                | ItemTypeSpec::Decimal
                | ItemTypeSpec::Float
                | ItemTypeSpec::AnyUri
                | ItemTypeSpec::Duration
                | ItemTypeSpec::YearMonthDuration
                | ItemTypeSpec::DayTimeDuration
                | ItemTypeSpec::DateTime
                | ItemTypeSpec::Date
                | ItemTypeSpec::Time
                | ItemTypeSpec::QName
                | ItemTypeSpec::SpecificAtomic(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamTypeSpec {
    pub item: ItemTypeSpec,
    pub occurrence: Occurrence,
}

impl ParamTypeSpec {
    pub fn any_item(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::AnyItem, occurrence }
    }

    pub fn node(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Node, occurrence }
    }

    pub fn element(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Element, occurrence }
    }

    pub fn any_atomic(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::AnyAtomic, occurrence }
    }

    pub fn untyped_promotable(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::UntypedPromotable, occurrence }
    }

    pub fn numeric(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Numeric, occurrence }
    }

    pub fn numeric_or_duration(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::NumericOrDuration, occurrence }
    }

    pub fn integer(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Integer, occurrence }
    }

    pub fn string(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::String, occurrence }
    }

    pub fn boolean(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Boolean, occurrence }
    }

    pub fn double(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Double, occurrence }
    }

    pub fn decimal(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Decimal, occurrence }
    }

    pub fn float(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Float, occurrence }
    }

    pub fn any_uri(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::AnyUri, occurrence }
    }

    pub fn qname(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::QName, occurrence }
    }

    pub fn duration(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Duration, occurrence }
    }

    pub fn year_month_duration(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::YearMonthDuration, occurrence }
    }

    pub fn day_time_duration(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::DayTimeDuration, occurrence }
    }

    pub fn date_time(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::DateTime, occurrence }
    }

    pub fn date(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Date, occurrence }
    }

    pub fn time(occurrence: Occurrence) -> Self {
        Self { item: ItemTypeSpec::Time, occurrence }
    }

    pub fn requires_atomization(&self) -> bool {
        self.item.is_atomic()
    }

    pub fn apply_to_sequence<N: XdmNode + Clone>(
        &self,
        seq: XdmSequence<N>,
        static_ctx: &StaticContext,
    ) -> Result<XdmSequence<N>, Error> {
        self.validate_cardinality(seq.len())?;
        if seq.is_empty() {
            return Ok(seq);
        }
        match self.item {
            ItemTypeSpec::AnyItem => Ok(seq),
            ItemTypeSpec::Node => ensure_node_sequence(seq),
            ItemTypeSpec::Element => ensure_element_sequence(seq),
            ItemTypeSpec::AnyAtomic | ItemTypeSpec::UntypedPromotable => ensure_atomic_sequence(seq),
            ItemTypeSpec::Numeric => convert_numeric_sequence(seq),
            ItemTypeSpec::NumericOrDuration => convert_numeric_or_duration_sequence(seq),
            ItemTypeSpec::Integer => convert_integer_sequence(seq),
            ItemTypeSpec::String => convert_string_sequence(seq),
            ItemTypeSpec::Boolean => convert_boolean_sequence(seq),
            ItemTypeSpec::Double => convert_double_sequence(seq),
            ItemTypeSpec::Decimal => convert_decimal_sequence(seq),
            ItemTypeSpec::Float => convert_float_sequence(seq),
            ItemTypeSpec::AnyUri => convert_any_uri_sequence(seq),
            ItemTypeSpec::Duration => convert_duration_sequence(seq),
            ItemTypeSpec::YearMonthDuration => convert_year_month_duration_sequence(seq),
            ItemTypeSpec::DayTimeDuration => convert_day_time_duration_sequence(seq),
            ItemTypeSpec::DateTime => convert_date_time_sequence(seq),
            ItemTypeSpec::Date => convert_date_sequence(seq),
            ItemTypeSpec::Time => convert_time_sequence(seq),
            ItemTypeSpec::QName => convert_qname_sequence(seq, static_ctx),
            ItemTypeSpec::SpecificAtomic(_) => ensure_atomic_sequence(seq),
        }
    }

    fn validate_cardinality(&self, len: usize) -> Result<(), Error> {
        match self.occurrence {
            Occurrence::ExactlyOne => {
                if len != 1 {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be a singleton"));
                }
            }
            Occurrence::ZeroOrOne => {
                if len > 1 {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "function argument allows at most one item"));
                }
            }
            Occurrence::ZeroOrMore => {}
            Occurrence::OneOrMore => {
                if len == 0 {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "function argument must contain at least one item",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn ensure_atomic_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    let mut out = Vec::with_capacity(seq.len());
    for item in seq {
        match item {
            XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a)),
            XdmItem::Node(_) => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be atomic"));
            }
        }
    }
    Ok(out)
}

fn ensure_node_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    for item in &seq {
        if matches!(item, XdmItem::Atomic(_)) {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be node()"));
        }
    }
    Ok(seq)
}

fn ensure_element_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error>
where
    N: XdmNode,
{
    for item in &seq {
        match item {
            XdmItem::Node(n) => {
                if !matches!(n.kind(), NodeKind::Element) {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be element()"));
                }
            }
            XdmItem::Atomic(_) => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be element()"));
            }
        }
    }
    Ok(seq)
}

fn convert_numeric_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_numeric_atomic)
}

fn convert_numeric_or_duration_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_numeric_or_duration_atomic)
}

fn convert_date_time_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_date_time_atomic)
}

fn convert_date_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_date_atomic)
}

fn convert_time_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_time_atomic)
}

fn convert_numeric_or_duration_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::YearMonthDuration(_) | V::DayTimeDuration(_) => Ok(a),
        other => convert_numeric_atomic(other),
    }
}

fn convert_integer_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_integer_atomic)
}

fn convert_integer_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::Integer(_) => a,
        V::Long(v) => V::Integer(v),
        V::Int(v) => V::Integer(v as i64),
        V::Short(v) => V::Integer(v as i64),
        V::Byte(v) => V::Integer(v as i64),
        V::UnsignedLong(v) => {
            if v <= i64::MAX as u64 {
                V::Integer(v as i64)
            } else {
                return Err(Error::from_code(ErrorCode::FOCA0001, "integer argument exceeds supported range"));
            }
        }
        V::UnsignedInt(v) => V::Integer(v as i64),
        V::UnsignedShort(v) => V::Integer(v as i64),
        V::UnsignedByte(v) => V::Integer(v as i64),
        V::NonPositiveInteger(v) => V::Integer(v),
        V::NegativeInteger(v) => V::Integer(v),
        V::NonNegativeInteger(v) => {
            let val = i64::try_from(v)
                .map_err(|_| Error::from_code(ErrorCode::FOCA0001, "integer argument exceeds supported range"))?;
            V::Integer(val)
        }
        V::PositiveInteger(v) => {
            let val = i64::try_from(v)
                .map_err(|_| Error::from_code(ErrorCode::FOCA0001, "integer argument exceeds supported range"))?;
            V::Integer(val)
        }
        V::Decimal(d) => {
            if d.fract() == 0.0 {
                if d >= (i64::MIN as f64) && d <= (i64::MAX as f64) {
                    V::Integer(d as i64)
                } else {
                    return Err(Error::from_code(ErrorCode::FOCA0001, "decimal value out of xs:integer range"));
                }
            } else {
                return Err(Error::from_code(ErrorCode::FOCA0001, "precision argument must be an integer"));
            }
        }
        V::Double(d) => {
            if d.is_nan() || d.is_infinite() {
                return Err(Error::from_code(ErrorCode::FOCA0001, "cannot cast NaN or INF to xs:integer"));
            }
            if d.fract() == 0.0 {
                if d >= (i64::MIN as f64) && d <= (i64::MAX as f64) {
                    V::Integer(d as i64)
                } else {
                    return Err(Error::from_code(ErrorCode::FOCA0001, "double value out of xs:integer range"));
                }
            } else {
                return Err(Error::from_code(ErrorCode::FOCA0001, "precision argument must be integral"));
            }
        }
        V::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                return Err(Error::from_code(ErrorCode::FOCA0001, "cannot cast NaN or INF to xs:integer"));
            }
            if f.fract() == 0.0 {
                let value = f as f64;
                if value >= (i64::MIN as f64) && value <= (i64::MAX as f64) {
                    V::Integer(value as i64)
                } else {
                    return Err(Error::from_code(ErrorCode::FOCA0001, "float value out of xs:integer range"));
                }
            } else {
                return Err(Error::from_code(ErrorCode::FOCA0001, "precision argument must be integral"));
            }
        }
        V::UntypedAtomic(s) => {
            let trimmed = s.trim();
            let parsed = trimmed
                .parse::<i64>()
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:integer"))?;
            V::Integer(parsed)
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be xs:integer"));
        }
    })
}

fn convert_numeric_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::UntypedAtomic(s) => {
            let parsed = s
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:double"))?;
            Ok(V::Double(parsed))
        }
        V::Float(_)
        | V::Integer(_)
        | V::Long(_)
        | V::Int(_)
        | V::Short(_)
        | V::Byte(_)
        | V::UnsignedLong(_)
        | V::UnsignedInt(_)
        | V::UnsignedShort(_)
        | V::UnsignedByte(_)
        | V::NonPositiveInteger(_)
        | V::NegativeInteger(_)
        | V::NonNegativeInteger(_)
        | V::PositiveInteger(_)
        | V::Decimal(_)
        | V::Double(_) => Ok(a),
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument is not numeric")),
    }
}

fn convert_string_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::String(_) => Ok(a),
        V::UntypedAtomic(s) => Ok(V::String(s)),
        V::AnyUri(s) => Ok(V::String(s)),
        V::NormalizedString(s)
        | V::Token(s)
        | V::Language(s)
        | V::Name(s)
        | V::NCName(s)
        | V::NMTOKEN(s)
        | V::Id(s)
        | V::IdRef(s)
        | V::Entity(s)
        | V::Notation(s) => Ok(V::String(s)),
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be castable to xs:string")),
    }
}

fn convert_boolean_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::Boolean(_) => a,
        V::UntypedAtomic(s) | V::String(s) => {
            let trimmed = s.trim();
            match trimmed {
                "true" | "1" => V::Boolean(true),
                "false" | "0" => V::Boolean(false),
                _ => {
                    return Err(Error::from_code(ErrorCode::FORG0001, "invalid lexical form for xs:boolean"));
                }
            }
        }
        V::Double(d) => V::Boolean(d != 0.0 && !d.is_nan()),
        V::Float(f) => V::Boolean(f != 0.0 && !f.is_nan()),
        V::Decimal(d) => V::Boolean(d != 0.0),
        V::Integer(i) => V::Boolean(i != 0),
        V::Long(i) => V::Boolean(i != 0),
        V::Int(i) => V::Boolean(i != 0),
        V::Short(i) => V::Boolean(i != 0),
        V::Byte(i) => V::Boolean(i != 0),
        V::NonPositiveInteger(i) => V::Boolean(i != 0),
        V::NegativeInteger(i) => V::Boolean(i != 0),
        V::UnsignedLong(i) => V::Boolean(i != 0),
        V::UnsignedInt(i) => V::Boolean(i != 0),
        V::UnsignedShort(i) => V::Boolean(i != 0),
        V::UnsignedByte(i) => V::Boolean(i != 0),
        V::NonNegativeInteger(i) => V::Boolean(i != 0),
        V::PositiveInteger(i) => V::Boolean(i != 0),
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:boolean"));
        }
    })
}

fn convert_double_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::Double(_) => Ok(a),
        V::Float(f) => Ok(V::Double(f as f64)),
        V::Decimal(d) => Ok(V::Double(d)),
        V::Integer(i) => Ok(V::Double(i as f64)),
        V::Long(i) => Ok(V::Double(i as f64)),
        V::Int(i) => Ok(V::Double(i as f64)),
        V::Short(i) => Ok(V::Double(i as f64)),
        V::Byte(i) => Ok(V::Double(i as f64)),
        V::NonPositiveInteger(i) => Ok(V::Double(i as f64)),
        V::NegativeInteger(i) => Ok(V::Double(i as f64)),
        V::UnsignedLong(i) => Ok(V::Double(i as f64)),
        V::UnsignedInt(i) => Ok(V::Double(i as f64)),
        V::UnsignedShort(i) => Ok(V::Double(i as f64)),
        V::UnsignedByte(i) => Ok(V::Double(i as f64)),
        V::NonNegativeInteger(i) => Ok(V::Double(i as f64)),
        V::PositiveInteger(i) => Ok(V::Double(i as f64)),
        V::UntypedAtomic(s) => {
            let parsed = s
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:double"))?;
            Ok(V::Double(parsed))
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:double")),
    }
}

fn convert_decimal_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::Decimal(_) => Ok(a),
        V::Integer(i) => Ok(V::Decimal(i as f64)),
        V::Long(i) => Ok(V::Decimal(i as f64)),
        V::Int(i) => Ok(V::Decimal(i as f64)),
        V::Short(i) => Ok(V::Decimal(i as f64)),
        V::Byte(i) => Ok(V::Decimal(i as f64)),
        V::NonPositiveInteger(i) => Ok(V::Decimal(i as f64)),
        V::NegativeInteger(i) => Ok(V::Decimal(i as f64)),
        V::UnsignedLong(i) => Ok(V::Decimal(i as f64)),
        V::UnsignedInt(i) => Ok(V::Decimal(i as f64)),
        V::UnsignedShort(i) => Ok(V::Decimal(i as f64)),
        V::UnsignedByte(i) => Ok(V::Decimal(i as f64)),
        V::NonNegativeInteger(i) => Ok(V::Decimal(i as f64)),
        V::PositiveInteger(i) => Ok(V::Decimal(i as f64)),
        V::Double(d) => Ok(V::Decimal(d)),
        V::Float(f) => Ok(V::Decimal(f as f64)),
        V::UntypedAtomic(s) => {
            let parsed = s
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:decimal"))?;
            Ok(V::Decimal(parsed))
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:decimal")),
    }
}

fn convert_float_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::Float(_) => Ok(a),
        V::Double(d) => Ok(V::Float(d as f32)),
        V::Decimal(d) => Ok(V::Float(d as f32)),
        V::Integer(i) => Ok(V::Float(i as f32)),
        V::Long(i) => Ok(V::Float(i as f32)),
        V::Int(i) => Ok(V::Float(i as f32)),
        V::Short(i) => Ok(V::Float(i as f32)),
        V::Byte(i) => Ok(V::Float(i as f32)),
        V::NonPositiveInteger(i) => Ok(V::Float(i as f32)),
        V::NegativeInteger(i) => Ok(V::Float(i as f32)),
        V::UnsignedLong(i) => Ok(V::Float(i as f32)),
        V::UnsignedInt(i) => Ok(V::Float(i as f32)),
        V::UnsignedShort(i) => Ok(V::Float(i as f32)),
        V::UnsignedByte(i) => Ok(V::Float(i as f32)),
        V::NonNegativeInteger(i) => Ok(V::Float(i as f32)),
        V::PositiveInteger(i) => Ok(V::Float(i as f32)),
        V::UntypedAtomic(s) => {
            let parsed = s
                .trim()
                .parse::<f32>()
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:float"))?;
            Ok(V::Float(parsed))
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:float")),
    }
}

fn convert_any_uri_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::AnyUri(_) => a,
        V::String(s) | V::UntypedAtomic(s) => V::AnyUri(s),
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:anyURI"));
        }
    })
}

fn convert_string_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_string_atomic)
}

fn convert_boolean_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_boolean_atomic)
}

fn convert_double_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_double_atomic)
}

fn convert_decimal_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_decimal_atomic)
}

fn convert_float_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_float_atomic)
}

fn convert_any_uri_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_any_uri_atomic)
}

fn convert_duration_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_duration_atomic)
}

fn convert_year_month_duration_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_year_month_duration_atomic)
}

fn convert_day_time_duration_sequence<N>(seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
    convert_atomic_sequence_with(seq, convert_day_time_duration_atomic)
}

fn convert_qname_sequence<N>(seq: XdmSequence<N>, static_ctx: &StaticContext) -> Result<XdmSequence<N>, Error> {
    let mut out = Vec::with_capacity(seq.len());
    for item in seq {
        match item {
            XdmItem::Atomic(a) => out.push(XdmItem::Atomic(convert_qname_atomic(a, static_ctx)?)),
            XdmItem::Node(_) => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be atomic"));
            }
        }
    }
    Ok(out)
}

fn convert_atomic_sequence_with<N, F>(seq: XdmSequence<N>, mut f: F) -> Result<XdmSequence<N>, Error>
where
    F: FnMut(XdmAtomicValue) -> Result<XdmAtomicValue, Error>,
{
    let mut out = Vec::with_capacity(seq.len());
    for item in seq {
        match item {
            XdmItem::Atomic(a) => out.push(XdmItem::Atomic(f(a)?)),
            XdmItem::Node(_) => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "function argument must be atomic"));
            }
        }
    }
    Ok(out)
}

fn convert_duration_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::YearMonthDuration(_) | V::DayTimeDuration(_) => a,
        V::UntypedAtomic(s) | V::String(s) => duration_from_string(&s)?,
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:duration"));
        }
    })
}

fn convert_year_month_duration_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::YearMonthDuration(_) => a,
        V::UntypedAtomic(s) | V::String(s) => {
            let months = parse_year_month_duration_months(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:yearMonthDuration"))?;
            V::YearMonthDuration(months)
        }
        V::DayTimeDuration(_) => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument is not xs:yearMonthDuration"));
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "function argument cannot be cast to xs:yearMonthDuration",
            ));
        }
    })
}

fn convert_day_time_duration_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::DayTimeDuration(_) => a,
        V::UntypedAtomic(s) | V::String(s) => {
            let secs = parse_day_time_duration_secs(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:dayTimeDuration"))?;
            V::DayTimeDuration(secs)
        }
        V::YearMonthDuration(_) => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument is not xs:dayTimeDuration"));
        }
        _ => {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                "function argument cannot be cast to xs:dayTimeDuration",
            ));
        }
    })
}

fn convert_date_time_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::DateTime(_) => Ok(a),
        V::String(s) | V::UntypedAtomic(s) => {
            let (date, time, tz) = crate::util::temporal::parse_date_time_lex(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:dateTime"))?;
            let dt = crate::util::temporal::build_naive_datetime(date, time, tz);
            Ok(V::DateTime(dt))
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:dateTime")),
    }
}

fn convert_date_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::Date { .. } => Ok(a),
        V::String(s) | V::UntypedAtomic(s) => {
            let (date, tz) = crate::util::temporal::parse_date_lex(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:date"))?;
            Ok(V::Date { date, tz })
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:date")),
    }
}

fn convert_time_atomic(a: XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match a {
        V::Time { .. } => Ok(a),
        V::String(s) | V::UntypedAtomic(s) => {
            let (time, tz) = crate::util::temporal::parse_time_lex(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:time"))?;
            Ok(V::Time { time, tz })
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:time")),
    }
}

fn duration_from_string(s: &str) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match parse_duration_lexical(s) {
        Ok((Some(months), None)) => Ok(V::YearMonthDuration(months)),
        Ok((None, Some(secs))) => Ok(V::DayTimeDuration(secs)),
        Ok((Some(_), Some(_))) => Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:duration")),
        Ok((None, None)) => Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:duration")),
        Err(e) => Err(e),
    }
}

fn convert_qname_atomic(a: XdmAtomicValue, static_ctx: &StaticContext) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    Ok(match a {
        V::QName { .. } => a,
        V::UntypedAtomic(s) | V::String(s) => {
            let (prefix_opt, local) = parse_qname_lexical(&s)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "cannot cast to xs:QName"))?;
            let ns_uri = match prefix_opt.as_deref() {
                None => None,
                Some("xml") => Some(crate::consts::XML_URI.to_string()),
                Some(prefix) => Some(
                    static_ctx
                        .namespaces
                        .by_prefix
                        .get(prefix)
                        .cloned()
                        .ok_or_else(|| Error::from_code(ErrorCode::FONS0004, "unknown namespace prefix"))?,
                ),
            };
            V::QName { ns_uri, prefix: prefix_opt, local }
        }
        _ => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "function argument cannot be cast to xs:QName"));
        }
    })
}

#[derive(Debug, Clone, Default)]
pub struct FunctionSignatures {
    entries: HashMap<ExpandedName, Vec<ArityRange>>,
    param_types: HashMap<(ExpandedName, usize), Vec<ParamTypeSpec>>,
}

impl FunctionSignatures {
    pub fn register(&mut self, name: ExpandedName, min: usize, max: Option<usize>) {
        let ranges = self.entries.entry(name).or_default();
        if !ranges.iter().any(|r| r.min == min && r.max == max) {
            ranges.push(ArityRange { min, max });
        }
    }

    pub fn set_param_types(&mut self, name: ExpandedName, arity: usize, specs: Vec<ParamTypeSpec>) {
        self.param_types.insert((name, arity), specs);
    }

    pub fn register_ns(&mut self, ns: &str, local: &str, min: usize, max: Option<usize>) {
        self.register(ExpandedName { ns_uri: Some(ns.to_string()), local: local.to_string() }, min, max);
    }

    pub fn register_local(&mut self, local: &str, min: usize, max: Option<usize>) {
        self.register(ExpandedName { ns_uri: None, local: local.to_string() }, min, max);
    }

    pub fn arities(&self, name: &ExpandedName) -> Option<&[ArityRange]> {
        self.entries.get(name).map(|v| v.as_slice())
    }

    pub fn supports(&self, name: &ExpandedName, arity: usize) -> bool {
        self.entries.get(name).map(|ranges| ranges.iter().any(|r| r.contains(arity))).unwrap_or(false)
    }

    pub fn param_types_for_call(
        &self,
        name: &ExpandedName,
        arity: usize,
        default_ns: Option<&str>,
    ) -> Option<&[ParamTypeSpec]> {
        if let Some(types) = self.param_types.get(&(name.clone(), arity)) {
            return Some(types.as_slice());
        }
        if name.ns_uri.is_none()
            && let Some(ns) = default_ns
        {
            let mut resolved = name.clone();
            resolved.ns_uri = Some(ns.to_string());
            if let Some(types) = self.param_types.get(&(resolved, arity)) {
                return Some(types.as_slice());
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct StaticContext {
    pub base_uri: Option<String>,
    pub default_function_namespace: Option<String>,
    pub default_element_namespace: Option<String>,
    pub default_collation: Option<String>,
    pub namespaces: NamespaceBindings,
    pub in_scope_variables: HashSet<ExpandedName>,
    pub function_signatures: FunctionSignatures,
    pub statically_known_collations: HashSet<String>,
    pub xpath_compatibility_mode: bool,
    pub context_item_type: Option<ir::SeqTypeIR>,
    pub(crate) compile_cache: Arc<Mutex<LruCache<String, Arc<ir::InstrSeq>>>>,
}

impl Default for StaticContext {
    fn default() -> Self {
        let mut ns = NamespaceBindings::default();
        // Ensure implicit xml namespace binding (cannot be overridden per spec)
        ns.by_prefix.insert("xml".to_string(), crate::consts::XML_URI.to_string());
        let mut collations: HashSet<String> = HashSet::new();
        collations.insert(CODEPOINT_URI.to_string());
        // STATIC_CONTEXT_COMPILE_CACHE_CAPACITY is a non-zero constant; be defensive anyway.
        let cache_capacity = NonZeroUsize::new(STATIC_CONTEXT_COMPILE_CACHE_CAPACITY)
            .unwrap_or_else(|| NonZeroUsize::new(1).expect("1 is a valid non-zero size"));
        Self {
            base_uri: None,
            default_function_namespace: Some(crate::consts::FNS.to_string()),
            default_element_namespace: None,
            default_collation: Some(CODEPOINT_URI.to_string()),
            namespaces: ns,
            in_scope_variables: HashSet::new(),
            function_signatures: crate::engine::functions::default_function_signatures(),
            statically_known_collations: collations,
            xpath_compatibility_mode: false,
            context_item_type: None,
            compile_cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
        }
    }
}

/// Builder for `StaticContext`: allows explicit namespace registrations
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
        Self { ctx: StaticContext::default() }
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

    pub fn with_default_element_namespace(mut self, uri: impl Into<String>) -> Self {
        self.ctx.default_element_namespace = Some(uri.into());
        self
    }

    pub fn with_xpath_compatibility_mode(mut self, enabled: bool) -> Self {
        self.ctx.xpath_compatibility_mode = enabled;
        self
    }

    pub fn with_context_item_type(mut self, ty: ir::SeqTypeIR) -> Self {
        self.ctx.context_item_type = Some(ty);
        self
    }

    pub fn clear_context_item_type(mut self) -> Self {
        self.ctx.context_item_type = None;
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

    /// Register an in-scope variable that may be referenced without being bound locally.
    pub fn with_variable(mut self, name: ExpandedName) -> Self {
        self.ctx.in_scope_variables.insert(name);
        self
    }

    pub fn with_function_signature(mut self, name: ExpandedName, min: usize, max: Option<usize>) -> Self {
        self.ctx.function_signatures.register(name, min, max);
        self
    }

    pub fn with_function_signature_ns(mut self, ns: &str, local: &str, min: usize, max: Option<usize>) -> Self {
        self.ctx.function_signatures.register_ns(ns, local, min, max);
        self
    }

    pub fn with_collation(mut self, uri: impl Into<String>) -> Self {
        self.ctx.statically_known_collations.insert(uri.into());
        self
    }

    pub fn with_function_signatures(mut self, sigs: FunctionSignatures) -> Self {
        self.ctx.function_signatures = sigs;
        self
    }

    pub fn with_collations(mut self, collations: HashSet<String>) -> Self {
        self.ctx.statically_known_collations = collations;
        self
    }

    pub fn build(self) -> StaticContext {
        self.ctx
    }
}

#[derive(Clone)]
pub struct DynamicContext<N> {
    pub context_item: Option<XdmItem<N>>,
    pub variables: VariableBindings<N>,
    pub default_collation: Option<String>,
    pub functions: Option<Rc<FunctionImplementations<N>>>,
    pub collations: Rc<CollationRegistry>,
    pub node_resolver: Option<Arc<dyn NodeResolver<N>>>,
    pub regex: Option<Rc<dyn RegexProvider>>,
    pub now: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub timezone_override: Option<chrono::FixedOffset>,
    pub cancel_flag: Option<Arc<AtomicBool>>,
}

impl<N: 'static + crate::model::XdmNode + Clone> Default for DynamicContext<N> {
    fn default() -> Self {
        Self {
            context_item: None,
            variables: VariableBindings::default(),
            default_collation: None,
            functions: None,
            collations: Rc::new(CollationRegistry::default()),
            node_resolver: None,
            regex: None,
            now: None,
            timezone_override: None,
            cancel_flag: None,
        }
    }
}

pub struct DynamicContextBuilder<N> {
    ctx: DynamicContext<N>,
}

impl<N: 'static + crate::model::XdmNode + Clone> Default for DynamicContextBuilder<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: 'static + crate::model::XdmNode + Clone> DynamicContextBuilder<N> {
    pub fn new() -> Self {
        Self { ctx: DynamicContext::default() }
    }

    pub fn with_context_item(mut self, item: impl Into<XdmItem<N>>) -> Self {
        self.ctx.context_item = Some(item.into());
        self
    }

    pub fn with_variable(mut self, name: ExpandedName, value: impl Into<XdmSequence<N>>) -> Self {
        self.ctx.variables = self.ctx.variables.with_binding(name, value.into());
        self
    }

    pub fn with_default_collation(mut self, uri: impl Into<String>) -> Self {
        self.ctx.default_collation = Some(uri.into());
        self
    }

    pub fn with_functions(mut self, reg: Rc<FunctionImplementations<N>>) -> Self {
        self.ctx.functions = Some(reg);
        self
    }

    pub fn with_collations(mut self, reg: Rc<CollationRegistry>) -> Self {
        self.ctx.collations = reg;
        self
    }

    pub fn with_node_resolver(mut self, res: Arc<dyn NodeResolver<N>>) -> Self {
        self.ctx.node_resolver = Some(res);
        self
    }

    pub fn with_regex(mut self, provider: Rc<dyn RegexProvider>) -> Self {
        self.ctx.regex = Some(provider);
        self
    }

    // Set a fixed 'now' instant for deterministic date/time functions
    pub fn with_now(mut self, now: chrono::DateTime<chrono::FixedOffset>) -> Self {
        self.ctx.now = Some(now);
        self
    }

    // Override timezone for current-* formatting (applied to 'now' if set)
    pub fn with_timezone(mut self, offset_minutes: i32) -> Self {
        let hours = offset_minutes / 60;
        let mins = offset_minutes % 60;
        if let Some(tz) = chrono::FixedOffset::east_opt(hours * 3600 + mins * 60) {
            self.ctx.timezone_override = Some(tz);
        }
        self
    }

    pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.ctx.cancel_flag = Some(flag);
        self
    }

    pub fn build(self) -> DynamicContext<N> {
        self.ctx
    }
}

impl<N: 'static + crate::model::XdmNode + Clone> DynamicContext<N> {
    pub fn provide_functions(&self) -> Rc<FunctionImplementations<N>> {
        if let Some(f) = &self.functions {
            f.clone()
        } else {
            crate::engine::functions::default_function_registry::<N>()
        }
    }

    pub fn with_context_item(&self, item: impl Into<Option<XdmItem<N>>>) -> Self {
        let mut clone = self.clone();
        clone.context_item = item.into();
        clone
    }

    pub fn with_variable(&self, name: ExpandedName, value: XdmSequence<N>) -> Self {
        let mut clone = self.clone();
        clone.variables = clone.variables.with_binding(name, value);
        clone
    }

    pub fn variable(&self, name: &ExpandedName) -> Option<XdmSequence<N>>
    where
        N: Clone,
    {
        self.variables.get(name)
    }
}
