//! View: Attributes table (right panel).
//!
//! Each cell is a read-only `TextEdit` so users can select text with the mouse
//! and copy with Ctrl+C. A right-click context menu offers quick "Copy Name",
//! "Copy Value", and "Copy Row" actions. Column headers are clickable to sort.

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::model::tree_data::{DisplayAttribute, xpath_attribute_name};

use super::tree_view::paint_chevron;

const ATTRIBUTE_NAME_COLUMN_WIDTH: f32 = 180.0;
const ATTRIBUTE_NAME_COLUMN_MIN_WIDTH: f32 = 80.0;
const ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH: f32 = 120.0;
const ATTRIBUTE_TYPE_COLUMN_WIDTH: f32 = 88.0;
const ATTRIBUTE_GROUP_INDENT: f32 = 18.0;
const ATTRIBUTE_GROUP_ICON_SIZE: f32 = 16.0;
const ATTRIBUTE_ROW_BASE_HEIGHT: f32 = 20.0;
const ATTRIBUTE_ROW_MAX_HEIGHT: f32 = 240.0;
const ATTRIBUTE_ROW_VPAD: f32 = 4.0;
const ATTRIBUTE_NAMESPACE_HEADER_HEIGHT: f32 = 22.0;
const ATTRIBUTE_SECTION_HEADER_HEIGHT: f32 = 18.0;

/// Which column to sort by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Value,
    Type,
}

/// Sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

/// Persistent sort state for the attributes table.
#[derive(Clone, Copy, Default)]
pub struct AttributesSortState {
    /// Current sort column.
    pub column: SortColumn,
    /// Current sort direction.
    pub direction: SortDirection,
}

impl AttributesSortState {
    /// Toggle: if same column, flip direction; if different column, sort ascending.
    pub fn toggle(&mut self, col: SortColumn) {
        if self.column == col {
            self.direction = match self.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.column = col;
            self.direction = SortDirection::Ascending;
        }
    }
}

/// Namespace presentation mode for the attributes pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AttributesViewMode {
    Ungrouped,
    #[default]
    Grouped,
}

