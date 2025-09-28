use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platynui_core::platform::{
    DesktopInfo, HighlightProvider, HighlightRequest, KeyboardDevice, KeyboardError,
    KeyboardOverrides, KeyboardSettings, MonitorInfo, PlatformError, PlatformErrorKind,
    PointerButton, PointerDevice, Screenshot, ScreenshotProvider, ScreenshotRequest, ScrollDelta,
    desktop_info_providers, highlight_providers, keyboard_devices, platform_modules,
    pointer_devices, screenshot_providers,
};
use platynui_core::provider::{
    ProviderError, ProviderErrorKind, ProviderEvent, ProviderEventKind, ProviderEventListener,
    UiTreeProvider,
};
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{
    DESKTOP_RUNTIME_ID, FocusableAction, FocusablePattern, Namespace, PatternError, PatternId,
    RuntimeId, UiAttribute, UiNode, UiValue, supported_patterns_value,
};
use thiserror::Error;

use crate::provider::ProviderRegistry;
use crate::provider::event::{ProviderEventDispatcher, ProviderEventSink};

use crate::keyboard::{KeyboardEngine, KeyboardMode, apply_overrides as apply_keyboard_overrides};
use crate::keyboard_sequence::{KeyboardSequence, KeyboardSequenceError};
use crate::pointer::{ClickStamp, PointerEngine, PointerError};
use crate::pointer::{PointerOverrides, PointerProfile, PointerSettings};
use crate::{EvaluateError, EvaluateOptions, EvaluationItem, evaluate};

/// Central orchestrator that owns provider instances and the provider event dispatcher.
pub struct Runtime {
    registry: ProviderRegistry,
    providers: Vec<Arc<ProviderRuntimeState>>,
    dispatcher: Arc<ProviderEventDispatcher>,
    desktop: Arc<DesktopNode>,
    highlight: Option<&'static dyn HighlightProvider>,
    screenshot: Option<&'static dyn ScreenshotProvider>,
    pointer: Option<&'static dyn PointerDevice>,
    pointer_settings: Mutex<PointerSettings>,
    pointer_profile: Mutex<PointerProfile>,
    pointer_sleep: fn(Duration),
    pointer_click_state: Mutex<Option<ClickStamp>>,
    keyboard: Option<&'static dyn KeyboardDevice>,
    keyboard_settings: Mutex<KeyboardSettings>,
}

struct ProviderRuntimeState {
    provider: Arc<dyn UiTreeProvider>,
    requires_full_refresh: bool,
    snapshot: Mutex<Vec<Arc<dyn UiNode>>>,
    dirty: AtomicBool,
}

impl ProviderRuntimeState {
    fn new(provider: Arc<dyn UiTreeProvider>, requires_full_refresh: bool) -> Self {
        Self {
            provider,
            requires_full_refresh,
            snapshot: Mutex::new(Vec::new()),
            dirty: AtomicBool::new(true),
        }
    }

    fn provider(&self) -> &Arc<dyn UiTreeProvider> {
        &self.provider
    }

