//! View: Results panel (bottom panel) for XPath search results.
//!
//! A keyboard-navigable table built on `egui_extras::TableBuilder`.
//! Up/Down arrows move the focused row, Enter reveals the focused
//! result in the tree, single-click selects, and double-click reveals.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::model::tree_data::SearchResultItem;

#[derive(Clone, Copy, Debug)]
/// Actions emitted by the results panel.
pub enum ResultAction {
    /// User requested tree reveal for a result.
    Reveal(usize),
    /// User requested highlight for a node-backed result.
    Highlight(usize),
    /// Copy the display label.
    CopyLabel(usize),
    /// Copy the owner runtime id for node-backed results.
    CopyRuntimeId(usize),
    /// Copy the raw attribute value for attribute results.
    CopyAttributeValue(usize),
    /// Copy the fullest string representation of the result.
    CopyFullResult(usize),
}

/// Stable egui id for keyboard focus in the results panel.
pub fn focus_id() -> egui::Id {
    egui::Id::new("inspector_results_focus")
}

/// Render the results panel. Returns a list of actions to process.
///
/// `focused_index` is the keyboard cursor position (mutable — updated
/// by arrow key navigation inside the panel).
pub fn show_results_panel(
    ui: &mut egui::Ui,
    results: &[SearchResultItem],
    status: Option<&str>,
    focused_index: &mut usize,
) -> Vec<ResultAction> {
    let mut actions = Vec::new();

    egui::Panel::bottom("results_panel")
        .resizable(true)
        .min_size(60.0)
        .max_size(ui.ctx().content_rect().height() * 0.6)
        .default_size(150.0)
        .show_inside(ui, |ui| {
            // ── Header ───────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.strong("Results");
            });
            ui.separator();

            // ── Empty states ─────────────────────────────────────────
            if results.is_empty() {
                if status.is_none() {
                    ui.colored_label(
                        egui::Color32::from_gray(120),
                        "Enter an XPath expression and press Enter or click Search.",
                    );
                } else {
                    ui.colored_label(egui::Color32::from_gray(120), "No results.");
                }
                return;
            }

            // ── Focus management ─────────────────────────────────────
            let panel_id = focus_id();
            let had_focus = ui.memory(|mem| mem.has_focus(panel_id));

            // Clamp focused index to valid range.
            *focused_index = (*focused_index).min(results.len().saturating_sub(1));

            // Track whether the focused index changed this frame (keyboard
            // nav or click) so we only scroll_to_row on actual navigation,
            // not every frame (which would fight mouse-wheel scrolling).
            let prev_focused_id = panel_id.with("prev_focused");
            let prev_focused: usize = ui.data(|d| d.get_temp(prev_focused_id).unwrap_or(usize::MAX));

            // ── Focus widget (placed BEFORE the table) ───────────────
            // Use Sense::click() so the widget is both clickable and
            // focusable (same pattern as tree_view).  Registered before
            // the table so table rows (registered later) have higher
            // hit-test priority and won't be stolen.  Clicks on empty
            // space (below/between rows) still reach this widget.
            let focus_rect = ui.available_rect_before_wrap();
            let focus_resp = ui.interact(focus_rect, panel_id, egui::Sense::click());

            if focus_resp.clicked() {
                ui.memory_mut(|mem| mem.request_focus(panel_id));
            }

            // ── Arrow-key lock ───────────────────────────────────────
            let has_focus_now = focus_resp.has_focus();
            if had_focus || has_focus_now {
                ui.memory_mut(|mem| {
                    mem.set_focus_lock_filter(
                        panel_id,
                        egui::EventFilter { vertical_arrows: true, ..Default::default() },
                    );
                });
            }

            // ── Keyboard navigation ──────────────────────────────────
            // Runs BEFORE the table so that index changes are visible
            // to scroll_to_row in the same frame.
            if had_focus || has_focus_now {
                // Approximate number of visible rows for PageUp/PageDown.
                let row_height = 20.0_f32;
                let page_rows = ((focus_rect.height() / row_height).floor() as usize).max(1);
                let events = ui.input(|i| i.events.clone());
                for event in &events {
                    if let egui::Event::Key { key, pressed: true, .. } = event {
                        match key {
                            egui::Key::ArrowUp => {
                                *focused_index = focused_index.saturating_sub(1);
                            }
                            egui::Key::ArrowDown if *focused_index + 1 < results.len() => {
                                *focused_index += 1;
                            }
                            egui::Key::PageUp => {
                                *focused_index = focused_index.saturating_sub(page_rows);
                            }
                            egui::Key::PageDown => {
                                *focused_index = (*focused_index + page_rows).min(results.len().saturating_sub(1));
                            }
                            egui::Key::Home => {
                                *focused_index = 0;
                            }
                            egui::Key::End if !results.is_empty() => {
                                *focused_index = results.len() - 1;
                            }
                            _ => {}
                        }
                    }
                }

                let reveal_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter);
                if ui.input_mut(|i| i.consume_shortcut(&reveal_shortcut))
                    && results.get(*focused_index).is_some_and(SearchResultItem::is_node)
                {
                    actions.push(ResultAction::Reveal(*focused_index));
                }

                let highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space);
                if ui.input_mut(|i| i.consume_shortcut(&highlight_shortcut))
                    && results.get(*focused_index).is_some_and(SearchResultItem::is_node)
                {
                    actions.push(ResultAction::Highlight(*focused_index));
                }

                let command_highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::H);
                if ui.input_mut(|i| i.consume_shortcut(&command_highlight_shortcut))
                    && results.get(*focused_index).is_some_and(SearchResultItem::is_node)
                {
                    actions.push(ResultAction::Highlight(*focused_index));
                }
            }

            // ── Table ────────────────────────────────────────────────

            // Captured by the body closure to communicate a clicked row
            // back to the outer scope (avoids borrowing `ui` inside the
            // table body which would conflict with TableBuilder's &mut).
            let mut clicked_row: Option<usize> = None;
            let mut double_clicked_row: Option<usize> = None;
            let mut context_action: Option<ResultAction> = None;

            // Limit the table to the remaining available height so the
            // vertical scrollbar is never clipped by the panel boundary.
            // Subtract the table header height (20 px) because max_scroll_height
            // applies only to the body scroll area; the header is rendered on
            // top of it and would otherwise push the table past the available space.
            let available_height = ui.available_height() - 20.0;

            let mut table = TableBuilder::new(ui)
                .auto_shrink([false, false])
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::exact(40.0)) // #
                .column(Column::exact(60.0)) // Type
                .column(Column::remainder().at_least(200.0)) // Label
                .max_scroll_height(available_height)
                .sense(egui::Sense::click());

            // Only scroll to the focused row when the index actually
            // changed (keyboard nav or click).  This avoids fighting
            // manual mouse-wheel scrolling.
            if *focused_index != prev_focused {
                table = table.scroll_to_row(*focused_index, None);
            }

            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("#");
                    });
                    header.col(|ui| {
                        ui.strong("Type");
                    });
                    header.col(|ui| {
                        ui.strong("Result");
                    });
                })
                .body(|body| {
                    body.rows(20.0, results.len(), |mut row| {
                        let i = row.index();
                        let result = &results[i];
                        let is_focused = *focused_index == i;

                        // Highlight focused row.
                        if is_focused && had_focus {
                            row.set_selected(true);
                        }

                        // Column 0: index
                        row.col(|ui| {
                            ui.add(egui::Label::new(format!("{}", i + 1)).selectable(false));
                        });

                        // Column 1: type
                        row.col(|ui| {
                            let (type_str, color) = match result {
                                SearchResultItem::Node { .. } => ("Node", ui.visuals().hyperlink_color),
                                SearchResultItem::Attribute { .. } => ("Attr", egui::Color32::from_rgb(180, 220, 140)),
                                SearchResultItem::Value { .. } => ("Value", egui::Color32::from_gray(160)),
                            };
                            ui.add(egui::Label::new(egui::RichText::new(type_str).color(color)).selectable(false));
                        });

                        // Column 2: label
                        row.col(|ui| {
                            let text = result.display_label();
                            let rich = if result.is_node() {
                                egui::RichText::new(text).color(ui.visuals().hyperlink_color)
                            } else {
                                egui::RichText::new(text)
                            };
                            ui.add(egui::Label::new(rich).selectable(false));
                        });

                        let row_response = row.response();
                        if row_response.clicked() || row_response.secondary_clicked() || row_response.double_clicked() {
                            clicked_row = Some(i);
                        }
                        if row_response.double_clicked() && result.is_node() {
                            double_clicked_row = Some(i);
                        }
                        show_result_context_menu(&row_response, result, i, &mut context_action);
                    });
                });

            // Process row actions after the table (no borrow conflict with `ui`).
            if let Some(i) = clicked_row {
                *focused_index = i;
                ui.memory_mut(|mem| mem.request_focus(panel_id));
            }

            if let Some(i) = double_clicked_row {
                actions.push(ResultAction::Reveal(i));
            }

            if let Some(action) = context_action {
                actions.push(action);
            }

            // Remember the current focused index for the next frame so
            // we can detect changes and only scroll when needed.
            ui.data_mut(|d| d.insert_temp(prev_focused_id, *focused_index));
        });

    actions
}