/// Render the attributes table for the selected node.
pub fn show_attributes(
    ui: &mut egui::Ui,
    selected_label: &str,
    attributes: &[DisplayAttribute],
    sort_state: &mut AttributesSortState,
    view_mode: &mut AttributesViewMode,
    filter_text: &mut String,
    pinned_attributes: &mut BTreeSet<String>,
    collapsed_attribute_groups: &mut BTreeSet<String>,
) {
    if attributes.is_empty() {
        ui.colored_label(egui::Color32::from_gray(120), "No attributes available for this node.");
        return;
    }

    ui.strong(selected_label);
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("View");
        ui.selectable_value(view_mode, AttributesViewMode::Grouped, "Grouped");
        ui.selectable_value(view_mode, AttributesViewMode::Ungrouped, "Ungrouped");
    });

    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(
            egui::TextEdit::singleline(filter_text)
                .hint_text("Filter by name, value, or type")
                .desired_width((ui.available_width() - 56.0).max(120.0)),
        );
        if !filter_text.is_empty() && ui.button("Clear").clicked() {
            filter_text.clear();
        }
    });

    let normalized_filter = filter_text.trim().to_lowercase();
    let filter_active = !normalized_filter.is_empty();

    // Build sorted index list
    let mut indices: Vec<usize> = (0..attributes.len()).collect();
    indices.sort_by(|&a, &b| compare_attributes(&attributes[a], &attributes[b], *sort_state, *view_mode));

    if filter_active {
        indices.retain(|&idx| attribute_matches_filter(&attributes[idx], &normalized_filter));
    }

    let pinned_count =
        attributes.iter().filter(|attribute| pinned_attributes.contains(&attribute_key(attribute))).count();

    if filter_active || pinned_count > 0 {
        let mut summary_parts = Vec::new();
        if filter_active {
            summary_parts.push(format!("Showing {} of {} attributes", indices.len(), attributes.len()));
        }
        if pinned_count > 0 {
            summary_parts.push(format!("Pinned: {pinned_count}"));
        }

        ui.colored_label(egui::Color32::from_gray(160), summary_parts.join(" | "));
        ui.separator();
    }

    if indices.is_empty() {
        ui.colored_label(egui::Color32::from_gray(120), "No attributes match the current filter.");
        return;
    }

    let (pinned_indices, unpinned_indices): (Vec<_>, Vec<_>) =
        indices.into_iter().partition(|&idx| pinned_attributes.contains(&attribute_key(&attributes[idx])));

    let entries =
        build_body_entries(attributes, &pinned_indices, &unpinned_indices, *view_mode, collapsed_attribute_groups);

    // Subtract the table header height because `max_scroll_height` only
    // bounds the body scroll area; the header is rendered above it.
    let available_height = (ui.available_height() - ATTRIBUTE_NAMESPACE_HEADER_HEIGHT).max(0.0);

    // ── Column sizing strategy ──────────────────────────────────────────────
    //
    // egui_extras gives us a hard trade-off: a column is either
    //   * `resizable(true)` — drag handle drawn on its right edge, but its
    //     cached width is pinned (never auto-distributes leftover space), or
    //   * `Column::remainder().resizable(false)` — auto-fills leftover space,
    //     but no drag handle.
    //
    // The user wants BOTH: a drag handle between Value and Type AND Value to
    // grow/shrink with the window. So both Name and Value are resizable
    // (handles between Name|Value and between Value|Type), Type is fixed, and
    // we push window-resize deltas into Value's cached width via a one-frame
    // tight `range` pin. On steady frames the range opens up so dragging
    // works normally.
    let pane = ui.available_width();
    let spacing = ui.spacing().item_spacing.x;
    let inner_spacing = 2.0 * spacing;
    let type_w = ATTRIBUTE_TYPE_COLUMN_WIDTH;

    let prev_pane_id = ui.id().with("attributes_prev_pane");
    let prev_widths_id = ui.id().with("attributes_prev_widths");

    let default_value_w =
        (pane - ATTRIBUTE_NAME_COLUMN_WIDTH - type_w - inner_spacing).max(ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH);
    let prev_widths: [f32; 2] =
        ui.memory(|m| m.data.get_temp(prev_widths_id).unwrap_or([ATTRIBUTE_NAME_COLUMN_WIDTH, default_value_w]));
    let prev_pane: f32 = ui.memory(|m| m.data.get_temp(prev_pane_id).unwrap_or(pane));

    let cached_name = prev_widths[0];
    let cached_value = prev_widths[1];
    let pane_delta = pane - prev_pane;

    // Compute the target widths for THIS frame. On steady frames they equal
    // the cached widths, so the table just uses its persisted state. When the
    // pane changed we re-distribute the delta: Value absorbs everything first
    // (capped so the total still fits), and Name is only shrunk as a last
    // resort when Value is already at its minimum and the pane is still too
    // small. Name and Type are NEVER reset to a default — we only ever shrink
    // Name when there is no other way to fit the table.
    let (target_name, target_value, force_widths) = if pane_delta.abs() > 0.5 {
        let mut new_name = cached_name;
        // What Value would need to be if Name stays at its current width.
        let max_value_for_current_name =
            (pane - new_name - type_w - inner_spacing).max(ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH);
        // Absorb the delta into Value, but cap by what fits next to current Name.
        let mut new_value =
            (cached_value + pane_delta).min(max_value_for_current_name).max(ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH);
        // If even Value-at-minimum still overflows the pane, shrink Name down.
        let min_total_with_current_name = new_name + ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH + type_w + inner_spacing;
        if min_total_with_current_name > pane {
            new_name =
                (pane - ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH - type_w - inner_spacing).max(ATTRIBUTE_NAME_COLUMN_MIN_WIDTH);
            new_value = ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH;
        }
        (new_name, new_value, true)
    } else {
        (cached_name, cached_value, false)
    };

    // Pin the range tight only on the frame the geometry actually changed, so
    // the cached width is snapped to our target. Otherwise leave the range
    // wide open so user drags are free.
    let name_col = if force_widths {
        Column::initial(target_name).range(target_name..=target_name)
    } else {
        Column::initial(target_name).at_least(ATTRIBUTE_NAME_COLUMN_MIN_WIDTH)
    }
    .resizable(true)
    .clip(true);

    let value_col = if force_widths {
        Column::initial(target_value).range(target_value..=target_value)
    } else {
        Column::initial(target_value).at_least(ATTRIBUTE_VALUE_COLUMN_MIN_WIDTH)
    }
    .resizable(true)
    .clip(true);

    // Type is `exact` AND explicitly non-resizable, so no drag handle is
    // drawn after it. (`Column::exact` does NOT set `resizable: Some(false)`
    // on its own — without this the table-level `.resizable(true)` flag is
    // inherited and a phantom handle appears at the right edge of Type.)
    let type_col = Column::exact(type_w).resizable(false).clip(true);

    let captured: std::cell::Cell<[f32; 2]> = std::cell::Cell::new([target_name, target_value]);

    TableBuilder::new(ui)
        .id_salt("attributes_table")
        .auto_shrink([false, false])
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::TOP))
        .column(name_col)
        .column(value_col)
        .column(type_col)
        .max_scroll_height(available_height)
        .min_scrolled_height(0.0)
        .header(22.0, |mut header| {
            header.col(|ui| render_sort_header_cell(ui, sort_state, SortColumn::Name, "Name", "sort_name"));
            header.col(|ui| {
                render_sort_header_cell(ui, sort_state, SortColumn::Value, "Value", "sort_value");
            });
            header.col(|ui| render_sort_header_cell(ui, sort_state, SortColumn::Type, "Type", "sort_type"));
        })
        .body(|mut body| {
            let widths = body.widths();
            let value_col_width = widths.get(1).copied().unwrap_or(target_value);
            captured
                .set([widths.first().copied().unwrap_or(target_name), widths.get(1).copied().unwrap_or(target_value)]);

            let heights: Vec<f32> = {
                let ui = body.ui_mut();
                entries.iter().map(|entry| compute_entry_height(ui, entry, attributes, value_col_width)).collect()
            };

            // NOTE: `heterogeneous_rows` is virtualized — the closure is
            // only invoked for currently visible rows, identified via
            // `row.index()`. Do NOT use a sequential iterator here, that
            // would mis-render rows after the user scrolls.
            body.heterogeneous_rows(heights.into_iter(), |mut row| {
                let idx = row.index();
                let Some(entry) = entries.get(idx) else {
                    return;
                };
                render_body_entry(&mut row, entry, attributes, pinned_attributes, collapsed_attribute_groups);
            });
        });

    ui.memory_mut(|m| {
        m.data.insert_temp(prev_pane_id, pane);
        m.data.insert_temp(prev_widths_id, captured.get());
    });
}

