use once_cell::sync::OnceCell;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use platynui_core::platform::{
    DesktopInfo, DesktopInfoProvider, HighlightProvider, HighlightRequest, KeyboardDevice, KeyboardError,
    KeyboardOverrides, KeyboardSettings, MonitorInfo, PlatformError, PlatformErrorKind, PointerButton, PointerDevice,
    Screenshot, ScreenshotProvider, ScreenshotRequest, ScrollDelta, desktop_info_providers, highlight_providers,
    keyboard_devices, platform_modules, pointer_devices, screenshot_providers,
};
use platynui_core::provider::{
    ProviderError, ProviderErrorKind, ProviderEvent, ProviderEventKind, ProviderEventListener, UiTreeProvider,
    UiTreeProviderFactory,
};
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{
    DESKTOP_RUNTIME_ID, FocusableAction, FocusablePattern, Namespace, PatternError, PatternId, RuntimeId, UiAttribute,
    UiNode, UiValue, supported_patterns_value,
};
use thiserror::Error;

use crate::provider::ProviderRegistry;
use crate::provider::event::{ProviderEventDispatcher, ProviderEventSink};

use crate::keyboard::{KeyboardEngine, KeyboardMode, apply_overrides as apply_keyboard_overrides};
use crate::keyboard_sequence::{KeyboardSequence, KeyboardSequenceError};
use crate::pointer::{PointerEngine, PointerError};
use crate::pointer::{PointerOverrides, PointerProfile, PointerSettings};
use crate::{EvaluateError, EvaluateOptions, EvaluationItem, evaluate};

/// Central orchestrator that owns provider instances and the provider event dispatcher.
pub struct Runtime {
    registry: ProviderRegistry,
    providers: Vec<Arc<dyn UiTreeProvider>>,
    dispatcher: Arc<ProviderEventDispatcher>,
    desktop: Arc<DesktopNode>,
    highlight: Option<&'static dyn HighlightProvider>,
    screenshot: Option<&'static dyn ScreenshotProvider>,
    pointer: Option<&'static dyn PointerDevice>,
    pointer_engine: Mutex<Option<PointerEngine<'static>>>,
    pointer_settings: Mutex<PointerSettings>,
    pointer_profile: Mutex<PointerProfile>,
    keyboard: Option<&'static dyn KeyboardDevice>,
    keyboard_settings: Mutex<KeyboardSettings>,
    is_shutdown: AtomicBool,
}

// ProviderRuntimeState removed: DesktopNode streams children on-demand.

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
    // no state tracking required; events are forwarded directly
}

impl RuntimeEventListener {
    fn new(dispatcher: Arc<ProviderEventDispatcher>) -> Self {
        Self { dispatcher }
    }
}

impl ProviderEventListener for RuntimeEventListener {
    fn on_event(&self, event: ProviderEvent) {
        if let ProviderEventKind::NodeUpdated { node } = &event.kind {
            node.invalidate();
        }
        self.dispatcher.on_event(event);
    }
}

pub struct PlatformOverrides {
    pub desktop_info: Option<&'static dyn DesktopInfoProvider>,
    pub highlight: Option<&'static dyn HighlightProvider>,
    pub screenshot: Option<&'static dyn ScreenshotProvider>,
    pub pointer: Option<&'static dyn PointerDevice>,
    pub keyboard: Option<&'static dyn KeyboardDevice>,
}

impl Runtime {
    /// Discovers all registered providers, instantiates them and prepares the event pipeline.
    pub fn new() -> Result<Self, ProviderError> {
        initialize_platform_modules()?;
        let registry = ProviderRegistry::discover();
        Self::from_registry_with_platforms(registry, None)
    }

    /// Builds a Runtime that only includes providers with the given `ids`.
    /// This is useful for tests to restrict the active providers deterministically.
    pub fn new_with_provider_ids(ids: &[&str]) -> Result<Self, ProviderError> {
        initialize_platform_modules()?;
        let registry = ProviderRegistry::discover().filter_by_ids(ids);
        Self::from_registry_with_platforms(registry, None)
    }