    fn mark_dirty(&self) {
        if !self.requires_full_refresh {
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn take_dirty(&self) -> bool {
        if self.requires_full_refresh {
            return true;
        }
        self.dirty.swap(false, Ordering::SeqCst)
    }

    fn reset_dirty(&self) {
        if !self.requires_full_refresh {
            self.dirty.store(false, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, Error)]
pub enum FocusError {
    #[error("node `{runtime_id}` does not expose the Focusable pattern")]
    PatternMissing { runtime_id: String },
    #[error("focus action failed for node `{runtime_id}`: {source}")]
    ActionFailed {
        runtime_id: String,
        #[source]
        source: PatternError,
    },
}

#[derive(Debug, Error)]
pub enum KeyboardActionError {
    #[error("invalid keyboard sequence: {0}")]
    Sequence(Box<KeyboardSequenceError>),
    #[error(transparent)]
    Keyboard(#[from] KeyboardError),
}

impl From<KeyboardSequenceError> for KeyboardActionError {
    fn from(err: KeyboardSequenceError) -> Self {
        KeyboardActionError::Sequence(Box::new(err))
    }
}

struct RuntimeEventListener {
    dispatcher: Arc<ProviderEventDispatcher>,
    state: Arc<ProviderRuntimeState>,
}

impl RuntimeEventListener {
    fn new(dispatcher: Arc<ProviderEventDispatcher>, state: Arc<ProviderRuntimeState>) -> Self {
        Self { dispatcher, state }
    }
}

impl ProviderEventListener for RuntimeEventListener {
    fn on_event(&self, event: ProviderEvent) {
        self.state.mark_dirty();
        if let ProviderEventKind::NodeUpdated { node } = &event.kind {
            node.invalidate();
        }
        self.dispatcher.on_event(event);
    }
}

impl Runtime {
    /// Discovers all registered providers, instantiates them and prepares the event pipeline.
    pub fn new() -> Result<Self, ProviderError> {
        initialize_platform_modules()?;
        let registry = ProviderRegistry::discover();
        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let provider_instances = registry.instantiate_all()?;
        let mut providers = Vec::with_capacity(provider_instances.len());
        for provider in provider_instances {
            let requires_full_refresh = provider.descriptor().event_capabilities().is_empty();
            let state = Arc::new(ProviderRuntimeState::new(provider, requires_full_refresh));
            let listener = Arc::new(RuntimeEventListener::new(dispatcher.clone(), state.clone()));
            state.provider().subscribe_events(listener)?;
            providers.push(state);
        }

        let desktop = build_desktop_node().map_err(map_desktop_error)?;

        let highlight = highlight_providers().next();
        let screenshot = screenshot_providers().next();
        let pointer = pointer_devices().next();
        let keyboard = keyboard_devices().next();

        let mut pointer_settings = PointerSettings::default();
        if let Some(device) = pointer {
            if let Ok(Some(time)) = device.double_click_time() {
                pointer_settings.double_click_time = time;
            }
            if let Ok(Some(size)) = device.double_click_size() {
                pointer_settings.double_click_size = size;
            }
        }
        let pointer_profile = PointerProfile::named_default();
        let keyboard_settings = KeyboardSettings::default();

        let runtime = Self {
            registry,
            providers,
            dispatcher,
            desktop,
            highlight,
            screenshot,
            pointer,
            pointer_settings: Mutex::new(pointer_settings),
            pointer_profile: Mutex::new(pointer_profile),
            pointer_sleep: default_pointer_sleep,
            pointer_click_state: Mutex::new(None),
            keyboard,
            keyboard_settings: Mutex::new(keyboard_settings),
        };
        runtime.refresh_desktop_nodes(true)?;

        Ok(runtime)
    }

    /// Returns a reference to the provider registry (discovered entries including metadata).
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Returns the instantiated providers in priority order.
    pub fn providers(&self) -> impl Iterator<Item = &Arc<dyn UiTreeProvider>> {
        self.providers.iter().map(|state| state.provider())
    }

    /// Returns providers registered for the given technology identifier.
    pub fn providers_for<'a>(
        &'a self,
        technology: &'a TechnologyId,
    ) -> impl Iterator<Item = &'a Arc<dyn UiTreeProvider>> + 'a {
        self.providers
            .iter()
            .filter(move |state| state.provider().descriptor().technology == *technology)
            .map(|state| state.provider())
    }

    /// Access to the shared provider event dispatcher.
    pub fn event_dispatcher(&self) -> Arc<ProviderEventDispatcher> {
        Arc::clone(&self.dispatcher)
    }

    /// Convenience helper that preconfigures `EvaluateOptions` with the runtime
    /// desktop node so callers do not have to wire it manually.
    pub fn evaluate_options(&self) -> EvaluateOptions {
        EvaluateOptions::new(self.desktop_node())
    }

    pub fn evaluate(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<Vec<EvaluationItem>, EvaluateError> {
        self.refresh_desktop_nodes(false).map_err(EvaluateError::from)?;
        evaluate(node, xpath, self.evaluate_options())
    }

    pub fn focus(&self, node: &Arc<dyn UiNode>) -> Result<(), FocusError> {
        let runtime_id = node.runtime_id().as_str().to_owned();
        let pattern = match node.pattern::<FocusableAction>() {
            Some(pattern) => pattern,
            None => return Err(FocusError::PatternMissing { runtime_id }),
        };

        if let Err(source) = pattern.focus() {
            return Err(FocusError::ActionFailed { runtime_id, source });
        }

        Ok(())
    }

    pub fn desktop_node(&self) -> Arc<dyn UiNode> {
        self.desktop.as_ui_node()
    }

    pub fn desktop_info(&self) -> &DesktopInfo {
        self.desktop.info()
    }

    /// Highlights the given regions using the registered highlight provider.
    pub fn highlight(&self, requests: &[HighlightRequest]) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => provider.highlight(requests),
            None => Err(PlatformError::new(
                PlatformErrorKind::UnsupportedPlatform,
                "no HighlightProvider registered",
            )),
        }
    }

    /// Clears an active highlight overlay if a provider is available.
    pub fn clear_highlight(&self) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => provider.clear(),
            None => Err(PlatformError::new(
                PlatformErrorKind::UnsupportedPlatform,
                "no HighlightProvider registered",
            )),
        }
    }

    /// Captures a screenshot using the registered screenshot provider.
    pub fn screenshot(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        match self.screenshot {
            Some(provider) => provider.capture(request),
            None => Err(PlatformError::new(
                PlatformErrorKind::UnsupportedPlatform,
                "no ScreenshotProvider registered",
            )),
        }
    }

    pub fn pointer_settings(&self) -> PointerSettings {
        self.pointer_settings.lock().unwrap().clone()
    }

    pub fn set_pointer_settings(&self, settings: PointerSettings) {
        *self.pointer_settings.lock().unwrap() = settings;
    }

    pub fn pointer_profile(&self) -> PointerProfile {
        self.pointer_profile.lock().unwrap().clone()
    }

    pub fn set_pointer_profile(&self, profile: PointerProfile) {
        *self.pointer_profile.lock().unwrap() = profile;
    }

    pub fn pointer_position(&self) -> Result<Point, PointerError> {
        let device = self.pointer_device()?;
        Ok(device.position()?)
    }

    pub fn keyboard_settings(&self) -> KeyboardSettings {
        self.keyboard_settings.lock().unwrap().clone()
    }

    pub fn set_keyboard_settings(&self, settings: KeyboardSettings) {
        *self.keyboard_settings.lock().unwrap() = settings;
    }

    pub fn pointer_move_to(
        &self,
        point: Point,
        overrides: Option<PointerOverrides>,
    ) -> Result<Point, PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        engine.move_to(point)
    }

