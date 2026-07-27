//! The agent's element payload, and its mapping into the PlatynUI vocabulary.
//!
//! Pure data and pure functions: no connection, no nodes. That is what lets the
//! mapping be covered by unit tests against recorded payloads, while the tree
//! behaviour itself stays real-provider-only.
//!
//! # One vocabulary, two backends
//!
//! The agent emits roles and states as their `Locale.ENGLISH` display strings,
//! which are exactly the `role_en_US` / `states_en_US` vocabularies the Java
//! Access Bridge reports. That is deliberate and it is what keeps a locator
//! written against the JAB backend matching when the agent takes over the same
//! window. The mapping below therefore mirrors
//! `platynui-provider-java-jab`'s — same inputs, same outputs — and lives here
//! rather than being imported from there because the router has to become
//! portable (`java-provider-linux`) while that crate stays Windows-only.

use platynui_core::platform::java::JavaToolkit;
use platynui_core::types::Rect;
use platynui_core::ui::Namespace;
use serde::Deserialize;

/// One element as the agent describes it. Every optional block is absent rather
/// than zeroed when it does not apply — "no bounds" and "a zero-sized
/// rectangle" are different answers.
#[derive(Debug, Clone, Deserialize)]
// A faithful mirror of the wire payload; the flags are independent facts about
// the element, and packing them into one type is what makes the frame coarse.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Element {
    /// The agent registry's id: identity-based and stable for the element's
    /// lifetime, which is what makes an identity-stable `RuntimeId` possible.
    pub id: u64,
    pub kind: Kind,
    /// Accessible role in the bridge's vocabulary (`"push button"`, …).
    pub role: String,
    #[serde(rename = "className")]
    pub class_name: String,
    /// `Component.getName()` — the spine's exclusive contribution, and for a
    /// table cell the model value. Invisible to any out-of-process bridge.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "accessibleName", default)]
    pub accessible_name: Option<String>,
    #[serde(rename = "accessibleDescription", default)]
    pub accessible_description: Option<String>,
    #[serde(default)]
    pub bounds: Option<Bounds>,
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(rename = "childCount", default)]
    pub child_count: u64,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub showing: bool,
    #[serde(default)]
    pub focusable: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub editable: Option<bool>,
    #[serde(rename = "toolTipText", default)]
    pub tool_tip_text: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub selection: Option<Selection>,
    #[serde(default)]
    pub table: Option<Table>,
    #[serde(default)]
    pub cell: Option<Cell>,
    #[serde(rename = "columnHeader", default)]
    pub column_header: Option<ColumnHeader>,
    #[serde(rename = "clientProperties", default)]
    pub client_properties: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub window: Option<Window>,
}

