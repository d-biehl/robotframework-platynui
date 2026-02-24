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

    let available_height = ui.available_height();

    egui::ScrollArea::horizontal().show(ui, |ui| {
        TableBuilder::new(ui)
            .auto_shrink([false, false])
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(180.0)) // Name
            .column(Column::remainder().at_least(200.0)) // Value
            .column(Column::auto().at_least(80.0)) // Type
            .min_scrolled_height(available_height)
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
                for &idx in &indices {
                    let attr = &attributes[idx];
                    let name_str = format!("{}:{}", attr.namespace, attr.name);
                    let row_str = format!("{}={}", name_str, attr.value);

                    body.row(20.0, |mut row| {
                        // Column 0: Name (read-only selectable text)
                        row.col(|ui| {
                            let mut text = name_str.clone();
                            let te = egui::TextEdit::singleline(&mut text)
                                .desired_width(ui.available_width())
                                .frame(false)
                                .interactive(true);
                            let resp = ui.add(te);
                            show_row_context_menu(&resp, &name_str, &attr.value, &attr.value_type, &row_str);
                        });

                        // Column 1: Value (read-only selectable text)
                        row.col(|ui| {
                            let mut text = attr.value.clone();
                            let te = egui::TextEdit::singleline(&mut text)
                                .desired_width(ui.available_width())
                                .frame(false)
                                .interactive(true);
                            let resp = ui.add(te);
                            show_row_context_menu(&resp, &name_str, &attr.value, &attr.value_type, &row_str);
                        });

                        // Column 2: Type (read-only selectable text)
                        row.col(|ui| {
                            let mut text = attr.value_type.clone();
                            let te = egui::TextEdit::singleline(&mut text)
                                .desired_width(ui.available_width())
                                .text_color(egui::Color32::from_gray(160))
                                .frame(false)
                                .interactive(true);
                            let resp = ui.add(te);
                            show_row_context_menu(&resp, &name_str, &attr.value, &attr.value_type, &row_str);
                        });
                    });
                }
            });
    });
}

/// Context menu for a properties row with quick copy options.
fn show_row_context_menu(response: &egui::Response, name: &str, value: &str, value_type: &str, row_text: &str) {
    response.context_menu(|ui| {
        if ui.button("Copy Name").clicked() {
            ui.ctx().copy_text(name.to_string());
            ui.close();
        }
        if ui.button("Copy Value").clicked() {
            ui.ctx().copy_text(value.to_string());
            ui.close();
        }
        if ui.button("Copy Type").clicked() {
            ui.ctx().copy_text(value_type.to_string());
            ui.close();
        }
        ui.separator();
        if ui.button("Copy Row").clicked() {
            ui.ctx().copy_text(row_text.to_string());
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
