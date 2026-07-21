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
//!     └── toolbar.rs       ← Menu, main toolbar, search bar
//! ```

mod model;
mod modifiers;
mod view;
mod viewmodel;

use crate::model::tree_data::UiNodeData;
use clap::{Parser, ValueEnum};
use eframe::egui;
#[cfg(target_os = "windows")]
use eframe::wgpu;
use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;
use std::collections::BTreeSet;
use std::sync::Arc;

#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::HKEY_CURRENT_USER;

use view::{attributes, results_panel, status_bar, toolbar, tree_view};
use viewmodel::inspector_vm::InspectorViewModel;
use viewmodel::picker::Modifiers as PickerModifiers;

/// Inspector settings persisted across runs via eframe's storage (a RON file
/// at [`settings_path`]). Only these explicit settings are persisted — egui
/// memory and window geometry are not, so runs stay deterministic apart from
/// what the user deliberately configured.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct PersistedSettings {
    /// The live picker's activation modifier combination.
    picker_combo: PickerModifiers,
    /// The toolbar display style (icons only / icons and text).
    toolbar_style: toolbar::ToolbarStyle,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self { picker_combo: PickerModifiers::CTRL_ALT_SHIFT, toolbar_style: toolbar::ToolbarStyle::default() }
    }
}

/// Path of the settings file: `<config dir>/platynui/inspector.ron`, following
/// the project convention already used by the compositor
/// (`$XDG_CONFIG_HOME/platynui/compositor.toml`). Settings are user
/// *configuration*, so they belong in the config directory — eframe's default
/// would be the *data* directory (`~/.local/share/<app_id>/app.ron`).
/// `PLATYNUI_INSPECTOR_SETTINGS_PATH` overrides the full file path — the
/// acceptance suites use it to keep test runs hermetic against the user's
/// real settings (a configured non-default picker combination would otherwise
/// change test behavior).
fn settings_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os(SETTINGS_PATH_ENV) {
        return Some(std::path::PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    let config_dir = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let config_dir = home::home_dir().map(|home| home.join("Library").join("Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home::home_dir().map(|home| home.join(".config")));
    config_dir.map(|dir| dir.join("platynui").join("inspector.ron"))
}

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

const RENDERER_ENV: &str = "PLATYNUI_INSPECTOR_RENDERER";
const GLOW_HARDWARE_ACCELERATION_ENV: &str = "PLATYNUI_INSPECTOR_GLOW_HARDWARE_ACCELERATION";
const SEARCH_RESULT_LIMIT_ENV: &str = "PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT";
const SETTINGS_PATH_ENV: &str = "PLATYNUI_INSPECTOR_SETTINGS_PATH";
const DEFAULT_SEARCH_RESULT_LIMIT: usize = 5_000;

/// CLI arguments for the inspector.
#[derive(Parser)]
#[command(author, version, about = "PlatynUI Inspector", long_about = None)]
struct InspectorArgs {
    /// Set the log level for diagnostic output (written to stderr).
    /// Overrides the `PLATYNUI_LOG_LEVEL` environment variable.
    /// Use `RUST_LOG` for fine-grained per-crate filtering.
    #[arg(long = "log-level", value_enum)]
    log_level: Option<LogLevel>,

    /// Rendering backend to use (`wgpu` or `glow`).
    /// Overrides the `PLATYNUI_INSPECTOR_RENDERER` environment variable.
    #[arg(long = "renderer", value_enum)]
    renderer: Option<RendererChoice>,

    /// Glow renderer hardware acceleration policy (`required`, `preferred`, or `off`).
    /// Overrides the `PLATYNUI_INSPECTOR_GLOW_HARDWARE_ACCELERATION` environment variable.
    #[arg(long = "glow-hardware-acceleration", value_enum)]
    glow_hardware_acceleration: Option<GlowHardwareAccelerationChoice>,

    /// Maximum XPath search results to collect in the Inspector (`unlimited` disables the guard).
    /// Overrides the `PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT` environment variable.
    #[arg(long = "search-result-limit", value_name = "COUNT|unlimited", value_parser = parse_search_result_limit)]
    search_result_limit: Option<SearchResultLimitChoice>,
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

/// Supported renderer backends for the inspector.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum RendererChoice {
    #[default]
    Wgpu,
    Glow,
}

impl RendererChoice {
    fn resolve(cli_choice: Option<Self>) -> Self {
        if let Some(choice) = cli_choice {
            return choice;
        }

        match std::env::var(RENDERER_ENV) {
            Ok(value) => Self::parse_env_value(&value).unwrap_or_else(|| {
                tracing::warn!(env = RENDERER_ENV, value = %value, "ignoring invalid inspector renderer");
                Self::default()
            }),
            Err(std::env::VarError::NotPresent) => Self::default(),
            Err(error) => {
                tracing::warn!(env = RENDERER_ENV, %error, "ignoring unreadable inspector renderer");
                Self::default()
            }
        }
    }

    fn parse_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "wgpu" => Some(Self::Wgpu),
            "glow" | "gl" | "opengl" => Some(Self::Glow),
            _ => None,
        }
    }

    fn to_eframe(self) -> eframe::Renderer {
        match self {
            Self::Wgpu => eframe::Renderer::Wgpu,
            Self::Glow => eframe::Renderer::Glow,
        }
    }
}

impl std::fmt::Display for RendererChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wgpu => "wgpu".fmt(formatter),
            Self::Glow => "glow".fmt(formatter),
        }
    }
}