    pub fn pointer_click(
        &self,
        point: Point,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        let mut last_click = self.pointer_click_state.lock().unwrap();
        engine.click(point, button, &mut *last_click)
    }

    pub fn pointer_multi_click(
        &self,
        point: Point,
        button: Option<PointerButton>,
        clicks: u32,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        let mut last_click = self.pointer_click_state.lock().unwrap();
        engine.multi_click(point, button, clicks, &mut *last_click)
    }

    pub fn pointer_press(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        let button = button.unwrap_or_else(|| engine.default_button());
        if let Some(point) = target {
            engine.move_to(point)?;
        }
        engine.press(button)
    }

    pub fn pointer_release(
        &self,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        let button = button.unwrap_or_else(|| engine.default_button());
        engine.release(button)
    }

    pub fn pointer_scroll(
        &self,
        delta: ScrollDelta,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        engine.scroll(delta)
    }

    pub fn pointer_drag(
        &self,
        start: Point,
        end: Point,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let engine = self.build_pointer_engine(overrides)?;
        engine.drag(start, end, button)
    }

    pub fn keyboard_press(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device)?;
        let overrides = overrides.unwrap_or_default();
        let settings = apply_keyboard_overrides(&self.keyboard_settings(), &overrides);
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?
            .execute(&resolved, KeyboardMode::Press)?;
        Ok(())
    }

    pub fn keyboard_release(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device)?;
        let overrides = overrides.unwrap_or_default();
        let settings = apply_keyboard_overrides(&self.keyboard_settings(), &overrides);
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?
            .execute(&resolved, KeyboardMode::Release)?;
        Ok(())
    }

    pub fn keyboard_type(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device)?;
        let overrides = overrides.unwrap_or_default();
        let settings = apply_keyboard_overrides(&self.keyboard_settings(), &overrides);
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?
            .execute(&resolved, KeyboardMode::Type)?;
        Ok(())
    }

    /// Registers a new event sink that will receive provider events.
    pub fn register_event_sink(&self, sink: Arc<dyn ProviderEventSink>) {
        self.dispatcher.register(sink);
    }

    /// Utility mainly for tests to inject provider events.
    pub fn dispatch_event(&self, event: ProviderEvent) {
        self.dispatcher.dispatch(event);
    }

    /// Invokes shutdown on dispatcher and providers.
    pub fn shutdown(&mut self) {
        self.dispatcher.shutdown();
        for state in &self.providers {
            state.provider().shutdown();
        }
    }

    fn refresh_desktop_nodes(&self, force_all: bool) -> Result<(), ProviderError> {
        for state in &self.providers {
            if !force_all && !state.take_dirty() {
                continue;
            }
            let parent = self.desktop.as_ui_node();
            let nodes = state.provider().get_nodes(parent)?.collect::<Vec<_>>();
            let mut snapshot = state.snapshot.lock().unwrap();
            *snapshot = nodes;
            state.reset_dirty();
        }

        let mut aggregated: Vec<Arc<dyn UiNode>> = Vec::new();
        for state in &self.providers {
            let snapshot = state.snapshot.lock().unwrap();
            aggregated.extend(snapshot.iter().cloned());
        }
        self.desktop.replace_children(aggregated);
        Ok(())
    }

    fn build_pointer_engine(
        &self,
        overrides: Option<PointerOverrides>,
    ) -> Result<PointerEngine<'_>, PointerError> {
        let device = self.pointer_device()?;
        let settings = self.pointer_settings.lock().unwrap().clone();
        let profile = self.pointer_profile.lock().unwrap().clone();
        let overrides = overrides.unwrap_or_default();
        let sleep_fn: &dyn Fn(Duration) = &self.pointer_sleep;
        Ok(PointerEngine::new(
            device,
            self.desktop.info().bounds,
            settings,
            profile,
            overrides,
            sleep_fn,
        ))
    }

    fn pointer_device(&self) -> Result<&'static dyn PointerDevice, PointerError> {
        self.pointer.ok_or(PointerError::MissingDevice)
    }

    fn keyboard_device(&self) -> Result<&'static dyn KeyboardDevice, KeyboardError> {
        self.keyboard.ok_or(KeyboardError::NotReady)
    }
}