    /// Builds a Runtime from an explicit list of provider factories.
    /// No inventory discovery is performed.
    pub fn new_with_factories(factories: &[&'static dyn UiTreeProviderFactory]) -> Result<Self, ProviderError> {
        initialize_platform_modules()?;
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_platforms(registry, None)
    }

    /// Builds a Runtime from factories plus explicit platform provider overrides.
    pub fn new_with_factories_and_platforms(
        factories: &[&'static dyn UiTreeProviderFactory],
        platforms: PlatformOverrides,
    ) -> Result<Self, ProviderError> {
        initialize_platform_modules()?;
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_platforms(registry, Some(platforms))
    }

    fn from_registry_with_platforms(
        registry: ProviderRegistry,
        platforms: Option<PlatformOverrides>,
    ) -> Result<Self, ProviderError> {
        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let provider_instances = registry.instantiate_all()?;
        let mut providers: Vec<Arc<dyn UiTreeProvider>> = Vec::with_capacity(provider_instances.len());
        for provider in provider_instances {
            let listener = Arc::new(RuntimeEventListener::new(dispatcher.clone()));
            provider.subscribe_events(listener)?;
            providers.push(provider);
        }

        // Build desktop info first
        let desktop = if let Some(p) = &platforms {
            if let Some(provider) = p.desktop_info {
                provider.desktop_info().map_err(map_desktop_error)?
            } else {
                build_desktop_info().map_err(map_desktop_error)?
            }
        } else {
            build_desktop_info().map_err(map_desktop_error)?
        };

        let (highlight, screenshot, pointer, keyboard) = if let Some(p) = platforms {
            (
                p.highlight.or_else(|| highlight_providers().next()),
                p.screenshot.or_else(|| screenshot_providers().next()),
                p.pointer.or_else(|| pointer_devices().next()),
                p.keyboard.or_else(|| keyboard_devices().next()),
            )
        } else {
            (
                highlight_providers().next(),
                screenshot_providers().next(),
                pointer_devices().next(),
                keyboard_devices().next(),
            )
        };

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
        let pointer_engine = pointer.map(|device| {
            PointerEngine::new(
                device,
                desktop.bounds,
                pointer_settings.clone(),
                pointer_profile.clone(),
                &default_pointer_sleep,
            )
        });

        let providers_for_desktop: Vec<Arc<dyn UiTreeProvider>> = providers.to_vec();

        let runtime = Self {
            registry,
            providers,
            dispatcher,
            desktop: {
                let node = DesktopNode::new(desktop, providers_for_desktop);
                DesktopNode::init_self(&node);
                node
            },
            highlight,
            screenshot,
            pointer,
            pointer_engine: Mutex::new(pointer_engine),
            pointer_settings: Mutex::new(pointer_settings),
            pointer_profile: Mutex::new(pointer_profile),
            keyboard,
            keyboard_settings: Mutex::new(keyboard_settings),
            is_shutdown: AtomicBool::new(false),
        };
        Ok(runtime)
    }

    /// Returns a reference to the provider registry (discovered entries including metadata).
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Returns the instantiated providers in priority order.
    pub fn providers(&self) -> impl Iterator<Item = &Arc<dyn UiTreeProvider>> {
        self.providers.iter()
    }

    /// Returns providers registered for the given technology identifier.
    pub fn providers_for<'a>(
        &'a self,
        technology: &'a TechnologyId,
    ) -> impl Iterator<Item = &'a Arc<dyn UiTreeProvider>> + 'a {
        self.providers.iter().filter(move |p| p.descriptor().technology == *technology)
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

    pub fn evaluate(&self, node: Option<Arc<dyn UiNode>>, xpath: &str) -> Result<Vec<EvaluationItem>, EvaluateError> {
        evaluate(node, xpath, self.evaluate_options())
    }

    pub fn evaluate_iter(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<impl Iterator<Item = crate::xpath::EvaluationItem>, EvaluateError> {
        crate::xpath::evaluate_iter(node, xpath, self.evaluate_options())
    }

    /// Evaluate an XPath and return the first resulting item, if any.
    /// The stream is not fully consumed and no uniqueness is enforced.
    pub fn evaluate_single(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<Option<EvaluationItem>, EvaluateError> {
        let mut iter = self.evaluate_iter(node, xpath)?;
        Ok(iter.next())
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
    /// If any request specifies a duration, the runtime schedules a clear() call
    /// after the minimum duration as a portability fallback.
    pub fn highlight(&self, requests: &[HighlightRequest]) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => {
                provider.highlight(requests)?;
                Ok(())
            }
            None => Err(PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "no HighlightProvider registered")),
        }
    }

    /// Clears an active highlight overlay if a provider is available.
    pub fn clear_highlight(&self) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => provider.clear(),
            None => Err(PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "no HighlightProvider registered")),
        }
    }

    /// Captures a screenshot using the registered screenshot provider.
    pub fn screenshot(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        match self.screenshot {
            Some(provider) => provider.capture(request),
            None => Err(PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "no ScreenshotProvider registered")),
        }
    }

    pub fn pointer_settings(&self) -> PointerSettings {
        self.pointer_settings.lock().unwrap().clone()
    }

    pub fn set_pointer_settings(&self, settings: PointerSettings) {
        {
            *self.pointer_settings.lock().unwrap() = settings.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().unwrap().as_mut() {
            engine.set_settings(settings);
        }
    }

    pub fn pointer_profile(&self) -> PointerProfile {
        self.pointer_profile.lock().unwrap().clone()
    }

    pub fn set_pointer_profile(&self, profile: PointerProfile) {
        {
            *self.pointer_profile.lock().unwrap() = profile.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().unwrap().as_mut() {
            engine.set_profile(profile);
        }
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

    pub fn pointer_move_to(&self, point: Point, overrides: Option<PointerOverrides>) -> Result<Point, PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.move_to(point, overrides_ref)
    }

    pub fn pointer_click(
        &self,
        point: Point,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.click(point, button, overrides_ref)
    }

    pub fn pointer_multi_click(
        &self,
        point: Point,
        button: Option<PointerButton>,
        clicks: u32,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.multi_click(point, button, clicks, overrides_ref)
    }

    pub fn pointer_press(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        let resolved_button = button.unwrap_or_else(|| engine.default_button());
        if let Some(point) = target {
            engine.move_to(point, overrides_ref)?;
        }
        engine.press(resolved_button, overrides_ref)
    }

    pub fn pointer_release(
        &self,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        let resolved_button = button.unwrap_or_else(|| engine.default_button());
        engine.release(resolved_button, overrides_ref)
    }

    pub fn pointer_scroll(&self, delta: ScrollDelta, overrides: Option<PointerOverrides>) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.scroll(delta, overrides_ref)
    }

    pub fn pointer_drag(
        &self,
        start: Point,
        end: Point,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().unwrap();
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.drag(start, end, button, overrides_ref)
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
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?.execute(&resolved, KeyboardMode::Press)?;
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
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?.execute(&resolved, KeyboardMode::Release)?;
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
        KeyboardEngine::new(device, settings, &default_keyboard_sleep)?.execute(&resolved, KeyboardMode::Type)?;
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
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down
        }
        self.dispatcher.shutdown();
        for provider in &self.providers {
            provider.shutdown();
        }
    }

    // refresh_desktop_nodes removed: DesktopNode streams children on-demand from providers

    fn pointer_device(&self) -> Result<&'static dyn PointerDevice, PointerError> {
        self.pointer.ok_or(PointerError::MissingDevice)
    }

    fn keyboard_device(&self) -> Result<&'static dyn KeyboardDevice, KeyboardError> {
        self.keyboard.ok_or(KeyboardError::NotReady)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Ensure providers and dispatcher are shut down exactly once.
        self.shutdown();
    }
}

