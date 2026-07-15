//! Live real-provider checks against the Swing fixture app.
//!
//! These tests need a desktop session, a JDK (`java` on `PATH`), and the built
//! fixture app (`just build-test-app-swing`); they are `#[ignore]`d so the
//! plain `just test` lane stays desktop-free. The Windows acceptance recipe
//! runs them explicitly:
//!
//! ```text
//! cargo nextest run -p platynui-provider-jab --run-ignored ignored-only
//! ```
//!
//! `live_fixture_contract_and_interaction` covers OpenSpec `add-jab-provider`
//! task 4.5 (core contract testkit against a live node set) plus the
//! `setTextContents` write path and the Closeable delegation;
//! `live_frozen_jvm_stays_contained` covers task 8.3 (robustness against an
//! unresponsive JVM — may be run manually if it proves flaky in CI).

#![cfg(windows)]
// Integration-test ergonomics: the scenarios are long and linear, define their
// expectation constants next to where they are used, and are full of API names
// in prose. None of these pedantic lints catch bugs here.
#![allow(clippy::too_many_lines, clippy::items_after_statements, clippy::doc_markdown)]

use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_core::platform::platform_factories;
use platynui_core::provider::{UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::contract::testkit::{AttributeExpectation, NodeExpectation, PatternExpectation, verify_node};
use platynui_core::ui::{
    Namespace, PatternName, RuntimeId, TextEditableAction, UiAttribute, UiNode, UiValue, attribute_names,
    pattern_names, validate_control_or_item,
};
use platynui_provider_jab::JabFactory;
// Force-link the Windows platform crate so its inventory-registered platform
// factory (id "windows") is available to `build_provider`.
use platynui_platform_windows as _;
use std::path::PathBuf;

// Crate dependencies of the library that this integration-test target does
// not use directly (`unused_crate_dependencies` is target-scoped).
use chrono as _;
use inventory as _;
use libloading as _;
use std::process::{Child, Command};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use sysinfo as _;
use tempfile as _;
use thiserror as _;
use tracing as _;

const DISCOVERY_DEADLINE: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------------------
// Fixture plumbing

/// The launched fixture JVM; killed on drop so a panicking test cleans up.
struct FixtureApp {
    child: Child,
    title: String,
}

impl FixtureApp {
    fn launch(title_suffix: &str) -> Self {
        let classes = swing_classes_dir();
        assert!(
            classes.is_dir(),
            "Swing fixture classes not found at {} — run `just build-test-app-swing` first",
            classes.display()
        );
        let title = format!("PlatynUI JAB Live {} {}", std::process::id(), title_suffix);
        let child = Command::new("java")
            .arg("-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge")
            .arg("-cp")
            .arg(&classes)
            .arg("platynui.testapp.Main")
            .arg("--title")
            .arg(&title)
            .arg("--auto-close")
            .arg("180")
            .spawn()
            .expect("failed to launch the fixture JVM — is `java` on PATH?");
        Self { child, title }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

impl Drop for FixtureApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn swing_classes_dir() -> PathBuf {
    std::env::var_os("PLATYNUI_TEST_APP_SWING_CLASSES").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("apps")
                .join("test-app-swing")
                .join("build")
                .join("classes")
        },
        PathBuf::from,
    )
}

/// Minimal desktop stand-in handed to `get_nodes` as the parent.
struct DesktopStub(RuntimeId);

#[allow(clippy::unnecessary_literal_bound)] // signatures fixed by the UiNode trait
impl UiNode for DesktopStub {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn role(&self) -> &str {
        "Desktop"
    }
    fn name(&self) -> String {
        "Desktop".into()
    }
    fn runtime_id(&self) -> &RuntimeId {
        &self.0
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        None
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }
    fn invalidate(&self) {}
}

fn desktop_stub() -> Arc<dyn UiNode> {
    Arc::new(DesktopStub(RuntimeId::from("live-test-desktop")))
}

fn build_provider(config: &RuntimeConfig) -> Arc<dyn UiTreeProvider> {
    let provider = JabFactory.create(config).expect("provider construction is infallible");
    // Inject the real Win32 window manager so window-capability patterns work.
    let windows_platform = platform_factories().find(|factory| factory.id() == "windows");
    if let Some(factory) = windows_platform {
        let bundle = factory.create(config).expect("windows platform bundle");
        provider.set_window_manager(bundle.window_manager);
    }
    provider
}

/// Poll the desktop stream until the fixture's window shows up.
fn wait_for_window(provider: &Arc<dyn UiTreeProvider>, parent: &Arc<dyn UiNode>, title: &str) -> Arc<dyn UiNode> {
    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    loop {
        let nodes = provider.get_nodes(Arc::clone(parent)).expect("get_nodes");
        for node in nodes {
            if node.name() == title && !node.runtime_id().as_str().starts_with("jab://app") {
                return node;
            }
        }
        assert!(Instant::now() < deadline, "fixture window {title:?} did not appear within {DISCOVERY_DEADLINE:?}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Depth-first walk collecting every node (parents stay alive in the Vec).
fn walk(node: &Arc<dyn UiNode>, out: &mut Vec<Arc<dyn UiNode>>, depth: usize) {
    assert!(depth < 48, "tree deeper than any sane Swing fixture — recursion guard");
    out.push(Arc::clone(node));
    for child in node.children() {
        walk(&child, out, depth + 1);
    }
}

fn find_by_name<'a>(nodes: &'a [Arc<dyn UiNode>], name: &str) -> &'a Arc<dyn UiNode> {
    nodes
        .iter()
        .find(|node| node.name() == name)
        .unwrap_or_else(|| panic!("fixture control {name:?} not found in the walked tree"))
}

fn attribute_value(node: &Arc<dyn UiNode>, name: &str) -> Option<UiValue> {
    node.attribute(Namespace::Control, name).map(|attr| attr.value())
}

fn structure_signature(nodes: &[Arc<dyn UiNode>]) -> Vec<(String, String, String)> {
    nodes.iter().map(|node| (node.runtime_id().as_str().to_string(), node.role().to_string(), node.name())).collect()
}

// ---------------------------------------------------------------------------
// Task 4.5: contract testkit + interaction against the live fixture

#[test]
#[ignore = "needs a desktop, a JDK on PATH, and the built Swing fixture (run via just test-acceptance-windows)"]
fn live_fixture_contract_and_interaction() {
    let mut app = FixtureApp::launch("contract");
    let provider = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();

    let window = wait_for_window(&provider, &parent, &app.title);
    assert_eq!(window.role(), "Window");
    assert_eq!(window.namespace(), Namespace::Control);
    assert!(window.is_valid(), "isSameObject-based liveness probe must answer for a live window");

    // Full walk; every JAB call in it must succeed against the live JVM.
    let mut nodes = Vec::new();
    walk(&window, &mut nodes, 0);
    assert!(nodes.len() >= 20, "expected the full stage-1/2 tree, got {} nodes", nodes.len());

    // Repeated walks stay structurally stable (handle-hygiene guard: leaked or
    // stale references would surface as failures or drift here).
    let first_signature = structure_signature(&nodes);
    for round in 0..3 {
        let mut again = Vec::new();
        walk(&window, &mut again, 0);
        assert_eq!(structure_signature(&again), first_signature, "walk #{} diverged", round + 2);
    }

    // Node contract + pattern honesty over the whole tree: every advertised
    // pattern resolves to a concrete instance, control/item nodes validate,
    // and the identifying attributes are present.
    for node in &nodes {
        if matches!(node.namespace(), Namespace::Control | Namespace::Item) {
            validate_control_or_item(node.as_ref()).expect("core node contract");
        }
        for pattern in node.supported_patterns() {
            assert!(
                node.pattern_by_name(&pattern).is_some(),
                "advertised pattern {pattern} has no instance on {}",
                node.runtime_id()
            );
        }
        let technology = attribute_value(node, attribute_names::common::TECHNOLOGY);
        assert_eq!(technology, Some(UiValue::from("JAB")), "Technology attribute on {}", node.runtime_id());
    }

    // Contract testkit expectations for the interesting fixture nodes.
    const FOCUSABLE_ATTRS: &[AttributeExpectation] =
        &[AttributeExpectation::required(Namespace::Control, attribute_names::focusable::IS_FOCUSED)];
    const TEXT_EDITABLE_ATTRS: &[AttributeExpectation] = &[
        AttributeExpectation::required(Namespace::Control, attribute_names::text_content::TEXT),
        AttributeExpectation::required(Namespace::Control, attribute_names::text_editable::IS_READ_ONLY),
    ];
    const WINDOW_PATTERN_NAMES: &[&str] = &[
        pattern_names::ACTIVATABLE,
        pattern_names::MINIMIZABLE,
        pattern_names::MAXIMIZABLE,
        pattern_names::RESTORABLE,
        pattern_names::CLOSEABLE,
        pattern_names::MOVABLE,
        pattern_names::RESIZABLE,
        pattern_names::RESPONSIVE,
    ];

    let mut window_expectation = NodeExpectation::default();
    for name in WINDOW_PATTERN_NAMES {
        window_expectation = window_expectation.with_pattern(PatternExpectation::new(PatternName::from(*name), &[]));
    }
    let issues = verify_node(window.as_ref(), &window_expectation);
    assert!(issues.is_empty(), "window contract issues: {issues:?}");

    let button = find_by_name(&nodes, "stage1-button");
    assert_eq!(button.role(), "Button");
    let issues = verify_node(
        button.as_ref(),
        &NodeExpectation::default()
            .with_pattern(PatternExpectation::new(PatternName::from(pattern_names::FOCUSABLE), FOCUSABLE_ATTRS)),
    );
    assert!(issues.is_empty(), "button contract issues: {issues:?}");
    let Some(UiValue::Rect(bounds)) = attribute_value(button, attribute_names::element::BOUNDS) else {
        panic!("button must expose Rect bounds");
    };
    let Some(UiValue::Point(point)) = attribute_value(button, attribute_names::activation_target::ACTIVATION_POINT)
    else {
        panic!("button must expose an activation point");
    };
    assert!(bounds.contains(point), "activation point must sit inside the bounds");

    // TextEditable write path (`setTextContents`) — the genuine JAB write, as
    // opposed to the acceptance lane's keyboard synthesis.
    let textfield = find_by_name(&nodes, "stage1-textfield");
    assert_eq!(textfield.role(), "Text");
    let issues = verify_node(
        textfield.as_ref(),
        &NodeExpectation::default().with_pattern(PatternExpectation::new(
            PatternName::from(pattern_names::TEXT_EDITABLE),
            TEXT_EDITABLE_ATTRS,
        )),
    );
    assert!(issues.is_empty(), "textfield contract issues: {issues:?}");
    let editable = textfield.pattern::<TextEditableAction>().expect("TextEditable pattern instance");
    use platynui_core::ui::TextEditablePattern as _;
    editable.set_text("hello-jab").expect("setTextContents");
    textfield.invalidate();
    assert_eq!(
        attribute_value(textfield, attribute_names::text_content::TEXT),
        Some(UiValue::from("hello-jab")),
        "text round-trip through setTextContents"
    );

    // Toggle/value surfaces on the stage-2 controls.
    let checkbox = find_by_name(&nodes, "stage2-checkbox");
    let toggle = attribute_value(checkbox, attribute_names::toggleable::TOGGLE_STATE);
    assert!(
        matches!(&toggle, Some(UiValue::String(s)) if s == "On" || s == "Off"),
        "checkbox ToggleState must be On/Off, got {toggle:?}"
    );
    let slider = find_by_name(&nodes, "stage2-slider");
    for attr in [
        attribute_names::stateful_value::VALUE,
        attribute_names::stateful_value::MIN_VALUE,
        attribute_names::stateful_value::MAX_VALUE,
    ] {
        assert!(matches!(attribute_value(slider, attr), Some(UiValue::Number(_))), "slider {attr} must be numeric");
    }

    // Closeable delegates to the window manager; the fixture process must die.
    use platynui_core::ui::CloseablePattern as _;
    let closeable = window.pattern::<platynui_core::ui::CloseableAction>().expect("Closeable pattern instance");
    closeable.close().expect("close the fixture window");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.has_exited() {
        assert!(Instant::now() < deadline, "fixture did not exit after Closeable.close()");
        std::thread::sleep(Duration::from_millis(200));
    }

    provider.shutdown();
}

// ---------------------------------------------------------------------------
// Task 8.3: a frozen JVM must not freeze anything but its own subtree

#[allow(unsafe_code)]
fn set_process_frozen(pid: u32, frozen: bool) {
    use windows::Win32::System::Diagnostics::Debug::{
        DebugActiveProcess, DebugActiveProcessStop, DebugSetProcessKillOnExit,
    };
    // SAFETY: plain debugger attach/detach on a process we own; kill-on-exit is
    // disabled so a panicking test does not tear the fixture down twice.
    unsafe {
        if frozen {
            DebugActiveProcess(pid).expect("DebugActiveProcess");
            let _ = DebugSetProcessKillOnExit(false);
        } else {
            DebugActiveProcessStop(pid).expect("DebugActiveProcessStop");
        }
    }
}

#[test]
#[ignore = "needs a desktop, a JDK on PATH, and the built Swing fixture; may be run manually if flaky in CI"]
fn live_frozen_jvm_stays_contained() {
    const CALL_TIMEOUT: Duration = Duration::from_millis(750);

    let app = FixtureApp::launch("frozen");
    let providers = ConfigMap::new()
        .with("jab", ConfigMap::new().with("call_timeout_ms", i64::try_from(CALL_TIMEOUT.as_millis()).expect("fits")));
    let config = RuntimeConfig::new(ConfigMap::new(), providers);
    let provider = build_provider(&config);
    let parent = desktop_stub();

    let window = wait_for_window(&provider, &parent, &app.title);
    assert!(window.is_valid());

    // Freeze every thread of the JVM (debugger attach) — the bridge can no
    // longer answer, exactly like a hung event-dispatch thread.
    set_process_frozen(app.pid(), true);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // First calls run into the deadline (the pump is stuck inside the OS
        // call, the caller gets its timeout error promptly)…
        let start = Instant::now();
        window.invalidate();
        assert!(!window.is_valid(), "a frozen JVM must not report a valid node");
        let elapsed = start.elapsed();
        assert!(
            elapsed < CALL_TIMEOUT * 4 + Duration::from_secs(1),
            "timeout must be bounded by the configured deadline, took {elapsed:?}"
        );

        // …and after enough consecutive timeouts the vm is degraded: calls now
        // fail fast instead of waiting out the deadline each time.
        for _ in 0..3 {
            window.invalidate();
            let _ = window.is_valid();
        }
        let start = Instant::now();
        window.invalidate();
        let _ = window.is_valid();
        assert!(start.elapsed() < CALL_TIMEOUT / 2, "degraded vm must fail fast, took {:?}", start.elapsed());

        // Other providers stay usable while the JAB pump is wedged: a UIA
        // desktop enumeration answers normally.
        let uia =
            platynui_provider_windows_uia::WindowsUiaFactory.create(&RuntimeConfig::default()).expect("uia provider");
        let start = Instant::now();
        let count = uia.get_nodes(desktop_stub()).expect("uia get_nodes").count();
        assert!(count > 0, "UIA must still see desktop windows");
        assert!(start.elapsed() < Duration::from_secs(20), "UIA enumeration must not hang");
        uia.shutdown();
    }));
    // Always thaw, even when an assertion above failed.
    set_process_frozen(app.pid(), false);
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }

    // Recovery: once the JVM answers again, the health probe clears the
    // degraded flag and the node becomes usable.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        window.invalidate();
        if window.is_valid() {
            break;
        }
        assert!(Instant::now() < deadline, "vm did not recover after thawing");
        std::thread::sleep(Duration::from_millis(500));
    }

    provider.shutdown();
}