/// Logical body row: section divider, namespace group header, or an attribute.
enum BodyEntry {
    Section { title: &'static str },
    Namespace { namespace: String, count: usize, group_key: String, collapsed: bool },
    Attribute { attr_idx: usize, display_name: String, name_indent: f32 },
}

fn build_body_entries(
    attributes: &[DisplayAttribute],
    pinned_indices: &[usize],
    unpinned_indices: &[usize],
    view_mode: AttributesViewMode,
    collapsed_attribute_groups: &BTreeSet<String>,
) -> Vec<BodyEntry> {
    let mut entries: Vec<BodyEntry> = Vec::new();

    match view_mode {
        AttributesViewMode::Ungrouped => {
            for &idx in pinned_indices.iter().chain(unpinned_indices.iter()) {
                let attr = &attributes[idx];
                entries.push(BodyEntry::Attribute {
                    attr_idx: idx,
                    display_name: xpath_attribute_name(&attr.namespace, &attr.name),
                    name_indent: 0.0,
                });
            }
        }
        AttributesViewMode::Grouped => {
            let has_pinned = !pinned_indices.is_empty();
            let has_unpinned = !unpinned_indices.is_empty();

            if has_pinned && has_unpinned {
                entries.push(BodyEntry::Section { title: "Pinned Attributes" });
            }
            if has_pinned {
                push_grouped_entries(&mut entries, attributes, pinned_indices, "pinned", collapsed_attribute_groups);
            }
            if has_unpinned {
                if has_pinned {
                    entries.push(BodyEntry::Section { title: "Other Attributes" });
                }
                push_grouped_entries(&mut entries, attributes, unpinned_indices, "other", collapsed_attribute_groups);
            }
        }
    }

    entries
}

fn push_grouped_entries(
    entries: &mut Vec<BodyEntry>,
    attributes: &[DisplayAttribute],
    indices: &[usize],
    group_scope: &str,
    collapsed_attribute_groups: &BTreeSet<String>,
) {
    let mut start = 0;
    while start < indices.len() {
        let namespace = attributes[indices[start]].namespace.as_str();
        let mut end = start + 1;
        while end < indices.len() && attributes[indices[end]].namespace == namespace {
            end += 1;
        }

        let group_key = format!("{group_scope}::{namespace}");
        let collapsed = collapsed_attribute_groups.contains(&group_key);

        entries.push(BodyEntry::Namespace {
            namespace: namespace.to_string(),
            count: end - start,
            group_key,
            collapsed,
        });

        if !collapsed {
            for &idx in &indices[start..end] {
                entries.push(BodyEntry::Attribute {
                    attr_idx: idx,
                    display_name: attributes[idx].name.clone(),
                    name_indent: ATTRIBUTE_GROUP_INDENT,
                });
            }
        }

        start = end;
    }
}

fn compute_entry_height(
    ui: &egui::Ui,
    entry: &BodyEntry,
    attributes: &[DisplayAttribute],
    value_col_width: f32,
) -> f32 {
    match entry {
        BodyEntry::Section { .. } => ATTRIBUTE_SECTION_HEADER_HEIGHT,
        BodyEntry::Namespace { .. } => ATTRIBUTE_NAMESPACE_HEADER_HEIGHT,
        BodyEntry::Attribute { attr_idx, .. } => {
            let value = attributes[*attr_idx].value.as_str();
            estimate_value_row_height(ui, value, value_col_width)
        }
    }
}

fn estimate_value_row_height(ui: &egui::Ui, value: &str, value_col_width: f32) -> f32 {
    if value.is_empty() {
        return ATTRIBUTE_ROW_BASE_HEIGHT;
    }

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    // Account for TextEdit inner margin (~4 px on each side).
    let max_width = (value_col_width - 8.0).max(40.0);
    let color = ui.visuals().text_color();
    let galley = ui.painter().layout(value.to_string(), font_id, color, max_width);
    let h = galley.size().y + ATTRIBUTE_ROW_VPAD;
    h.clamp(ATTRIBUTE_ROW_BASE_HEIGHT, ATTRIBUTE_ROW_MAX_HEIGHT)
}

fn render_body_entry(
    row: &mut egui_extras::TableRow<'_, '_>,
    entry: &BodyEntry,
    attributes: &[DisplayAttribute],
    pinned_attributes: &mut BTreeSet<String>,
    collapsed_attribute_groups: &mut BTreeSet<String>,
) {
    match entry {
        BodyEntry::Section { title } => {
            row.col(|ui| {
                ui.add_space(2.0);
                ui.colored_label(egui::Color32::from_gray(150), *title);
            });
            row.col(|ui| {
                ui.separator();
            });
            row.col(|_| {});
        }
        BodyEntry::Namespace { namespace, count, group_key, collapsed } => {
            render_namespace_header_row(row, namespace, *count, group_key, *collapsed, collapsed_attribute_groups);
        }
        BodyEntry::Attribute { attr_idx, display_name, name_indent } => {
            render_attribute_row_cells(row, &attributes[*attr_idx], display_name, *name_indent, pinned_attributes);
        }
    }
}

fn render_sort_header_cell(
    ui: &mut egui::Ui,
    sort_state: &mut AttributesSortState,
    column: SortColumn,
    label: &str,
    id_source: &str,
) {
    let active = sort_state.column == column;
    let direction = sort_state.direction;

    ui.horizontal(|ui| {
        ui.strong(label);
        if active {
            // Reserve a small fixed-size square next to the label and paint
            // a real triangle into it, the same way the tree view paints its
            // chevron. Avoids the rectangle-glyph fallback some fonts give
            // for `↑` / `↓`.
            let size = 10.0_f32;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            paint_sort_arrow(ui, rect, direction);
        }
    });

    let resp = ui.interact(ui.max_rect(), ui.id().with(id_source), egui::Sense::click());
    if resp.clicked() {
        sort_state.toggle(column);
    }
}

/// Paint a small filled triangle indicating sort direction, mirroring the
/// triangle style used by [`paint_chevron`] in the tree view.
fn paint_sort_arrow(ui: &egui::Ui, rect: egui::Rect, direction: SortDirection) {
    let color = ui.visuals().text_color();
    let center = rect.center();
    let half = 4.0_f32;

    let points = match direction {
        // Ascending: smallest on top → arrow points UP (▲).
        SortDirection::Ascending => vec![
            egui::pos2(center.x, center.y - half * 0.5),
            egui::pos2(center.x - half, center.y + half * 0.5),
            egui::pos2(center.x + half, center.y + half * 0.5),
        ],
        // Descending: largest on top → arrow points DOWN (▼).
        SortDirection::Descending => vec![
            egui::pos2(center.x - half, center.y - half * 0.5),
            egui::pos2(center.x + half, center.y - half * 0.5),
            egui::pos2(center.x, center.y + half * 0.5),
        ],
    };

    ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
}

fn compare_attributes(
    left: &DisplayAttribute,
    right: &DisplayAttribute,
    sort_state: AttributesSortState,
    view_mode: AttributesViewMode,
) -> Ordering {
    match view_mode {
        AttributesViewMode::Ungrouped => {
            let cmp = compare_flat_attributes(left, right, sort_state.column);
            if sort_state.direction == SortDirection::Ascending { cmp } else { cmp.reverse() }
        }
        AttributesViewMode::Grouped => {
            let namespace_cmp = left.namespace.to_lowercase().cmp(&right.namespace.to_lowercase());
            if namespace_cmp != Ordering::Equal {
                return namespace_cmp;
            }

            let cmp = compare_grouped_attributes(left, right, sort_state.column);
            if sort_state.direction == SortDirection::Ascending { cmp } else { cmp.reverse() }
        }
    }
}

fn compare_flat_attributes(left: &DisplayAttribute, right: &DisplayAttribute, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Name => attribute_key(left).to_lowercase().cmp(&attribute_key(right).to_lowercase()),
        SortColumn::Value => left
            .value
            .to_lowercase()
            .cmp(&right.value.to_lowercase())
            .then_with(|| attribute_key(left).to_lowercase().cmp(&attribute_key(right).to_lowercase())),
        SortColumn::Type => left
            .value_type
            .to_lowercase()
            .cmp(&right.value_type.to_lowercase())
            .then_with(|| attribute_key(left).to_lowercase().cmp(&attribute_key(right).to_lowercase())),
    }
}

