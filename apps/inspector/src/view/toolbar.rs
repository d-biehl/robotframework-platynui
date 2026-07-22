//! View: Top bars (menu, toolbar, search) and bottom results panel.

use eframe::egui;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Toolbar display style, persisted with the Inspector's other settings.
/// Both styles render buttons of identical height (the icon is sized to the
/// font height; text only adds width), so switching never shifts the layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ToolbarStyle {
    #[default]
    IconsOnly,
    IconsAndText,
}

/// Static description of one toolbar control: its icon (plus the fill-weight
/// variant shown while a toggle is active), the action name used for the
/// tooltip and the accessible name, the stable AccessKit author id (surfacing
/// as the provider-independent `@Id` — the acceptance-test contract), and the
/// shortcut shown in the tooltip.
struct ToolbarButtonSpec {
    icon: egui::ImageSource<'static>,
    active_icon: Option<egui::ImageSource<'static>>,
    label: &'static str,
    author_id: &'static str,
    shortcut: Option<egui::KeyboardShortcut>,
}

/// Construct and show one toolbar control. Every toolbar button goes through
/// this helper so they all share the same mechanics: theme-following icon
/// tint, tooltip (also on the disabled state), a mode-independent accessible
/// name, and the stable author id. `selected` is `Some` for toggles (picker,
/// always-on-top), which then use the fill-weight icon and selected styling
/// while active. `tooltip` overrides the default "Action — Shortcut" text
/// where a control needs a richer hint (the picker's gesture).
fn toolbar_button(
    ui: &mut egui::Ui,
    style: ToolbarStyle,
    spec: &ToolbarButtonSpec,
    enabled: bool,
    selected: Option<bool>,
    tooltip: Option<String>,
) -> egui::Response {
    let active = selected == Some(true);
    let source = if active { spec.active_icon.clone().unwrap_or_else(|| spec.icon.clone()) } else { spec.icon.clone() };
    let image = egui::Image::new(source).alt_text(spec.label);

    let mut button = match style {
        ToolbarStyle::IconsOnly => egui::Button::image(image),
        ToolbarStyle::IconsAndText => egui::Button::image_and_text(image, spec.label),
    }
    .image_tint_follows_text_color(true);
    if let Some(active) = selected {
        button = button.selected(active).frame_when_inactive(active).frame(true);
    }

    let response = ui.add_enabled(enabled, button);

    let tooltip = tooltip.unwrap_or_else(|| match &spec.shortcut {
        Some(shortcut) => format!("{} \u{2014} {}", spec.label, ui.ctx().format_shortcut(shortcut)),
        None => spec.label.to_string(),
    });
    let response = response.on_hover_text(tooltip.clone()).on_disabled_hover_text(tooltip);

    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_author_id(spec.author_id);
        // The icon's alt text should already provide the accessible name; set
        // it explicitly only when nothing else did (e.g. icons-only mode where
        // alt text does not propagate to the button node).
        if node.label().is_none_or(str::is_empty) {
            node.set_label(spec.label);
        }
    });

    response
}

fn pick_element_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/crosshair.svg"),
        active_icon: Some(egui::include_image!("../../assets/icons/crosshair-fill.svg")),
        label: "Pick Element",
        author_id: "picker-toggle",
        shortcut: None,
    }
}

fn refresh_node_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/arrow-clockwise.svg"),
        active_icon: None,
        label: "Refresh Node",
        author_id: "refresh-node",
        shortcut: Some(egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5)),
    }
}

fn refresh_subtree_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/arrows-clockwise.svg"),
        active_icon: None,
        label: "Refresh Subtree",
        author_id: "refresh-subtree",
        shortcut: Some(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::F5)),
    }
}

fn highlight_node_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/highlighter.svg"),
        active_icon: None,
        label: "Highlight Node",
        author_id: "highlight-node",
        shortcut: Some(egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::H)),
    }
}

fn always_on_top_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/push-pin.svg"),
        active_icon: Some(egui::include_image!("../../assets/icons/push-pin-fill.svg")),
        label: "Always on Top",
        author_id: "always-on-top",
        shortcut: None,
    }
}

