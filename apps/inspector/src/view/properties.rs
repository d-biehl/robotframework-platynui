//! View: Properties table (right panel).
//!
//! Each cell is a read-only `TextEdit` so users can select text with the mouse
//! and copy with Ctrl+C. A right-click context menu offers quick "Copy Name",
//! "Copy Value", and "Copy Row" actions. Column headers are clickable to sort.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

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

/// Persistent sort state for the properties table.
#[derive(Default)]
pub struct PropertiesSortState {
    /// Current sort column.
    pub column: SortColumn,
    /// Current sort direction.
    pub direction: SortDirection,
}

impl PropertiesSortState {
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

/// Render the properties table for the selected node.
pub fn show_properties(
    ui: &mut egui::Ui,
    selected_label: &str,
    attributes: &[DisplayAttribute],
    sort_state: &mut PropertiesSortState,
) {
    if attributes.is_empty() {
        ui.colored_label(egui::Color32::from_gray(120), "No attributes available for this node.");
        return;
    }

    ui.strong(format!("Properties: {selected_label}"));
    ui.separator();

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
            .column(Column::auto().at_least(180.0)) // Name
            .column(Column::remainder().at_least(200.0)) // Value
            .column(Column::auto().at_least(80.0)) // Type
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
                for (row_idx, &idx) in indices.iter().enumerate() {
                    let attr = &attributes[idx];
                    let name_str = format!("{}:{}", attr.namespace, attr.name);
                    let row_str = format!("{}={}", name_str, attr.value);

                    body.row(20.0, |mut row| {
                        // Column 0: Name (read-only selectable text)
                        row.col(|ui| {
                            let mut text = name_str.clone();
                            let cell_id = ui.id().with(("prop_name", row_idx));
                            // Snapshot state BEFORE TextEdit runs so we can restore it
                            // if a right-click press wipes the selection (see below).
                            let prev_state = egui::text_edit::TextEditState::load(ui.ctx(), cell_id);
                            let prev_sel = cell_selection_from_state(prev_state.as_ref(), &name_str);
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .id(cell_id)
                                    .desired_width(ui.available_width())
                                    .frame(egui::Frame::NONE)
                                    .interactive(true),
                            );
                            // TextEdit's pointer_interaction() calls any_pressed() which
                            // fires on the secondary (right) button too, resetting the
                            // cursor and wiping the selection. Restore the snapshot so the
                            // selection survives into the next frame when the context menu
                            // actually opens (secondary_clicked = button released).
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
                                &name_str,
                                &attr.value,
                                &attr.value_type,
                                &row_str,
                            );
                        });

                        // Column 1: Value (read-only selectable text)
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
                                &name_str,
                                &attr.value,
                                &attr.value_type,
                                &row_str,
                            );
                        });

                        // Column 2: Type (read-only selectable text)
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
                                &name_str,
                                &attr.value,
                                &attr.value_type,
                                &row_str,
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

/// Context menu for text cells in the properties table.
///
/// `prev_sel` is the selection captured **before** the TextEdit was rendered this
/// frame (see [`read_cell_selection`]).  Passing it in means right-click no longer
/// wipes the selection before the menu can use it.
fn show_text_cell_context_menu(
    response: &egui::Response,
    cell_id: egui::Id,
    cell_text: &str,
    prev_sel: Option<String>,
    name: &str,
    value: &str,
    value_type: &str,
    row_text: &str,
) {
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
    });
}

/// Render a placeholder when no node is selected.
pub fn show_no_selection(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.colored_label(egui::Color32::from_gray(120), "Select a node in the tree to view its properties.");
    });
}
