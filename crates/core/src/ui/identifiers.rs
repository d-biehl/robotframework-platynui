use std::fmt::{Display, Formatter};
use std::sync::Arc;

/// Identifies a node uniquely within the provider/runtime boundary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeId(Arc<str>);

impl RuntimeId {
    pub fn new<T: Into<Arc<str>>>(value: T) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RuntimeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for RuntimeId {
    fn from(value: &str) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<String> for RuntimeId {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<Arc<str>> for RuntimeId {
    fn from(value: Arc<str>) -> Self {
        Self::new(value)
    }
}

/// Identifies the technology that surfaced a node (UIAutomation, AT-SPI, ...).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TechnologyId(Arc<str>);

impl TechnologyId {
    pub fn new<T: Into<Arc<str>>>(value: T) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for TechnologyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for TechnologyId {
    fn from(value: &str) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<String> for TechnologyId {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<Arc<str>> for TechnologyId {
    fn from(value: Arc<str>) -> Self {
        Self::new(value)
    }
}

/// Identifies capability patterns (see `docs/architecture.md` §6).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternId(Arc<str>);

impl PatternId {
    pub fn new<T: Into<Arc<str>>>(value: T) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PatternId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for PatternId {
    fn from(value: &str) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<String> for PatternId {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

impl From<Arc<str>> for PatternId {
    fn from(value: Arc<str>) -> Self {
        Self::new(value)
    }
}

/// Canonical pattern identifier strings in reverse-DNS form.
///
/// All `PatternId` values used inside the workspace MUST be constructed from
/// one of these constants (or via the `provider-mock` XML loader, which
/// accepts the same strings verbatim). The reverse-DNS scheme keeps Rust and
/// Python identifiers literally interchangeable — see
/// `docs/python-library-design.md` §13.6 and §11.3.
///
/// New patterns extend this module; do not introduce ad-hoc string literals
/// at call sites.
pub mod pattern_ids {
    /// Production patterns (currently exposed by real providers).
    pub const FOCUSABLE: &str = "org.platynui.patterns.Focusable";
    pub const WINDOW_SURFACE: &str = "org.platynui.patterns.WindowSurface";
    pub const ACTIVATABLE: &str = "org.platynui.patterns.Activatable";
    pub const DESKTOP: &str = "org.platynui.patterns.Desktop";
    pub const APPLICATION: &str = "org.platynui.patterns.Application";

    /// Patterns currently only used by tests, contract testkit, and mock
    /// providers. Listed here for symmetry; once promoted to production they
    /// stay in the same module.
    pub const ELEMENT: &str = "org.platynui.patterns.Element";
    pub const TEXT_CONTENT: &str = "org.platynui.patterns.TextContent";
    pub const ACTIVATION_TARGET: &str = "org.platynui.patterns.ActivationTarget";

    /// Synthetic patterns used exclusively by unit tests and contract fixtures.
    /// Reverse-DNS form preserves consistency with real patterns.
    pub const MOCK: &str = "org.platynui.patterns.Mock";
    pub const DUMMY: &str = "org.platynui.patterns.Dummy";
    pub const LAZY: &str = "org.platynui.patterns.Lazy";
    pub const OTHER: &str = "org.platynui.patterns.Other";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_id_display_roundtrip() {
        let id = RuntimeId::from("abc");
        assert_eq!(id.to_string(), "abc");
    }

    #[test]
    fn technology_id_display_roundtrip() {
        let tech = TechnologyId::from("UIAutomation");
        assert_eq!(tech.to_string(), "UIAutomation");
    }

    #[test]
    fn pattern_id_display_roundtrip() {
        let pattern = PatternId::from(pattern_ids::TEXT_CONTENT);
        assert_eq!(pattern.to_string(), "org.platynui.patterns.TextContent");
    }

    #[test]
    fn pattern_ids_are_reverse_dns() {
        for id in [
            pattern_ids::FOCUSABLE,
            pattern_ids::WINDOW_SURFACE,
            pattern_ids::ACTIVATABLE,
            pattern_ids::DESKTOP,
            pattern_ids::APPLICATION,
            pattern_ids::ELEMENT,
            pattern_ids::TEXT_CONTENT,
            pattern_ids::ACTIVATION_TARGET,
            pattern_ids::MOCK,
            pattern_ids::DUMMY,
            pattern_ids::LAZY,
            pattern_ids::OTHER,
        ] {
            assert!(id.starts_with("org.platynui.patterns."), "pattern id `{id}` must use reverse-DNS prefix",);
        }
    }
}