fn compare_grouped_attributes(left: &DisplayAttribute, right: &DisplayAttribute, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        SortColumn::Value => left
            .value
            .to_lowercase()
            .cmp(&right.value.to_lowercase())
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase())),
        SortColumn::Type => left
            .value_type
            .to_lowercase()
            .cmp(&right.value_type.to_lowercase())
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase())),
    }
}

fn render_namespace_header_row(
    row: &mut egui_extras::TableRow<'_, '_>,
    namespace: &str,
    item_count: usize,
    group_key: &str,
    is_collapsed: bool,
    collapsed_attribute_groups: &mut BTreeSet<String>,
) {
    let mut toggle = false;

    row.col(|ui| {
        let cell_rect = ui.max_rect();
        let fill = ui.visuals().faint_bg_color;
        if ui.is_rect_visible(cell_rect) {
            ui.painter().rect_filled(cell_rect, 0.0, fill);
        }

        let builder =
            egui::UiBuilder::new().max_rect(cell_rect).layout(egui::Layout::left_to_right(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            ui.add_space(4.0);
            let (icon_rect, _) = ui.allocate_exact_size(
                egui::vec2(ATTRIBUTE_GROUP_ICON_SIZE, ATTRIBUTE_GROUP_ICON_SIZE),
                egui::Sense::hover(),
            );
            if ui.is_rect_visible(icon_rect) {
                paint_chevron(ui, icon_rect, !is_collapsed);
            }
            ui.add_space(2.0);
            ui.strong(namespace);
        });

        // Click handling LAST, on a tiny inset rect so we don't register
        // an interaction widget that exactly overlaps the cell's own
        // child-Ui rect (which would trigger egui's id-clash debug overlay
        // — the red flicker frame).
        let interact_rect = cell_rect.shrink(0.5);
        let interact_id = ui.id().with(("ns_header_click", group_key));
        let response = ui.interact(interact_rect, interact_id, egui::Sense::click());
        if response.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            toggle = true;
        }
    });

    row.col(|ui| {
        let cell_rect = ui.max_rect();
        let fill = ui.visuals().faint_bg_color;
        let separator_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        if ui.is_rect_visible(cell_rect) {
            ui.painter().rect_filled(cell_rect, 0.0, fill);
            let y = cell_rect.center().y;
            ui.painter().hline(cell_rect.x_range(), y, egui::Stroke::new(1.0_f32, separator_color));
        }
    });

    row.col(|ui| {
        let cell_rect = ui.max_rect();
        let fill = ui.visuals().faint_bg_color;
        if ui.is_rect_visible(cell_rect) {
            ui.painter().rect_filled(cell_rect, 0.0, fill);
        }
        let builder =
            egui::UiBuilder::new().max_rect(cell_rect).layout(egui::Layout::right_to_left(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::from_gray(140), item_count.to_string());
        });
    });

    if toggle {
        if is_collapsed {
            collapsed_attribute_groups.remove(group_key);
        } else {
            collapsed_attribute_groups.insert(group_key.to_string());
        }
    }
}

