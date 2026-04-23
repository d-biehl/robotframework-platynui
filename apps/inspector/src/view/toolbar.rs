//! View: Top bars (menu, search) and bottom results panel.

use eframe::egui;

/// Actions emitted by the menu bar.
pub enum MenuAction {
    /// Evaluate XPath from the search field.
    EvaluateXPath,
    /// Cancel active XPath search.
    CancelSearch,
    /// Clear current search results/status.
    ClearResults,
    /// Refresh the currently selected node.
    RefreshNode,
    /// Refresh the currently selected node and descendants.
    RefreshSubtree,
    /// Highlight the currently selected node.
    HighlightNode,
    /// Expand the currently selected node.
    ExpandNode,
    /// Collapse the currently selected node.
    CollapseNode,
    /// Open the About dialog.
    ShowAbout,
}

/// Actions emitted by the search toolbar.
pub enum ToolbarAction {
    /// User pressed Enter in the search bar — evaluate XPath.
    EvaluateXPath,
    /// User clicked Stop — cancel running search.
    CancelSearch,
    /// User clicked Refresh Node.
    RefreshNode,
    /// User clicked Refresh Subtree.
    RefreshSubtree,
}

/// Stable egui id of the XPath search input field.
pub fn search_field_id() -> egui::Id {
    egui::Id::new("inspector_xpath_search_field")
}

/// Render the application menu bar.
pub fn show_menu_bar(ui: &mut egui::Ui, has_node_selection: bool, is_searching: bool) -> Vec<MenuAction> {
    let mut actions = Vec::new();

    egui::Panel::top("menu_bar").show_inside(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Search", |ui| {
                let search_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
                if ui
                    .add(egui::Button::new("Search").shortcut_text(ui.ctx().format_shortcut(&search_shortcut)))
                    .clicked()
                {
                    actions.push(MenuAction::EvaluateXPath);
                    ui.close();
                }

                let cancel_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape);
                if ui
                    .add_enabled(
                        is_searching,
                        egui::Button::new("Cancel Search").shortcut_text(ui.ctx().format_shortcut(&cancel_shortcut)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::CancelSearch);
                    ui.close();
                }
                ui.separator();
                if ui.button("Clear Results").clicked() {
                    actions.push(MenuAction::ClearResults);
                    ui.close();
                }
            });

            ui.menu_button("Node", |ui| {
                let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5);
                if ui
                    .add_enabled(
                        has_node_selection,
                        egui::Button::new("Refresh Node").shortcut_text(ui.ctx().format_shortcut(&refresh_shortcut)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::RefreshNode);
                    ui.close();
                }

                let refresh_subtree_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::F5);
                if ui
                    .add_enabled(
                        has_node_selection,
                        egui::Button::new("Refresh Subtree")
                            .shortcut_text(ui.ctx().format_shortcut(&refresh_subtree_shortcut)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::RefreshSubtree);
                    ui.close();
                }

                let highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::H);
                if ui
                    .add_enabled(
                        has_node_selection,
                        egui::Button::new("Highlight Node")
                            .shortcut_text(ui.ctx().format_shortcut(&highlight_shortcut)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::HighlightNode);
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(has_node_selection, egui::Button::new("Expand Node")).clicked() {
                    actions.push(MenuAction::ExpandNode);
                    ui.close();
                }
                if ui.add_enabled(has_node_selection, egui::Button::new("Collapse Node")).clicked() {
                    actions.push(MenuAction::CollapseNode);
                    ui.close();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About PlatynUI Inspector").clicked() {
                    actions.push(MenuAction::ShowAbout);
                    ui.close();
                }
            });
        });
    });

    actions
}

/// Render the search toolbar. Returns actions to process.
///
/// When `is_searching` is `true`, the Search button becomes a Stop button.
pub fn show_search_bar(
    ui: &mut egui::Ui,
    search_text: &mut String,
    always_on_top: &mut bool,
    is_searching: bool,
    has_node_selection: bool,
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

    egui::Panel::top("search_bar").exact_size(ui_height).show_inside(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("\u{1F50D}");

            let text_edit = ui.add(
                egui::TextEdit::multiline(search_text)
                    .id(search_field_id())
                    .desired_rows(desired_rows)
                    .hint_text("XPath expression (Shift+Enter for new line)")
                    .desired_width(ui.available_width() - 320.0),
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

            let search_had_focus = ui.memory(|mem| mem.has_focus(search_field_id()));
            let search_has_focus_now = text_edit.has_focus();
            if search_had_focus || search_has_focus_now {
                ui.memory_mut(|mem| {
                    mem.set_focus_lock_filter(
                        search_field_id(),
                        egui::EventFilter { escape: true, ..Default::default() },
                    );
                });
            }

            if search_has_focus_now && ui.input(|i| i.key_pressed(egui::Key::Escape)) && is_searching {
                actions.push(ToolbarAction::CancelSearch);
            }

            // Toggle Search / Stop button
            if is_searching {
                if ui.button("\u{23F9} Stop").clicked() {
                    actions.push(ToolbarAction::CancelSearch);
                }
            } else if ui.button("\u{25B6} Search").clicked() {
                actions.push(ToolbarAction::EvaluateXPath);
            }

            if ui.add_enabled(has_node_selection, egui::Button::new("\u{21BB} Refresh")).clicked() {
                actions.push(ToolbarAction::RefreshNode);
            }
            if ui.add_enabled(has_node_selection, egui::Button::new("\u{21BB} Subtree")).clicked() {
                actions.push(ToolbarAction::RefreshSubtree);
            }

            ui.checkbox(always_on_top, "Always On Top");
        });
        ui.add_space(4.0);
    });

    actions
}