fn inspector_wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut options = eframe::egui_wgpu::WgpuConfiguration::default();
    configure_inspector_wgpu_options(&mut options);
    options
}

#[cfg(target_os = "windows")]
fn configure_inspector_wgpu_options(options: &mut eframe::egui_wgpu::WgpuConfiguration) {
    let eframe::egui_wgpu::WgpuSetup::CreateNew(create_new) = &mut options.wgpu_setup else {
        return;
    };

    let backends = wgpu::Backends::from_env().unwrap_or(preferred_windows_wgpu_backends());
    create_new.instance_descriptor.backends = backends;
    let selector: eframe::egui_wgpu::NativeAdapterSelectorMethod = Arc::new(select_windows_wgpu_adapter);
    create_new.native_adapter_selector = Some(selector);
}

#[cfg(not(target_os = "windows"))]
fn configure_inspector_wgpu_options(_options: &mut eframe::egui_wgpu::WgpuConfiguration) {}

#[cfg(target_os = "windows")]
fn preferred_windows_wgpu_backends() -> wgpu::Backends {
    // Do not include GL in the default Windows mask: eframe/wgpu enumerates all adapters in the
    // mask before the adapter selector runs, and GL enumeration can trigger the slow UIA path.
    // Users can still opt into GL explicitly via WGPU_BACKEND=gl when diagnosing renderer issues.
    wgpu::Backends::VULKAN | wgpu::Backends::DX12
}

#[cfg(target_os = "windows")]
fn select_windows_wgpu_adapter(
    adapters: &[wgpu::Adapter],
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    for preferred_backend in [wgpu::Backend::Vulkan, wgpu::Backend::Dx12, wgpu::Backend::Gl] {
        if let Some(adapter) = adapters.iter().find(|adapter| {
            adapter.get_info().backend == preferred_backend && adapter_supports_surface(adapter, compatible_surface)
        }) {
            return Ok(adapter.clone());
        }
    }

    if let Some(adapter) = adapters.iter().find(|adapter| adapter_supports_surface(adapter, compatible_surface)) {
        return Ok(adapter.clone());
    }

    let available_adapters = adapters
        .iter()
        .map(|adapter| {
            let info = adapter.get_info();
            format!("{} ({}, {:?})", info.name, info.backend, info.device_type)
        })
        .collect::<Vec<_>>()
        .join(", ");

    if available_adapters.is_empty() {
        Err("no wgpu adapters are available for the configured backend mask".to_string())
    } else {
        Err(format!(
            "no surface-compatible wgpu adapter found for the configured backend mask; available adapters: {available_adapters}"
        ))
    }
}

