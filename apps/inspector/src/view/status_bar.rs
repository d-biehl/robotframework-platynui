//! View: Bottom status bar — transient activity/result messages on the left,
//! persistent state (the picker) right-aligned.

use eframe::egui;

/// Render the bottom status bar.
///
/// The left segment shows a simple overall async-task status (green circle
/// for idle, red rotating indicator for active tasks) plus the transient
/// `status_text` (search/result statuses, one-off events like a completed
/// pick). The right segment renders `picker_state_text` — persistent state
/// that transient messages never overwrite or hide.
pub fn show_status_bar(ui: &mut egui::Ui, has_active_tasks: bool, status_text: Option<&str>, picker_state_text: &str) {
    egui::Panel::bottom("status_bar").resizable(false).exact_size(28.0).show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if has_active_tasks {
                // Red rotating indicator for active background tasks.
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                let time = ui.input(|i| i.time) as f32;
                let angle = (time * 4.0) % std::f32::consts::TAU;

                ui.painter().circle_stroke(
                    rect.center(),
                    5.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 50, 50)),
                );

                let indicator_length = 3.5;
                let start = rect.center() + egui::Vec2::angled(angle) * indicator_length;
                let end = rect.center() + egui::Vec2::angled(angle) * 5.0;
                ui.painter()
                    .line_segment([start, end], egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 100, 100)));

                ui.ctx().request_repaint();
            } else {
                // Green circle for idle.
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(100, 200, 100));
            }

            // Right-to-left: the persistent segment claims the right edge
            // first, the transient message gets whatever width remains.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(egui::Color32::from_gray(170), picker_state_text);
                ui.separator();

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some(status_text) = status_text {
                        ui.separator();
                        if status_text.starts_with("Error") {
                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), status_text);
                        } else {
                            ui.colored_label(egui::Color32::from_gray(170), status_text);
                        }
                    }
                });
            });
        });
    });
}