/// The Maximize/Restore window button (headerbar mode): icon and label follow
/// the current window state under the one stable author id.
fn window_maximize_spec(maximized: bool) -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: if maximized {
            egui::include_image!("../../assets/icons/corners-in.svg")
        } else {
            egui::include_image!("../../assets/icons/corners-out.svg")
        },
        active_icon: None,
        label: if maximized { "Restore" } else { "Maximize" },
        author_id: "window-maximize",
        shortcut: None,
    }
}

fn window_close_spec() -> ToolbarButtonSpec {
    ToolbarButtonSpec {
        icon: egui::include_image!("../../assets/icons/x.svg"),
        active_icon: None,
        label: "Close",
        author_id: "window-close",
        shortcut: None,
    }
}

/// Actions emitted by the main toolbar.
pub enum MainToolbarAction {
    /// Arm/disarm the live mouse-picker.
    SetPickerArmed(bool),
    /// Refresh the currently selected node.
    RefreshNode,
    /// Refresh the currently selected node and descendants.
    RefreshSubtree,
    /// Highlight the currently selected node.
    HighlightNode,
    /// Raise the window to always-on-top / restore the normal level.
    SetAlwaysOnTop(bool),
}

/// Render the main toolbar (a themed panel directly below the menu bar):
/// the Pick Element toggle, the node actions, and — right-aligned — the
/// Always-on-Top pin toggle. Returns actions to process.
pub fn show_toolbar(
    ui: &mut egui::Ui,
    style: ToolbarStyle,
    picker_supported: bool,
    picker_armed: bool,
    picker_combo_label: &str,
    always_on_top: bool,
    has_node_selection: bool,
) -> Vec<MainToolbarAction> {
    let mut actions = Vec::new();

    egui::Panel::top("main_toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            let picker_tooltip = if picker_supported {
                format!("Pick Element \u{2014} hold {picker_combo_label} while armed to pick")
            } else {
                "Pick Element \u{2014} live picking is unavailable on this platform".to_string()
            };
            if toolbar_button(
                ui,
                style,
                &pick_element_spec(),
                picker_supported,
                Some(picker_armed),
                Some(picker_tooltip),
            )
            .clicked()
            {
                actions.push(MainToolbarAction::SetPickerArmed(!picker_armed));
            }

            ui.separator();

            if toolbar_button(ui, style, &refresh_node_spec(), has_node_selection, None, None).clicked() {
                actions.push(MainToolbarAction::RefreshNode);
            }
            if toolbar_button(ui, style, &refresh_subtree_spec(), has_node_selection, None, None).clicked() {
                actions.push(MainToolbarAction::RefreshSubtree);
            }
            if toolbar_button(ui, style, &highlight_node_spec(), has_node_selection, None, None).clicked() {
                actions.push(MainToolbarAction::HighlightNode);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if toolbar_button(ui, style, &always_on_top_spec(), true, Some(always_on_top), None).clicked() {
                    actions.push(MainToolbarAction::SetAlwaysOnTop(!always_on_top));
                }
            });
        });
    });

    actions
}

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
    /// Open the Settings dialog.
    ShowSettings,
    /// Open the About dialog.
    ShowAbout,
}

/// Actions emitted by the search row.
pub enum ToolbarAction {
    /// User pressed Enter in the search bar — evaluate XPath.
    EvaluateXPath,
    /// User clicked Stop — cancel running search.
    CancelSearch,
    /// Search input text changed.
    SearchTextChanged,
}

/// Stable egui id of the XPath search input field.
pub fn search_field_id() -> egui::Id {
    egui::Id::new("inspector_xpath_search_field")
}

/// Render the application menu bar. With `window_controls` (headerbar mode on
/// decoration-less Wayland sessions) the bar additionally hosts right-aligned
/// Maximize/Restore and Close window buttons, and the empty space between the
/// menus and those buttons acts as a move grip: drag starts a compositor-side
/// interactive window move, double-click toggles maximized.
pub fn show_menu_bar(
    ui: &mut egui::Ui,
    has_node_selection: bool,
    is_searching: bool,
    window_controls: bool,
) -> Vec<MenuAction> {
    let mut actions = Vec::new();

    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Settings\u{2026}").clicked() {
                    actions.push(MenuAction::ShowSettings);
                    ui.close();
                }
                ui.separator();
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

            if window_controls {
                show_window_controls(ui);
            }
        });
    });

    actions
}