#[cfg(target_os = "windows")]
fn adapter_supports_surface(adapter: &wgpu::Adapter, compatible_surface: Option<&wgpu::Surface<'_>>) -> bool {
    compatible_surface.is_none_or(|surface| adapter.is_surface_supported(surface))
}

/// Supported hardware acceleration policies for the glow renderer.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum GlowHardwareAccelerationChoice {
    Required,
    #[default]
    Preferred,
    Off,
}

impl GlowHardwareAccelerationChoice {
    fn resolve(cli_choice: Option<Self>) -> Self {
        if let Some(choice) = cli_choice {
            return choice;
        }

        match std::env::var(GLOW_HARDWARE_ACCELERATION_ENV) {
            Ok(value) => Self::parse_env_value(&value).unwrap_or_else(|| {
                tracing::warn!(
                    env = GLOW_HARDWARE_ACCELERATION_ENV,
                    value = %value,
                    "ignoring invalid inspector glow hardware acceleration policy"
                );
                Self::default()
            }),
            Err(std::env::VarError::NotPresent) => Self::default(),
            Err(error) => {
                tracing::warn!(
                    env = GLOW_HARDWARE_ACCELERATION_ENV,
                    %error,
                    "ignoring unreadable inspector glow hardware acceleration policy"
                );
                Self::default()
            }
        }
    }

    fn parse_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "required" | "require" => Some(Self::Required),
            "preferred" | "prefer" | "auto" | "on" | "true" | "yes" | "1" => Some(Self::Preferred),
            "off" | "false" | "no" | "0" | "disabled" | "disable" => Some(Self::Off),
            _ => None,
        }
    }

    fn to_eframe(self) -> eframe::HardwareAcceleration {
        match self {
            Self::Required => eframe::HardwareAcceleration::Required,
            Self::Preferred => eframe::HardwareAcceleration::Preferred,
            Self::Off => eframe::HardwareAcceleration::Off,
        }
    }

    fn effective_for_renderer(self, renderer: RendererChoice) -> Self {
        if matches!((renderer, self), (RendererChoice::Glow, Self::Off)) {
            tracing::warn!(
                renderer = %renderer,
                requested_glow_hardware_acceleration = %self,
                effective_glow_hardware_acceleration = %Self::Preferred,
                "glow renderer cannot reliably force software OpenGL; falling back to preferred hardware acceleration"
            );
            return Self::Preferred;
        }

        self
    }
}

impl std::fmt::Display for GlowHardwareAccelerationChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Required => "required".fmt(formatter),
            Self::Preferred => "preferred".fmt(formatter),
            Self::Off => "off".fmt(formatter),
        }
    }
}

/// Supported Inspector XPath search result limit values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchResultLimitChoice {
    Limited(usize),
    Unlimited,
}

impl SearchResultLimitChoice {
    fn resolve(cli_choice: Option<Self>) -> Self {
        if let Some(choice) = cli_choice {
            return choice;
        }

        match std::env::var(SEARCH_RESULT_LIMIT_ENV) {
            Ok(value) => Self::parse_env_value(&value).unwrap_or_else(|| {
                tracing::warn!(
                    env = SEARCH_RESULT_LIMIT_ENV,
                    value = %value,
                    default_limit = DEFAULT_SEARCH_RESULT_LIMIT,
                    "ignoring invalid inspector search result limit"
                );
                Self::default()
            }),
            Err(std::env::VarError::NotPresent) => Self::default(),
            Err(error) => {
                tracing::warn!(
                    env = SEARCH_RESULT_LIMIT_ENV,
                    %error,
                    default_limit = DEFAULT_SEARCH_RESULT_LIMIT,
                    "ignoring unreadable inspector search result limit"
                );
                Self::default()
            }
        }
    }

    fn parse_env_value(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "unlimited" | "none" | "off" | "disabled" | "disable" => Some(Self::Unlimited),
            _ => value
                .trim()
                .parse::<usize>()
                .ok()
                .map(|limit| if limit == 0 { Self::Unlimited } else { Self::Limited(limit) }),
        }
    }

    fn into_limit(self) -> Option<usize> {
        match self {
            Self::Limited(limit) => Some(limit),
            Self::Unlimited => None,
        }
    }
}

