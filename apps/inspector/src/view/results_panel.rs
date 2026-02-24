//! View: Results panel (bottom panel) for XPath search results.
//!
//! A keyboard-navigable table built on `egui_extras::TableBuilder`.
//! Up/Down arrows move the focused row, Enter reveals the focused
//! result in the tree, clicking a row reveals it immediately.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::model::tree_data::SearchResultItem;

/// Actions emitted by the results panel.
pub enum ResultAction {
    /// User selected a result to reveal in the tree.
    Reveal(usize),
}

/// Render the results panel. Returns a list of actions to process.
///
/// `focused_index` is the keyboard cursor position (mutable — updated
/// by arrow key navigation inside the panel).
pub fn show_results_panel(
    ctx: &egui::Context,
    results: &[SearchResultItem],
    status: Option<&str>,
    focused_index: &mut usize,
) -> Vec<ResultAction> {
    let mut actions = Vec::new();

    egui::TopBottomPanel::bottom("results_panel")
        .resizable(true)
        .min_height(60.0)
        .max_height(ctx.content_rect().height() * 0.6)
        .default_height(150.0)
        .show(ctx, |ui| {
            // ── Header ───────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.strong("Results");
                if !results.is_empty() {
                    ui.colored_label(egui::Color32::from_gray(160), format!("({})", results.len()));
                }
                if let Some(status) = status {
                    ui.separator();
                    if status.starts_with("Error") {
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), status);
                    } else {
                        ui.colored_label(egui::Color32::from_gray(160), status);
                    }
                }
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
            let panel_id = ui.id().with("results_focus");
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
                let old_index = *focused_index;
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
                            egui::Key::ArrowDown => {
                                if *focused_index + 1 < results.len() {
                                    *focused_index += 1;
                                }
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
                            egui::Key::End => {
                                if !results.is_empty() {
                                    *focused_index = results.len() - 1;
                                }
                            }
                            egui::Key::Enter => {}
                            _ => {}
                        }
                    }
                }
                // Reveal in the tree whenever the focused index changes
                // (arrow keys, Home, End).
                if *focused_index != old_index && results.get(*focused_index).is_some_and(SearchResultItem::is_node) {
                    actions.push(ResultAction::Reveal(*focused_index));
                }
            }

            // ── Table ────────────────────────────────────────────────

            // Captured by the body closure to communicate a clicked row
            // back to the outer scope (avoids borrowing `ui` inside the
            // table body which would conflict with TableBuilder's &mut).
            let mut clicked_row: Option<usize> = None;

            let mut table = TableBuilder::new(ui)
                .auto_shrink([false, false])
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::exact(40.0)) // #
                .column(Column::exact(60.0)) // Type
                .column(Column::remainder().at_least(200.0)) // Label
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

                        // Click detection — row.response() borrows only
                        // the table's internal Ui, not the outer one.
                        if row.response().clicked() {
                            clicked_row = Some(i);
                        }
                    });
                });

            // Process click after the table (no borrow conflict with `ui`).
            if let Some(i) = clicked_row {
                *focused_index = i;
                ui.memory_mut(|mem| mem.request_focus(panel_id));
                if results.get(i).is_some_and(SearchResultItem::is_node) {
                    actions.push(ResultAction::Reveal(i));
                }
            }

            // Remember the current focused index for the next frame so
            // we can detect changes and only scroll when needed.
            ui.data_mut(|d| d.insert_temp(prev_focused_id, *focused_index));
        });

    actions
}
