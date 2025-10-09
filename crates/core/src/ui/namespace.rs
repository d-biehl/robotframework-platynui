use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Known namespaces within the PlatynUI document model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Namespace {
    /// Default namespace for Controls (Buttons, TextBoxes, etc.).
    #[default]
    Control,
    /// Namespace for items belonging to container controls (ListItem, TreeItem, ...).
    Item,
    /// Namespace exposing application level information (processes, packages).
    App,
    /// Namespace mirroring the raw technology specific data.
    Native,
}

impl Namespace {
    /// Returns the canonical prefix (used as namespace identifier inside XPath expressions).
    pub const fn as_str(self) -> &'static str {
        match self {
            Namespace::Control => "control",
            Namespace::Item => "item",
            Namespace::App => "app",
            Namespace::Native => "native",
        }
    }

    /// Returns whether this namespace is the default one for XPath queries.
    pub const fn is_default(self) -> bool {
        matches!(self, Namespace::Control)
    }
}

impl Display for Namespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Namespace {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "control" | "" => Ok(Namespace::Control),
            "item" => Ok(Namespace::Item),
            "app" => Ok(Namespace::App),
            "native" => Ok(Namespace::Native),
            _ => Err("unknown namespace"),
        }
    }
}

static PREFIX_LOOKUP: Lazy<HashMap<&'static str, Namespace>> = Lazy::new(|| {
    let mut map = HashMap::with_capacity(4);
    map.insert("control", Namespace::Control);
    map.insert("item", Namespace::Item);
    map.insert("app", Namespace::App);
    map.insert("native", Namespace::Native);
    map
});

/// Resolve a namespace prefix (``None`` or empty string selects the default namespace).
pub fn resolve_namespace(prefix: Option<&str>) -> Namespace {
    match prefix {
        None => Namespace::Control,
        Some("") => Namespace::Control,
        Some(p) => match PREFIX_LOOKUP.get(p) {
            Some(ns) => *ns,
            None => panic!("unknown namespace prefix: {}", p),
        },
    }
}

/// Convenience iterator exposing every known namespace.
pub fn all_namespaces() -> impl Iterator<Item = Namespace> {
    [Namespace::Control, Namespace::Item, Namespace::App, Namespace::Native].into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(None, Namespace::Control)]
    #[case(Some(""), Namespace::Control)]
    #[case(Some("control"), Namespace::Control)]
    #[case(Some("item"), Namespace::Item)]
    #[case(Some("app"), Namespace::App)]
    #[case(Some("native"), Namespace::Native)]
    fn resolve_namespace_handles_known_prefixes(#[case] input: Option<&'static str>, #[case] expected: Namespace) {
        assert_eq!(resolve_namespace(input), expected);
    }

    #[test]
    fn display_matches_prefix() {
        for ns in all_namespaces() {
            assert_eq!(ns.to_string(), ns.as_str());
        }
    }

    #[rstest]
    fn default_namespace_is_control() {
        assert_eq!(Namespace::default(), Namespace::Control);
    }

    #[rstest]
    #[case("control", Namespace::Control)]
    #[case("item", Namespace::Item)]
    #[case("app", Namespace::App)]
    #[case("native", Namespace::Native)]
    fn from_str_recognises_known_names(#[case] input: &str, #[case] expected: Namespace) {
        assert_eq!(Namespace::from_str(input).unwrap(), expected);
    }
}