impl Default for SearchResultLimitChoice {
    fn default() -> Self {
        Self::Limited(DEFAULT_SEARCH_RESULT_LIMIT)
    }
}

impl std::fmt::Display for SearchResultLimitChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limited(limit) => limit.fmt(formatter),
            Self::Unlimited => "unlimited".fmt(formatter),
        }
    }
}

fn parse_search_result_limit(value: &str) -> Result<SearchResultLimitChoice, String> {
    SearchResultLimitChoice::parse_env_value(value)
        .ok_or_else(|| format!("expected a positive integer or 'unlimited', got '{value}'"))
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

#[cfg(target_os = "windows")]
fn system_text_scale_factor() -> Option<f32> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("Software\\Microsoft\\Accessibility").ok()?;
    let percent: u32 = key.get_value("TextScaleFactor").ok()?;
    let clamped = u16::try_from(percent).ok()?.clamp(100, 225);
    Some(f32::from(clamped) / 100.0)
}

#[cfg(not(target_os = "windows"))]
fn system_text_scale_factor() -> Option<f32> {
    None
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
    show_settings_dialog: bool,
    last_pixels_per_point: f32,
    /// Global modifier reader for the live picker (`None` where unsupported).
    modifier_reader: Option<modifiers::ModifierReader>,
    /// Whether live picking is supported here (modifier reader + live cursor
    /// position + hit-test). Computed once at startup; gates the toggle.
    picker_supported: bool,
    /// Toolbar display style (persisted, editable in the Settings dialog).
    toolbar_style: toolbar::ToolbarStyle,
}

#[derive(Clone, Copy, Debug)]
enum AppCommand {
    ShowSettings,
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
    fn apply_system_fonts(ctx: &egui::Context) {
        // Use the OS-resolved UI font as the default egui font family.
        // If detection fails on a platform, egui keeps its own defaults.
        let _ = egui_system_fonts::set_auto(ctx, egui_system_fonts::FontStyle::Sans);

        // Honor Windows accessibility text scaling when available.
        if let Some(scale_factor) = system_text_scale_factor() {
            ctx.set_zoom_factor(scale_factor);
        }
    }

    fn new(
        runtime: Arc<Runtime>,
        initial_root: Arc<UiNodeData>,
        search_result_limit: Option<usize>,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        Self::apply_system_fonts(ctx);
        // Image loaders (incl. the SVG loader) back the toolbar's embedded icons.
        egui_extras::install_image_loaders(ctx);

        // Probe live-picker support once: it needs a global modifier reader and
        // a real, live cursor position. Done before `runtime` moves into the vm.
        let modifier_reader = modifiers::ModifierReader::new();
        let pointer_pos = runtime.pointer_position();
        let picker_supported = modifier_reader.is_some() && pointer_pos.is_ok();
        tracing::info!(
            picker_supported,
            has_modifier_reader = modifier_reader.is_some(),
            pointer_position_ok = pointer_pos.is_ok(),
            "live picker support probe"
        );

        let settings: PersistedSettings =
            cc.storage.and_then(|storage| eframe::get_value(storage, eframe::APP_KEY)).unwrap_or_default();

        let mut vm = InspectorViewModel::new(runtime, initial_root, search_result_limit);
        // An empty stored combination (hand-edited file) is rejected by the vm,
        // which then keeps the default.
        vm.set_picker_combo(settings.picker_combo);

        Self {
            vm,
            attributes_sort: attributes::AttributesSortState::default(),
            attributes_view_mode: attributes::AttributesViewMode::default(),
            attribute_filter: String::new(),
            pinned_attributes: BTreeSet::new(),
            collapsed_attribute_groups: BTreeSet::new(),
            prev_always_on_top: None,
            show_about_dialog: false,
            show_settings_dialog: false,
            last_pixels_per_point: ctx.pixels_per_point(),
            modifier_reader,
            picker_supported,
            toolbar_style: settings.toolbar_style,
        }
    }