/// Render the headerbar window controls into the space right of the menus:
/// right-aligned Close and Maximize/Restore buttons, then the leftover width
/// as the move grip. The grip is allocated after every clickable widget and
/// only covers genuinely empty space, so it can never shadow a menu or button
/// click; it carries no label, so the accessibility tree gains no addressable
/// element.
fn show_window_controls(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));

        // Window buttons are always icon-only (GNOME headerbar style),
        // independent of the toolbar's display-style setting.
        if toolbar_button(ui, ToolbarStyle::IconsOnly, &window_close_spec(), true, None, None).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if toolbar_button(ui, ToolbarStyle::IconsOnly, &window_maximize_spec(maximized), true, None, None).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        let grip = ui.allocate_response(ui.available_size(), egui::Sense::click_and_drag());
        // Only a started drag begins the move — a plain press must stay a
        // no-op so it remains available for the double-click.
        if grip.drag_started_by(egui::PointerButton::Primary) {
            // egui-winit honors StartDrag only while the window has keyboard
            // focus — worth having in a trace when a drag seems swallowed.
            let focused = ctx.input(|input| input.viewport().focused);
            tracing::debug!(?focused, "menu-bar move grip: drag started, requesting interactive window move");
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if grip.double_clicked() {
            tracing::debug!("menu-bar move grip: double click, toggling maximized");
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
    });
}

/// Render the search row: the XPath field and the Search/Stop button, laid
/// out right-to-left so the field takes all remaining row width. Returns
/// actions to process.
///
/// When `is_searching` is `true`, the Search button becomes a Stop button.
pub fn show_search_bar(
    ui: &mut egui::Ui,
    search_text: &mut String,
    search_error_hint: Option<&str>,
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

    egui::Panel::top("search_bar").exact_size(ui_height).show(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("\u{1F50D}");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Toggle Search / Stop button — added first so the field can take
                // exactly the width that remains.
                if is_searching {
                    if ui.button("\u{23F9} Stop").clicked() {
                        actions.push(ToolbarAction::CancelSearch);
                    }
                } else if ui.button("\u{25B6} Search").clicked() {
                    actions.push(ToolbarAction::EvaluateXPath);
                }

                let text_edit = ui.add(
                    egui::TextEdit::multiline(search_text)
                        .id(search_field_id())
                        .desired_rows(desired_rows)
                        .hint_text("XPath expression (Shift+Enter for new line)")
                        .desired_width(ui.available_width()),
                );

                if let Some(error_hint) = search_error_hint {
                    let theme_stroke_width = ui.visuals().widgets.hovered.bg_stroke.width;
                    let icon_color = ui.visuals().error_fg_color;
                    let horizontal_inset = ui.spacing().button_padding.x.max(4.0);
                    let vertical_inset = (ui.spacing().button_padding.y + theme_stroke_width).max(2.0);
                    let icon_size = (text_edit.rect.height() - vertical_inset * 2.0).clamp(12.0, 15.0);
                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(
                            text_edit.rect.right() - horizontal_inset - icon_size / 2.0,
                            text_edit.rect.center().y,
                        ),
                        egui::vec2(icon_size, icon_size),
                    );
                    let icon_response =
                        ui.interact(icon_rect, egui::Id::new("search_error_icon_button"), egui::Sense::click());
                    paint_warning_triangle_icon(ui, icon_response.rect, icon_color, theme_stroke_width);

                    let mut hover_text = String::from("Show XPath error details");
                    if let Some((line, column)) = extract_line_column(error_hint) {
                        hover_text = format!("XPath error at line {line}, column {column}");
                    }
                    if let Some(summary) = first_non_empty_line(error_hint) {
                        hover_text = format!("{hover_text}\n{summary}");
                    }

                    let icon_hovered = icon_response.hovered();
                    let icon_clicked = icon_response.clicked();
                    icon_response.on_hover_text(hover_text.clone());
                    if icon_clicked {
                        toggle_search_error_popup(ui.ctx());
                    }
                    show_search_error_popup(ui, icon_rect, error_hint);

                    // Spy-like behavior: show a short hover-style preview automatically
                    // when the current error text changes.
                    show_auto_error_preview(
                        ui,
                        icon_rect,
                        error_hint,
                        &hover_text,
                        !icon_hovered && !is_search_error_popup_open(ui.ctx()),
                    );
                } else {
                    set_search_error_popup_open(ui.ctx(), false);
                    reset_search_error_auto_preview(ui.ctx());
                }

                if text_edit.changed() {
                    actions.push(ToolbarAction::SearchTextChanged);
                }

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
                    // Always evaluate on Enter. The ViewModel cancels any in-flight
                    // search first, then starts a new one with the current query.
                    actions.push(ToolbarAction::EvaluateXPath);
                }

                let search_had_focus = ui.memory(|mem| mem.has_focus(search_field_id()));
                let search_has_focus_now = text_edit.has_focus();
                if search_had_focus || search_has_focus_now {
                    ui.memory_mut(|mem| {
                        mem.set_focus_lock_filter(
                            search_field_id(),
                            egui::EventFilter {
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                escape: true,
                                ..Default::default()
                            },
                        );
                    });
                }

                if search_has_focus_now && ui.input(|i| i.key_pressed(egui::Key::Escape)) && is_searching {
                    actions.push(ToolbarAction::CancelSearch);
                }
            });
        });
        ui.add_space(4.0);
    });

    actions
}

