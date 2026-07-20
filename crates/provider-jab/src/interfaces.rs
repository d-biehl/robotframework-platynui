//! Interface-property catalog: projection of the data behind an element's
//! supported JAB accessibility interfaces as `native:<Interface>.<Property>`
//! attributes (`jab-interface-attributes`).
//!
//! This is the JAB analogue of the UIA provider's `collect_native_properties`
//! / `get_native_property_by_name` pair: a single catalog is the source of
//! truth for names and readers, gated by the element's `accessibleInterfaces`
//! bitfield (a free in-memory check — the info snapshot already carries it)
//! instead of UIA's per-pattern availability property. Two tiers keep walks
//! bounded:
//!
//! - **Container-level** properties ([`CONTAINER_CATALOG`]) are appended
//!   during `attributes()` enumeration; each value is read live from the
//!   bridge at `value()` time (a bounded, constant number of calls per node).
//! - **Per-cell** `TableCell.*` properties ([`CELL_CATALOG`]) are resolved
//!   only through a targeted `attribute()` lookup, so a full tree walk of a
//!   large `JTable` never issues per-cell `getAccessibleTableCellInfo` calls.
//!
//! `AccessibleKeyBindings` and `AccessibleRelationSet` have no bit in the
//! `accessibleInterfaces` bitfield (every Swing component can carry them), so
//! their groups are marked `bit == 0`: listed on every element node and probed
//! at value-read time — absent data reads as `Null` ("surfaced where present").

use crate::client::{JabClient, TableCellInfo};
use crate::ffi;
use crate::handle::JabObject;
use platynui_core::ui::{Namespace, UiAttribute, UiValue};
use std::sync::Arc;

/// Reads one container-level property live from the bridge.
pub(crate) type Reader = fn(&JabClient, &JabObject) -> UiValue;

pub(crate) struct InterfaceProperty {
    pub name: &'static str,
    pub read: Reader,
}

/// One interface's property group, gated by its `accessibleInterfaces` bit
/// (`bit == 0` = ungated, see the module docs).
pub(crate) struct InterfaceGroup {
    pub bit: i32,
    pub properties: &'static [InterfaceProperty],
}

pub(crate) const CONTAINER_CATALOG: &[InterfaceGroup] = &[
    InterfaceGroup {
        bit: ffi::INTERFACE_TABLE,
        properties: &[
            InterfaceProperty { name: "Table.RowCount", read: read_table_row_count },
            InterfaceProperty { name: "Table.ColumnCount", read: read_table_column_count },
            InterfaceProperty { name: "Table.SelectedRowCount", read: read_table_selected_row_count },
            InterfaceProperty { name: "Table.SelectedColumnCount", read: read_table_selected_column_count },
            InterfaceProperty { name: "Table.HasCaption", read: read_table_has_caption },
            InterfaceProperty { name: "Table.HasSummary", read: read_table_has_summary },
        ],
    },
    InterfaceGroup {
        bit: ffi::INTERFACE_VALUE,
        properties: &[
            InterfaceProperty { name: "Value.Current", read: read_value_current },
            InterfaceProperty { name: "Value.Minimum", read: read_value_minimum },
            InterfaceProperty { name: "Value.Maximum", read: read_value_maximum },
        ],
    },
    InterfaceGroup {
        bit: ffi::INTERFACE_TEXT,
        properties: &[
            InterfaceProperty { name: "Text.CharCount", read: read_text_char_count },
            InterfaceProperty { name: "Text.CaretIndex", read: read_text_caret_index },
            // Selection bounds surface only while a selection exists (Null otherwise).
            InterfaceProperty { name: "Text.SelectionStart", read: read_text_selection_start },
            InterfaceProperty { name: "Text.SelectionEnd", read: read_text_selection_end },
        ],
    },
    InterfaceGroup {
        bit: ffi::INTERFACE_ACTION,
        properties: &[InterfaceProperty { name: "Action.Names", read: read_action_names }],
    },
    InterfaceGroup {
        bit: ffi::INTERFACE_HYPERTEXT,
        properties: &[InterfaceProperty { name: "Hypertext.LinkCount", read: read_hypertext_link_count }],
    },
    InterfaceGroup {
        bit: 0,
        properties: &[InterfaceProperty { name: "KeyBindings.Bindings", read: read_key_bindings }],
    },
    InterfaceGroup { bit: 0, properties: &[InterfaceProperty { name: "RelationSet.Relations", read: read_relations }] },
];

