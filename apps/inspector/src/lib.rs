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

use view::{properties, toolbar, tree_view};
use viewmodel::inspector_vm::InspectorViewModel;

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
        let search_actions = toolbar::show_search_bar(ctx, &mut self.vm.search_text, &mut self.vm.always_on_top);

        // View: Results Panel (bottom)
        let result_actions = toolbar::show_results_panel(ctx, &self.vm.results, self.vm.result_status.as_deref());

        // Process toolbar actions
        for action in search_actions.into_iter().chain(result_actions) {
            match action {
                toolbar::ToolbarAction::EvaluateXPath => self.vm.evaluate_xpath(),
                toolbar::ToolbarAction::RevealResult(i) => self.vm.reveal_and_select_result(i),
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

                // Handle keyboard navigation only when no text widget has focus
                let wants_keyboard = ctx.memory(|mem| mem.focused().is_some());
                if !wants_keyboard {
                    let events = ui.input(|i| i.events.clone());
                    for event in &events {
                        if let egui::Event::Key { key, pressed: true, .. } = event {
                            match key {
                                egui::Key::ArrowUp => self.vm.navigate_up(),
                                egui::Key::ArrowDown => self.vm.navigate_down(),
                                egui::Key::ArrowLeft => self.vm.navigate_left(),
                                egui::Key::ArrowRight => self.vm.navigate_right(),
                                egui::Key::Home => self.vm.navigate_home(),
                                egui::Key::End => self.vm.navigate_end(),
                                egui::Key::PageUp => self.vm.navigate_page_up(),
                                egui::Key::PageDown => self.vm.navigate_page_down(),
                                _ => {}
                            }
                        }
                    }
                }

                // View renders tree, returns actions
                let snapshot: Vec<_> = self.vm.tree.rows().to_vec();
                let scroll = self.vm.scroll_to_focused;
                let actions =
                    tree_view::show_tree(ui, &snapshot, self.vm.selected_index, self.vm.focused_index, scroll);
                // Consume the scroll flag after rendering
                self.vm.scroll_to_focused = false;

                // Process actions back into ViewModel
                for action in actions {
                    match action {
                        tree_view::TreeAction::Select(i) => self.vm.select_node(i),
                        tree_view::TreeAction::Toggle(i) => self.vm.tree.toggle(i),
                        tree_view::TreeAction::Refresh(i) => self.vm.refresh_row(i),
                        tree_view::TreeAction::RefreshSubtree(i) => self.vm.refresh_subtree(i),
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 750.0]).with_title("PlatynUI Inspector"),
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
