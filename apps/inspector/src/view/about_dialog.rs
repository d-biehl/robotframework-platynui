//! View: About dialog for application metadata and project links.

use eframe::egui;

const ABOUT_SUBTITLE: &str = "Cross-platform UI automation inspector for Robot Framework";

fn issue_tracker_url(repo_url: &str) -> String {
    format!("{}/issues", repo_url.trim_end_matches('/'))
}

fn contributors_url(repo_url: &str) -> String {
    format!("{}/graphs/contributors", repo_url.trim_end_matches('/'))
}

/// Render the About dialog when `open` is `true`.
pub fn show_about_dialog(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    let app_name = "PlatynUI Inspector";
    let version = env!("CARGO_PKG_VERSION");
    let license = env!("CARGO_PKG_LICENSE");
    let repository = env!("CARGO_PKG_REPOSITORY");
    let documentation = option_env!("CARGO_PKG_DOCUMENTATION").unwrap_or(repository);
    let issues = issue_tracker_url(repository);
    let contributors = contributors_url(repository);
    let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    let copy_payload = format!(
        "{app_name}\nVersion: {version}\nLicense: {license}\nPlatform: {platform}\nRepository: {repository}\nDocumentation: {documentation}\nIssue tracker: {issues}"
    );

    let mut close = false;

    egui::Window::new("About PlatynUI Inspector")
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(open)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading(app_name);
                ui.label(ABOUT_SUBTITLE);
                ui.add_space(8.0);

                egui::Grid::new("about_info_grid").num_columns(2).striped(true).show(ui, |ui| {
                    ui.strong("Version:");
                    ui.label(version);
                    ui.end_row();

                    ui.strong("License:");
                    ui.label(license);
                    ui.end_row();

                    ui.strong("Platform:");
                    ui.label(&platform);
                    ui.end_row();
                });

                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to("Repository", repository);
                    ui.separator();
                    ui.hyperlink_to("Documentation", documentation);
                    ui.separator();
                    ui.hyperlink_to("Issue tracker", &issues);
                    ui.separator();
                    ui.hyperlink_to("Contributors", &contributors);
                });

                ui.add_space(8.0);
                ui.label("Open-source licenses and notices are provided with the distributed project artifacts.");

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy build info").clicked() {
                        ui.ctx().copy_text(copy_payload.clone());
                    }

                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });

    if close {
        *open = false;
    }
}