    /// Advance the live picker: read global modifiers and let the vm decide.
    /// Repaints continuously while armed so polling continues even when the
    /// Inspector is unfocused (the user is hovering another app).
    fn poll_picker(&mut self, ctx: &egui::Context) {
        if !self.vm.picker_armed() {
            return;
        }
        let modifiers = self.modifier_reader.as_ref().and_then(modifiers::ModifierReader::read);
        self.vm.poll_picker(self.picker_supported, modifiers, ctx);
        ctx.request_repaint();
    }

    fn maybe_refresh_system_fonts(&mut self, ctx: &egui::Context) {
        let pixels_per_point = ctx.pixels_per_point();
        let dpi_changed = (pixels_per_point - self.last_pixels_per_point).abs() > f32::EPSILON;

        if dpi_changed {
            Self::apply_system_fonts(ctx);
            self.last_pixels_per_point = pixels_per_point;
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
            AppCommand::ShowSettings => self.show_settings_dialog = true,
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

    fn poll_background_work(&mut self, ctx: &egui::Context) {
        self.vm.poll_initial_load(ctx);
        self.vm.poll_child_load(ctx);
        self.vm.poll_search(ctx);
        self.vm.poll_reveal(ctx);
        self.vm.poll_selection(ctx);
        self.vm.poll_highlight(ctx);
    }
}

impl eframe::App for InspectorApp {
    /// Persist the Inspector's explicit settings (called by eframe periodically
    /// and on shutdown).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(
            storage,
            eframe::APP_KEY,
            &PersistedSettings { picker_combo: self.vm.picker_combo(), toolbar_style: self.toolbar_style },
        );
    }

    /// Only the explicit [`PersistedSettings`] are stored — not egui's own
    /// memory (open state, scroll positions, …), which would make runs
    /// non-deterministic.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.plugin_or_default::<egui_async::EguiAsyncPlugin>();
        self.maybe_refresh_system_fonts(ctx);

        for command in Self::collect_shortcut_commands(ctx) {
            self.execute_command(ctx, command);
        }

        self.poll_background_work(ctx);
        self.poll_picker(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let is_searching = self.vm.is_searching();
        let has_node_selection = self.vm.selected_index.is_some();

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
                toolbar::MenuAction::ShowSettings => self.execute_command(&ctx, AppCommand::ShowSettings),
                toolbar::MenuAction::ShowAbout => self.execute_command(&ctx, AppCommand::ShowAbout),
            }
        }

        // View: Toolbar (below menu)
        let toolbar_actions = toolbar::show_toolbar(
            ui,
            self.toolbar_style,
            self.picker_supported,
            self.vm.picker_armed(),
            &self.vm.picker_combo_label(),
            self.vm.always_on_top,
            has_node_selection,
        );

        for action in toolbar_actions {
            match action {
                toolbar::MainToolbarAction::SetPickerArmed(armed) => self.vm.set_picker_armed(armed),
                toolbar::MainToolbarAction::RefreshNode => self.execute_command(&ctx, AppCommand::RefreshNode),
                toolbar::MainToolbarAction::RefreshSubtree => self.execute_command(&ctx, AppCommand::RefreshSubtree),
                toolbar::MainToolbarAction::HighlightNode => self.execute_command(&ctx, AppCommand::HighlightNode),
                toolbar::MainToolbarAction::SetAlwaysOnTop(on_top) => self.vm.always_on_top = on_top,
            }
        }

        // View: Search Bar (below toolbar)
        let search_error_hint = self.vm.search_error_hint().map(ToOwned::to_owned);

        let search_actions =
            toolbar::show_search_bar(ui, &mut self.vm.search_text, search_error_hint.as_deref(), is_searching);

        // Apply "Always On Top" setting only when it changes to avoid
        // flooding the window manager with _NET_WM_STATE requests every frame.
        // Must run after toolbar rendering so toggle changes take effect
        // in the same frame.
        if self.prev_always_on_top != Some(self.vm.always_on_top) {
            self.prev_always_on_top = Some(self.vm.always_on_top);
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.vm.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
        }

        // Process search-row actions (must happen before poll so a new
        // search is started before the first poll in the same frame).
        for action in search_actions {
            match action {
                toolbar::ToolbarAction::EvaluateXPath => self.execute_command(&ctx, AppCommand::EvaluateXPath),
                toolbar::ToolbarAction::CancelSearch => self.execute_command(&ctx, AppCommand::CancelSearch),
                toolbar::ToolbarAction::SearchTextChanged => self.vm.on_search_text_changed(),
            }
        }

        let status_text = self.vm.status_bar_text();
        let picker_state_text = self.vm.picker_status_text(self.picker_supported);

        // View: Status Bar (bottom-most)
        status_bar::show_status_bar(
            ui,
            self.vm.has_pending_background_work(),
            status_text.as_deref(),
            &picker_state_text,
        );

        // View: Results Panel (above status bar)
        let result_actions = results_panel::show_results_panel(
            ui,
            &self.vm.results,
            status_text.as_deref(),
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
                            self.vm.expand_row(i);
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
                    self.vm.toggle_row(i);
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

        // The Settings dialog edits the persisted picker combination (the vm
        // rejects an empty set; the dialog locks the last checked modifier)
        // and the toolbar display style.
        if let Some(combo) = view::settings_dialog::show_settings_dialog(
            &ctx,
            &mut self.show_settings_dialog,
            self.vm.picker_combo(),
            &mut self.toolbar_style,
        ) {
            self.vm.set_picker_combo(combo);
        }
    }
}

