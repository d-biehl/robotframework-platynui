use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::Duration;

use platynui_core::platform::KeyboardOverrides;
use platynui_core::provider::UiTreeProvider;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
    UiTreeProviderFactory,
};
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{
    FocusableAction, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue, pattern_ids,
};
use platynui_platform_mock as _;
use platynui_provider_mock as _;
use rstest::fixture;

use crate::PointerOverrides;
use crate::provider::event::ProviderEventSink;
use crate::test_support::runtime_with_factories_and_mock_platform as rt_with_pf;

use super::Runtime;

// --- rstest fixtures ---

#[fixture]
pub fn rt_runtime_stub() -> Runtime {
    return rt_with_pf(&[&RUNTIME_FACTORY]);
}

#[fixture]
pub fn rt_runtime_focus() -> Runtime {
    return rt_with_pf(&[&FOCUS_FACTORY]);
}

#[fixture]
pub fn rt_runtime_platform() -> Runtime {
    return rt_with_pf(&[]);
}

// --- Global flags used by StubProvider ---

pub static SHUTDOWN_TRIGGERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
pub static SUBSCRIPTION_REGISTERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

// --- StubAttribute / StubNode ---

pub struct StubAttribute;
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

pub struct StubNode {
    runtime_id: RuntimeId,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
}

impl StubNode {
    pub fn new(id: &str) -> Self {
        Self { runtime_id: RuntimeId::from(id), parent: Mutex::new(None) }
    }

    pub fn set_parent(&self, parent: &Arc<dyn UiNode>) {
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

// --- StubProvider / StubFactory ---

pub struct StubProvider {
    descriptor: &'static ProviderDescriptor,
    node: Arc<StubNode>,
}

impl StubProvider {
    pub fn new(descriptor: &'static ProviderDescriptor) -> Self {
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

pub struct StubFactory;

impl StubFactory {
    pub fn descriptor_static() -> &'static ProviderDescriptor {
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

pub static RUNTIME_FACTORY: StubFactory = StubFactory;

// --- RecordingSink ---

pub struct RecordingSink {
    pub events: Mutex<Vec<ProviderEventKind>>,
}

impl RecordingSink {
    pub fn new() -> Self {
        Self { events: Mutex::new(Vec::new()) }
    }
}

impl ProviderEventSink for RecordingSink {
    fn dispatch(&self, event: ProviderEvent) {
        self.events.lock().unwrap().push(event.kind);
    }
}

// --- Focus test provider ---

#[derive(Clone)]
pub struct SimpleAttribute {
    pub namespace: Namespace,
    pub name: &'static str,
    pub value: UiValue,
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

pub struct FocusNode {
    runtime_id: RuntimeId,
    role: &'static str,
    name: &'static str,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    focusable: bool,
}
impl FocusNode {
    pub fn new(id: &str, role: &'static str, name: &'static str, focusable: bool) -> Self {
        Self { runtime_id: RuntimeId::from(id), role, name, parent: Mutex::new(None), focusable }
    }
    pub fn set_parent(&self, parent: &Arc<dyn UiNode>) {
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
        if self.focusable { vec![PatternId::from(pattern_ids::FOCUSABLE)] } else { Vec::new() }
    }
    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        if self.focusable && *pattern == PatternId::from(pattern_ids::FOCUSABLE) {
            let action: Arc<dyn UiPattern> = Arc::new(FocusableAction::new(|| Ok(())));
            Some(action)
        } else {
            None
        }
    }
    fn invalidate(&self) {}
}

pub struct FocusProvider {
    desc: &'static ProviderDescriptor,
    button: Arc<FocusNode>,
    panel: Arc<FocusNode>,
}
impl FocusProvider {
    pub fn new(desc: &'static ProviderDescriptor) -> Self {
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
        Ok(Box::new(vec![self.button.clone() as Arc<dyn UiNode>, self.panel.clone() as Arc<dyn UiNode>].into_iter()))
    }
    fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        Ok(())
    }
    fn shutdown(&self) {}
}

pub struct FocusFactory;
impl FocusFactory {
    pub fn descriptor_static() -> &'static ProviderDescriptor {
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
pub static FOCUS_FACTORY: FocusFactory = FocusFactory;

// --- Keyboard test helpers ---

pub fn configure_keyboard_for_tests(runtime: &Runtime) {
    let mut profile = runtime.keyboard_profile();
    profile.press_delay = Duration::ZERO;
    profile.release_delay = Duration::ZERO;
    profile.between_keys_delay = Duration::ZERO;
    profile.chord_press_delay = Duration::ZERO;
    profile.chord_release_delay = Duration::ZERO;
    profile.after_sequence_delay = Duration::ZERO;
    profile.after_text_delay = Duration::ZERO;
    runtime.set_keyboard_profile(profile);
}

pub fn zero_keyboard_overrides() -> KeyboardOverrides {
    KeyboardOverrides::new()
        .press_delay(Duration::ZERO)
        .release_delay(Duration::ZERO)
        .between_keys_delay(Duration::ZERO)
        .chord_press_delay(Duration::ZERO)
        .chord_release_delay(Duration::ZERO)
        .after_sequence_delay(Duration::ZERO)
        .after_text_delay(Duration::ZERO)
}

// --- Pointer test helpers ---

pub fn configure_pointer_for_tests(runtime: &Runtime) {
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

pub fn zero_overrides() -> PointerOverrides {
    PointerOverrides::new()
        .after_move_delay(Duration::ZERO)
        .after_input_delay(Duration::ZERO)
        .press_release_delay(Duration::ZERO)
        .after_click_delay(Duration::ZERO)
        .scroll_delay(Duration::ZERO)
}