/// Per-cell properties (`getAccessibleTableCellInfo`), deliberately absent
/// from [`CONTAINER_CATALOG`]: resolved only via a targeted `attribute()`
/// lookup (see [`resolve_cell_info`]).
pub(crate) struct CellProperty {
    pub name: &'static str,
    pub read: fn(&TableCellInfo) -> UiValue,
}

pub(crate) const CELL_CATALOG: &[CellProperty] = &[
    CellProperty { name: "TableCell.Index", read: |cell| integer(cell.index) },
    CellProperty { name: "TableCell.Row", read: |cell| integer(cell.row) },
    CellProperty { name: "TableCell.Column", read: |cell| integer(cell.column) },
    CellProperty { name: "TableCell.RowExtent", read: |cell| integer(cell.row_extent) },
    CellProperty { name: "TableCell.ColumnExtent", read: |cell| integer(cell.column_extent) },
    CellProperty { name: "TableCell.IsSelected", read: |cell| UiValue::from(cell.is_selected) },
];

/// Container-level property lookup by name, with the group's gate bit.
pub(crate) fn container_property(name: &str) -> Option<(i32, &'static InterfaceProperty)> {
    CONTAINER_CATALOG
        .iter()
        .find_map(|group| group.properties.iter().find(|property| property.name == name).map(|p| (group.bit, p)))
}

/// Per-cell property lookup by name.
pub(crate) fn cell_property(name: &str) -> Option<&'static CellProperty> {
    CELL_CATALOG.iter().find(|property| property.name == name)
}

/// Append every interface attribute the `interfaces` bitfield admits (plus
/// the ungated groups) — used by `attributes()` enumeration. Creating the
/// attributes issues no bridge call; each value is read live on demand.
pub(crate) fn append_interface_attributes(
    attrs: &mut Vec<Arc<dyn UiAttribute>>,
    client: &Arc<JabClient>,
    ctx: &Arc<JabObject>,
    interfaces: i32,
) {
    for group in CONTAINER_CATALOG {
        if group.bit != 0 && interfaces & group.bit == 0 {
            continue;
        }
        for property in group.properties {
            attrs.push(interface_attr(client, ctx, property));
        }
    }
}

/// Append the per-cell `TableCell.*` attributes — used by `attributes()`
/// enumeration on children of a table (the caller gates on the captured
/// parent role, so listing costs no bridge call). Each value still resolves
/// lazily via [`resolve_cell_info`] at read time, so a tree walk that does
/// not read per-cell values issues no per-cell bridge calls.
pub(crate) fn append_cell_attributes(
    attrs: &mut Vec<Arc<dyn UiAttribute>>,
    client: &Arc<JabClient>,
    parent_ctx: &Arc<JabObject>,
    index: i32,
) {
    for property in CELL_CATALOG {
        attrs.push(Arc::new(TableCellAttr {
            client: Arc::clone(client),
            parent_ctx: Arc::clone(parent_ctx),
            index,
            property,
        }));
    }
}

pub(crate) fn interface_attr(
    client: &Arc<JabClient>,
    ctx: &Arc<JabObject>,
    property: &'static InterfaceProperty,
) -> Arc<dyn UiAttribute> {
    Arc::new(InterfaceAttr { client: Arc::clone(client), ctx: Arc::clone(ctx), property })
}

struct InterfaceAttr {
    client: Arc<JabClient>,
    ctx: Arc<JabObject>,
    property: &'static InterfaceProperty,
}

impl UiAttribute for InterfaceAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Native
    }

    fn name(&self) -> &str {
        self.property.name
    }

    fn value(&self) -> UiValue {
        (self.property.read)(&self.client, &self.ctx)
    }
}

