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

/// Identifies capability patterns (see `docs/patterns.md`).
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
        let pattern = PatternId::from("TextContent");
        assert_eq!(pattern.to_string(), "TextContent");
    }
}
