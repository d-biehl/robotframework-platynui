//! View: Settings dialog for the Inspector's persisted configuration.

use eframe::egui;

use crate::viewmodel::picker::Modifiers;

/// Render the Settings dialog when `open` is `true`. Returns the new picker
/// activation combination when the user changed it this frame.
pub fn show_settings_dialog(ctx: &egui::Context, open: &mut bool, combo: Modifiers) -> Option<Modifiers> {
    if !*open {
        return None;
    }

    let mut changed_combo = None;
    let mut close = false;

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(open)
        .show(ctx, |ui| {
            ui.strong("Live picker activation");
            ui.label("Modifiers to hold while the picker is armed to pick the element under the cursor:");
            ui.add_space(4.0);

            let mut mods = combo;
            // Lock the last remaining checked modifier so the combination can
            // never become empty ("picking whenever no key is held").
            let only_one = [mods.ctrl, mods.alt, mods.shift].into_iter().filter(|&m| m).count() == 1;
            let mut changed = false;
            ui.horizontal(|ui| {
                changed |=
                    ui.add_enabled(!(only_one && mods.ctrl), egui::Checkbox::new(&mut mods.ctrl, "Ctrl")).changed();
                changed |= ui.add_enabled(!(only_one && mods.alt), egui::Checkbox::new(&mut mods.alt, "Alt")).changed();
                changed |=
                    ui.add_enabled(!(only_one && mods.shift), egui::Checkbox::new(&mut mods.shift, "Shift")).changed();
            });
            if changed {
                changed_combo = Some(mods);
            }

            ui.add_space(10.0);
            if ui.button("Close").clicked() {
                close = true;
            }
        });

    if close {
        *open = false;
    }
    changed_combo
}