/// Live resolution of a cell's [`TableCellInfo`] from its *tree* parent's
/// context and the cell's enumeration `index`: the parent must support
/// `AccessibleTable` (documented fallback — `TableCell.*` is omitted
/// otherwise), and the index is mapped row-major through the table's column
/// count, matching `AccessibleJTable`'s child order (`getAccessibleChild(i)`
/// is the cell at `(i / columnCount, i % columnCount)`).
///
/// The cell's *own* context is deliberately not involved: the JDK's
/// AccessBridge answers JTable child lookups with the shared cell-renderer
/// component (`AccessBridge.getAccessibleChildFromContext`'s table
/// special-case), so the cell context aliases every other cell of the table
/// and its bridge parent is the `CellRendererPane`. The coordinate-based
/// `getAccessibleTableCellInfo` answers from the `AccessibleTable` interface
/// instead, which is stable.
pub(crate) fn resolve_cell_info(client: &JabClient, parent_ctx: &JabObject, index: i32) -> Option<TableCellInfo> {
    if index < 0 {
        return None;
    }
    let parent_info = client.context_info(parent_ctx).ok()?;
    if !parent_info.has_interface(ffi::INTERFACE_TABLE) {
        return None;
    }
    let table = client.table_info(parent_ctx).ok().flatten()?;
    if table.column_count <= 0 || table.row_count <= 0 {
        return None;
    }
    if index >= table.row_count.saturating_mul(table.column_count) {
        return None;
    }
    client.table_cell_info(&table.table, index / table.column_count, index % table.column_count).ok().flatten()
}

/// Attribute for one `TableCell.*` property; re-resolves the cell info live
/// on every `value()` read (no sticky cache).
pub(crate) struct TableCellAttr {
    pub client: Arc<JabClient>,
    /// The tree parent's (the table's) context.
    pub parent_ctx: Arc<JabObject>,
    /// The cell's enumeration index within its parent table.
    pub index: i32,
    pub property: &'static CellProperty,
}

impl UiAttribute for TableCellAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Native
    }

    fn name(&self) -> &str {
        self.property.name
    }

    fn value(&self) -> UiValue {
        resolve_cell_info(&self.client, &self.parent_ctx, self.index)
            .map_or(UiValue::Null, |cell| (self.property.read)(&cell))
    }
}

// ---------------------------------------------------------------------------
// Readers

fn integer(value: i32) -> UiValue {
    UiValue::from(i64::from(value))
}

/// JAB value strings are numbers for the stock Swing models; parse them so
/// selectors can compare numerically, keep the raw string otherwise (same
/// policy as the `control:` StatefulValue attributes).
fn numeric_or_string(text: &str) -> UiValue {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        UiValue::Null
    } else if let Ok(number) = trimmed.parse::<f64>() {
        UiValue::from(number)
    } else {
        UiValue::from(trimmed.to_string())
    }
}

fn read_table_row_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.table_info(ctx).ok().flatten().map_or(UiValue::Null, |table| integer(table.row_count))
}

fn read_table_column_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.table_info(ctx).ok().flatten().map_or(UiValue::Null, |table| integer(table.column_count))
}

fn read_table_selected_row_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    let Ok(Some(table)) = client.table_info(ctx) else {
        return UiValue::Null;
    };
    client.table_row_selection_count(&table.table).ok().flatten().map_or(UiValue::Null, integer)
}

fn read_table_selected_column_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    let Ok(Some(table)) = client.table_info(ctx) else {
        return UiValue::Null;
    };
    client.table_column_selection_count(&table.table).ok().flatten().map_or(UiValue::Null, integer)
}

fn read_table_has_caption(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.table_info(ctx).ok().flatten().map_or(UiValue::Null, |table| UiValue::from(table.has_caption))
}

fn read_table_has_summary(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.table_info(ctx).ok().flatten().map_or(UiValue::Null, |table| UiValue::from(table.has_summary))
}

fn read_value_current(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.current_value(ctx).ok().flatten().map_or(UiValue::Null, |text| numeric_or_string(&text))
}

fn read_value_minimum(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.minimum_value(ctx).ok().flatten().map_or(UiValue::Null, |text| numeric_or_string(&text))
}

fn read_value_maximum(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.maximum_value(ctx).ok().flatten().map_or(UiValue::Null, |text| numeric_or_string(&text))
}

