//! PlatynUI Inspector — egui-based GUI for exploring the UI accessibility tree.
//!
//! Architecture: Model–ViewModel–View (MVVM)
//!
//! ```text
//! src/
//! ├── main.rs              ← Entry point, wires M-VM-V together
//! ├── lib.rs               ← Library entry (run function, app struct)
//! ├── model/               ← M: Data structures, PlatynUI integration
//! │   ├── mod.rs
//! │   └── tree_data.rs     ← UiNodeData (cached wrapper around UiNode)
//! ├── viewmodel/           ← VM: Application state & logic
//! │   ├── mod.rs
//! │   ├── tree_vm.rs       ← TreeViewModel (expand/collapse/navigate)
//! │   └── inspector_vm.rs  ← InspectorViewModel (overall app state)
//! └── view/                ← V: Pure UI rendering (egui)
//!     ├── mod.rs
//!     ├── tree_view.rs     ← TreeView panel
//!     ├── properties.rs    ← Properties table
//!     └── toolbar.rs       ← Menu, search bar, results panel
//! ```

mod model;
mod view;
mod viewmodel;

use clap::{Parser, ValueEnum};
use eframe::egui;
use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;
use std::sync::Arc;

use view::{properties, results_panel, toolbar, tree_view};
use viewmodel::inspector_vm::InspectorViewModel;

/// Load the embedded application icon as [`egui::IconData`].
///
/// The PNG is compiled into the binary via `include_bytes!` and decoded at
/// startup so every platform (Windows, macOS, Linux) gets a window icon
/// without external files.
fn load_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(png_bytes).expect("Failed to decode embedded icon PNG");
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    egui::IconData { rgba: rgba.into_raw(), width, height }
}

// Link platform-specific providers (AT-SPI on Linux, UIA on Windows, AX on macOS)
platynui_link_providers!();

/// CLI arguments for the inspector.
#[derive(Parser)]
#[command(author, version, about = "PlatynUI Inspector", long_about = None)]
struct InspectorArgs {
    /// Set the log level for diagnostic output (written to stderr).
    /// Overrides the `PLATYNUI_LOG_LEVEL` environment variable.
    /// Use `RUST_LOG` for fine-grained per-crate filtering.
    #[arg(long = "log-level", value_enum)]
    log_level: Option<LogLevel>,
}

/// Supported log level values for the `--log-level` CLI flag.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Initialize the tracing subscriber.
///
/// Priority (highest wins):
/// 1. `RUST_LOG` environment variable (fine-grained per-crate filtering)
/// 2. `--log-level` CLI argument
/// 3. `PLATYNUI_LOG_LEVEL` environment variable
/// 4. Default: `warn`
fn init_tracing(cli_level: Option<LogLevel>) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let directive = if let Some(level) = cli_level {
            match level {
                LogLevel::Error => "error",
                LogLevel::Warn => "warn",
                LogLevel::Info => "info",
                LogLevel::Debug => "debug",
                LogLevel::Trace => "trace",
            }
            .to_string()
        } else if let Ok(val) = std::env::var("PLATYNUI_LOG_LEVEL") {
            val
        } else {
            "warn".to_string()
        };
        EnvFilter::new(directive)
    };

    tracing_subscriber::fmt().with_env_filter(filter).with_target(true).with_writer(std::io::stderr).init();
}

/// The eframe `App` that connects ViewModel to View.
struct InspectorApp {
    vm: InspectorViewModel,
    properties_sort: properties::PropertiesSortState,
}

impl InspectorApp {
    fn new(runtime: Arc<Runtime>) -> Self {
        Self { vm: InspectorViewModel::new(runtime), properties_sort: properties::PropertiesSortState::default() }
    }
}

