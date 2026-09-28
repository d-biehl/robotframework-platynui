use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::Duration;

use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_core::platform::{
    KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent, KeyboardOverrides, PlatformBundle, PlatformError,
    PlatformFactory,
};
use platynui_core::provider::UiTreeProvider;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
    UiTreeProviderFactory,
};
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{
    ActivatableAction, FocusableAction, Namespace, PatternError, PatternName, RuntimeId, UiAttribute, UiNode,
    UiPattern, UiValue, pattern_names,
};
use platynui_platform_mock::create_mock_bundle;
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
pub fn rt_runtime_rejecting_window() -> Runtime {
    return rt_with_pf(&[&REJECTING_WINDOW_FACTORY]);
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
    fn name(&self) -> &'static str {
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
    fn role(&self) -> &'static str {
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
    fn supported_patterns(&self) -> Vec<PatternName> {
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

    fn create(&self, _config: &platynui_core::config::RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
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
    fn supported_patterns(&self) -> Vec<PatternName> {
        if self.focusable { vec![PatternName::from(pattern_names::FOCUSABLE)] } else { Vec::new() }
    }
    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        if self.focusable && *pattern == PatternName::from(pattern_names::FOCUSABLE) {
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
    fn create(&self, _config: &platynui_core::config::RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(FocusProvider::new(Self::descriptor_static())))
    }
}
pub static FOCUS_FACTORY: FocusFactory = FocusFactory;

// --- Window whose activation fails ---

pub struct RejectingWindowNode {
    runtime_id: RuntimeId,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
}
impl UiNode for RejectingWindowNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn role(&self) -> &'static str {
        "Window"
    }
    fn name(&self) -> String {
        "Rejecting".to_string()
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
        Box::new(std::iter::empty())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        vec![PatternName::from(pattern_names::ACTIVATABLE)]
    }
    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        (*pattern == PatternName::from(pattern_names::ACTIVATABLE)).then(|| {
            Arc::new(ActivatableAction::new(|| Err(PatternError::new("window rejected activation"))))
                as Arc<dyn UiPattern>
        })
    }
    fn invalidate(&self) {}
}

pub struct RejectingWindowProvider {
    desc: &'static ProviderDescriptor,
    window: Arc<RejectingWindowNode>,
}
impl UiTreeProvider for RejectingWindowProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.desc
    }
    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        *self.window.parent.lock().unwrap() = Some(Arc::downgrade(&parent));
        Ok(Box::new(std::iter::once(self.window.clone() as Arc<dyn UiNode>)))
    }
    fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        Ok(())
    }
    fn shutdown(&self) {}
}

pub struct RejectingWindowFactory;
impl RejectingWindowFactory {
    pub fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                "runtime-rejecting-window",
                "Runtime Rejecting Window",
                TechnologyId::from("Runtime"),
                ProviderKind::Native,
            )
        });
        &DESCRIPTOR
    }
}
impl UiTreeProviderFactory for RejectingWindowFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }
    fn create(&self, _config: &platynui_core::config::RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(RejectingWindowProvider {
            desc: Self::descriptor_static(),
            window: Arc::new(RejectingWindowNode {
                runtime_id: RuntimeId::from("rejecting-window"),
                parent: Mutex::new(None),
            }),
        }))
    }
}
pub static REJECTING_WINDOW_FACTORY: RejectingWindowFactory = RejectingWindowFactory;

// --- A node of a chosen namespace and role ---

/// A node with a chosen namespace and role and the children [`Self::adopt`]
/// gave it; `activatable` makes it a top-level window.
pub struct ShapedNode {
    runtime_id: RuntimeId,
    namespace: Namespace,
    role: &'static str,
    activatable: bool,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    children: Mutex<Vec<Arc<dyn UiNode>>>,
}

impl ShapedNode {
    pub fn new(id: &str, namespace: Namespace, role: &'static str, activatable: bool) -> Arc<Self> {
        Arc::new(Self {
            runtime_id: RuntimeId::from(id),
            namespace,
            role,
            activatable,
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    /// Makes `child` a child of `parent`.
    pub fn adopt(parent: &Arc<Self>, child: &Arc<Self>) {
        let erased: Arc<dyn UiNode> = parent.clone();
        *child.parent.lock().unwrap() = Some(Arc::downgrade(&erased));
        parent.children.lock().unwrap().push(child.clone());
    }
}

impl UiNode for ShapedNode {
    fn namespace(&self) -> Namespace {
        self.namespace
    }
    fn role(&self) -> &str {
        self.role
    }
    fn name(&self) -> String {
        self.role.to_string()
    }
    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        Box::new(self.children.lock().unwrap().clone().into_iter())
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        if self.activatable { vec![PatternName::from(pattern_names::ACTIVATABLE)] } else { Vec::new() }
    }
    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        (self.activatable && *pattern == PatternName::from(pattern_names::ACTIVATABLE))
            .then(|| Arc::new(ActivatableAction::new(|| Ok(()))) as Arc<dyn UiPattern>)
    }
    fn invalidate(&self) {}
}

// --- A lazy tree whose window counts its activations ---

/// What happened to the nodes of [`LAZY_TREE_FACTORY`]'s trees, shared by
/// every runtime of the process; tests that read it run `#[serial(lazy_tree)]`.
pub struct LazyTreeLog {
    live: AtomicUsize,
    activations: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
    keep_parents: AtomicBool,
}

impl LazyTreeLog {
    pub fn reset(&self) {
        self.live.store(0, Ordering::SeqCst);
        self.activations.store(0, Ordering::SeqCst);
        self.events.lock().unwrap().clear();
        self.keep_parents.store(true, Ordering::SeqCst);
    }

