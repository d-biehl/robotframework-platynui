//! Generic TreeView widget for egui.
//!
//! A self-contained, reusable tree component modelled after egui's own
//! [`Table`](egui_extras::Table) and [`CollapsingHeader`] patterns:
//!
//! - **Paint-first backgrounds** using [`egui::Frame`] (like `StripLayout::add`)
//! - **One-frame-delayed hover** tracking via `ui.data_mut()` (like `TableBody`)
//! - **Focus via `Sense::click()`** on the scroll area rect so Tab/Shift+Tab work
//!   and accesskit has a real node
//! - **Arrow-key lock** via `set_focus_lock_filter` with proper two-frame timing
//! - **Builder API** stays unchanged for callers

use eframe::egui;

// ── Data trait ───────────────────────────────────────────────────────────────

/// Trait that tree row data must implement to be displayed by [`TreeView`].
pub trait TreeRowData {
    /// Display label for the row.
    fn label(&self) -> &str;
    /// Nesting depth (0 = root).
    fn depth(&self) -> usize;
    /// Whether this node has children (shows disclosure triangle).
    fn has_children(&self) -> bool;
    /// Whether this node is currently expanded.
    fn is_expanded(&self) -> bool;
    /// Whether the underlying data is still valid.
    fn is_valid(&self) -> bool;
}

// ── Response types ───────────────────────────────────────────────────────────

/// Keyboard navigation actions reported by the tree widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeNavigate {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Response returned after rendering the tree.
///
/// The caller inspects these fields to update its ViewModel accordingly.
pub struct TreeResponse {
    /// Row that was clicked to select, if any.
    pub selected: Option<usize>,
    /// Row whose expand/collapse was toggled (click on chevron or double-click), if any.
    pub toggled: Option<usize>,
    /// Keyboard navigation action, if any (only when the tree has focus).
    pub navigate: Option<TreeNavigate>,
}

// ── Widget builder ───────────────────────────────────────────────────────────

/// Callback for building a context menu. Return `true` to close the menu.
type ContextMenuFn<'a> = Box<dyn FnMut(&mut egui::Ui, usize) -> bool + 'a>;

/// A generic, self-contained tree view widget for egui.
///
/// # Usage
///
/// ```ignore
/// let response = TreeView::new(&rows)
///     .selected(self.selected_index)
///     .focused(self.focused_index)
///     .scroll_to_focused(true)
///     .context_menu(|ui, idx| { /* custom menu items */ })
///     .show(ui);
/// ```
pub struct TreeView<'a, R: TreeRowData> {
    rows: &'a [R],
    selected: Option<usize>,
    focused: usize,
    scroll_to_focused: bool,
    indent_width: f32,
    context_menu_fn: Option<ContextMenuFn<'a>>,
}

impl<'a, R: TreeRowData> TreeView<'a, R> {
    /// Create a new tree view with the given rows.
    pub fn new(rows: &'a [R]) -> Self {
        Self {
            rows,
            selected: None,
            focused: 0,
            scroll_to_focused: false,
            indent_width: 16.0,
            context_menu_fn: None,
        }
    }

    /// Set the currently selected row index.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// Set the currently focused row index (keyboard cursor).
    pub fn focused(mut self, index: usize) -> Self {
        self.focused = index;
        self
    }

    /// When `true`, scroll the focused row into view on the next frame.
    pub fn scroll_to_focused(mut self, scroll: bool) -> Self {
        self.scroll_to_focused = scroll;
        self
    }

    /// Set the per-level indentation width (default: 16.0).
    #[allow(dead_code)]
    pub fn indent_width(mut self, width: f32) -> Self {
        self.indent_width = width;
        self
    }

    /// Provide a context menu builder. Return `true` if the menu should close.
    pub fn context_menu(mut self, f: impl FnMut(&mut egui::Ui, usize) -> bool + 'a) -> Self {
        self.context_menu_fn = Some(Box::new(f));
        self
    }