fn build_desktop_info() -> Result<DesktopInfo, PlatformError> {
    let mut providers = desktop_info_providers();
    let info = if let Some(provider) = providers.next() { provider.desktop_info()? } else { fallback_desktop_info() };
    Ok(info)
}

fn map_desktop_error(err: PlatformError) -> ProviderError {
    ProviderError::new(ProviderErrorKind::InitializationFailed, format!("desktop initialization failed: {err}"))
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
    providers: Vec<Arc<dyn UiTreeProvider>>,
    self_weak: OnceCell<Weak<dyn UiNode>>,
}

impl DesktopNode {
    fn new(info: DesktopInfo, providers: Vec<Arc<dyn UiTreeProvider>>) -> Arc<Self> {
        let mut info = info;
        info.runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);
        let namespace = Namespace::Control;
        let mut attributes: Vec<Arc<dyn UiAttribute>> = Vec::new();
        let supported = vec![PatternId::from("Desktop")];

        attributes.push(attr(namespace, attribute_names::common::ROLE, UiValue::from("Desktop")));
        attributes.push(attr(namespace, attribute_names::common::NAME, UiValue::from(info.name.clone())));
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

        attributes.push(attr(namespace, attribute_names::element::BOUNDS, UiValue::from(info.bounds)));
        attributes.extend(rect_alias_attributes(namespace, "Bounds", &info.bounds));
        attributes.push(attr(namespace, attribute_names::element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_ENABLED, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_OFFSCREEN, UiValue::from(false)));