fn extract_line_column(error: &str) -> Option<(usize, usize)> {
    let line = number_after_keyword(error, "line")?;
    let column = number_after_keyword(error, "column")
        .or_else(|| number_after_keyword(error, "col"))
        .or_else(|| number_after_keyword(error, "position"))?;
    Some((line, column))
}

fn number_after_keyword(input: &str, keyword: &str) -> Option<usize> {
    let lower = input.to_ascii_lowercase();
    let idx = lower.find(keyword)?;
    let rest = &lower[idx + keyword.len()..];

    let mut digits = String::new();
    let mut seen_digit = false;
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            seen_digit = true;
        } else if seen_digit {
            break;
        }
    }

    if digits.is_empty() { None } else { digits.parse::<usize>().ok() }
}

fn first_non_empty_line(input: &str) -> Option<String> {
    input.lines().find(|line| !line.trim().is_empty()).map(|line| {
        const MAX_PREVIEW_CHARS: usize = 120;
        let trimmed = line.trim();
        if trimmed.chars().count() <= MAX_PREVIEW_CHARS {
            trimmed.to_string()
        } else {
            let shortened: String = trimmed.chars().take(MAX_PREVIEW_CHARS - 1).collect();
            format!("{shortened}\u{2026}")
        }
    })
}

fn paint_warning_triangle_icon(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32, stroke_width: f32) {
    let icon_rect = rect.shrink(1.0);
    let side = icon_rect.width().min(icon_rect.height()) * 0.95;
    let height = side * 0.866;
    let center = icon_rect.center();
    let top = egui::pos2(center.x, center.y - height / 2.0);
    let left = egui::pos2(center.x - side / 2.0, center.y + height / 2.0);
    let right = egui::pos2(center.x + side / 2.0, center.y + height / 2.0);
    let marker_color = ui.visuals().extreme_bg_color;

    ui.painter().add(egui::Shape::convex_polygon(vec![top, left, right], color, egui::Stroke::NONE));

    ui.painter().line_segment(
        [egui::pos2(center.x, center.y - height * 0.18), egui::pos2(center.x, center.y + height * 0.16)],
        egui::Stroke::new((stroke_width * 1.6).max(1.4), marker_color),
    );
    ui.painter().circle_filled(
        egui::pos2(center.x, center.y + height * 0.32),
        stroke_width.max(1.0) * 0.75,
        marker_color,
    );
}

fn search_error_popup_state_id() -> egui::Id {
    egui::Id::new("search_error_popup_open_state")
}

fn is_search_error_popup_open(ctx: &egui::Context) -> bool {
    ctx.data_mut(|data| data.get_temp::<bool>(search_error_popup_state_id()).unwrap_or(false))
}

fn set_search_error_popup_open(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|data| data.insert_temp(search_error_popup_state_id(), open));
}

