//! `PlatynUI` egui test application.
//!
//! A configurable egui application that provides a variety of standard widgets for
//! integration testing. It can be used as:
//!
//! - **Wayland client** for compositor IPC tests (window management commands)
//! - **Accessibility target** for `PlatynUI` functional tests via `AccessKit`/AT-SPI
//!
//! # Usage
//!
//! ```sh
//! # Basic — opens a window with default widgets
//! platynui-test-app-egui
//!
//! # Custom app ID and title (for compositor window matching)
//! platynui-test-app-egui --app-id "com.platynui.test" --title "My Test Window"
//!
//! # Auto-close after 10 seconds (for CI)
//! platynui-test-app-egui --auto-close 10
//!
//! # Connect to a specific Wayland compositor
//! WAYLAND_DISPLAY=platynui-test-xyz platynui-test-app-egui
//! ```

use std::time::Instant;

use clap::Parser;
use eframe::egui;

/// egui test application for `PlatynUI` integration testing.
#[derive(Parser, Debug)]
#[command(name = "platynui-test-app-egui")]
#[command(about = "egui test application for PlatynUI integration and functional testing")]
struct Cli {
    /// Application ID (Wayland `app_id` / X11 `WM_CLASS`).
    #[arg(long, default_value = "org.platynui.test.egui")]
    app_id: String,

    /// Window title.
    #[arg(long, default_value = "PlatynUI Test App (egui)")]
    title: String,

    /// Auto-close after N seconds (0 = never).
    #[arg(long, default_value_t = 0)]
    auto_close: u64,

    /// Log level.
    #[arg(long = "log-level", value_enum, default_value = "warn")]
    log_level: LogLevel,
}

/// Supported log levels.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        };
        f.write_str(s)
    }
}

fn init_tracing(level: LogLevel) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new(level.to_string())
    };

    tracing_subscriber::fmt().with_env_filter(filter).with_target(true).with_writer(std::io::stderr).init();
}

fn main() -> eframe::Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.log_level);

    tracing::info!(app_id = %cli.app_id, title = %cli.title, "starting test application");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&cli.title)
            .with_app_id(&cli.app_id)
            .with_inner_size([600.0, 500.0]),
        ..Default::default()
    };

    let auto_close_secs = cli.auto_close;
    let title = cli.title.clone();

    eframe::run_native(
        &cli.title,
        options,
        Box::new(move |cc| {
            // Use default egui style with slightly larger text for readability
            let mut style = (*cc.egui_ctx.global_style()).clone();
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::new(14.0, egui::FontFamily::Proportional));
            cc.egui_ctx.set_global_style(style);

            Ok(Box::new(TestApp::new(auto_close_secs, title)))
        }),
    )
}

/// Main application state holding all widget values.
struct TestApp {
    /// Start time for auto-close feature.
    start_time: Instant,
    /// Auto-close timeout in seconds (0 = disabled).
    auto_close_secs: u64,
    /// Window title — forwarded to the AccessKit root node so the window is
    /// discoverable by name on the AT-SPI tree (egui does not do this itself).
    title: String,

    // --- Widget state ---
    /// Single-line text input.
    text_input: String,
    /// Multi-line text area.
    text_area: String,
    /// Checkbox state.
    checkbox_checked: bool,
    /// Second checkbox for testing multiple.
    checkbox_enabled: bool,
    /// Radio button selection.
    radio_selection: RadioChoice,
    /// Slider value.
    slider_value: f32,
    /// Spinner / `DragValue`.
    spinner_value: i32,
    /// `ComboBox` selection index.
    combo_selection: usize,
    /// Progress bar value (0.0 – 1.0).
    progress: f32,
    /// Click counter for button.
    click_count: u32,
    /// Toggle switch state.
    toggle_on: bool,
    /// Collapsing header open state (managed by egui).
    /// We track a value inside it.
    collapsing_value: String,
}

/// Radio button choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RadioChoice {
    OptionA,
    OptionB,
    OptionC,
}

impl std::fmt::Display for RadioChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OptionA => write!(f, "Option A"),
            Self::OptionB => write!(f, "Option B"),
            Self::OptionC => write!(f, "Option C"),
        }
    }
}

const COMBO_ITEMS: &[&str] = &["Apple", "Banana", "Cherry", "Date", "Elderberry"];

impl TestApp {
    fn new(auto_close_secs: u64, title: String) -> Self {
        Self {
            start_time: Instant::now(),
            auto_close_secs,
            title,
            text_input: String::from("PlatynUI"),
            text_area: String::from("Hello,\nWorld!"),
            checkbox_checked: false,
            checkbox_enabled: true,
            radio_selection: RadioChoice::OptionA,
            slider_value: 50.0,
            spinner_value: 0,
            combo_selection: 0,
            progress: 0.35,
            click_count: 0,
            toggle_on: false,
            collapsing_value: String::from("Hidden text"),
        }
    }
}