        attributes.push(attr(
            namespace,
            attribute_names::desktop::DISPLAY_COUNT,
            UiValue::from(info.display_count() as i64),
        ));
        attributes.push(attr(namespace, attribute_names::desktop::OS_NAME, UiValue::from(info.os_name.clone())));
        attributes.push(attr(namespace, attribute_names::desktop::OS_VERSION, UiValue::from(info.os_version.clone())));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::MONITORS,
            UiValue::Array(info.monitors.iter().map(monitor_to_value).collect()),
        ));

        Arc::new(Self { info, attributes, supported, providers, self_weak: OnceCell::new() })
    }

    fn info(&self) -> &DesktopInfo {
        &self.info
    }

    fn as_ui_node(self: &Arc<Self>) -> Arc<dyn UiNode> {
        Arc::clone(self) as Arc<dyn UiNode>
    }
    fn init_self(this: &Arc<Self>) {
        let arc: Arc<dyn UiNode> = this.clone();
        let _ = this.self_weak.set(Arc::downgrade(&arc));
    }

    // children are provided on-demand from providers; no replacement snapshot
}

impl UiNode for DesktopNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn role(&self) -> &str {
        "Desktop"
    }

    fn name(&self) -> String {
        self.info.name.clone()
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.info.runtime_id
    }

    fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        struct DesktopChildrenIter {
            providers: Vec<Arc<dyn UiTreeProvider>>,
            idx: usize,
            parent: Arc<dyn UiNode>,
            current: Option<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>>,
        }
        impl Iterator for DesktopChildrenIter {
            type Item = Arc<dyn UiNode>;
            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    if let Some(it) = self.current.as_mut() {
                        if let Some(next) = it.next() {
                            return Some(next);
                        }
                        self.current = None;
                    }
                    if self.idx >= self.providers.len() {
                        return None;
                    }
                    let prov = &self.providers[self.idx];
                    self.idx += 1;
                    match prov.get_nodes(Arc::clone(&self.parent)) {
                        Ok(iter) => {
                            self.current = Some(iter);
                        }
                        Err(_) => { /* skip provider */ }
                    }
                }
            }
        }
        let parent = self.self_weak.get().and_then(|w| w.upgrade()).expect("desktop self weak set");
        let providers = self.providers.to_vec();
        Box::new(DesktopChildrenIter { providers, idx: 0, parent, current: None })
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
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