/// What kind of thing an element is. Unknown kinds map to
/// [`Kind::Accessible`] rather than failing the frame: a newer agent adding one
/// must not make an older provider unable to read the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Window,
    Component,
    Cell,
    #[serde(other)]
    Accessible,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<Bounds> for Rect {
    fn from(bounds: Bounds) -> Self {
        Rect::new(bounds.x, bounds.y, bounds.width, bounds.height)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Value {
    #[serde(default)]
    pub current: Option<f64>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Selection {
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub indices: Vec<u64>,
    /// Element ids of the selected children, when the agent could name them
    /// exactly. Absent when the accessible order and the tree order are not
    /// guaranteed to agree, or when the scan hit its bound — in both cases the
    /// honest answer is no list rather than a partial one.
    #[serde(default)]
    pub ids: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Table {
    #[serde(default)]
    pub rows: u64,
    #[serde(default)]
    pub columns: u64,
    #[serde(rename = "selectedRows", default)]
    pub selected_rows: Vec<u64>,
    #[serde(rename = "selectedColumns", default)]
    pub selected_columns: Vec<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Cell {
    pub row: u64,
    pub column: u64,
    #[serde(rename = "rowExtent", default = "one")]
    pub row_extent: u64,
    #[serde(rename = "columnExtent", default = "one")]
    pub column_extent: u64,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub editable: bool,
}

fn one() -> u64 {
    1
}

/// A table column header, described from the header component rather than from
/// the accessible wrapper that reports it as an unlaid-out label.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct ColumnHeader {
    /// View position — what the user sees, left to right.
    pub column: u64,
    /// Position in the table model, which survives the user reordering columns.
    #[serde(rename = "modelIndex", default)]
    pub model_index: u64,
    #[serde(default)]
    pub resizable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // as above: independent window facts, one frame
pub(crate) struct Window {
    /// Native handle read from inside the JVM, or `None` when no in-JVM strategy
    /// worked — which is when the provider's PID+geometry fallback takes over.
    #[serde(default)]
    pub handle: Option<u64>,
    /// Which in-JVM strategy answered, or `"none"`. Diagnostic only, and worth
    /// keeping: it turns "no handle on this JDK" into a named, reportable cell.
    #[serde(rename = "handleSource", default)]
    pub handle_source: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub resizable: bool,
    #[serde(rename = "extendedState", default)]
    pub extended_state: i64,
    #[serde(rename = "alwaysOnTop", default)]
    pub always_on_top: bool,
}

/// `Frame.MAXIMIZED_BOTH`, from `java.awt.Frame`.
pub(crate) const FRAME_MAXIMIZED_BOTH: i64 = 6;
/// `Frame.ICONIFIED`, from `java.awt.Frame`.
pub(crate) const FRAME_ICONIFIED: i64 = 1;

impl Element {
    /// The node's display name.
    ///
    /// For anything but a top-level window, `Component.getName()` first: it is the
    /// developer's own identifier, stable across relayouts, and the one thing no
    /// out-of-process bridge can see. The accessible name is the fallback, which
    /// is also what keeps JAB-era locators matching — those were written against
    /// it because it was all there was.
    ///
    /// **A window is named by its title**, and that ordering is deliberate: a
    /// window's title is what the user sees, what the window manager shows and
    /// what every other provider names it by, so a locator like
    /// `//Window[@Name="..."]` has to keep working. A window's component name is
    /// an internal identifier at best (the agent already drops AWT's
    /// auto-generated ones), and letting it win here would rename every window in
    /// the tree.
    pub fn display_name(&self) -> String {
        if let Some(title) =
            self.window.as_ref().and_then(|window| window.title.as_deref()).filter(|title| !title.is_empty())
        {
            return title.to_owned();
        }
        self.name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(self.accessible_name.as_deref())
            .unwrap_or_default()
            .to_owned()
    }

    /// The developer-provided stable id (`control:Id`), when there is one.
    ///
    /// **Only real components have one.** `name` carries different things for
    /// different kinds: for a component it is `Component.getName()`, which the
    /// application set deliberately; for a cell it is the *model value*, and for
    /// an accessibility-only child the accessible name. Publishing those as
    /// `control:Id` would promise stability that content does not have — a
    /// locator `//*[@Id="r2c1"]` would match a table cell until somebody edits
    /// the data.
    pub fn stable_id(&self) -> Option<String> {
        if !matches!(self.kind, Kind::Component | Kind::Window) {
            return None;
        }
        self.name.as_deref().filter(|name| !name.is_empty()).map(std::borrow::ToOwned::to_owned)
    }

    pub fn state_flags(&self) -> StateFlags {
        StateFlags::from_display_strings(&self.states)
    }

    pub fn rect(&self) -> Option<Rect> {
        self.bounds.map(Rect::from)
    }

    pub fn is_top_level(&self) -> bool {
        self.kind == Kind::Window
    }
}

/// The accessible states the provider consumes. The verbatim list stays
/// available as `native:States`, so nothing is lost by narrowing here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // faithful bitset of independent accessibility states
pub(crate) struct StateFlags {
    pub selectable: bool,
    pub selected: bool,
    pub multiselectable: bool,
    pub checked: bool,
    pub expandable: bool,
    pub expanded: bool,
    pub editable: bool,
    pub modal: bool,
    pub indeterminate: bool,
}

impl StateFlags {
    fn from_display_strings(states: &[String]) -> Self {
        let mut flags = Self::default();
        for state in states {
            match state.as_str() {
                "selectable" => flags.selectable = true,
                "selected" => flags.selected = true,
                "multiselectable" => flags.multiselectable = true,
                "checked" => flags.checked = true,
                "expandable" => flags.expandable = true,
                "expanded" => flags.expanded = true,
                "editable" => flags.editable = true,
                "modal" => flags.modal = true,
                "indeterminate" => flags.indeterminate = true,
                _ => {}
            }
        }
        flags
    }
}

/// Map an element to `(namespace, PlatynUI role)`.
///
/// `parent_role` is the parent's *raw* agent role, because two of the promotions
/// depend on the container: Swing reports list entries and tree rows with the
/// role of whatever renders them.
pub(crate) fn map_role(element: &Element, parent_role: Option<&str>) -> (Namespace, String) {
    if element.is_top_level() {
        return match element.role.as_str() {
            "dialog" => (Namespace::Control, "Dialog".to_owned()),
            _ => (Namespace::Control, "Window".to_owned()),
        };
    }

    // A table cell is identified by the payload carrying cell coordinates, not
    // by its role — Swing reports whatever the shared renderer is, usually
    // `label`. Reporting `Label` would be carrying the Access Bridge's defect
    // into a backend that does not have it: a cell is an item of its table, and
    // the `item:` namespace is exactly for that. Existing suites address cells
    // positionally, so nothing that worked stops working.
    if element.cell.is_some() {
        return (Namespace::Item, "TableCell".to_owned());
    }

    let states = element.state_flags();
    if element.role == "label" {
        match parent_role {
            Some("list") if states.selectable => return (Namespace::Item, "ListItem".to_owned()),
            Some("tree") if states.selectable => return (Namespace::Item, "TreeItem".to_owned()),
            _ => {}
        }
    }

    map_role_name(&element.role)
}

/// The shared `role_en_US` table. Where the JAB and AT-SPI2 vocabularies
/// coincide the mapping follows the AT-SPI2 column of
/// `dev-docs/architecture.md` §6.4, so the same Swing app tends to answer the
/// same selectors on Windows and Linux.
fn map_role_name(role: &str) -> (Namespace, String) {
    let (namespace, name): (Namespace, &str) = match role {
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
    (namespace, name.to_owned())
}

/// The JVM's host toolkit, in the shared `native:JvmToolkit` vocabulary.
///
/// The agent's answer is the authoritative one — it reads the *loaded classes*
/// from inside the process, where "which toolkit is this" has no ambiguity —
/// whereas the platform classifier infers it from a top-level window class and
/// says so. Mapped onto `JavaToolkit` rather than passed through raw, so the same
/// window answers the same string whichever backend served it; a consumer
/// comparing two runs must not have to know that one said `swing` and the other
/// `Swing/AWT`.
///
/// Swing and plain AWT collapse into one label deliberately: they are one
/// component hierarchy, and `JavaToolkit` has always spelled that `Swing/AWT`.
pub(crate) fn map_toolkit(toolkits: &[String]) -> JavaToolkit {
    // Order matters only for a JVM running more than one, which is the deferred
    // mixed-toolkit case; the richer toolkit wins so the label is not misleading.
    if toolkits.iter().any(|name| name == "javafx") {
        return JavaToolkit::JavaFx;
    }
    if toolkits.iter().any(|name| name == "swt") {
        return JavaToolkit::Swt;
    }
    if toolkits.iter().any(|name| name == "swing" || name == "awt") {
        return JavaToolkit::SwingAwt;
    }
    JavaToolkit::Unknown
}

/// Generic fallback: whitespace-separated words, each capitalized. Empty input
/// maps to `"Unknown"` so a role is never an empty XPath name.
fn pascal_case(role: &str) -> String {
    let mut out = String::with_capacity(role.len());
    for word in role.split_whitespace() {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    if out.is_empty() { "Unknown".to_owned() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Payloads recorded from the fixture, so the mapping is covered without a
    /// live JVM (task 4.5). Trimmed to the fields under test; `serde` defaults
    /// cover the rest, which is itself worth pinning — a payload from an older
    /// agent must still map.
    fn parse(json: &str) -> Element {
        serde_json::from_str(json).expect("recorded payload must parse")
    }

    const WINDOW: &str = r#"{
        "id": 1, "kind": "window", "role": "frame", "className": "javax.swing.JFrame",
        "name": "frame0", "accessibleName": "PlatynUI Probe 39852",
        "bounds": {"x": 156.0, "y": 156.0, "width": 810.0, "height": 317.0},
        "states": ["enabled", "focusable", "visible", "showing", "resizable"],
        "childCount": 1, "enabled": true, "visible": true, "showing": true,
        "focusable": true, "focused": false,
        "window": {"handle": 1705826, "handleSource": "sun.awt.windows.WComponentPeer#getHWnd",
                   "title": "PlatynUI Probe 39852", "active": true, "focused": true,
                   "resizable": true, "extendedState": 0, "alwaysOnTop": false}
    }"#;

    const SELECTED_CELL: &str = r#"{
        "id": 38, "kind": "cell", "role": "label", "className": "javax.swing.JTable",
        "name": "r2c0", "accessibleName": "r2c0",
        "bounds": {"x": 230.0, "y": 470.0, "width": 74.0, "height": 15.0},
        "cell": {"row": 2, "column": 0, "rowExtent": 1, "columnExtent": 1,
                 "selected": true, "editable": false},
        "states": ["enabled", "focusable", "visible", "opaque", "showing", "selected", "transient"],
        "childCount": 0, "enabled": true, "visible": true, "showing": true
    }"#;

    const TABLE: &str = r#"{
        "id": 31, "kind": "component", "role": "table", "className": "javax.swing.JTable",
        "accessibleName": "main-table", "childCount": 12,
        "table": {"rows": 4, "columns": 3, "selectedRows": [2], "selectedColumns": []},
        "selection": {"count": 3, "indices": [6, 7, 8]},
        "states": ["enabled", "focusable", "visible", "showing", "opaque"]
    }"#;

    #[test]
    fn a_recorded_window_payload_maps_to_a_window() {
        let element = parse(WINDOW);
        assert_eq!(map_role(&element, None), (Namespace::Control, "Window".to_owned()));
        // `frame` is only a Window because it is top-level; nested frames stay frames.
        let nested = Element { kind: Kind::Component, ..element.clone() };
        assert_eq!(map_role(&nested, Some("desktop pane")), (Namespace::Control, "Frame".to_owned()));
    }

    /// The name precedence is the whole reason to read the instance tree: the
    /// developer's own identifier wins over the accessible name.
    #[test]
    fn component_get_name_wins_over_the_accessible_name() {
        let button = parse(
            r#"{"id": 4, "kind": "component", "role": "push button", "className": "javax.swing.JButton",
                "name": "okButton", "accessibleName": "OK"}"#,
        );
        assert_eq!(button.display_name(), "okButton");
        assert_eq!(button.stable_id().as_deref(), Some("okButton"));

        let anonymous = Element { name: None, ..button };
        assert_eq!(anonymous.display_name(), "OK", "the accessible name is the fallback");
        assert_eq!(anonymous.stable_id(), None, "an absent name is not an id");
    }

    /// A window is named by its title, whatever else it carries.
    ///
    /// This is not a preference, it is a regression guard. AWT manufactures a
    /// component name for every unnamed `Frame` (`"frame0"`), so letting the
    /// component name win would rename every Swing window in the tree and break
    /// every `//Window[@Name="..."]` locator at once. The agent drops
    /// auto-generated names, and this ordering is the second line of defence.
    #[test]
    fn a_window_is_named_by_its_title_not_by_its_component_name() {
        let window = parse(WINDOW);
        assert_eq!(window.display_name(), "PlatynUI Probe 39852");
        // Even with a component name present — the recorded payload has one.
        assert_eq!(window.name.as_deref(), Some("frame0"));
        // A titleless window still falls back rather than reporting nothing.
        let untitled = parse(
            r#"{"id": 2, "kind": "window", "role": "window", "className": "javax.swing.JWindow",
                "accessibleName": "popup", "window": {"handleSource": "none"}}"#,
        );
        assert_eq!(untitled.display_name(), "popup");
    }

    /// A cell is an item of its table, identified by carrying coordinates — not
    /// by its role, which Swing reports as the shared renderer's (`label`).
    #[test]
    fn a_table_cell_is_an_item_not_the_renderers_label() {
        let cell = parse(SELECTED_CELL);
        assert_eq!(map_role(&cell, Some("table")), (Namespace::Item, "TableCell".to_owned()));
        let coordinates = cell.cell.expect("cell block");
        assert_eq!((coordinates.row, coordinates.column), (2, 0));
        assert!(coordinates.selected, "row 2 is preselected in the fixture");
        assert_eq!(coordinates.row_extent, 1);
        assert_eq!(cell.display_name(), "r2c0", "the model value, not the renderer's last configuration");
        assert_eq!(cell.rect(), Some(Rect::new(230.0, 470.0, 74.0, 15.0)));
    }

    #[test]
    fn the_table_itself_carries_its_shape_and_selection() {
        let table = parse(TABLE);
        assert_eq!(map_role(&table, None), (Namespace::Control, "Table".to_owned()));
        let shape = table.table.expect("table block");
        assert_eq!((shape.rows, shape.columns), (4, 3));
        assert_eq!(shape.selected_rows, vec![2]);
        assert_eq!(table.selection.expect("selection").indices, vec![6, 7, 8]);
    }

    /// A list entry reports the renderer's role too; the parent plus the
    /// `selectable` state is what identifies it, exactly as in the JAB backend.
    #[test]
    fn list_entries_are_promoted_to_items() {
        let entry = parse(
            r#"{"id": 9, "kind": "accessible", "role": "label", "className": "javax.swing.JList",
                "states": ["selectable", "visible"]}"#,
        );
        assert_eq!(map_role(&entry, Some("list")), (Namespace::Item, "ListItem".to_owned()));
        assert_eq!(map_role(&entry, Some("tree")), (Namespace::Item, "TreeItem".to_owned()));
        // Not under a container that makes it an item: still a label.
        assert_eq!(map_role(&entry, Some("panel")), (Namespace::Control, "Label".to_owned()));
    }

    #[test]
    fn the_fixture_role_vocabulary_matches_the_bridges() {
        // Verbatim roles observed from the fixture through the agent. These are
        // the same strings the Access Bridge reports, which is what keeps a
        // locator matching when the serving backend changes.
        for (role, namespace, mapped) in [
            ("root pane", Namespace::Control, "RootPane"),
            ("layered pane", Namespace::Control, "LayeredPane"),
            ("menu bar", Namespace::Control, "MenuBar"),
            ("menu", Namespace::Control, "Menu"),
            ("menu item", Namespace::Control, "MenuItem"),
            ("panel", Namespace::Control, "Panel"),
            ("push button", Namespace::Control, "Button"),
            ("text", Namespace::Control, "Text"),
            ("label", Namespace::Control, "Label"),
            ("check box", Namespace::Control, "CheckBox"),
            ("radio button", Namespace::Control, "RadioButton"),
            ("combo box", Namespace::Control, "ComboBox"),
            ("slider", Namespace::Control, "Slider"),
            ("spinbox", Namespace::Control, "SpinButton"),
            ("progress bar", Namespace::Control, "ProgressBar"),
            ("scroll pane", Namespace::Control, "ScrollPane"),
            ("viewport", Namespace::Control, "Viewport"),
            ("table", Namespace::Control, "Table"),
        ] {
            assert_eq!(map_role_name(role), (namespace, mapped.to_owned()), "role {role:?}");
        }
    }

    #[test]
    fn unknown_roles_and_kinds_never_fail_a_frame() {
        // A role the map has never seen becomes a usable XPath name …
        assert_eq!(map_role_name("hyperlink thing"), (Namespace::Control, "HyperlinkThing".to_owned()));
        assert_eq!(pascal_case(""), "Unknown");
        // … and a kind a newer agent invented degrades instead of breaking the
        // provider's ability to read the tree at all.
        let future = parse(r#"{"id": 5, "kind": "something-new", "role": "panel", "className": "X"}"#);
        assert_eq!(future.kind, Kind::Accessible);
        assert!(!future.is_top_level());
    }

    /// Absent bounds must stay absent: an element that is not on screen has no
    /// rectangle, and a zero-sized one at the origin would be a lie the
    /// highlighter and the pointer input would both act on.
    #[test]
    fn absent_bounds_do_not_become_a_zero_rectangle() {
        let offscreen = parse(r#"{"id": 7, "kind": "component", "role": "panel", "className": "X"}"#);
        assert_eq!(offscreen.rect(), None);
    }

    /// The toolkit label must be the shared one, not the agent's wire spelling —
    /// the same window has to answer the same string whichever backend served it.
    #[test]
    fn toolkit_names_map_onto_the_shared_vocabulary() {
        assert_eq!(map_toolkit(&["swing".into()]), JavaToolkit::SwingAwt);
        assert_eq!(map_toolkit(&["awt".into()]), JavaToolkit::SwingAwt, "plain AWT is the same hierarchy");
        assert_eq!(map_toolkit(&["javafx".into()]), JavaToolkit::JavaFx);
        assert_eq!(map_toolkit(&["swt".into()]), JavaToolkit::Swt);
        assert_eq!(map_toolkit(&[]), JavaToolkit::Unknown, "a JVM with no UI yet");
        assert_eq!(map_toolkit(&["something-new".into()]), JavaToolkit::Unknown);
        // A JVM running two: the label names the richer one rather than whichever
        // happened to sort first.
        assert_eq!(map_toolkit(&["swing".into(), "javafx".into()]), JavaToolkit::JavaFx);
        assert_eq!(map_toolkit(&["awt".into(), "swt".into()]), JavaToolkit::Swt);
        // And the labels are the ones the other providers already publish.
        assert_eq!(JavaToolkit::SwingAwt.label(), "Swing/AWT");
        assert_eq!(JavaToolkit::JavaFx.label(), "JavaFX");
    }

    #[test]
    fn states_narrow_to_what_the_provider_consumes() {
        let cell = parse(SELECTED_CELL);
        let flags = cell.state_flags();
        assert!(flags.selected);
        assert!(!flags.editable && !flags.checked && !flags.expanded);
        // The verbatim list survives for `native:States`.
        assert!(cell.states.contains(&"transient".to_owned()));
    }
}