    /// Whether listed nodes keep their parent alive (the provider rule, on by
    /// default); off, they keep it only as a `Weak`.
    pub fn set_keep_parents(&self, keep: bool) {
        self.keep_parents.store(keep, Ordering::SeqCst);
    }

    /// Nodes of the lazy tree alive right now.
    pub fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// How often the window was activated.
    pub fn activations(&self) -> usize {
        self.activations.load(Ordering::SeqCst)
    }

    /// `"drop"` for every node released and `"shutdown"` for the provider's
    /// shutdown, in order.
    pub fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }
}

pub static LAZY_TREE_LOG: LazyLock<LazyTreeLog> = LazyLock::new(|| LazyTreeLog {
    live: AtomicUsize::new(0),
    activations: AtomicUsize::new(0),
    events: Mutex::new(Vec::new()),
    keep_parents: AtomicBool::new(true),
});

/// A node of a tree that, like a real provider, creates fresh nodes on every
/// listing: a window with a pane with a button. Listed nodes keep their parent
/// alive, unless [`LazyTreeLog::set_keep_parents`] turned that off, and the
/// window reaches the desktop only weakly (the provider rule).
pub struct LazyTreeNode {
    runtime_id: RuntimeId,
    role: &'static str,
    parent: Weak<dyn UiNode>,
    _kept_parent: Option<Arc<dyn UiNode>>,
    self_weak: std::sync::OnceLock<Weak<dyn UiNode>>,
}

impl LazyTreeNode {
    fn create(role: &'static str, parent: &Arc<dyn UiNode>, keep_parent: bool) -> Arc<dyn UiNode> {
        let node = Arc::new(Self {
            runtime_id: RuntimeId::from(format!("lazy-{}", role.to_lowercase())),
            role,
            parent: Arc::downgrade(parent),
            _kept_parent: keep_parent.then(|| Arc::clone(parent)),
            self_weak: std::sync::OnceLock::new(),
        });
        LAZY_TREE_LOG.live.fetch_add(1, Ordering::SeqCst);
        let erased: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&erased));
        erased
    }
}

impl Drop for LazyTreeNode {
    fn drop(&mut self) {
        LAZY_TREE_LOG.live.fetch_sub(1, Ordering::SeqCst);
        LAZY_TREE_LOG.events.lock().unwrap().push("drop");
    }
}

impl UiNode for LazyTreeNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn role(&self) -> &str {
        self.role
    }
    fn name(&self) -> String {
        self.role.to_string()
    }
    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        Some(self.parent.clone())
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let child = match self.role {
            "Window" => "Pane",
            "Pane" => "Button",
            _ => return Box::new(std::iter::empty()),
        };
        let me = self.self_weak.get().and_then(Weak::upgrade).expect("a node is alive while it lists");
        Box::new(std::iter::once(Self::create(child, &me, LAZY_TREE_LOG.keep_parents.load(Ordering::SeqCst))))
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let attribute = |name, value| Arc::new(SimpleAttribute { namespace: Namespace::Control, name, value });
        let attributes: Vec<Arc<dyn UiAttribute>> = vec![
            attribute(attribute_names::common::ROLE, UiValue::from(self.role)),
            attribute(attribute_names::common::NAME, UiValue::from(self.name())),
            attribute(attribute_names::common::RUNTIME_ID, UiValue::from(self.runtime_id.as_str().to_owned())),
        ];
        Box::new(attributes.into_iter())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        if self.role == "Window" { vec![PatternName::from(pattern_names::ACTIVATABLE)] } else { Vec::new() }
    }
    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        (self.role == "Window" && *pattern == PatternName::from(pattern_names::ACTIVATABLE)).then(|| {
            Arc::new(ActivatableAction::new(|| {
                LAZY_TREE_LOG.activations.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })) as Arc<dyn UiPattern>
        })
    }
    fn invalidate(&self) {}
}

