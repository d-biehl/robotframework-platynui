//! The one-line description of a UI element used in diagnostics and errors.

use std::fmt::Write as _;

use super::UiNode;

/// How many characters of a name a description keeps.
const NAME_LIMIT: usize = 60;

/// Describes `node` in one line: `Role "Name" #Id`.
///
/// The name is cut to at most 60 characters (a longer one keeps its first 59
/// and `…`) and then escaped, so quotes, backslashes and line breaks never
/// break the line. The id follows after `#` when it is set and not blank.
///
/// This asks the provider for the name and the id, which can cost a call into
/// the application per element; build a description only where it is logged
/// or raised.
#[must_use]
pub fn describe(node: &dyn UiNode) -> String {
    describe_parts(node.role(), &node.name(), node.id().as_deref())
}

/// [`describe`] for a role, name and id that are already known.
#[must_use]
pub fn describe_parts(role: &str, name: &str, id: Option<&str>) -> String {
    let mut description = String::with_capacity(role.len() + name.len().min(4 * NAME_LIMIT) + 4);
    description.push_str(role);
    description.push_str(" \"");
    if name.chars().nth(NAME_LIMIT).is_some() {
        let cut: String = name.chars().take(NAME_LIMIT - 1).collect();
        push_escaped(&mut description, &cut);
        description.push('…');
    } else {
        push_escaped(&mut description, name);
    }
    description.push('"');
    if let Some(id) = id.filter(|id| !id.trim().is_empty()) {
        description.push_str(" #");
        push_escaped(&mut description, id);
    }
    description
}

/// Escapes like `str`'s `Debug`: quotes, backslashes and control characters.
fn push_escaped(out: &mut String, text: &str) {
    let quoted = format!("{text:?}");
    let _ = write!(out, "{}", &quoted[1..quoted.len() - 1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode};
    use std::sync::{Arc, Weak};

    struct Stub {
        role: &'static str,
        name: String,
        id: Option<String>,
        runtime_id: RuntimeId,
    }

    impl Stub {
        fn new(role: &'static str, name: &str, id: Option<&str>) -> Self {
            Self { role, name: name.to_owned(), id: id.map(str::to_owned), runtime_id: RuntimeId::from("stub") }
        }
    }

    impl UiNode for Stub {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            self.role
        }
        fn name(&self) -> String {
            self.name.clone()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn id(&self) -> Option<String> {
            self.id.clone()
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn supported_patterns(&self) -> Vec<PatternName> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    fn described(role: &'static str, name: &str, id: Option<&str>) -> String {
        describe(&Stub::new(role, name, id))
    }

    #[test]
    fn role_and_quoted_name() {
        assert_eq!(described("Button", "OK", None), r#"Button "OK""#);
    }

    #[test]
    fn an_id_follows_after_a_hash() {
        assert_eq!(described("Button", "OK", Some("ok")), r#"Button "OK" #ok"#);
    }

    #[test]
    fn a_blank_id_counts_as_absent() {
        assert_eq!(described("Button", "OK", Some("  ")), r#"Button "OK""#);
        assert_eq!(described("Button", "OK", Some("")), r#"Button "OK""#);
    }

    #[test]
    fn quotes_backslashes_and_line_breaks_are_escaped() {
        let description = described("Text", "say \"hi\"\\\nbye", Some("a\nb"));
        assert_eq!(description, r#"Text "say \"hi\"\\\nbye" #a\nb"#);
        assert!(!description.contains('\n'));
    }

    #[test]
    fn a_long_name_is_cut_at_a_character_boundary() {
        let name: String = "ä".repeat(61);
        let expected = format!("Text \"{}…\"", "ä".repeat(59));
        assert_eq!(described("Text", &name, None), expected);
        let sixty: String = "x".repeat(60);
        assert_eq!(described("Text", &sixty, None), format!("Text \"{sixty}\""), "60 characters are kept whole");
    }

    #[test]
    fn an_empty_name_is_shown_as_empty_quotes() {
        assert_eq!(described("Pane", "", None), r#"Pane """#);
    }

    #[test]
    fn the_name_is_cut_before_it_is_escaped() {
        let name = format!("{}\n{}", "x".repeat(58), "y".repeat(2));
        assert_eq!(name.chars().count(), 61);
        let description = described("Text", &name, None);
        assert!(description.ends_with("\\n…\""), "{description}");
    }
}