impl eframe::App for TestApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    #[expect(deprecated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-close logic
        if self.auto_close_secs > 0 && self.start_time.elapsed().as_secs() >= self.auto_close_secs {
            tracing::info!("auto-close timeout reached, shutting down");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Forward the window title to the AccessKit root (Role::Window) node so
        // the window is discoverable by name on the AT-SPI tree. egui creates
        // the root node without a label and does not propagate the OS window
        // title to the accessibility tree on its own.
        ctx.accesskit_node_builder(egui::accesskit_root_id(), |node| {
            node.set_label(self.title.clone());
        });

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| Self::show_menu_bar(ui, ctx));
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| self.show_status_bar(ui));
        // The scroll-box fixture lives in its own panel, outside the central ScrollArea, so
        // over-scrolling it clamps at the box's own edges instead of leaking to an outer area —
        // which keeps the box (and a fixed hover point inside it) put for the acceptance suite.
        egui::SidePanel::right("scroll_fixture").resizable(false).exact_width(250.0).show(ctx, |ui| {
            Self::show_scroll_box(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.show_widgets(ui));
        });
    }
}

/// Extension to tag a widget's AccessKit node with a stable author id, surfaced to
/// the accessibility tree as `@Id` so acceptance tests can locate a widget by a
/// stable identifier instead of its visible label. The id must be unique among the
/// node's siblings (we use globally-unique ids, which is stronger).
trait WithAuthorId {
    fn with_id(self, author_id: &str) -> Self;
}

impl WithAuthorId for egui::Response {
    fn with_id(self, author_id: &str) -> Self {
        self.ctx.accesskit_node_builder(self.id, |node| node.set_author_id(author_id));
        self
    }
}

/// Extension to set a widget's AccessKit description, surfaced to the accessibility
/// tree as `@Description` (via AT-SPI `Accessible.Description`) so the acceptance
/// suite can verify the common `control:Description` attribute end-to-end.
trait WithDescription {
    fn with_description(self, description: &str) -> Self;
}

impl WithDescription for egui::Response {
    fn with_description(self, description: &str) -> Self {
        self.ctx.accesskit_node_builder(self.id, |node| node.set_description(description));
        self
    }
}

