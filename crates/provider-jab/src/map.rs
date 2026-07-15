//! Role and state mapping from JAB's `*_en_US` strings into the PlatynUI
//! vocabulary.
//!
//! The map is keyed on `role_en_US` (locale-independent); unknown roles fall
//! back to generic PascalCase and are never an error. Where the JAB and
//! AT-SPI2 vocabularies coincide ("push button", "check box", …) the mapping
//! follows the AT-SPI2 column of `dev-docs/architecture.md` §6.4, so the same
//! Swing app tends to answer the same selectors on Windows and Linux — a soft
//! goal, resolved in favor of PlatynUI semantics where they diverge (JAB's
//! `spinbox` → `SpinButton`).

use platynui_core::ui::Namespace;

/// Parsed `states_en_US` tokens (comma-separated, e.g.
/// `"enabled,focusable,visible,showing"`). Only the states the provider
/// consumes are surfaced; the raw string stays available as `native:States`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // faithful bitset of independent accessibility states
pub(crate) struct StateFlags {
    pub enabled: bool,
    pub focusable: bool,
    pub focused: bool,
    pub visible: bool,
    pub showing: bool,
    pub checked: bool,
    pub selectable: bool,
    pub selected: bool,
    pub multiselectable: bool,
    pub expandable: bool,
    pub expanded: bool,
    pub editable: bool,
    pub modal: bool,
}

pub(crate) fn parse_states(states_en_us: &str) -> StateFlags {
    let mut flags = StateFlags::default();
    for token in states_en_us.split(',') {
        match token.trim() {
            "enabled" => flags.enabled = true,
            "focusable" => flags.focusable = true,
            "focused" => flags.focused = true,
            "visible" => flags.visible = true,
            "showing" => flags.showing = true,
            "checked" => flags.checked = true,
            "selectable" => flags.selectable = true,
            "selected" => flags.selected = true,
            "multiselectable" => flags.multiselectable = true,
            "expandable" => flags.expandable = true,
            "expanded" => flags.expanded = true,
            "editable" => flags.editable = true,
            "modal" => flags.modal = true,
            _ => {}
        }
    }
    flags
}