fn read_text_char_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.text_info(ctx).ok().flatten().map_or(UiValue::Null, |info| integer(info.char_count))
}

fn read_text_caret_index(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.text_info(ctx).ok().flatten().map_or(UiValue::Null, |info| integer(info.caret_index))
}

fn read_text_selection_start(client: &JabClient, ctx: &JabObject) -> UiValue {
    match client.text_selection(ctx) {
        Ok(Some(selection)) if selection.start_index != selection.end_index => integer(selection.start_index),
        _ => UiValue::Null,
    }
}

fn read_text_selection_end(client: &JabClient, ctx: &JabObject) -> UiValue {
    match client.text_selection(ctx) {
        Ok(Some(selection)) if selection.start_index != selection.end_index => integer(selection.end_index),
        _ => UiValue::Null,
    }
}

fn read_action_names(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.action_names(ctx).ok().flatten().map_or(UiValue::Null, UiValue::from)
}

fn read_hypertext_link_count(client: &JabClient, ctx: &JabObject) -> UiValue {
    client.hypertext_link_count(ctx).ok().flatten().map_or(UiValue::Null, integer)
}

fn read_key_bindings(client: &JabClient, ctx: &JabObject) -> UiValue {
    match client.key_bindings(ctx) {
        Ok(Some(bindings)) if !bindings.is_empty() => UiValue::from(
            bindings.iter().map(|binding| format_key_binding(binding.character, binding.modifiers)).collect::<Vec<_>>(),
        ),
        _ => UiValue::Null,
    }
}

fn read_relations(client: &JabClient, ctx: &JabObject) -> UiValue {
    match client.relation_summaries(ctx) {
        Ok(Some(relations)) if !relations.is_empty() => UiValue::from(
            relations.iter().map(|relation| format!("{}:{}", relation.key, relation.target_count)).collect::<Vec<_>>(),
        ),
        _ => UiValue::Null,
    }
}

/// Human-readable rendering of one key binding, e.g. `Ctrl+Shift+X`, `Alt+F4`
/// or `F5` — modifiers from the `ACCESSIBLE_*_KEYSTROKE` bits, the key from
/// the raw character (F-key numbers and `ACCESSIBLE_VK_*` control codes get
/// their names; anything unprintable falls back to `#<code>`).
pub(crate) fn format_key_binding(character: u16, modifiers: i32) -> String {
    const MODIFIER_NAMES: [(i32, &str); 8] = [
        (ffi::KEYSTROKE_SHIFT, "Shift"),
        (ffi::KEYSTROKE_CONTROL, "Ctrl"),
        (ffi::KEYSTROKE_META, "Meta"),
        (ffi::KEYSTROKE_ALT, "Alt"),
        (ffi::KEYSTROKE_ALT_GRAPH, "AltGraph"),
        (ffi::KEYSTROKE_BUTTON1, "Button1"),
        (ffi::KEYSTROKE_BUTTON2, "Button2"),
        (ffi::KEYSTROKE_BUTTON3, "Button3"),
    ];
    let mut parts: Vec<&str> =
        MODIFIER_NAMES.iter().filter(|(bit, _)| modifiers & bit != 0).map(|(_, name)| *name).collect();
    let key = if modifiers & ffi::KEYSTROKE_FKEY != 0 {
        format!("F{character}")
    } else if modifiers & ffi::KEYSTROKE_CONTROLCODE != 0 {
        control_code_name(character)
    } else {
        char::from_u32(u32::from(character))
            .filter(|c| !c.is_control())
            .map_or_else(|| format!("#{character}"), |c| c.to_string())
    };
    parts.push(&key);
    parts.join("+")
}