    /// Render the tree view widget. Returns a [`TreeResponse`].
    pub fn show(mut self, ui: &mut egui::Ui) -> TreeResponse {
        let mut response = TreeResponse { selected: None, toggled: None, navigate: None };

        // ── Stable IDs ───────────────────────────────────────────────────
        let tree_id = ui.id().with("tree_view");
        let hovered_id = tree_id.with("hovered_row");
        let ctx_row_id = tree_id.with("ctx_row");

        // ── Cross-frame state ────────────────────────────────────────────
        // Focus: read from last frame.  The real focusable widget is
        // registered *after* the ScrollArea (see below).
        let tree_had_focus = ui.memory(|mem| mem.has_focus(tree_id));

        // Hover: one-frame-delayed, same pattern as egui's Table.
        let hovered_row: Option<usize> = ui.data_mut(|d| d.remove_temp(hovered_id));

        // Row geometry collected during rendering — used for hit-testing
        // after the ScrollArea so we need only ONE click handler on the
        // whole tree area (avoids overlapping Sense::CLICK widgets).
        let mut row_rects: Vec<egui::Rect> = Vec::with_capacity(self.rows.len());
        let mut chevron_rects: Vec<Option<egui::Rect>> = Vec::with_capacity(self.rows.len());

        // ── Scroll area with all rows ────────────────────────────────────
        let scroll_output = egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            for (i, row) in self.rows.iter().enumerate() {
                let is_selected = self.selected == Some(i);
                let is_focused = self.focused == i;
                let is_hovered = hovered_row == Some(i);

                // ── Row background (paint-first via Frame) ───────────
                let fill = if is_selected {
                    ui.visuals().selection.bg_fill
                } else if is_hovered {
                    ui.visuals().widgets.hovered.bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };

                let stroke = if tree_had_focus && (is_selected || is_focused) {
                    egui::Stroke::new(1.0, ui.visuals().selection.stroke.color)
                } else {
                    egui::Stroke::new(1.0, egui::Color32::TRANSPARENT)
                };

                let row_frame = egui::Frame::NONE
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(2.0)
                    .inner_margin(egui::Margin::symmetric(2, 0));

                let frame_resp = row_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // ── Indent ───────────────────────────────────
                        let indent = row.depth() as f32 * self.indent_width;
                        ui.add_space(indent);

                        // ── Disclosure chevron ───────────────────────
                        let chevron_rect = if row.has_children() {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            if ui.is_rect_visible(rect) {
                                paint_chevron(ui, rect, row.is_expanded());
                            }
                            Some(rect)
                        } else {
                            ui.add_space(20.0);
                            None
                        };

                        // ── Label ────────────────────────────────────
                        let mut text = egui::RichText::new(row.label());
                        if !row.is_valid() {
                            text = text.strikethrough().color(egui::Color32::from_gray(150));
                        }
                        if is_selected {
                            text = text.strong();
                        }
                        ui.label(text);

                        // Consume remaining horizontal space so the
                        // Frame spans the full available width.
                        let remaining = ui.available_width();
                        if remaining > 0.0 {
                            ui.add_space(remaining);
                        }

                        chevron_rect
                    })
                    .inner
                });

                // Collect geometry for hit-testing after the ScrollArea.
                row_rects.push(frame_resp.response.rect);
                chevron_rects.push(frame_resp.inner);

                // Scroll to focused row.
                if self.scroll_to_focused && self.focused == i {
                    frame_resp.response.scroll_to_me(None);
                }
            }
        });

        // ── Single focusable + clickable widget ──────────────────────────
        // Sense::click() = CLICK | FOCUSABLE — makes the tree reachable
        // via Tab/Shift+Tab and creates a real accesskit node.  This is
        // the ONLY click handler for the whole tree area; individual rows
        // have no click sense, so there is no overlap/stealing.
        let tree_focus =
            ui.interact(scroll_output.inner_rect, tree_id, egui::Sense::click());

        // Sense::click() is focusable via Tab but does NOT auto-focus on
        // click.  We must explicitly request focus on any click so that
        // keyboard navigation works afterwards.
        if tree_focus.clicked() || tree_focus.secondary_clicked() || tree_focus.double_clicked() {
            ui.memory_mut(|mem| mem.request_focus(tree_id));
        }

        // ── Click → select or toggle ─────────────────────────────────────
        if tree_focus.clicked()
            && let Some(pos) = tree_focus.interact_pointer_pos()
        {
            for (i, rect) in row_rects.iter().enumerate() {
                if rect.contains(pos) {
                    let on_chevron =
                        chevron_rects[i].is_some_and(|cr| cr.contains(pos));
                    if on_chevron {
                        response.toggled = Some(i);
                    } else {
                        response.selected = Some(i);
                    }
                    break;
                }
            }
        }

        // ── Double-click → toggle ────────────────────────────────────────
        if tree_focus.double_clicked()
            && let Some(pos) = tree_focus.interact_pointer_pos()
        {
            for (i, rect) in row_rects.iter().enumerate() {
                if rect.contains(pos) {
                    if self.rows.get(i).is_some_and(TreeRowData::has_children) {
                        response.toggled = Some(i);
                    }
                    break;
                }
            }
        }

        // ── Context menu ─────────────────────────────────────────────────
        // On right-click, store which row was targeted.  The stored index
        // persists via temp data while the popup is open.
        if tree_focus.secondary_clicked()
            && let Some(pos) = tree_focus.interact_pointer_pos()
        {
            for (i, rect) in row_rects.iter().enumerate() {
                if rect.contains(pos) {
                    ui.data_mut(|d| d.insert_temp(ctx_row_id, i));
                    break;
                }
            }
        }
        if let Some(ctx_fn) = &mut self.context_menu_fn {
            tree_focus.context_menu(|menu_ui| {
                if let Some(row_idx) = menu_ui.data(|d| d.get_temp::<usize>(ctx_row_id))
                    && ctx_fn(menu_ui, row_idx)
                {
                    menu_ui.close();
                }
            });
        }

        // ── Hover tracking ───────────────────────────────────────────────
        // Write hovered row for next frame (one-frame-delayed like Table).
        if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
            && scroll_output.inner_rect.contains(pointer_pos)
        {
            for (i, rect) in row_rects.iter().enumerate() {
                if rect.contains(pointer_pos) {
                    ui.data_mut(|d| d.insert_temp(hovered_id, i));
                    break;
                }
            }
        }

        // ── Arrow-key lock ───────────────────────────────────────────────
        // Always set the filter when we have focus.  On the very first
        // frame of focus acquisition the filter won't take effect (egui
        // requires had_focus_last_frame), but it will be ready for the
        // next frame's begin_pass().
        let tree_has_focus_now = tree_focus.has_focus();
        if tree_had_focus || tree_has_focus_now {
            ui.memory_mut(|mem| {
                mem.set_focus_lock_filter(
                    tree_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
        }

        // ── Keyboard navigation ──────────────────────────────────────────
        // Process when we had focus last frame (filter was active, events
        // are ours) OR when we just gained focus this frame (first key
        // press might still arrive before the lock takes effect).
        if tree_had_focus || tree_has_focus_now {
            let events = ui.input(|i| i.events.clone());
            for event in &events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    response.navigate = match key {
                        egui::Key::ArrowUp => Some(TreeNavigate::Up),
                        egui::Key::ArrowDown => Some(TreeNavigate::Down),
                        egui::Key::ArrowLeft => Some(TreeNavigate::Left),
                        egui::Key::ArrowRight => Some(TreeNavigate::Right),
                        egui::Key::Home => Some(TreeNavigate::Home),
                        egui::Key::End => Some(TreeNavigate::End),
                        egui::Key::PageUp => Some(TreeNavigate::PageUp),
                        egui::Key::PageDown => Some(TreeNavigate::PageDown),
                        _ => None,
                    };
                    if response.navigate.is_some() {
                        break;
                    }
                }
            }
        }

        response
    }
}

// ── Chevron painter ──────────────────────────────────────────────────────────

/// Paint a small disclosure triangle inside `rect`.
///
/// When `expanded` the triangle points down; when collapsed it points right.
/// This mirrors egui's own `CollapsingHeader` icon and avoids relying on
/// Unicode characters that may be missing from the default font.
fn paint_chevron(ui: &egui::Ui, rect: egui::Rect, expanded: bool) {
    let color = ui.visuals().text_color();
    let center = rect.center();
    let half = 4.0_f32; // half-size of the triangle

    let points = if expanded {
        // Down-pointing triangle: ▾
        vec![
            egui::pos2(center.x - half, center.y - half * 0.5),
            egui::pos2(center.x + half, center.y - half * 0.5),
            egui::pos2(center.x, center.y + half * 0.5),
        ]
    } else {
        // Right-pointing triangle: ▶
        vec![
            egui::pos2(center.x - half * 0.5, center.y - half),
            egui::pos2(center.x + half * 0.5, center.y),
            egui::pos2(center.x - half * 0.5, center.y + half),
        ]
    };

    ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
}