fn create_initial_root(runtime: &Arc<Runtime>) -> Arc<UiNodeData> {
    Arc::new(UiNodeData::new(runtime.desktop_node()))
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
    let renderer = RendererChoice::resolve(args.renderer);
    let requested_glow_hardware_acceleration = GlowHardwareAccelerationChoice::resolve(args.glow_hardware_acceleration);
    let glow_hardware_acceleration = requested_glow_hardware_acceleration.effective_for_renderer(renderer);
    let search_result_limit = SearchResultLimitChoice::resolve(args.search_result_limit);

    tracing::info!(
        renderer = %renderer,
        glow_hardware_acceleration = %glow_hardware_acceleration,
        requested_glow_hardware_acceleration = %requested_glow_hardware_acceleration,
        search_result_limit = %search_result_limit,
        "starting inspector renderer"
    );

    let runtime = Runtime::new().expect("Failed to create PlatynUI runtime");
    let runtime = Arc::new(runtime);
    let initial_root = create_initial_root(&runtime);

    let icon = load_icon();
    let options = eframe::NativeOptions {
        renderer: renderer.to_eframe(),
        hardware_acceleration: glow_hardware_acceleration.to_eframe(),
        wgpu_options: inspector_wgpu_options(),
        // Storage only carries the explicit [`PersistedSettings`]; window
        // geometry stays at the defaults below on every start.
        persist_window: false,
        // Settings live in the config directory (see [`settings_path`]); `None`
        // (no resolvable home) falls back to eframe's data-dir default.
        persistence_path: settings_path(),
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

            Ok(Box::new(InspectorApp::new(Arc::clone(&runtime), initial_root, search_result_limit.into_limit(), cc)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_settings_round_trip_keeps_toolbar_style() {
        let settings = PersistedSettings {
            picker_combo: PickerModifiers { ctrl: true, alt: false, shift: true },
            toolbar_style: view::toolbar::ToolbarStyle::IconsAndText,
        };
        let ron = ron::to_string(&settings).expect("settings must serialize");
        let loaded: PersistedSettings = ron::from_str(&ron).expect("settings must deserialize");
        assert_eq!(loaded.picker_combo, settings.picker_combo);
        assert_eq!(loaded.toolbar_style, settings.toolbar_style);
    }

    #[test]
    fn settings_file_without_toolbar_style_loads_icons_only_default() {
        // A file written before the display-style setting existed.
        let old = "(picker_combo:(ctrl:true,alt:true,shift:true))";
        let loaded: PersistedSettings = ron::from_str(old).expect("old settings must still load");
        assert_eq!(loaded.picker_combo, PickerModifiers::CTRL_ALT_SHIFT);
        assert_eq!(loaded.toolbar_style, view::toolbar::ToolbarStyle::IconsOnly);
    }
}