/// Names for the `ACCESSIBLE_VK_*` control codes.
fn control_code_name(code: u16) -> String {
    match code {
        8 => "Backspace".to_string(),
        127 => "Delete".to_string(),
        33 => "PageUp".to_string(),
        34 => "PageDown".to_string(),
        35 => "End".to_string(),
        36 => "Home".to_string(),
        37 => "Left".to_string(),
        38 => "Up".to_string(),
        39 => "Right".to_string(),
        40 => "Down".to_string(),
        155 => "Insert".to_string(),
        224 => "KpUp".to_string(),
        225 => "KpDown".to_string(),
        226 => "KpLeft".to_string(),
        227 => "KpRight".to_string(),
        other => format!("#{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The interface prefixes the catalog may use (the `<Interface>` half of
    /// the `<Interface>.<Property>` convention).
    const KNOWN_INTERFACES: [&str; 8] =
        ["Table", "TableCell", "Value", "Text", "Action", "Hypertext", "KeyBindings", "RelationSet"];

    fn all_catalog_names() -> Vec<&'static str> {
        CONTAINER_CATALOG
            .iter()
            .flat_map(|group| group.properties.iter().map(|property| property.name))
            .chain(CELL_CATALOG.iter().map(|property| property.name))
            .collect()
    }

    #[test]
    fn catalog_names_follow_the_dotted_pascal_case_convention() {
        for name in all_catalog_names() {
            let (interface, property) =
                name.split_once('.').unwrap_or_else(|| panic!("{name}: missing the '.' separator"));
            assert!(KNOWN_INTERFACES.contains(&interface), "{name}: unknown interface prefix {interface:?}");
            assert!(
                property.chars().next().is_some_and(|c| c.is_ascii_uppercase()),
                "{name}: property must start uppercase"
            );
            assert!(
                property.chars().all(|c| c.is_ascii_alphanumeric()),
                "{name}: property must be plain PascalCase alphanumerics"
            );
        }
    }

    #[test]
    fn catalog_names_do_not_collide() {
        let mut seen = HashSet::new();
        for name in all_catalog_names() {
            assert!(seen.insert(name), "duplicate catalog entry: {name}");
        }
    }

    #[test]
    fn catalog_gates_are_known_interface_bits() {
        const KNOWN_BITS: [i32; 7] = [
            ffi::INTERFACE_VALUE,
            ffi::INTERFACE_ACTION,
            ffi::INTERFACE_COMPONENT,
            ffi::INTERFACE_SELECTION,
            ffi::INTERFACE_TABLE,
            ffi::INTERFACE_TEXT,
            ffi::INTERFACE_HYPERTEXT,
        ];
        for group in CONTAINER_CATALOG {
            assert!(
                group.bit == 0 || KNOWN_BITS.contains(&group.bit),
                "group gate {:#x} is not a known interface bit",
                group.bit
            );
        }
    }

    #[test]
    fn catalog_lookups_resolve_names_and_gates() {
        let (bit, property) = container_property("Table.RowCount").expect("Table.RowCount is cataloged");
        assert_eq!(bit, ffi::INTERFACE_TABLE);
        assert_eq!(property.name, "Table.RowCount");
        let (bit, _) = container_property("KeyBindings.Bindings").expect("KeyBindings.Bindings is cataloged");
        assert_eq!(bit, 0, "key bindings have no interface bit and must be ungated");
        assert!(container_property("TableCell.Row").is_none(), "per-cell names must not be container properties");
        assert_eq!(cell_property("TableCell.Row").expect("TableCell.Row is cataloged").name, "TableCell.Row");
        assert!(cell_property("Table.RowCount").is_none());
    }

    #[test]
    fn key_bindings_format_readably() {
        assert_eq!(format_key_binding(u16::from(b'A'), 0), "A");
        assert_eq!(format_key_binding(u16::from(b'X'), ffi::KEYSTROKE_CONTROL | ffi::KEYSTROKE_SHIFT), "Shift+Ctrl+X");
        assert_eq!(format_key_binding(5, ffi::KEYSTROKE_FKEY), "F5");
        assert_eq!(format_key_binding(4, ffi::KEYSTROKE_ALT | ffi::KEYSTROKE_FKEY), "Alt+F4");
        assert_eq!(format_key_binding(127, ffi::KEYSTROKE_CONTROLCODE), "Delete");
        assert_eq!(format_key_binding(3, ffi::KEYSTROKE_CONTROLCODE), "#3");
        assert_eq!(format_key_binding(0, 0), "#0", "NUL must not print as a control character");
    }
}