fn render_attribute_row_cells(
    row: &mut egui_extras::TableRow<'_, '_>,
    attr: &DisplayAttribute,
    display_name: &str,
    name_indent: f32,
    pinned_attributes: &mut BTreeSet<String>,
) {
    let attribute_key = attribute_key(attr);
    let xpath_name = xpath_attribute_name(&attr.namespace, &attr.name);
    let is_pinned = pinned_attributes.contains(&attribute_key);
    let row_str = format!("{}={}", xpath_name, attr.value);

    row.col(|ui| {
        ui.horizontal_top(|ui| {
            if name_indent > 0.0 {
                ui.add_space(name_indent);
            }

            let mut text = display_name.to_string();
            let cell_id = ui.id().with(("prop_name", &attribute_key));
            let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
            let prev_sel = cell_selection_from_state(prev_state.as_ref(), display_name);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(cell_id)
                    .desired_width(ui.available_width())
                    .frame(egui::Frame::NONE)
                    .interactive(true),
            );
            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
                && resp.hovered()
                && let Some(state) = prev_state
            {
                state.store(ui.ctx(), cell_id);
            }
            show_text_cell_context_menu(
                &resp,
                cell_id,
                display_name,
                prev_sel,
                AttributeRowMenu {
                    attribute_key: &attribute_key,
                    is_pinned,
                    pinned_attributes,
                    name: &xpath_name,
                    value: &attr.value,
                    value_type: &attr.value_type,
                    row_text: &row_str,
                },
            );
        });
    });

    row.col(|ui| {
        let mut text = attr.value.clone();
        let cell_id = ui.id().with(("prop_value", &attribute_key));
        let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
        let prev_sel = cell_selection_from_state(prev_state.as_ref(), &attr.value);
        let resp = ui.add(
            egui::TextEdit::multiline(&mut text)
                .id(cell_id)
                .desired_width(ui.available_width())
                .desired_rows(1)
                .frame(egui::Frame::NONE)
                .interactive(true),
        );
        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
            && resp.hovered()
            && let Some(state) = prev_state
        {
            state.store(ui.ctx(), cell_id);
        }
        show_text_cell_context_menu(
            &resp,
            cell_id,
            &attr.value,
            prev_sel,
            AttributeRowMenu {
                attribute_key: &attribute_key,
                is_pinned,
                pinned_attributes,
                name: &xpath_name,
                value: &attr.value,
                value_type: &attr.value_type,
                row_text: &row_str,
            },
        );
    });

    row.col(|ui| {
        let mut text = attr.value_type.clone();
        let cell_id = ui.id().with(("prop_type", &attribute_key));
        let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
        let prev_sel = cell_selection_from_state(prev_state.as_ref(), &attr.value_type);
        let resp = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(cell_id)
                .desired_width(ui.available_width())
                .text_color(egui::Color32::from_gray(160))
                .frame(egui::Frame::NONE)
                .interactive(true),
        );
        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
            && resp.hovered()
            && let Some(state) = prev_state
        {
            state.store(ui.ctx(), cell_id);
        }
        show_text_cell_context_menu(
            &resp,
            cell_id,
            &attr.value_type,
            prev_sel,
            AttributeRowMenu {
                attribute_key: &attribute_key,
                is_pinned,
                pinned_attributes,
                name: &xpath_name,
                value: &attr.value,
                value_type: &attr.value_type,
                row_text: &row_str,
            },
        );
    });
}

