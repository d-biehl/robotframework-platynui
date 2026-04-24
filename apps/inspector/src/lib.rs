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
//!     ├── attributes.rs    ← Attributes table
//!     └── toolbar.rs       ← Menu, search bar, results panel
//! ```

mod model;
mod view;
mod viewmodel;

use crate::model::automation;
use crate::model::tree_data::UiNodeData;

use clap::{Parser, ValueEnum};
use eframe::egui;
use platynui_core::ui::UiNode;
use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;
use std::collections::BTreeSet;
use std::sync::Arc;

use view::{attributes, results_panel, status_bar, toolbar, tree_view};
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
    attributes_sort: attributes::AttributesSortState,
    attributes_view_mode: attributes::AttributesViewMode,
    attribute_filter: String,
    pinned_attributes: BTreeSet<String>,
    collapsed_attribute_groups: BTreeSet<String>,
    prev_always_on_top: Option<bool>,
    show_about_dialog: bool,
}

#[derive(Clone, Copy, Debug)]
enum AppCommand {
    ShowAbout,
    EvaluateXPath,
    CancelSearch,
    ClearResults,
    RefreshNode,
    RefreshSubtree,
    HighlightNode,
    ExpandNode,
    CollapseNode,
    FocusSearch,
}

impl InspectorApp {
    fn new(runtime: Arc<Runtime>, preloaded_root_children: Vec<Arc<UiNodeData>>) -> Self {
        Self {
            vm: InspectorViewModel::new(runtime, preloaded_root_children),
            attributes_sort: attributes::AttributesSortState::default(),
            attributes_view_mode: attributes::AttributesViewMode::default(),
            attribute_filter: String::new(),
            pinned_attributes: BTreeSet::new(),
            collapsed_attribute_groups: BTreeSet::new(),
            prev_always_on_top: None,
            show_about_dialog: false,
        }
    }

    fn collect_shortcut_commands(ctx: &egui::Context) -> Vec<AppCommand> {
        let mut commands = Vec::new();

        let search_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
        if ctx.input_mut(|i| i.consume_shortcut(&search_shortcut)) {
            commands.push(AppCommand::EvaluateXPath);
        }

        let focus_search_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::F);
        if ctx.input_mut(|i| i.consume_shortcut(&focus_search_shortcut)) {
            commands.push(AppCommand::FocusSearch);
        }

        let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5);
        if ctx.input_mut(|i| i.consume_shortcut(&refresh_shortcut)) {
            commands.push(AppCommand::RefreshNode);
        }

        let refresh_subtree_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::F5);
        if ctx.input_mut(|i| i.consume_shortcut(&refresh_subtree_shortcut)) {
            commands.push(AppCommand::RefreshSubtree);
        }

        let results_have_focus = ctx.memory(|mem| mem.has_focus(results_panel::focus_id()));
        let highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::H);
        if !results_have_focus && ctx.input_mut(|i| i.consume_shortcut(&highlight_shortcut)) {
            commands.push(AppCommand::HighlightNode);
        }

        let search_has_focus = ctx.memory(|mem| mem.has_focus(toolbar::search_field_id()));
        if !search_has_focus && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            commands.push(AppCommand::CancelSearch);
        }

        commands
    }

    fn execute_command(&mut self, ctx: &egui::Context, command: AppCommand) {
        match command {
            AppCommand::ShowAbout => self.show_about_dialog = true,
            AppCommand::EvaluateXPath => self.vm.evaluate_xpath(),
            AppCommand::CancelSearch => self.vm.cancel_search(),
            AppCommand::ClearResults => self.vm.clear_results(),
            AppCommand::RefreshNode => self.vm.refresh_selected_row(),
            AppCommand::RefreshSubtree => self.vm.refresh_selected_subtree(),
            AppCommand::HighlightNode => self.vm.highlight_selected_row(),
            AppCommand::ExpandNode => self.vm.expand_selected_row(),
            AppCommand::CollapseNode => self.vm.collapse_selected_row(),
            AppCommand::FocusSearch => {
                ctx.memory_mut(|mem| mem.request_focus(toolbar::search_field_id()));
            }
        }
    }
}