impl eframe::App for InspectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply "Always On Top" setting
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.vm.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));

        // View: Menu Bar
        toolbar::show_menu_bar(ctx);

        // View: Search Bar
        let is_searching = self.vm.is_searching();
        let search_actions =
            toolbar::show_search_bar(ctx, &mut self.vm.search_text, &mut self.vm.always_on_top, is_searching);

        // Process toolbar actions (must happen before poll so a new
        // search is started before the first poll in the same frame).
        for action in search_actions {
            match action {
                toolbar::ToolbarAction::EvaluateXPath => self.vm.evaluate_xpath(),
                toolbar::ToolbarAction::CancelSearch => self.vm.cancel_search(),
            }
        }

        // Poll background search for new results BEFORE rendering the
        // results panel so the count shown in the header and the status
        // text are always consistent within a single frame.
        self.vm.poll_search(ctx);

        // Poll background reveal (tree sync) so the tree updates once
        // the ancestor path is pre-loaded.
        self.vm.poll_reveal(ctx);

        // View: Results Panel (bottom)
        let result_actions = results_panel::show_results_panel(
            ctx,
            &self.vm.results,
            self.vm.result_status.as_deref(),
            &mut self.vm.result_focused_index,
        );

        // Process result actions
        for action in result_actions {
            match action {
                results_panel::ResultAction::Reveal(i) => self.vm.reveal_and_select_result(i),
            }
        }

        // View: Tree Panel (left side)
        egui::SidePanel::left("tree_panel")
            .resizable(true)
            .default_width(450.0)
            .min_width(200.0)
            .max_width(ctx.content_rect().width() - 200.0)
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                ui.strong("UI Elements");
                ui.separator();

                // View renders tree via TreeView widget, returns TreeResponse
                let snapshot: Vec<_> = self.vm.tree.rows().to_vec();
                let scroll = self.vm.scroll_to_focused;
                let response = tree_view::TreeView::new(&snapshot)
                    .selected(self.vm.selected_index)
                    .focused(self.vm.focused_index)
                    .scroll_to_focused(scroll)
                    .context_menu(|ui, i| {
                        let mut close = false;
                        if ui.button("Refresh").clicked() {
                            self.vm.refresh_row(i);
                            close = true;
                        }
                        if ui.button("Refresh subtree").clicked() {
                            self.vm.refresh_subtree(i);
                            close = true;
                        }
                        close
                    })
                    .show(ui);

                // Consume the scroll flag after rendering
                self.vm.scroll_to_focused = false;

                // Process TreeResponse back into ViewModel
                if let Some(i) = response.selected {
                    self.vm.select_node(i);
                }
                if let Some(i) = response.toggled {
                    self.vm.tree.toggle(i);
                }
                if let Some(nav) = response.navigate {
                    match nav {
                        tree_view::TreeNavigate::Up => self.vm.navigate_up(),
                        tree_view::TreeNavigate::Down => self.vm.navigate_down(),
                        tree_view::TreeNavigate::Left => self.vm.navigate_left(),
                        tree_view::TreeNavigate::Right => self.vm.navigate_right(),
                        tree_view::TreeNavigate::Home => self.vm.navigate_home(),
                        tree_view::TreeNavigate::End => self.vm.navigate_end(),
                        tree_view::TreeNavigate::PageUp => self.vm.navigate_page_up(response.page_size),
                        tree_view::TreeNavigate::PageDown => self.vm.navigate_page_down(response.page_size),
                    }
                }
            });

        // View: Properties Panel (center)
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.vm.selected_index.is_some() {
                properties::show_properties(
                    ui,
                    &self.vm.selected_label,
                    &self.vm.selected_attributes,
                    &mut self.properties_sort,
                );
            } else {
                properties::show_no_selection(ui);
            }
        });
    }
}

/// Run the inspector application.
///
/// Creates the PlatynUI runtime, initializes tracing, and opens the egui window.
///
/// # Errors
///
/// Returns an error if runtime creation or the GUI event loop fails.
pub fn run() -> eframe::Result {
    let args = InspectorArgs::parse();
    init_tracing(args.log_level);

    let runtime = Runtime::new().expect("Failed to create PlatynUI runtime");
    let runtime = Arc::new(runtime);

    let icon = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_title("PlatynUI Inspector")
            .with_app_id("org.platynui.inspector")
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "PlatynUI Inspector",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(InspectorApp::new(runtime)))
        }),
    )
}