impl TestApp {
    /// Render the top menu bar.
    fn show_menu_bar(ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").with_id("menu-file-new").clicked() {
                    tracing::debug!("menu: File > New");
                    ui.close();
                }
                if ui.button("Open").with_id("menu-file-open").clicked() {
                    tracing::debug!("menu: File > Open");
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").with_id("menu-file-quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            })
            .response
            .with_id("menu-file");
            ui.menu_button("Edit", |ui| {
                if ui.button("Cut").with_id("menu-edit-cut").clicked() {
                    ui.close();
                }
                if ui.button("Copy").with_id("menu-edit-copy").clicked() {
                    ui.close();
                }
                if ui.button("Paste").with_id("menu-edit-paste").clicked() {
                    ui.close();
                }
            })
            .response
            .with_id("menu-edit");
            ui.menu_button("Help", |ui| {
                if ui.button("About").with_id("menu-help-about").clicked() {
                    ui.close();
                }
            })
            .response
            .with_id("menu-help");
        });
    }

    /// Render the bottom status bar.
    fn show_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Clicks: {}", self.click_count)).with_id("status-clicks");
            ui.separator();
            ui.label(format!("Slider: {:.0}", self.slider_value)).with_id("status-slider");
            ui.separator();
            ui.label(format!("Radio: {}", self.radio_selection)).with_id("status-radio");
        });
    }

    /// Render a dedicated, fixed-size scroll box plus an external offset readout, used by the
    /// `Pointer Scroll` acceptance suite. The box always overflows on both axes (its content is far
    /// larger than its fixed size), and its rows are plain labels that do not consume the wheel, so a
    /// scroll over the box drives the `ScrollArea` itself. The current offset is surfaced *outside*
    /// the box — so it stays put while the content moves — as two `@Id`-tagged labels whose text is
    /// the bare integer offset, ready for a numeric assertion (`number(@Name)`).
    fn show_scroll_box(ui: &mut egui::Ui) {
        ui.heading("Scroll Box");
        let output = egui::ScrollArea::both()
            .id_salt("scrollbox")
            .max_width(220.0)
            .max_height(120.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Disable wrapping so each row stays wide and the area overflows horizontally too.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                for row in 0..40 {
                    ui.label(format!("scroll row {row:02} — drag the wheel to move this content around"))
                        .with_id(&format!("scrollbox-row-{row:02}"));
                }
            });
        let offset = output.state.offset;
        ui.horizontal(|ui| {
            ui.label("Offset x:");
            ui.label(format!("{:.0}", offset.x)).with_id("scrollbox-offset-x");
            ui.label("y:");
            ui.label(format!("{:.0}", offset.y)).with_id("scrollbox-offset-y");
        });
    }

    /// Render the central widget area with all test widgets.
    #[allow(clippy::too_many_lines)]
    fn show_widgets(&mut self, ui: &mut egui::Ui) {
        // --- Buttons ---
        ui.heading("Buttons");
        ui.horizontal(|ui| {
            if ui.button("Click Me").with_id("btn-click-me").with_description("Increments the click counter").clicked()
            {
                self.click_count += 1;
                tracing::debug!(count = self.click_count, "button clicked");
            }
            if ui.button("Reset").with_id("btn-reset").clicked() {
                self.click_count = 0;
                self.slider_value = 50.0;
                self.spinner_value = 0;
                self.progress = 0.35;
                tracing::debug!("reset clicked");
            }
            let enabled = self.checkbox_enabled;
            ui.add_enabled(enabled, egui::Button::new("Conditional")).with_id("btn-conditional");
        });

        ui.add_space(8.0);

        // --- Text Input ---
        ui.heading("Text Input");
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.text_input).with_id("input-name");
        });
        ui.horizontal(|ui| {
            ui.label("Notes:");
            ui.add(egui::TextEdit::multiline(&mut self.text_area).desired_rows(3)).with_id("input-notes");
        });

        ui.add_space(8.0);

        // --- Checkboxes ---
        ui.heading("Checkboxes");
        ui.checkbox(&mut self.checkbox_checked, "Accept Terms").with_id("chk-accept-terms");
        ui.checkbox(&mut self.checkbox_enabled, "Enable Conditional Button").with_id("chk-enable-conditional");

        ui.add_space(8.0);

        // --- Toggle ---
        ui.heading("Toggle");
        ui.horizontal(|ui| {
            ui.label("Dark Mode:");
            ui.add(toggle(&mut self.toggle_on)).with_id("toggle-dark-mode");
        });

        ui.add_space(8.0);

        // --- Radio Buttons ---
        ui.heading("Radio Buttons");
        ui.radio_value(&mut self.radio_selection, RadioChoice::OptionA, "Option A").with_id("radio-option-a");
        ui.radio_value(&mut self.radio_selection, RadioChoice::OptionB, "Option B").with_id("radio-option-b");
        ui.radio_value(&mut self.radio_selection, RadioChoice::OptionC, "Option C").with_id("radio-option-c");

        ui.add_space(8.0);

        // --- Slider ---
        ui.heading("Slider");
        ui.add(egui::Slider::new(&mut self.slider_value, 0.0..=100.0).text("Value")).with_id("slider-value");

        ui.add_space(8.0);

        // --- Spinner / DragValue ---
        ui.heading("Spinner");
        ui.horizontal(|ui| {
            ui.label("Count:");
            ui.add(egui::DragValue::new(&mut self.spinner_value).range(-100..=100)).with_id("spin-count");
        });

        ui.add_space(8.0);

        // --- ComboBox ---
        ui.heading("ComboBox");
        egui::ComboBox::from_label("Fruit")
            .selected_text(COMBO_ITEMS[self.combo_selection])
            .show_ui(ui, |ui| {
                for (i, item) in COMBO_ITEMS.iter().enumerate() {
                    ui.selectable_value(&mut self.combo_selection, i, *item)
                        .with_id(&format!("combo-item-{}", item.to_lowercase()));
                }
            })
            .response
            .with_id("combo-fruit");

        ui.add_space(8.0);

        // --- Progress Bar ---
        ui.heading("Progress");
        ui.add(egui::ProgressBar::new(self.progress).text(format!("{:.0}%", self.progress * 100.0)).animate(false))
            .with_id("progress-bar");
        ui.add(egui::Slider::new(&mut self.progress, 0.0..=1.0).text("Set Progress")).with_id("slider-progress");

        ui.add_space(8.0);

        // --- Collapsing Section ---
        ui.heading("Collapsing");
        egui::CollapsingHeader::new("Advanced Settings")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Secret:");
                    ui.text_edit_singleline(&mut self.collapsing_value).with_id("input-secret");
                });
                ui.label("This section is collapsible.");
            })
            .header_response
            .with_id("collapse-advanced");

        ui.add_space(8.0);

        // --- Tooltip ---
        ui.heading("Tooltip");
        ui.label("Hover me!").on_hover_text("This is a tooltip with extra information.").with_id("label-hover");

        ui.add_space(16.0);

        // --- Separator + clickable link ---
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Test app for");
            ui.hyperlink_to("PlatynUI", "https://github.com/imbus/robotframework-PlatynUI").with_id("link-platynui");
        });
    }
}

/// Simple toggle switch widget (borrowed from egui demo).
fn toggle(on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| -> egui::Response {
        let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));

        if ui.is_rect_visible(rect) {
            let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
            let visuals = ui.style().interact_selectable(&response, *on);
            let rect = rect.expand(visuals.expansion);
            let radius = 0.5 * rect.height();
            ui.painter().rect(rect, radius, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
            let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
            let center = egui::pos2(circle_x, rect.center().y);
            ui.painter().circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
        }

        response
    }
}
