//! View: Top bars (menu, search) and bottom results panel.

use eframe::egui;

use crate::model::tree_data::SearchResultItem;

/// Actions emitted by the toolbar views.
pub enum ToolbarAction {
    /// User pressed Enter in the search bar — evaluate XPath.
    EvaluateXPath,
    /// User clicked Stop — cancel running search.
    CancelSearch,
    /// User clicked a result node — reveal in tree.
    RevealResult(usize),
}

/// Render the application menu bar.
pub fn show_menu_bar(ctx: &egui::Context) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                let _ = ui.button("Undo");
                let _ = ui.button("Redo");
                ui.separator();
                let _ = ui.button("Cut");
                let _ = ui.button("Copy");
                let _ = ui.button("Paste");
            });
            ui.menu_button("Help", |ui| {
                let _ = ui.button("About PlatynUI Inspector");
            });
        });
    });
}

/// Render the search toolbar. Returns actions to process.
///
/// When `is_searching` is `true`, the Search button becomes a Stop button.
pub fn show_search_bar(
    ctx: &egui::Context,
    search_text: &mut String,
    always_on_top: &mut bool,
    is_searching: bool,
) -> Vec<ToolbarAction> {
    let mut actions = Vec::new();

    // Compute panel height dynamically based on number of text lines.
    let num_lines = search_text.chars().filter(|&c| c == '\n').count() + 1;
    let desired_rows = num_lines.clamp(1, 6);
    // Approximate: line height ~18px, plus padding (4+4) and spacing.
    let line_height = 18.0;
    let ui_height = (desired_rows as f32 * line_height) + 16.0;

    // Save text before TextEdit processes events so we can undo
    // an unwanted newline insertion on plain Enter.
    let text_before = search_text.clone();

    egui::TopBottomPanel::top("search_bar").exact_height(ui_height).show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("\u{1F50D}");

            let text_edit = ui.add(
                egui::TextEdit::multiline(search_text)
                    .desired_rows(desired_rows)
                    .hint_text("XPath expression (Shift+Enter for new line)")
                    .desired_width(ui.available_width() - 200.0),
            );

            // Plain Enter (without Shift) triggers search; Shift+Enter
            // inserts a newline (default multiline behavior).
            let enter_no_shift = text_edit.has_focus()
                && ui.input(|i| {
                    i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key {
                                key: egui::Key::Enter,
                                pressed: true,
                                modifiers,
                                ..
                            } if !modifiers.shift
                        )
                    })
                });

            if enter_no_shift {
                // Undo the newline that multiline TextEdit just inserted.
                *search_text = text_before;
                if is_searching {
                    actions.push(ToolbarAction::CancelSearch);
                } else {
                    actions.push(ToolbarAction::EvaluateXPath);
                }
            }

            // Toggle Search / Stop button
            if is_searching {
                if ui.button("\u{23F9} Stop").clicked() {
                    actions.push(ToolbarAction::CancelSearch);
                }
            } else if ui.button("\u{25B6} Search").clicked() {
                actions.push(ToolbarAction::EvaluateXPath);
            }

            ui.checkbox(always_on_top, "Always On Top");
        });
        ui.add_space(4.0);
    });

    actions
}

/// Render the bottom results panel. Returns actions if a result was clicked.
pub fn show_results_panel(
    ctx: &egui::Context,
    results: &[SearchResultItem],
    status: Option<&str>,
) -> Vec<ToolbarAction> {
    let mut actions = Vec::new();

    egui::TopBottomPanel::bottom("results_panel")
        .resizable(true)
        .min_height(60.0)
        .max_height(ctx.content_rect().height() * 0.6)
        .default_height(120.0)
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.strong("Results");
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
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if results.is_empty() && status.is_none() {
                    ui.colored_label(
                        egui::Color32::from_gray(120),
                        "Enter an XPath expression and press Enter or click Search.",
                    );
                } else if results.is_empty() && status.is_some() {
                    ui.colored_label(egui::Color32::from_gray(120), "No results.");
                } else {
                    for (i, result) in results.iter().enumerate() {
                        let label_text = result.display_label();
                        if result.is_node() {
                            // Clickable result (node or attribute with owner node)
                            let icon = match result {
                                SearchResultItem::Node { .. } => "\u{1F517}",
                                SearchResultItem::Attribute { .. } => "\u{1F4CE}",
                                _ => "\u{2022}",
                            };
                            let response = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{icon} {label_text}"))
                                        .color(ui.visuals().hyperlink_color),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if response.clicked() {
                                actions.push(ToolbarAction::RevealResult(i));
                            }
                            response.on_hover_text("Click to reveal in tree");
                        } else {
                            // Non-clickable value result
                            ui.label(format!("  {label_text}"));
                        }
                    }
                }
            });
        });

    actions
}