fn build_desktop_node() -> Result<Arc<DesktopNode>, PlatformError> {
    let mut providers = desktop_info_providers();
    let info = if let Some(provider) = providers.next() {
        provider.desktop_info()?
    } else {
        fallback_desktop_info()
    };
    Ok(DesktopNode::new(info))
}

fn map_desktop_error(err: PlatformError) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::InitializationFailed,
        format!("desktop initialization failed: {err}"),
    )
}

fn initialize_platform_modules() -> Result<(), ProviderError> {
    for module in platform_modules() {
        module.initialize().map_err(|err| {
            ProviderError::new(
                ProviderErrorKind::InitializationFailed,
                format!("platform module `{}` failed to initialize: {err}", module.name()),
            )
        })?;
    }
    Ok(())
}

fn fallback_desktop_info() -> DesktopInfo {
    let os_name = std::env::consts::OS;
    let os_version = std::env::consts::ARCH;
    DesktopInfo {
        runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID),
        name: format!("Fallback Desktop ({os_name})"),
        technology: TechnologyId::from("Fallback"),
        bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
        os_name: os_name.into(),
        os_version: os_version.into(),
        monitors: Vec::new(),
    }
}

fn default_pointer_sleep(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    std::thread::sleep(duration);
}

fn default_keyboard_sleep(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    std::thread::sleep(duration);
}

struct DesktopNode {
    info: DesktopInfo,
    attributes: Vec<Arc<dyn UiAttribute>>,
    supported: Vec<PatternId>,
    children: Mutex<Vec<Arc<dyn UiNode>>>,
}

impl DesktopNode {
    fn new(info: DesktopInfo) -> Arc<Self> {
        let mut info = info;
        info.runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);
        let namespace = Namespace::Control;
        let mut attributes: Vec<Arc<dyn UiAttribute>> = Vec::new();
        let supported = vec![PatternId::from("Desktop")];