impl eframe::App for InspectorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let is_searching = self.vm.is_searching();
        let has_node_selection = self.vm.selected_index.is_some();

        for command in Self::collect_shortcut_commands(&ctx) {
            self.execute_command(&ctx, command);
        }

        // View: Menu Bar (top)
        let menu_actions = toolbar::show_menu_bar(ui, has_node_selection, is_searching);

        for action in menu_actions {
            match action {
                toolbar::MenuAction::EvaluateXPath => self.execute_command(&ctx, AppCommand::EvaluateXPath),
                toolbar::MenuAction::CancelSearch => self.execute_command(&ctx, AppCommand::CancelSearch),
                toolbar::MenuAction::ClearResults => self.execute_command(&ctx, AppCommand::ClearResults),
                toolbar::MenuAction::RefreshNode => self.execute_command(&ctx, AppCommand::RefreshNode),
                toolbar::MenuAction::RefreshSubtree => self.execute_command(&ctx, AppCommand::RefreshSubtree),
                toolbar::MenuAction::HighlightNode => self.execute_command(&ctx, AppCommand::HighlightNode),
                toolbar::MenuAction::ExpandNode => self.execute_command(&ctx, AppCommand::ExpandNode),
                toolbar::MenuAction::CollapseNode => self.execute_command(&ctx, AppCommand::CollapseNode),
                toolbar::MenuAction::ShowAbout => self.execute_command(&ctx, AppCommand::ShowAbout),
            }
        }

        // View: Search Bar (below menu)
        let search_actions = toolbar::show_search_bar(
            ui,
            &mut self.vm.search_text,
            &mut self.vm.always_on_top,
            is_searching,
            has_node_selection,
        );

        // Apply "Always On Top" setting only when it changes to avoid
        // flooding the window manager with _NET_WM_STATE requests every frame.
        // Must run after toolbar rendering so checkbox changes take effect
        // in the same frame.
        if self.prev_always_on_top != Some(self.vm.always_on_top) {
            self.prev_always_on_top = Some(self.vm.always_on_top);
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.vm.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
        }

        // Process toolbar actions (must happen before poll so a new
        // search is started before the first poll in the same frame).
        for action in search_actions {
            match action {
                toolbar::ToolbarAction::EvaluateXPath => self.execute_command(&ctx, AppCommand::EvaluateXPath),
                toolbar::ToolbarAction::CancelSearch => self.execute_command(&ctx, AppCommand::CancelSearch),
                toolbar::ToolbarAction::RefreshNode => self.execute_command(&ctx, AppCommand::RefreshNode),
                toolbar::ToolbarAction::RefreshSubtree => self.execute_command(&ctx, AppCommand::RefreshSubtree),
            }
        }

        // Poll background initial tree load (must run every frame while loading).
        self.vm.poll_initial_load(&ctx);

        // Poll background search for new results BEFORE rendering the
        // results panel so the count shown in the header and the status
        // text are always consistent within a single frame.
        self.vm.poll_search(&ctx);

        // Poll background reveal (tree sync) so the tree updates once
        // the ancestor path is pre-loaded.
        self.vm.poll_reveal(&ctx);

        // Poll background selected-node details so selection never blocks UI.
        self.vm.poll_selection(&ctx);

        // View: Status Bar (bottom-most)
        status_bar::show_status_bar(ui);

        // View: Results Panel (above status bar)
        let result_actions = results_panel::show_results_panel(
            ui,
            &self.vm.results,
            self.vm.result_status.as_deref(),
            &mut self.vm.result_focused_index,
        );

        // Process result actions
        for action in result_actions {
            match action {
                results_panel::ResultAction::Reveal(i) => self.vm.reveal_and_select_result(i),
                results_panel::ResultAction::Highlight(i) => self.vm.highlight_result(i),
                results_panel::ResultAction::CopyLabel(i) => {
                    if let Some(result) = self.vm.results.get(i) {
                        ctx.copy_text(result.display_label().to_string());
                    }
                }
                results_panel::ResultAction::CopyRuntimeId(i) => {
                    if let Some(runtime_id) = self.vm.results.get(i).and_then(|result| result.runtime_id()) {
                        ctx.copy_text(runtime_id);
                    }
                }
                results_panel::ResultAction::CopyAttributeValue(i) => {
                    if let Some(value) = self.vm.results.get(i).and_then(|result| result.attribute_value()) {
                        ctx.copy_text(value.to_string());
                    }
                }
                results_panel::ResultAction::CopyFullResult(i) => {
                    if let Some(result) = self.vm.results.get(i) {
                        ctx.copy_text(result.full_copy_text());
                    }
                }
            }
        }

        // View: Tree Panel (left side)
        egui::Panel::left("tree_panel")
            .resizable(true)
            .default_size(450.0)
            .min_size(200.0)
            .max_size(ctx.content_rect().width() - 200.0)
            .show_inside(ui, |ui| {
                ui.set_min_height(ui.available_height());
                ui.strong("UI Elements");
                ui.separator();

                // View renders tree via TreeView widget, returns TreeResponse.
                // While the initial background load is in progress the row list
                // is empty; it fills in automatically once poll_initial_load()
                // calls expand_root() and requests a repaint.
                let snapshot: Vec<_> = self.vm.tree.rows().to_vec();
                let scroll = self.vm.scroll_to_focused;
                let response = tree_view::TreeView::new(&snapshot)
                    .selected(self.vm.selected_index)
                    .focused(self.vm.focused_index)
                    .scroll_to_focused(scroll)
                    .context_menu(|ui, i| {
                        let mut close = false;

                        let refresh_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5);
                        if ui
                            .add(
                                egui::Button::new("Refresh Node")
                                    .shortcut_text(ui.ctx().format_shortcut(&refresh_shortcut)),
                            )
                            .clicked()
                        {
                            self.vm.refresh_row(i);
                            close = true;
                        }

                        let refresh_subtree_shortcut =
                            egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::F5);
                        if ui
                            .add(
                                egui::Button::new("Refresh Subtree")
                                    .shortcut_text(ui.ctx().format_shortcut(&refresh_subtree_shortcut)),
                            )
                            .clicked()
                        {
                            self.vm.refresh_subtree(i);
                            close = true;
                        }

                        let highlight_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::H);
                        if ui
                            .add(
                                egui::Button::new("Highlight Node")
                                    .shortcut_text(ui.ctx().format_shortcut(&highlight_shortcut)),
                            )
                            .clicked()
                        {
                            self.vm.highlight_row(i);
                            close = true;
                        }
                        ui.separator();

                        let expand_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowRight);
                        if ui
                            .add(
                                egui::Button::new("Expand Node")
                                    .shortcut_text(ui.ctx().format_shortcut(&expand_shortcut)),
                            )
                            .clicked()
                        {
                            self.vm.tree.expand(i);
                            close = true;
                        }

                        let collapse_shortcut =
                            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowLeft);
                        if ui
                            .add(
                                egui::Button::new("Collapse Node")
                                    .shortcut_text(ui.ctx().format_shortcut(&collapse_shortcut)),
                            )
                            .clicked()
                        {
                            self.vm.tree.collapse(i);
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

        // View: Attributes Panel (remaining central area)
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.vm.selected_index.is_some() {
                attributes::show_attributes(
                    ui,
                    &self.vm.selected_label,
                    &self.vm.selected_attributes,
                    &mut self.attributes_sort,
                    &mut self.attributes_view_mode,
                    &mut self.attribute_filter,
                    &mut self.pinned_attributes,
                    &mut self.collapsed_attribute_groups,
                );
            } else {
                attributes::show_no_selection(ui);
            }
        });

        view::about_dialog::show_about_dialog(&ctx, &mut self.show_about_dialog);
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

    // Snapshot top-level nodes before creating the inspector window.
    // This avoids expensive UIA root traversal while the inspector is
    // already advertising its own accessibility tree.
    let rt_for_preload = Arc::clone(&runtime);
    let preloaded_root_children = automation::run(move || {
        let root = rt_for_preload.desktop_node();
        let raw_children: Vec<Arc<dyn UiNode>> = root.children().collect();
        let mut out = Vec::with_capacity(raw_children.len());
        for node in raw_children {
            let data = Arc::new(UiNodeData::new(node));
            data.preload_caches();
            out.push(data);
        }
        out
    });

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
            automation::register_ui_thread();
            cc.egui_ctx.set_visuals(egui::Visuals::dark());

            // Load system fonts as additional fallbacks so that the system UI
            // font is used for script-specific glyphs (CJK, Arabic, etc.) while
            // egui's built-in proportional font remains the primary face.
            let mut fonts = egui::FontDefinitions::default();
            egui_system_fonts::add_auto(&cc.egui_ctx, &mut fonts, egui_system_fonts::FontStyle::Sans);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(InspectorApp::new(Arc::clone(&runtime), preloaded_root_children)))
        }),
    )
}