/// Map a JAB role to `(namespace, PlatynUI role)`.
///
/// `is_top_level` overrides window-ish roles for real top-level windows
/// (`frame`/`window` → `Window`, `dialog` → `Dialog`).
///
/// JList quirk (decided here, see the `add-jab-provider` design's open
/// question): Swing list entries report role `label` with the `selectable`
/// state — both through JAB and through the Linux ATK wrapper. They are
/// promoted to `item:ListItem` when the parent is a `list`, so Swing lists
/// answer `item:` selectors like every other toolkit's lists; the verbatim
/// role stays visible as `native:Role`.
pub(crate) fn map_role(
    role_en_us: &str,
    parent_role_en_us: Option<&str>,
    states: StateFlags,
    is_top_level: bool,
) -> (Namespace, String) {
    if is_top_level {
        match role_en_us {
            "dialog" => return (Namespace::Control, "Dialog".to_string()),
            "frame" | "window" => return (Namespace::Control, "Window".to_string()),
            _ => {}
        }
    }

    if role_en_us == "label" && parent_role_en_us == Some("list") && states.selectable {
        return (Namespace::Item, "ListItem".to_string());
    }

    let (namespace, name): (Namespace, &str) = match role_en_us {
        "alert" => (Namespace::Control, "Alert"),
        "canvas" => (Namespace::Control, "Canvas"),
        "check box" => (Namespace::Control, "CheckBox"),
        "color chooser" => (Namespace::Control, "ColorChooser"),
        "column header" => (Namespace::Item, "ColumnHeader"),
        "combo box" => (Namespace::Control, "ComboBox"),
        "desktop icon" => (Namespace::Control, "DesktopIcon"),
        "dialog" => (Namespace::Control, "Dialog"),
        "directory pane" => (Namespace::Control, "DirectoryPane"),
        "file chooser" => (Namespace::Control, "FileChooser"),
        "filler" => (Namespace::Control, "Filler"),
        "frame" => (Namespace::Control, "Frame"),
        "glass pane" => (Namespace::Control, "GlassPane"),
        "icon" => (Namespace::Control, "Icon"),
        "internal frame" => (Namespace::Control, "InternalFrame"),
        "label" => (Namespace::Control, "Label"),
        "layered pane" => (Namespace::Control, "LayeredPane"),
        "list" => (Namespace::Control, "List"),
        "list item" => (Namespace::Item, "ListItem"),
        "menu" => (Namespace::Control, "Menu"),
        "menu bar" => (Namespace::Control, "MenuBar"),
        "menu item" => (Namespace::Control, "MenuItem"),
        "option pane" => (Namespace::Control, "OptionPane"),
        "page tab" => (Namespace::Item, "TabItem"),
        "page tab list" => (Namespace::Control, "Tab"),
        "panel" => (Namespace::Control, "Panel"),
        "password text" => (Namespace::Control, "PasswordText"),
        "popup menu" => (Namespace::Control, "PopupMenu"),
        "progress bar" => (Namespace::Control, "ProgressBar"),
        "push button" => (Namespace::Control, "Button"),
        "radio button" => (Namespace::Control, "RadioButton"),
        "root pane" => (Namespace::Control, "RootPane"),
        "row header" => (Namespace::Item, "RowHeader"),
        "scroll bar" => (Namespace::Control, "ScrollBar"),
        "scroll pane" => (Namespace::Control, "ScrollPane"),
        "separator" => (Namespace::Control, "Separator"),
        "slider" => (Namespace::Control, "Slider"),
        // Swing reports `spinbox` where AT-SPI says `spin button`; PlatynUI
        // uses the AT-SPI-aligned name.
        "spinbox" => (Namespace::Control, "SpinButton"),
        "split pane" => (Namespace::Control, "SplitPane"),
        "status bar" => (Namespace::Control, "StatusBar"),
        "table" => (Namespace::Control, "Table"),
        "text" => (Namespace::Control, "Text"),
        "toggle button" => (Namespace::Control, "ToggleButton"),
        "tool bar" => (Namespace::Control, "ToolBar"),
        "tool tip" => (Namespace::Control, "ToolTip"),
        "tree" => (Namespace::Control, "Tree"),
        "unknown" => (Namespace::Control, "Unknown"),
        "viewport" => (Namespace::Control, "Viewport"),
        "window" => (Namespace::Control, "Window"),
        other => return (Namespace::Control, pascal_case(other)),
    };
    (namespace, name.to_string())
}