pub struct LazyTreeProvider {
    desc: &'static ProviderDescriptor,
}
impl UiTreeProvider for LazyTreeProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.desc
    }
    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        Ok(Box::new(std::iter::once(LazyTreeNode::create("Window", &parent, false))))
    }
    fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        Ok(())
    }
    fn shutdown(&self) {
        LAZY_TREE_LOG.events.lock().unwrap().push("shutdown");
    }
}

pub struct LazyTreeFactory;
impl LazyTreeFactory {
    pub fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                "runtime-lazy-tree",
                "Runtime Lazy Tree",
                TechnologyId::from("Runtime"),
                ProviderKind::Native,
            )
        });
        &DESCRIPTOR
    }
}
impl UiTreeProviderFactory for LazyTreeFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }
    fn create(&self, _config: &platynui_core::config::RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(LazyTreeProvider { desc: Self::descriptor_static() }))
    }
}
pub static LAZY_TREE_FACTORY: LazyTreeFactory = LazyTreeFactory;

#[fixture]
pub fn rt_runtime_lazy_tree() -> Runtime {
    LAZY_TREE_LOG.reset();
    rt_with_pf(&[&LAZY_TREE_FACTORY])
}

// --- A platform whose keyboard rejects some keys ---

/// The one character the stub keyboard cannot type.
pub const REJECTED_CHAR: char = 'é';
/// A character the stub keyboard maps, but fails to send.
pub const UNSENDABLE_CHAR: char = '#';
/// A fragment of the stub keyboard's send failure, which only the normal
/// rendering of an error may contain.
pub const SEND_FAILURE_TEXT: &str = "Vb8 wire cut";

/// A keyboard that rejects [`REJECTED_CHAR`] and every multi-character name
/// except `Ctrl`, and fails to send [`UNSENDABLE_CHAR`]. With `busy`, it
/// cannot start an input.
pub struct StubKeyboard {
    busy: bool,
}

impl KeyboardDevice for StubKeyboard {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        let mut chars = name.chars();
        let single = chars.next().is_some() && chars.next().is_none();
        let rejected = if single { name.starts_with(REJECTED_CHAR) } else { name != "Ctrl" };
        if rejected {
            return Err(KeyboardError::UnsupportedKey(name.to_owned()));
        }
        Ok(KeyCode::new(name.to_owned()))
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        if self.busy {
            Err(KeyboardError::Platform(PlatformError::OperationFailed {
                operation: "stub start",
                details: Some(SEND_FAILURE_TEXT.into()),
            }))
        } else {
            Ok(())
        }
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        if event.code.downcast_ref::<String>().is_some_and(|name| name.starts_with(UNSENDABLE_CHAR)) {
            return Err(KeyboardError::Platform(PlatformError::OperationFailed {
                operation: "stub send",
                details: Some(SEND_FAILURE_TEXT.into()),
            }));
        }
        Ok(())
    }
}

/// Platform backend `stub-keyboard` (and `stub-keyboard-busy`): the mock
/// devices with a [`StubKeyboard`], served only when a config selects it.
pub struct StubKeyboardPlatform {
    id: &'static str,
    busy: bool,
}

impl PlatformFactory for StubKeyboardPlatform {
    fn id(&self) -> &'static str {
        self.id
    }

    fn can_serve(&self, config: &RuntimeConfig) -> bool {
        config.platform_backend() == Some(self.id)
    }

    fn create(&self, _config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
        Ok(PlatformBundle { keyboard: Arc::new(StubKeyboard { busy: self.busy }), ..create_mock_bundle() })
    }
}

static STUB_KEYBOARD_PLATFORM: StubKeyboardPlatform = StubKeyboardPlatform { id: "stub-keyboard", busy: false };
static BUSY_KEYBOARD_PLATFORM: StubKeyboardPlatform = StubKeyboardPlatform { id: "stub-keyboard-busy", busy: true };
platynui_core::register_platform_factory!(&STUB_KEYBOARD_PLATFORM);
platynui_core::register_platform_factory!(&BUSY_KEYBOARD_PLATFORM);

/// A runtime whose keyboard is a [`StubKeyboard`] without delays.
pub fn runtime_with_stub_keyboard(busy: bool) -> Runtime {
    let id = if busy { BUSY_KEYBOARD_PLATFORM.id } else { STUB_KEYBOARD_PLATFORM.id };
    let config = RuntimeConfig::new(ConfigMap::new().with("backend", id), ConfigMap::new());
    let runtime = Runtime::new_with_factories_and_config(&[&RUNTIME_FACTORY], config).expect("runtime");
    configure_keyboard_for_tests(&runtime);
    runtime
}

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
