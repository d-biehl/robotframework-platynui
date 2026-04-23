//! View: Bottom status bar for a simple worker activity indicator.

use crate::model::automation;
use eframe::egui;

/// Render the bottom status bar.
///
/// Shows a simple overall worker status: green circle for idle, red rotating indicator for active tasks.
pub fn show_status_bar(ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").resizable(false).exact_size(28.0).show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            // Status indicator: green circle (idle) or red rotating element (running)
            let is_idle = automation::active_task_counts().iter().all(|(_, count)| *count == 0);

            if is_idle {
                // Green circle for idle
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(100, 200, 100));
            } else {
                // Red rotating indicator for active tasks
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                let time = ui.input(|i| i.time) as f32;
                let angle = (time * 4.0) % std::f32::consts::TAU;

                // Draw red outer circle
                ui.painter().circle_stroke(
                    rect.center(),
                    5.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 50, 50)),
                );

                // Draw rotating indicator
                let indicator_length = 3.5;
                let start = rect.center() + egui::Vec2::angled(angle) * indicator_length;
                let end = rect.center() + egui::Vec2::angled(angle) * 5.0;
                ui.painter().line_segment([start, end], egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 100, 100)));

                ui.ctx().request_repaint(); // Keep animating
            }
        });
    });
}