fn show_result_context_menu(
    response: &egui::Response,
    result: &SearchResultItem,
    index: usize,
    action: &mut Option<ResultAction>,
) {
    response.context_menu(|ui| {
        if result.is_node() {
            let reveal_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter);
            if ui
                .add(egui::Button::new("Reveal in Tree").shortcut_text(ui.ctx().format_shortcut(&reveal_shortcut)))
                .clicked()
            {
                *action = Some(ResultAction::Reveal(index));
                ui.close();
            }

            let highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space);
            if ui
                .add(egui::Button::new("Highlight Result").shortcut_text(ui.ctx().format_shortcut(&highlight_shortcut)))
                .clicked()
            {
                *action = Some(ResultAction::Highlight(index));
                ui.close();
            }
            ui.separator();
        }

        if ui.button("Copy Label").clicked() {
            *action = Some(ResultAction::CopyLabel(index));
            ui.close();
        }

        if result.is_node() && ui.button("Copy Runtime ID").clicked() {
            *action = Some(ResultAction::CopyRuntimeId(index));
            ui.close();
        }

        if result.attribute_value().is_some() && ui.button("Copy Attribute Value").clicked() {
            *action = Some(ResultAction::CopyAttributeValue(index));
            ui.close();
        }

        ui.separator();

        if ui.button("Copy Full Result").clicked() {
            *action = Some(ResultAction::CopyFullResult(index));
            ui.close();
        }
    });
}
