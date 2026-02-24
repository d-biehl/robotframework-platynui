//! View: TreeView panel rendering.
//!
//! Pure rendering code — reads from ViewModel, emits actions back.

use eframe::egui;

use crate::viewmodel::tree_vm::VisibleRow;

/// Actions that the tree view can emit back to the ViewModel.
pub enum TreeAction {
    /// User clicked a row to select it.
    Select(usize),
    /// User toggled expand/collapse on a row.
    Toggle(usize),
    /// User requested a refresh for a single row.
    Refresh(usize),
    /// User requested a recursive subtree refresh.
    RefreshSubtree(usize),
}

/// Render the tree view into the given `Ui`.
///
/// Returns a list of deferred actions to process in the ViewModel.
pub fn show_tree(
    ui: &mut egui::Ui,
    rows: &[VisibleRow],
    selected_index: Option<usize>,
    focused_index: usize,
    scroll_to_focused: bool,
) -> Vec<TreeAction> {
    let mut actions = Vec::new();

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        for (i, row) in rows.iter().enumerate() {
            let is_selected = selected_index == Some(i);
            let indent = row.depth as f32 * 16.0;

            let row_rect = ui.horizontal(|ui| {
                ui.add_space(indent);

                // Disclosure triangle
                if row.has_children {
                    let chevron = if row.is_expanded { "\u{25BC}" } else { "\u{25B6}" };
                    let chevron_btn = ui.add(
                        egui::Button::new(egui::RichText::new(chevron).size(10.0))
                            .frame(false)
                            .min_size(egui::vec2(16.0, 16.0)),
                    );
                    if chevron_btn.clicked() {
                        actions.push(TreeAction::Toggle(i));
                    }
                } else {
                    ui.add_space(20.0);
                }

                // Role icon
                ui.label(role_icon(&row.label));

                // Label
                let mut text = egui::RichText::new(&row.label);
                if !row.is_valid {
                    text = text.strikethrough().color(egui::Color32::from_gray(150));
                }
                if is_selected {
                    text = text.strong();
                }

                let label_response = ui.selectable_label(is_selected, text);
                if label_response.clicked() {
                    actions.push(TreeAction::Select(i));
                }
                if label_response.double_clicked() && row.has_children {
                    actions.push(TreeAction::Toggle(i));
                }

                // Context menu
                label_response.context_menu(|ui| {
                    if ui.button("Refresh").clicked() {
                        actions.push(TreeAction::Refresh(i));
                        ui.close();
                    }
                    if ui.button("Refresh subtree").clicked() {
                        actions.push(TreeAction::RefreshSubtree(i));
                        ui.close();
                    }
                });
            });

            // Selection indicator bar (left edge)
            if is_selected {
                let rect = row_rect.response.rect;
                let bar = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height()));
                ui.painter().rect_filled(bar, 0.0, ui.visuals().selection.stroke.color.linear_multiply(0.6));
            }

            // Focus ring for keyboard navigation
            if focused_index == i && !is_selected {
                let rect = row_rect.response.rect;
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
                    egui::StrokeKind::Outside,
                );
            }

            // Scroll to keep the focused/selected row visible
            if scroll_to_focused && focused_index == i {
                row_rect.response.scroll_to_me(None);
            }
        }
    });

    actions
}

/// Map a role name (first word of label) to a display icon.
fn role_icon(label: &str) -> &'static str {
    match label.split_whitespace().next().unwrap_or("") {
        "Desktop" => "\u{1F5A5}",
        "Window" | "window" | "frame" => "\u{1FA9F}",
        "MenuBar" | "menu bar" => "\u{2630}",
        "MenuItem" | "menu item" | "menu" => "\u{25AA}",
        "ToolBar" | "tool bar" => "\u{1F527}",
        "Button" | "push button" => "\u{1F518}",
        "TextBox" | "text" | "entry" => "\u{1F4DD}",
        "Document" | "document frame" => "\u{1F4C4}",
        "Heading" | "heading" => "\u{1F524}",
        "Paragraph" | "paragraph" => "\u{00B6}",
        "Link" | "link" => "\u{1F517}",
        "TreeView" | "tree" => "\u{1F333}",
        "TreeItem" | "tree item" => "\u{1F33F}",
        "Editor" | "editor" => "\u{270F}\u{FE0F}",
        "CheckBox" | "check box" => "\u{2611}",
        "RadioButton" | "radio button" => "\u{1F518}",
        "ComboBox" | "combo box" => "\u{1F4CB}",
        "List" | "list" | "list box" => "\u{1F4CB}",
        "ListItem" | "list item" => "\u{2022}",
        "Tab" | "page tab" => "\u{1F4D1}",
        "TabList" | "page tab list" => "\u{1F4D1}",
        "StatusBar" | "status bar" => "\u{2139}",
        "ScrollBar" | "scroll bar" => "\u{2195}",
        "Slider" | "slider" => "\u{1F39A}",
        "Image" | "image" | "icon" => "\u{1F5BC}",
        "Table" | "table" => "\u{1F4CA}",
        "Panel" | "panel" | "filler" => "\u{25AB}",
        "Separator" | "separator" => "\u{2500}",
        "Label" | "label" => "\u{1F3F7}",
        "ProgressBar" | "progress bar" => "\u{1F4CA}",
        "Dialog" | "dialog" => "\u{1F4AC}",
        "Alert" | "alert" => "\u{26A0}",
        _ => "\u{2022}",
    }
}