/// Generic fallback: whitespace-separated words, each capitalized
/// (`"desktop pane"` → `"DesktopPane"`). Empty input maps to `"Unknown"` so a
/// role is never an empty XPath name.
pub(crate) fn pascal_case(role: &str) -> String {
    let mut out = String::with_capacity(role.len());
    for word in role.split_whitespace() {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    if out.is_empty() { "Unknown".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(tokens: &str) -> StateFlags {
        parse_states(tokens)
    }

    // ---- parse_states ----

    #[test]
    fn parse_states_typical_control() {
        let flags = parse_states("enabled,focusable,visible,showing");
        assert!(flags.enabled && flags.focusable && flags.visible && flags.showing);
        assert!(!flags.focused && !flags.checked && !flags.editable);
    }

    #[test]
    fn parse_states_handles_spaces_and_unknown_tokens() {
        let flags = parse_states("enabled, focused , opaque,horizontal");
        assert!(flags.enabled && flags.focused);
        assert!(!flags.visible, "unknown tokens must not bleed into other flags");
    }

    #[test]
    fn parse_states_empty_is_all_false() {
        assert_eq!(parse_states(""), StateFlags::default());
    }

    #[test]
    fn parse_states_checked_selected_expanded_editable_modal() {
        let flags = parse_states("checked,selectable,selected,multiselectable,expandable,expanded,editable,modal");
        assert!(flags.checked && flags.selectable && flags.selected);
        assert!(flags.multiselectable && flags.expandable && flags.expanded);
        assert!(flags.editable && flags.modal);
    }

    // ---- role map: the spike-harvested vocabulary, pinned ----

    #[test]
    fn spike_role_table_is_pinned() {
        // Verbatim `role_en_US` values harvested from the Swing fixture app
        // (add-swing-test-app spike, Temurin 8). AT-SPI2-aligned targets.
        let expectations = [
            ("root pane", Namespace::Control, "RootPane"),
            ("panel", Namespace::Control, "Panel"),
            ("layered pane", Namespace::Control, "LayeredPane"),
            ("glass pane", Namespace::Control, "GlassPane"),
            ("menu bar", Namespace::Control, "MenuBar"),
            ("menu", Namespace::Control, "Menu"),
            ("menu item", Namespace::Control, "MenuItem"),
            ("push button", Namespace::Control, "Button"),
            ("text", Namespace::Control, "Text"),
            ("label", Namespace::Control, "Label"),
            ("check box", Namespace::Control, "CheckBox"),
            ("radio button", Namespace::Control, "RadioButton"),
            ("combo box", Namespace::Control, "ComboBox"),
            ("popup menu", Namespace::Control, "PopupMenu"),
            ("scroll pane", Namespace::Control, "ScrollPane"),
            ("viewport", Namespace::Control, "Viewport"),
            ("list", Namespace::Control, "List"),
            ("scroll bar", Namespace::Control, "ScrollBar"),
            ("slider", Namespace::Control, "Slider"),
            ("spinbox", Namespace::Control, "SpinButton"),
            ("progress bar", Namespace::Control, "ProgressBar"),
        ];
        for (role, namespace, name) in expectations {
            let (ns, mapped) = map_role(role, None, StateFlags::default(), false);
            assert_eq!((ns, mapped.as_str()), (namespace, name), "role {role:?}");
        }
    }

    #[test]
    fn top_level_frame_maps_to_window() {
        let (ns, name) = map_role("frame", None, StateFlags::default(), true);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "Window"));
        let (ns, name) = map_role("dialog", None, StateFlags::default(), true);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "Dialog"));
    }

    #[test]
    fn non_top_level_frame_stays_frame() {
        let (ns, name) = map_role("frame", Some("desktop pane"), StateFlags::default(), false);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "Frame"));
    }

    #[test]
    fn jlist_label_entries_are_promoted_to_list_items() {
        let (ns, name) = map_role("label", Some("list"), flags("selectable,visible"), false);
        assert_eq!((ns, name.as_str()), (Namespace::Item, "ListItem"));
    }

    #[test]
    fn plain_labels_are_not_promoted() {
        // Not under a list …
        let (ns, name) = map_role("label", Some("panel"), flags("selectable"), false);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "Label"));
        // … or under a list but not selectable (e.g. a decorative label).
        let (ns, name) = map_role("label", Some("list"), StateFlags::default(), false);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "Label"));
    }

    #[test]
    fn item_namespace_roles() {
        assert_eq!(map_role("list item", None, StateFlags::default(), false).0, Namespace::Item);
        assert_eq!(map_role("page tab", None, StateFlags::default(), false).0, Namespace::Item);
        assert_eq!(map_role("column header", None, StateFlags::default(), false).0, Namespace::Item);
    }

    #[test]
    fn unknown_roles_fall_back_to_pascal_case() {
        let (ns, name) = map_role("desktop pane", None, StateFlags::default(), false);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "DesktopPane"));
        let (ns, name) = map_role("hyperlink thing", None, StateFlags::default(), false);
        assert_eq!((ns, name.as_str()), (Namespace::Control, "HyperlinkThing"));
    }

    // ---- pascal_case ----

    #[test]
    fn pascal_case_examples() {
        assert_eq!(pascal_case("internal frame"), "InternalFrame");
        assert_eq!(pascal_case("push button"), "PushButton");
        assert_eq!(pascal_case("text"), "Text");
        assert_eq!(pascal_case("  spaced   out  "), "SpacedOut");
        assert_eq!(pascal_case(""), "Unknown");
    }
}