        attributes.push(attr(namespace, attribute_names::common::ROLE, UiValue::from("Desktop")));
        attributes.push(attr(
            namespace,
            attribute_names::common::NAME,
            UiValue::from(info.name.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::RUNTIME_ID,
            UiValue::from(info.runtime_id.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::TECHNOLOGY,
            UiValue::from(info.technology.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::SUPPORTED_PATTERNS,
            supported_patterns_value(&supported),
        ));

        attributes.push(attr(
            namespace,
            attribute_names::element::BOUNDS,
            UiValue::from(info.bounds),
        ));
        attributes.extend(rect_alias_attributes(namespace, "Bounds", &info.bounds));
        attributes.push(attr(namespace, attribute_names::element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_ENABLED, UiValue::from(true)));
        attributes.push(attr(
            namespace,
            attribute_names::element::IS_OFFSCREEN,
            UiValue::from(false),
        ));

        attributes.push(attr(
            namespace,
            attribute_names::desktop::DISPLAY_COUNT,
            UiValue::from(info.display_count() as i64),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::OS_NAME,
            UiValue::from(info.os_name.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::OS_VERSION,
            UiValue::from(info.os_version.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::MONITORS,
            UiValue::Array(info.monitors.iter().map(monitor_to_value).collect()),
        ));

        Arc::new(Self { info, attributes, supported, children: Mutex::new(Vec::new()) })
    }

    fn info(&self) -> &DesktopInfo {
        &self.info
    }

    fn as_ui_node(self: &Arc<Self>) -> Arc<dyn UiNode> {
        Arc::clone(self) as Arc<dyn UiNode>
    }

    fn replace_children(&self, nodes: Vec<Arc<dyn UiNode>>) {
        *self.children.lock().unwrap() = nodes;
    }
}

impl UiNode for DesktopNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn role(&self) -> &str {
        "Desktop"
    }

    fn name(&self) -> &str {
        &self.info.name
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.info.runtime_id
    }

    fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        let snapshot = self.children.lock().unwrap().clone();
        Box::new(snapshot.into_iter())
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(self.attributes.clone().into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        self.supported.clone()
    }

    fn invalidate(&self) {}
}

fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(DesktopAttribute { namespace, name: name.into(), value })
}

fn rect_alias_attributes(
    namespace: Namespace,
    base: &str,
    rect: &Rect,
) -> Vec<Arc<dyn UiAttribute>> {
    vec![
        attr(namespace, format!("{base}.X"), UiValue::from(rect.x())),
        attr(namespace, format!("{base}.Y"), UiValue::from(rect.y())),
        attr(namespace, format!("{base}.Width"), UiValue::from(rect.width())),
        attr(namespace, format!("{base}.Height"), UiValue::from(rect.height())),
    ]
}

fn monitor_to_value(info: &MonitorInfo) -> UiValue {
    let mut map = BTreeMap::new();
    map.insert("Id".to_string(), UiValue::from(info.id.clone()));
    if let Some(name) = &info.name {
        map.insert("Name".to_string(), UiValue::from(name.clone()));
    }
    map.insert("Bounds".to_string(), UiValue::from(info.bounds));
    map.insert("IsPrimary".to_string(), UiValue::from(info.is_primary));
    if let Some(scale) = info.scale_factor {
        map.insert("ScaleFactor".to_string(), UiValue::from(scale));
    }
    UiValue::Object(map)
}

struct DesktopAttribute {
    namespace: Namespace,
    name: String,
    value: UiValue,
}

impl UiAttribute for DesktopAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EvaluationItem;
    use crate::PointerOverrides;
    use platynui_core::platform::{
        HighlightRequest, KeyboardOverrides, PointerButton, ScreenshotRequest, ScrollDelta,
    };
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory, register_provider,
    };
    use platynui_core::types::{Point, Rect};
    use platynui_core::ui::attribute_names::focusable;
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
    use platynui_platform_mock as _;
    use platynui_platform_mock::{
        KeyboardLogEntry, PointerLogEntry, reset_highlight_state, reset_keyboard_state,
        reset_pointer_state, reset_screenshot_state, take_highlight_log, take_keyboard_log,
        take_pointer_log, take_screenshot_log,
    };
    use platynui_provider_mock as _;
    use rstest::rstest;
    use serial_test::serial;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, Weak};
    use std::time::Duration;

    fn configure_keyboard_for_tests(runtime: &Runtime) {
        let mut settings = runtime.keyboard_settings();
        settings.press_delay = Duration::ZERO;
        settings.release_delay = Duration::ZERO;
        settings.between_keys_delay = Duration::ZERO;
        settings.chord_press_delay = Duration::ZERO;
        settings.chord_release_delay = Duration::ZERO;
        settings.after_sequence_delay = Duration::ZERO;
        settings.after_text_delay = Duration::ZERO;
        runtime.set_keyboard_settings(settings);
    }

    fn zero_keyboard_overrides() -> KeyboardOverrides {
        KeyboardOverrides::new()
            .press_delay(Duration::ZERO)
            .release_delay(Duration::ZERO)
            .between_keys_delay(Duration::ZERO)
            .chord_press_delay(Duration::ZERO)
            .chord_release_delay(Duration::ZERO)
            .after_sequence_delay(Duration::ZERO)
            .after_text_delay(Duration::ZERO)
    }

    struct StubAttribute;
    impl UiAttribute for StubAttribute {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn name(&self) -> &str {
            "Role"
        }
        fn value(&self) -> UiValue {
            UiValue::from("Stub")
        }
    }

    struct StubNode {
        runtime_id: RuntimeId,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
    }

    impl StubNode {
        fn new(id: &str) -> Self {
            Self { runtime_id: RuntimeId::from(id), parent: Mutex::new(None) }
        }

        fn set_parent(&self, parent: &Arc<dyn UiNode>) {
            *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
        }
    }

    impl UiNode for StubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Button"
        }
        fn name(&self) -> &str {
            "Stub"
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(vec![Arc::new(StubAttribute) as Arc<dyn UiAttribute>].into_iter())
        }
        fn supported_patterns(&self) -> Vec<PatternId> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    static SHUTDOWN_TRIGGERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
    static SUBSCRIPTION_REGISTERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

    fn configure_pointer_for_tests(runtime: &Runtime) {
        let settings = runtime.pointer_settings();
        runtime.set_pointer_settings(settings);

        let mut profile = runtime.pointer_profile();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.before_next_click_delay = Duration::ZERO;
        profile.multi_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.ensure_move_threshold = 1.0;
        profile.ensure_move_timeout = Duration::from_millis(10);
        profile.scroll_delay = Duration::ZERO;
        profile.acceleration_profile =
            platynui_core::platform::PointerAccelerationProfile::Constant;
        runtime.set_pointer_profile(profile);
    }

    fn zero_overrides() -> PointerOverrides {
        PointerOverrides::new()
            .after_move_delay(Duration::ZERO)
            .after_input_delay(Duration::ZERO)
            .press_release_delay(Duration::ZERO)
            .after_click_delay(Duration::ZERO)
            .scroll_delay(Duration::ZERO)
    }

    struct StubProvider {
        descriptor: &'static ProviderDescriptor,
        node: Arc<StubNode>,
    }

    impl StubProvider {
        fn new(descriptor: &'static ProviderDescriptor) -> Self {
            Self { descriptor, node: Arc::new(StubNode::new(descriptor.id)) }
        }
    }

    impl UiTreeProvider for StubProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            self.descriptor
        }
        fn get_nodes(
            &self,
            parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            self.node.set_parent(&parent);
            Ok(Box::new(std::iter::once(self.node.clone() as Arc<dyn UiNode>)))
        }
        fn subscribe_events(
            &self,
            listener: Arc<dyn ProviderEventListener>,
        ) -> Result<(), ProviderError> {
            listener.on_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });
            SUBSCRIPTION_REGISTERED.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn shutdown(&self) {
            SHUTDOWN_TRIGGERED.store(true, Ordering::SeqCst);
        }
    }

    struct StubFactory;

    impl StubFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-stub",
                    "Runtime Stub",
                    TechnologyId::from("RuntimeTech"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }

    impl UiTreeProviderFactory for StubFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(StubProvider::new(Self::descriptor_static())))
        }
    }

    static RUNTIME_FACTORY: StubFactory = StubFactory;

    register_provider!(&RUNTIME_FACTORY);

    struct RecordingSink {
        events: Mutex<Vec<ProviderEventKind>>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self { events: Mutex::new(Vec::new()) }
        }
    }

    impl ProviderEventSink for RecordingSink {
        fn dispatch(&self, event: ProviderEvent) {
            self.events.lock().unwrap().push(event.kind);
        }
    }

    #[rstest]
    fn runtime_initializes_providers() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        SUBSCRIPTION_REGISTERED.store(false, Ordering::SeqCst);

        let runtime = Runtime::new().expect("runtime initializes");
        let providers: Vec<_> = runtime.providers().collect();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|provider| provider.descriptor().id == "runtime-stub"));
        assert!(SUBSCRIPTION_REGISTERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn runtime_dispatcher_forwards_events() {
        let runtime = Runtime::new().expect("runtime initializes");
        let sink = Arc::new(RecordingSink::new());
        runtime.register_event_sink(sink.clone());

        runtime.dispatch_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });

        let events = sink.events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(matches!(events.last().unwrap(), ProviderEventKind::TreeInvalidated));
    }

    #[rstest]
    fn runtime_filters_providers_by_technology() {
        let runtime = Runtime::new().expect("runtime initializes");
        let tech = TechnologyId::from("RuntimeTech");
        let providers: Vec<_> = runtime.providers_for(&tech).collect();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].descriptor().id, "runtime-stub");
    }

    #[rstest]
    fn runtime_shutdown_invokes_provider_shutdown() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        let mut runtime = Runtime::new().expect("runtime initializes");
        runtime.shutdown();
        assert!(SHUTDOWN_TRIGGERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn runtime_evaluate_executes_xpath() {
        let runtime = Runtime::new().expect("runtime initializes");
        let results = runtime.evaluate(None, "//control:Button").expect("evaluation");
        assert!(!results.is_empty());
    }

    #[rstest]
    fn provider_nodes_link_parent() {
        let runtime = Runtime::new().expect("runtime initializes");
        let parent: Arc<dyn UiNode> = Arc::new(StubNode::new("parent"));
        let node = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| {
                provider.get_nodes(Arc::clone(&parent)).ok().and_then(|mut nodes| nodes.next())
            })
            .expect("runtime stub provider node available");
        assert!(node.parent().is_some());
    }

    #[rstest]
    fn mock_provider_attaches_to_desktop() {
        let runtime = Runtime::new().expect("runtime initializes");
        let desktop = runtime.desktop_node();
        let app = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "mock")
            .and_then(|provider| provider.get_nodes(Arc::clone(&desktop)).ok())
            .and_then(|mut nodes| nodes.next())
            .expect("mock provider root node");

        assert_eq!(app.namespace(), Namespace::App);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), runtime.desktop_info().runtime_id.as_str());
    }

    #[rstest]
    fn runtime_focus_sets_focus_state() {
        let mut runtime = Runtime::new().expect("runtime initializes");
        let results =
            runtime.evaluate(None, "//control:Button[@Name='OK']").expect("button evaluation");

        let button = results
            .into_iter()
            .find_map(|item| match item {
                EvaluationItem::Node(node) => Some(node),
                _ => None,
            })
            .expect("button node available");

        runtime.focus(&button).expect("focus succeeds");

        let value = button
            .attribute(Namespace::Control, focusable::IS_FOCUSED)
            .expect("focus attribute present")
            .value();
        assert_eq!(value, UiValue::from(true));

        runtime.shutdown();
    }

    #[rstest]
    fn runtime_focus_requires_focusable_pattern() {
        let mut runtime = Runtime::new().expect("runtime initializes");
        let results =
            runtime.evaluate(None, "//control:Panel[@Name='Workspace']").expect("panel evaluation");

        let panel = results
            .into_iter()
            .find_map(|item| match item {
                EvaluationItem::Node(node) => Some(node),
                _ => None,
            })
            .expect("panel node available");

        let err = runtime.focus(&panel).expect_err("panel should not support focus");
        assert!(matches!(err, FocusError::PatternMissing { .. }));

        runtime.shutdown();
    }

    #[rstest]
    fn highlight_invokes_registered_provider() {
        reset_highlight_state();
        let runtime = Runtime::new().expect("runtime initializes");
        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 50.0, 25.0));
        runtime.highlight(std::slice::from_ref(&request)).expect("highlight succeeds");

        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].len(), 1);
        assert_eq!(log[0][0], request);
    }

    #[rstest]
    fn screenshot_invokes_registered_provider() {
        reset_screenshot_state();
        let runtime = Runtime::new().expect("runtime initializes");
        let request = ScreenshotRequest::with_region(Rect::new(0.0, 0.0, 20.0, 10.0));
        let screenshot = runtime.screenshot(&request).expect("screenshot captures");

        assert_eq!(screenshot.width, 20);
        assert_eq!(screenshot.height, 10);
        assert_eq!(take_screenshot_log().len(), 1);
    }

    #[rstest]
    #[serial]
    fn keyboard_press_logs_events() {
        reset_keyboard_state();
        let mut runtime = Runtime::new().expect("runtime initializes");
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_press("<Ctrl+Alt+T>", Some(overrides.clone())).expect("press succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Press("Control".into()),
                KeyboardLogEntry::Press("Alt".into()),
                KeyboardLogEntry::Press("T".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime
            .keyboard_release("<Ctrl+Alt+T>", Some(overrides))
            .expect("cleanup release succeeds");
        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn keyboard_release_logs_events() {
        reset_keyboard_state();
        let mut runtime = Runtime::new().expect("runtime initializes");
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_press("<Ctrl+Alt+T>", Some(overrides.clone())).expect("press succeeds");
        reset_keyboard_state();

        runtime
            .keyboard_release("<Ctrl+Alt+T>", Some(overrides.clone()))
            .expect("release succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Release("T".into()),
                KeyboardLogEntry::Release("Alt".into()),
                KeyboardLogEntry::Release("Control".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn keyboard_type_emits_press_and_release() {
        reset_keyboard_state();
        let mut runtime = Runtime::new().expect("runtime initializes");
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_type("Ab", Some(overrides)).expect("type succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Press("A".into()),
                KeyboardLogEntry::Release("A".into()),
                KeyboardLogEntry::Press("b".into()),
                KeyboardLogEntry::Release("b".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn pointer_move_uses_device_log() {
        reset_pointer_state();
        let runtime = Runtime::new().expect("runtime initializes");
        configure_pointer_for_tests(&runtime);

        runtime
            .pointer_move_to(Point::new(50.0, 25.0), Some(zero_overrides()))
            .expect("move succeeds");

        let log = take_pointer_log();
        assert!(log.iter().any(
            |event| matches!(event, PointerLogEntry::Move(p) if *p == Point::new(50.0, 25.0))
        ));
    }

    #[rstest]
    #[serial]
    fn pointer_click_emits_press_and_release() {
        reset_pointer_state();
        let runtime = Runtime::new().expect("runtime initializes");
        configure_pointer_for_tests(&runtime);

        runtime
            .pointer_click(Point::new(10.0, 10.0), None, Some(zero_overrides()))
            .expect("click succeeds");

        let log = take_pointer_log();
        assert!(
            log.iter().any(|event| matches!(event, PointerLogEntry::Press(PointerButton::Left)))
        );
        assert!(
            log.iter().any(|event| matches!(event, PointerLogEntry::Release(PointerButton::Left)))
        );
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_emits_multiple_events() {
        reset_pointer_state();
        let runtime = Runtime::new().expect("runtime initializes");
        configure_pointer_for_tests(&runtime);

        runtime
            .pointer_multi_click(
                Point::new(20.0, 20.0),
                Some(PointerButton::Right),
                3,
                Some(zero_overrides()),
            )
            .expect("multi-click succeeds");

        let log = take_pointer_log();
        let presses = log
            .iter()
            .filter(|event| matches!(event, PointerLogEntry::Press(PointerButton::Right)))
            .count();
        let releases = log
            .iter()
            .filter(|event| matches!(event, PointerLogEntry::Release(PointerButton::Right)))
            .count();
        assert_eq!(presses, 3);
        assert_eq!(releases, 3);
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_rejects_zero() {
        reset_pointer_state();
        let runtime = Runtime::new().expect("runtime initializes");
        configure_pointer_for_tests(&runtime);

        let error = runtime
            .pointer_multi_click(Point::new(5.0, 5.0), None, 0, Some(zero_overrides()))
            .unwrap_err();
        match error {
            PointerError::InvalidClickCount { provided } => assert_eq!(provided, 0),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[rstest]
    #[serial]
    fn pointer_scroll_chunks_delta() {
        reset_pointer_state();
        let runtime = Runtime::new().expect("runtime initializes");
        configure_pointer_for_tests(&runtime);

        let overrides = zero_overrides().scroll_step(ScrollDelta::new(0.0, -10.0));
        runtime
            .pointer_scroll(ScrollDelta::new(0.0, -25.0), Some(overrides))
            .expect("scroll succeeds");

        let scrolls: Vec<_> = take_pointer_log()
            .into_iter()
            .filter_map(|event| match event {
                PointerLogEntry::Scroll(delta) => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(scrolls.len(), 3);
        let total: f64 = scrolls.iter().map(|delta| delta.vertical).sum();
        assert!((total + 25.0).abs() < f64::EPSILON);
    }
}