fn toggle_search_error_popup(ctx: &egui::Context) {
    let open = is_search_error_popup_open(ctx);
    set_search_error_popup_open(ctx, !open);
}

fn search_error_auto_preview_state_id() -> egui::Id {
    egui::Id::new("search_error_auto_preview_state")
}

fn reset_search_error_auto_preview(ctx: &egui::Context) {
    let state_id = search_error_auto_preview_state_id();
    ctx.data_mut(|data| {
        let _ = data.remove_temp::<u64>(state_id.with("hash"));
        let _ = data.remove_temp::<f64>(state_id.with("since"));
    });
}

fn show_search_error_popup(ui: &egui::Ui, icon_rect: egui::Rect, error_hint: &str) {
    if !is_search_error_popup_open(ui.ctx()) {
        return;
    }

    let popup_id = egui::Id::new("search_error_popup");
    let popup_pos = egui::pos2(icon_rect.right() + 8.0, icon_rect.bottom() + 6.0);
    let popup = egui::Area::new(popup_id).order(egui::Order::Foreground).fixed_pos(popup_pos).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(640.0);
            if let Some((line, column)) = extract_line_column(error_hint) {
                ui.small(format!("Line {line}, Column {column}"));
            }
            if ui.small_button("Copy full error").clicked() {
                ui.ctx().copy_text(error_hint.to_string());
            }
            ui.separator();
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(error_hint).monospace()).selectable(true).wrap());
            });
        });
    });

    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        set_search_error_popup_open(ui.ctx(), false);
        return;
    }

    if ui.ctx().input(|i| i.pointer.any_pressed())
        && let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.press_origin())
        && !popup.response.rect.contains(pointer_pos)
        && !icon_rect.contains(pointer_pos)
    {
        set_search_error_popup_open(ui.ctx(), false);
    }
}

fn show_auto_error_preview(
    ui: &egui::Ui,
    icon_rect: egui::Rect,
    error_hint: &str,
    hover_text: &str,
    allow_auto_preview: bool,
) {
    let state_id = search_error_auto_preview_state_id();
    let mut hasher = DefaultHasher::new();
    error_hint.hash(&mut hasher);
    let current_hash = hasher.finish();
    let now = ui.ctx().input(|i| i.time);

    let (stored_hash, first_seen_at) = ui.ctx().data_mut(|data| {
        let stored_hash = data.get_temp::<u64>(state_id.with("hash")).unwrap_or_default();
        let stored_since = data.get_temp::<f64>(state_id.with("since")).unwrap_or(now);

        if stored_hash != current_hash {
            data.insert_temp(state_id.with("hash"), current_hash);
            data.insert_temp(state_id.with("since"), now);
            (current_hash, now)
        } else {
            (stored_hash, stored_since)
        }
    });

    if !allow_auto_preview || stored_hash != current_hash {
        return;
    }

    const AUTO_PREVIEW_SECONDS: f64 = 3.2;
    if now - first_seen_at > AUTO_PREVIEW_SECONDS {
        return;
    }

    ui.ctx().request_repaint();
    let pos = egui::pos2(icon_rect.right() + 8.0, icon_rect.bottom() + 6.0);
    egui::Area::new(state_id.with("bubble")).order(egui::Order::Foreground).fixed_pos(pos).interactable(false).show(
        ui.ctx(),
        |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(420.0);
                ui.label(egui::RichText::new("XPath error").strong());
                if let Some((line, column)) = extract_line_column(error_hint) {
                    ui.small(format!("Line {line}, Column {column}"));
                }
                ui.separator();
                ui.label(hover_text);
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximize_spec_swaps_icon_and_label_with_window_state() {
        let normal = window_maximize_spec(false);
        let maximized = window_maximize_spec(true);
        assert_eq!(normal.label, "Maximize");
        assert_eq!(maximized.label, "Restore");
        assert_ne!(normal.icon.uri(), maximized.icon.uri());
        // The author id — the acceptance-test contract — never changes.
        assert_eq!(normal.author_id, "window-maximize");
        assert_eq!(maximized.author_id, "window-maximize");
    }

    #[test]
    fn close_spec_has_the_stable_author_id() {
        assert_eq!(window_close_spec().author_id, "window-close");
    }
}