fn rect_alias_attributes(namespace: Namespace, base: &str, rect: &Rect) -> Vec<Arc<dyn UiAttribute>> {
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
    use crate::PointerOverrides;
    use platynui_core::platform::{HighlightRequest, KeyboardOverrides, PointerButton, ScreenshotRequest, ScrollDelta};
    use platynui_core::platform::{PlatformError, PlatformModule};
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory,
    };
    use platynui_core::register_platform_module;
    use platynui_core::types::{Point, Rect};
    use platynui_core::ui::UiPattern;
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
    use platynui_platform_mock as _;
    use platynui_platform_mock::{
        KeyboardLogEntry, PointerLogEntry, reset_highlight_state, reset_keyboard_state, reset_pointer_state,
        reset_screenshot_state, take_highlight_log, take_keyboard_log, take_pointer_log, take_screenshot_log,
    };
    // Provider werden in diesen Tests explizit injiziert (keine inventory-Discovery)
    use crate::test_support::runtime_with_factories_and_mock_platform as rt_with_pf;
    use rstest::{fixture, rstest};
    use serial_test::serial;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, Weak};
    use std::time::Duration;

    // --- 19.4: Ensure platform modules initialize before provider creation ----------------------
    // A tiny test-only platform module toggles a flag in initialize(). A test-only provider
    // asserts that the flag is set in its factory `create()`. Runtime::new() must call
    // platform initialization before instantiating providers for this test to pass.
    static TEST_PLATFORM_INITIALIZED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

    struct TestInitOrderPlatform;
    impl PlatformModule for TestInitOrderPlatform {
        fn name(&self) -> &'static str {
            "test-init-order-platform"
        }
        fn initialize(&self) -> Result<(), PlatformError> {
            TEST_PLATFORM_INITIALIZED.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    static TEST_PLATFORM: TestInitOrderPlatform = TestInitOrderPlatform;
    register_platform_module!(&TEST_PLATFORM);

    struct InitOrderProviderFactory;
    impl UiTreeProviderFactory for InitOrderProviderFactory {
        fn descriptor(&self) -> &'static ProviderDescriptor {
            Self::descriptor_static()
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            assert!(
                TEST_PLATFORM_INITIALIZED.load(Ordering::SeqCst),
                "platform modules must be initialized before providers are created"
            );
            struct NoopProvider {
                desc: &'static ProviderDescriptor,
            }
            impl UiTreeProvider for NoopProvider {
                fn descriptor(&self) -> &'static ProviderDescriptor {
                    self.desc
                }
                fn get_nodes(
                    &self,
                    _parent: Arc<dyn UiNode>,
                ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
                    Ok(Box::new(std::iter::empty()))
                }
                fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
                    Ok(())
                }
                fn shutdown(&self) {}
            }
            Ok(Arc::new(NoopProvider { desc: Self::descriptor_static() }))
        }
    }

    impl InitOrderProviderFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-init-order",
                    "Runtime InitOrder",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }
    static INIT_ORDER_PROVIDER: InitOrderProviderFactory = InitOrderProviderFactory;

    #[test]
    fn platform_init_happens_before_provider_instantiation() {
        // The assertions happen inside the provider factory `create()`.
        let _runtime = Runtime::new_with_factories(&[&INIT_ORDER_PROVIDER]).expect("runtime initializes");
    }

    #[test]
    fn runtime_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Runtime>();
    }

    #[fixture]
    fn rt_runtime_stub() -> Runtime {
        return rt_with_pf(&[&RUNTIME_FACTORY]);
    }

    #[fixture]
    fn rt_runtime_focus() -> Runtime {
        return rt_with_pf(&[&FOCUS_FACTORY]);
    }

    #[fixture]
    fn rt_runtime_platform() -> Runtime {
        return rt_with_pf(&[]);
    }

    // no UIA-specific fixture here; targeted UIA tests live in consumer crates

    #[rstest]
    fn query_windows_streams_and_completes(rt_runtime_stub: Runtime) {
        // Broad window query should return promptly and complete without hanging
        let res = rt_runtime_stub.evaluate(None, "//control:Window").expect("evaluate windows");
        // Pull all items to ensure completion
        for _ in res {}
    }

    #[rstest]
    fn query_window_by_name_streams_and_completes(rt_runtime_stub: Runtime) {
        // Narrow query with attribute predicate must stream and complete
        let res = rt_runtime_stub
            .evaluate(None, "//control:Window[@Name='Operations Console']")
            .expect("evaluate windows by name");
        // It's fine if mock doesn't match; ensure evaluation completes
        for _ in res {}
    }

    use crate::test_support::rt_runtime_mock;

    #[rstest]
    fn union_windows_or_buttons_returns_both(rt_runtime_mock: Runtime) {
        // The mock tree contains 1 Window and 2 Buttons
        let res = rt_runtime_mock.evaluate(None, "//control:Window | //control:Button").expect("evaluate union");
        let mut count = 0usize;
        let mut names = Vec::new();
        for item in res {
            if let EvaluationItem::Node(node) = item {
                count += 1;
                names.push(node.name().to_string());
            }
        }
        assert!(count >= 3, "expected at least 3 nodes (1 window + 2 buttons), got {}", count);
        assert!(names.iter().any(|n| n == "Operations Console"));
        assert!(names.iter().any(|n| n == "OK"));
        assert!(names.iter().any(|n| n == "Cancel"));
    }

    #[rstest]
    fn intersect_windows_and_buttons_is_empty(rt_runtime_mock: Runtime) {
        let res =
            rt_runtime_mock.evaluate(None, "//control:Window intersect //control:Button").expect("evaluate intersect");
        let mut count = 0usize;
        for _ in res {
            count += 1;
        }
        assert_eq!(count, 0, "Windows and Buttons should be disjoint");
    }

    #[rstest]
    fn except_windows_minus_buttons_equals_windows(rt_runtime_mock: Runtime) {
        // Count all windows
        let windows = rt_runtime_mock.evaluate(None, "//control:Window").expect("evaluate windows");
        let mut windows_count = 0usize;
        for _ in windows {
            windows_count += 1;
        }

        // Subtract buttons from windows (disjoint sets) — result should equal windows
        let diff = rt_runtime_mock.evaluate(None, "//control:Window except //control:Button").expect("evaluate except");
        let mut diff_count = 0usize;
        for _ in diff {
            diff_count += 1;
        }

        assert_eq!(diff_count, windows_count);
    }

    #[rstest]
    fn intersect_buttons_with_self_is_identity(rt_runtime_mock: Runtime) {
        let buttons = rt_runtime_mock.evaluate(None, "//control:Button").expect("evaluate buttons");
        let mut buttons_count = 0usize;
        for _ in buttons {
            buttons_count += 1;
        }

        let inter = rt_runtime_mock
            .evaluate(None, "//control:Button intersect //control:Button")
            .expect("evaluate intersect self");
        let mut inter_count = 0usize;
        for _ in inter {
            inter_count += 1;
        }

        assert_eq!(inter_count, buttons_count);
    }

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
        fn name(&self) -> String {
            "Stub".to_string()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
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
        profile.acceleration_profile = platynui_core::platform::PointerAccelerationProfile::Constant;
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
        fn subscribe_events(&self, listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
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

    struct RecordingSink {
        events: Mutex<Vec<ProviderEventKind>>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self { events: Mutex::new(Vec::new()) }
        }
    }

    // --- Focus test provider ---------------------------------------------------------------
    #[derive(Clone)]
    struct SimpleAttribute {
        namespace: Namespace,
        name: &'static str,
        value: UiValue,
    }
    impl UiAttribute for SimpleAttribute {
        fn namespace(&self) -> Namespace {
            self.namespace
        }
        fn name(&self) -> &str {
            self.name
        }
        fn value(&self) -> UiValue {
            self.value.clone()
        }
    }

    struct FocusNode {
        runtime_id: RuntimeId,
        role: &'static str,
        name: &'static str,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
        focusable: bool,
    }
    impl FocusNode {
        fn new(id: &str, role: &'static str, name: &'static str, focusable: bool) -> Self {
            Self { runtime_id: RuntimeId::from(id), role, name, parent: Mutex::new(None), focusable }
        }
        fn set_parent(&self, parent: &Arc<dyn UiNode>) {
            *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
        }
    }
    impl UiNode for FocusNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            self.role
        }
        fn name(&self) -> String {
            self.name.to_string()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            let attrs: Vec<Arc<dyn UiAttribute>> = vec![
                Arc::new(SimpleAttribute {
                    namespace: Namespace::Control,
                    name: attribute_names::common::ROLE,
                    value: UiValue::from(self.role),
                }) as Arc<dyn UiAttribute>,
                Arc::new(SimpleAttribute {
                    namespace: Namespace::Control,
                    name: attribute_names::common::NAME,
                    value: UiValue::from(self.name),
                }) as Arc<dyn UiAttribute>,
                Arc::new(SimpleAttribute {
                    namespace: Namespace::Control,
                    name: attribute_names::common::RUNTIME_ID,
                    value: UiValue::from(self.runtime_id.as_str().to_owned()),
                }) as Arc<dyn UiAttribute>,
                Arc::new(SimpleAttribute {
                    namespace: Namespace::Control,
                    name: attribute_names::common::TECHNOLOGY,
                    value: UiValue::from("Runtime"),
                }) as Arc<dyn UiAttribute>,
            ];
            Box::new(attrs.into_iter())
        }
        fn supported_patterns(&self) -> Vec<PatternId> {
            if self.focusable { vec![PatternId::from("Focusable")] } else { Vec::new() }
        }
        fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
            if self.focusable && *pattern == PatternId::from("Focusable") {
                let action: Arc<dyn UiPattern> = Arc::new(FocusableAction::new(|| Ok(())));
                Some(action)
            } else {
                None
            }
        }
        fn invalidate(&self) {}
    }

    struct FocusProvider {
        desc: &'static ProviderDescriptor,
        button: Arc<FocusNode>,
        panel: Arc<FocusNode>,
    }
    impl FocusProvider {
        fn new(desc: &'static ProviderDescriptor) -> Self {
            Self {
                desc,
                button: Arc::new(FocusNode::new("focus-btn", "Button", "OK", true)),
                panel: Arc::new(FocusNode::new("focus-panel", "Panel", "Workspace", false)),
            }
        }
    }
    impl UiTreeProvider for FocusProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            self.desc
        }
        fn get_nodes(
            &self,
            parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            self.button.set_parent(&parent);
            self.panel.set_parent(&parent);
            Ok(Box::new(
                vec![self.button.clone() as Arc<dyn UiNode>, self.panel.clone() as Arc<dyn UiNode>].into_iter(),
            ))
        }
        fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
            Ok(())
        }
        fn shutdown(&self) {}
    }

    struct FocusFactory;
    impl FocusFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-focus",
                    "Runtime Focus",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }
    impl UiTreeProviderFactory for FocusFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }
        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(FocusProvider::new(Self::descriptor_static())))
        }
    }
    static FOCUS_FACTORY: FocusFactory = FocusFactory;

    impl ProviderEventSink for RecordingSink {
        fn dispatch(&self, event: ProviderEvent) {
            self.events.lock().unwrap().push(event.kind);
        }
    }

    #[rstest]
    fn runtime_initializes_providers() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        SUBSCRIPTION_REGISTERED.store(false, Ordering::SeqCst);

        // Build runtime after resetting flags so subscribe_events sets the flag now
        let runtime = Runtime::new_with_factories(&[&RUNTIME_FACTORY]).expect("runtime initializes");
        let providers: Vec<_> = runtime.providers().collect();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|provider| provider.descriptor().id == "runtime-stub"));
        assert!(SUBSCRIPTION_REGISTERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn runtime_dispatcher_forwards_events(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let sink = Arc::new(RecordingSink::new());
        runtime.register_event_sink(sink.clone());

        runtime.dispatch_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });

        let events = sink.events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(matches!(events.last().unwrap(), ProviderEventKind::TreeInvalidated));
    }

    #[rstest]
    fn runtime_filters_providers_by_technology(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let tech = TechnologyId::from("RuntimeTech");
        let providers: Vec<_> = runtime.providers_for(&tech).collect();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].descriptor().id, "runtime-stub");
    }

    #[rstest]
    fn runtime_shutdown_invokes_provider_shutdown(rt_runtime_stub: Runtime) {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        let mut runtime = rt_runtime_stub;
        runtime.shutdown();
        assert!(SHUTDOWN_TRIGGERED.load(Ordering::SeqCst));
    }

    // --- Drop lifecycle tests ---------------------------------------------------------------
    static DROP_COUNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    struct DropCounterProvider {
        desc: &'static ProviderDescriptor,
    }
    impl UiTreeProvider for DropCounterProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            self.desc
        }
        fn get_nodes(
            &self,
            _parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            Ok(Box::new(std::iter::empty()))
        }
        fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
            Ok(())
        }
        fn shutdown(&self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct DropCounterFactory;
    impl DropCounterFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-drop-counter",
                    "Runtime Drop Counter",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }
    impl UiTreeProviderFactory for DropCounterFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }
        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(DropCounterProvider { desc: Self::descriptor_static() }))
        }
    }
    static DROP_COUNTER_FACTORY: DropCounterFactory = DropCounterFactory;

    #[test]
    fn runtime_drop_triggers_shutdown_once() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            let _rt = Runtime::new_with_factories(&[&DROP_COUNTER_FACTORY]).expect("runtime");
        } // drop here
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn runtime_shutdown_then_drop_is_idempotent() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            let mut rt = Runtime::new_with_factories(&[&DROP_COUNTER_FACTORY]).expect("runtime");
            rt.shutdown();
            assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1, "shutdown should be called once");
        } // drop should not call shutdown again
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1, "drop must be idempotent after shutdown");
    }

    #[rstest]
    fn runtime_evaluate_executes_xpath(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let results = runtime.evaluate(None, "//control:Button").expect("evaluation");
        assert!(!results.is_empty());
    }

    #[rstest]
    fn provider_nodes_link_parent(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let parent: Arc<dyn UiNode> = Arc::new(StubNode::new("parent"));
        let node = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| provider.get_nodes(Arc::clone(&parent)).ok().and_then(|mut nodes| nodes.next()))
            .expect("runtime stub provider node available");
        assert!(node.parent().is_some());
    }

    #[rstest]
    fn injected_provider_attaches_to_desktop(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let desktop = runtime.desktop_node();
        let app = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| provider.get_nodes(Arc::clone(&desktop)).ok())
            .and_then(|mut nodes| nodes.next())
            .expect("injected provider root node");

        assert_eq!(app.namespace(), Namespace::Control);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), runtime.desktop_info().runtime_id.as_str());
    }

    #[rstest]
    fn runtime_focus_succeeds_on_focusable(rt_runtime_focus: Runtime) {
        let mut runtime = rt_runtime_focus;
        let desktop = runtime.desktop_node();
        let focus = FOCUS_FACTORY.create().expect("focus provider");
        let nodes = focus.get_nodes(desktop).expect("children");
        let mut button = None;
        for node in nodes {
            if node.role() == "Button" {
                button = Some(node);
            }
        }
        let button = button.expect("button node available");
        runtime.focus(&button).expect("focus succeeds");
        runtime.shutdown();
    }

    #[rstest]
    fn runtime_focus_requires_focusable_pattern(rt_runtime_focus: Runtime) {
        let mut runtime = rt_runtime_focus;
        let desktop = runtime.desktop_node();
        let focus = FOCUS_FACTORY.create().expect("focus provider");
        let nodes = focus.get_nodes(desktop).expect("children");
        let mut panel = None;
        for node in nodes {
            if node.role() == "Panel" {
                panel = Some(node);
            }
        }
        let panel = panel.expect("panel node available");
        let err = runtime.focus(&panel).expect_err("panel should not support focus");
        assert!(matches!(err, FocusError::PatternMissing { .. }));
        runtime.shutdown();
    }

    #[rstest]
    fn highlight_invokes_registered_provider(rt_runtime_platform: Runtime) {
        reset_highlight_state();
        let runtime = rt_runtime_platform;
        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 50.0, 25.0));
        runtime.highlight(std::slice::from_ref(&request)).expect("highlight succeeds");

        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].len(), 1);
        assert_eq!(log[0][0], request);
    }

    #[rstest]
    fn screenshot_invokes_registered_provider(rt_runtime_platform: Runtime) {
        reset_screenshot_state();
        let runtime = rt_runtime_platform;
        let request = ScreenshotRequest::with_region(Rect::new(0.0, 0.0, 20.0, 10.0));
        let screenshot = runtime.screenshot(&request).expect("screenshot captures");

        assert_eq!(screenshot.width, 20);
        assert_eq!(screenshot.height, 10);
        assert_eq!(take_screenshot_log().len(), 1);
    }

    #[rstest]
    #[serial]
    fn keyboard_press_logs_events(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
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

        runtime.keyboard_release("<Ctrl+Alt+T>", Some(overrides)).expect("cleanup release succeeds");
        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn keyboard_release_logs_events(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_press("<Ctrl+Alt+T>", Some(overrides.clone())).expect("press succeeds");
        reset_keyboard_state();

        runtime.keyboard_release("<Ctrl+Alt+T>", Some(overrides.clone())).expect("release succeeds");

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
    fn keyboard_type_emits_press_and_release(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
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
    fn pointer_move_uses_device_log(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime.pointer_move_to(Point::new(50.0, 25.0), Some(zero_overrides())).expect("move succeeds");

        let log = take_pointer_log();
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Move(p) if *p == Point::new(50.0, 25.0))));
    }

    #[rstest]
    #[serial]
    fn pointer_click_emits_press_and_release(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime.pointer_click(Point::new(10.0, 10.0), None, Some(zero_overrides())).expect("click succeeds");

        let log = take_pointer_log();
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Press(PointerButton::Left))));
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Release(PointerButton::Left))));
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_emits_multiple_events(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime
            .pointer_multi_click(Point::new(20.0, 20.0), Some(PointerButton::Right), 3, Some(zero_overrides()))
            .expect("multi-click succeeds");

        let log = take_pointer_log();
        let presses = log.iter().filter(|event| matches!(event, PointerLogEntry::Press(PointerButton::Right))).count();
        let releases =
            log.iter().filter(|event| matches!(event, PointerLogEntry::Release(PointerButton::Right))).count();
        assert_eq!(presses, 3);
        assert_eq!(releases, 3);
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_rejects_zero(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        let error = runtime.pointer_multi_click(Point::new(5.0, 5.0), None, 0, Some(zero_overrides())).unwrap_err();
        match error {
            PointerError::InvalidClickCount { provided } => assert_eq!(provided, 0),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[rstest]
    #[serial]
    fn pointer_scroll_chunks_delta(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        let overrides = zero_overrides().scroll_step(ScrollDelta::new(0.0, -10.0));
        runtime.pointer_scroll(ScrollDelta::new(0.0, -25.0), Some(overrides)).expect("scroll succeeds");

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