/// Read the selected text out of a TextEditState snapshot.
fn cell_selection_from_state(state: Option<&egui::text_edit::TextEditState>, cell_text: &str) -> Option<String> {
    state
        .and_then(|s| s.cursor.char_range())
        .map(|range| {
            let r = range.as_sorted_char_range();
            cell_text.chars().skip(r.start.into()).take((r.end - r.start).into()).collect::<String>()
        })
        .filter(|s| !s.is_empty())
}

fn attribute_matches_filter(attr: &DisplayAttribute, filter_text: &str) -> bool {
    let attribute_name = format!("{}:{}", attr.namespace, attr.name).to_lowercase();
    attribute_name.contains(filter_text)
        || attr.value.to_lowercase().contains(filter_text)
        || attr.value_type.to_lowercase().contains(filter_text)
}

fn attribute_key(attr: &DisplayAttribute) -> String {
    format!("{}:{}", attr.namespace, attr.name)
}

struct AttributeRowMenu<'a> {
    attribute_key: &'a str,
    is_pinned: bool,
    pinned_attributes: &'a mut BTreeSet<String>,
    name: &'a str,
    value: &'a str,
    value_type: &'a str,
    row_text: &'a str,
}

/// Context menu for text cells in the attributes table.
///
/// `prev_sel` is the selection captured **before** the TextEdit was rendered this
/// frame (see [`cell_selection_from_state`]). Passing it in means right-click no longer
/// wipes the selection before the menu can use it.
fn show_text_cell_context_menu(
    response: &egui::Response,
    cell_id: egui::Id,
    cell_text: &str,
    prev_sel: Option<String>,
    row_menu: AttributeRowMenu<'_>,
) {
    let AttributeRowMenu { attribute_key, is_pinned, pinned_attributes, name, value, value_type, row_text } = row_menu;

    response.context_menu(|ui| {
        let ctx = ui.ctx().clone();

        let copy_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::C);
        let label = if prev_sel.is_some() { "Copy Selection" } else { "Copy" };
        if ui.add(egui::Button::new(label).shortcut_text(ctx.format_shortcut(&copy_shortcut))).clicked() {
            ctx.copy_text(prev_sel.unwrap_or_else(|| cell_text.to_string()));
            ui.close();
        }

        let select_all_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A);
        if ui.add(egui::Button::new("Select All").shortcut_text(ctx.format_shortcut(&select_all_shortcut))).clicked() {
            if let Some(mut state) = egui::text_edit::TextEditState::load(&ctx, cell_id) {
                let len = cell_text.chars().count();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                )));
                state.store(&ctx, cell_id);
            }
            ctx.memory_mut(|m| m.request_focus(cell_id));
            ui.close();
        }

        ui.separator();

        if ui.button("Copy Name").clicked() {
            ctx.copy_text(name.to_string());
            ui.close();
        }
        if ui.button("Copy Value").clicked() {
            ctx.copy_text(value.to_string());
            ui.close();
        }
        if ui.button("Copy Type").clicked() {
            ctx.copy_text(value_type.to_string());
            ui.close();
        }
        ui.separator();

        let copy_row_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::C);
        if ui.add(egui::Button::new("Copy Row").shortcut_text(ctx.format_shortcut(&copy_row_shortcut))).clicked() {
            ctx.copy_text(row_text.to_string());
            ui.close();
        }

        ui.separator();

        let pin_label = if is_pinned { "Unpin Attribute" } else { "Pin Attribute" };
        if ui.button(pin_label).clicked() {
            if is_pinned {
                pinned_attributes.remove(attribute_key);
            } else {
                pinned_attributes.insert(attribute_key.to_string());
            }
            ui.close();
        }
    });
}

/// Render a placeholder when no node is selected.
pub fn show_no_selection(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.colored_label(egui::Color32::from_gray(120), "Select a node in the tree to view its attributes.");
    });
}
