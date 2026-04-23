//! View: Attributes table (right panel).
//!
//! Each cell is a read-only `TextEdit` so users can select text with the mouse
//! and copy with Ctrl+C. A right-click context menu offers quick "Copy Name",
//! "Copy Value", and "Copy Row" actions. Column headers are clickable to sort.

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::BTreeSet;

use crate::model::tree_data::DisplayAttribute;

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
#[derive(Default)]
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

/// Render the attributes table for the selected node.
pub fn show_attributes(
    ui: &mut egui::Ui,
    selected_label: &str,
    attributes: &[DisplayAttribute],
    sort_state: &mut AttributesSortState,
    filter_text: &mut String,
    pinned_attributes: &mut BTreeSet<String>,
) {
    if attributes.is_empty() {
        ui.colored_label(egui::Color32::from_gray(120), "No attributes available for this node.");
        return;
    }

    ui.strong(selected_label);
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(
            egui::TextEdit::singleline(filter_text)
                .hint_text("Filter by name, value, or type")
                .desired_width(ui.available_width() - 56.0),
        );
        if !filter_text.is_empty() && ui.button("Clear").clicked() {
            filter_text.clear();
        }
    });

    let normalized_filter = filter_text.trim().to_lowercase();
    let filter_active = !normalized_filter.is_empty();

    // Build sorted index list
    let mut indices: Vec<usize> = (0..attributes.len()).collect();
    let asc = sort_state.direction == SortDirection::Ascending;
    indices.sort_by(|&a, &b| {
        let cmp = match sort_state.column {
            SortColumn::Name => {
                let ka = format!("{}:{}", attributes[a].namespace, attributes[a].name);
                let kb = format!("{}:{}", attributes[b].namespace, attributes[b].name);
                ka.to_lowercase().cmp(&kb.to_lowercase())
            }
            SortColumn::Value => attributes[a].value.to_lowercase().cmp(&attributes[b].value.to_lowercase()),
            SortColumn::Type => attributes[a].value_type.to_lowercase().cmp(&attributes[b].value_type.to_lowercase()),
        };
        if asc { cmp } else { cmp.reverse() }
    });

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

    egui::ScrollArea::horizontal().show(ui, |ui| {
        // Compute available_height inside the ScrollArea so the horizontal
        // scrollbar chrome (if visible) is already accounted for.
        // Subtract the table header height (22 px) because max_scroll_height
        // applies only to the body scroll area; the header is rendered on top
        // of it and would otherwise push the table past the available space.
        let available_height = ui.available_height() - 22.0;
        TableBuilder::new(ui)
            .auto_shrink([false, false])
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(180.0))
            .column(Column::remainder().at_least(200.0))
            .column(Column::auto().at_least(80.0))
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(22.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Name");
                    let resp = ui.interact(ui.max_rect(), ui.id().with("sort_name"), egui::Sense::click());
                    if resp.clicked() {
                        sort_state.toggle(SortColumn::Name);
                    }
                });
                header.col(|ui| {
                    ui.strong("Value");
                    let resp = ui.interact(ui.max_rect(), ui.id().with("sort_value"), egui::Sense::click());
                    if resp.clicked() {
                        sort_state.toggle(SortColumn::Value);
                    }
                });
                header.col(|ui| {
                    ui.strong("Type");
                    let resp = ui.interact(ui.max_rect(), ui.id().with("sort_type"), egui::Sense::click());
                    if resp.clicked() {
                        sort_state.toggle(SortColumn::Type);
                    }
                });
            })
            .body(|mut body| {
                for (row_idx, idx) in pinned_indices.into_iter().chain(unpinned_indices).enumerate() {
                    let attr = &attributes[idx];
                    let attribute_key = attribute_key(attr);
                    let is_pinned = pinned_attributes.contains(&attribute_key);
                    let name_str = format!("{}:{}", attr.namespace, attr.name);
                    let row_str = format!("{}={}", name_str, attr.value);

                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            let mut text = name_str.clone();
                            let cell_id = ui.id().with(("prop_name", row_idx));
                            let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
                            let prev_sel = cell_selection_from_state(prev_state.as_ref(), &name_str);
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
                                &name_str,
                                prev_sel,
                                AttributeRowMenu {
                                    attribute_key: &attribute_key,
                                    is_pinned,
                                    pinned_attributes,
                                    name: &name_str,
                                    value: &attr.value,
                                    value_type: &attr.value_type,
                                    row_text: &row_str,
                                },
                            );
                        });

                        row.col(|ui| {
                            let mut text = attr.value.clone();
                            let cell_id = ui.id().with(("prop_value", row_idx));
                            let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
                            let prev_sel = cell_selection_from_state(prev_state.as_ref(), &attr.value);
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
                                &attr.value,
                                prev_sel,
                                AttributeRowMenu {
                                    attribute_key: &attribute_key,
                                    is_pinned,
                                    pinned_attributes,
                                    name: &name_str,
                                    value: &attr.value,
                                    value_type: &attr.value_type,
                                    row_text: &row_str,
                                },
                            );
                        });

                        row.col(|ui| {
                            let mut text = attr.value_type.clone();
                            let cell_id = ui.id().with(("prop_type", row_idx));
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
                                    name: &name_str,
                                    value: &attr.value,
                                    value_type: &attr.value_type,
                                    row_text: &row_str,
                                },
                            );
                        });
                    });
                }
            });
    });
}

/// Read the selected text out of a TextEditState snapshot.
fn cell_selection_from_state(state: Option<&egui::text_edit::TextEditState>, cell_text: &str) -> Option<String> {
    state
        .and_then(|s| s.cursor.char_range())
        .map(|range| {
            let r = range.as_sorted_char_range();
            cell_text.chars().skip(r.start).take(r.end - r.start).collect::<String>()
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
