//! Live real-provider checks against the Swing fixture app.
//!
//! These tests need a desktop session, a Java runtime (the explicit selection
//! in `PLATYNUI_TEST_APP_SWING_JAVA`, or `java` on `PATH`), and the built
//! fixture app (`just build-test-app-swing`); they are `#[ignore]`d so the
//! plain `just test` lane stays desktop-free. The Windows acceptance recipe
//! runs them explicitly:
//!
//! ```text
//! cargo nextest run -p platynui-provider-jab --run-ignored ignored-only
//! ```
//!
//! `live_fixture_contract_and_interaction` covers `OpenSpec` `add-jab-provider`
//! task 4.5 (core contract testkit against a live node set) plus the
//! `TextEditable` marker semantics and the Closeable delegation;
//! `live_frozen_jvm_stays_contained` covers task 8.3 (robustness against an
//! unresponsive JVM — may be run manually if it proves flaky in CI).

// Integration-test ergonomics: the scenarios are long and linear and define
// their expectation constants next to where they are used. Neither pedantic
// lint catches bugs here.
#![allow(clippy::too_many_lines, clippy::items_after_statements)]
// On non-Windows targets the `cfg` below strips the whole crate body, leaving
// every dev-dependency of this test target unused. These attributes must stay
// above the `cfg` — inner attributes after it get stripped too.
#![cfg_attr(not(windows), allow(unused_crate_dependencies))]
#![cfg(windows)]

use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_core::platform::platform_factories;
use platynui_core::provider::{UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::contract::testkit::{AttributeExpectation, NodeExpectation, PatternExpectation, verify_node};
use platynui_core::ui::{
    Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiValue, attribute_names, pattern_names,
    validate_control_or_item,
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
        Self::launch_with_bridge(title_suffix, true)
    }

    /// Launch the fixture JVM with the Access Bridge explicitly enabled or
    /// disabled. `bridge: false` sets an **empty**
    /// `javax.accessibility.assistive_technologies` — system properties
    /// override the persistent properties file, so this disables the bridge
    /// for the fixture regardless of any `jabswitch -enable` state on the
    /// machine.
    fn launch_with_bridge(title_suffix: &str, bridge: bool) -> Self {
        let classes = swing_classes_dir();
        assert!(
            classes.is_dir(),
            "Swing fixture classes not found at {} — run `just build-test-app-swing` first",
            classes.display()
        );
        let assistive_technologies = if bridge {
            "-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge"
        } else {
            "-Djavax.accessibility.assistive_technologies="
        };
        let title = format!("PlatynUI JAB Live {} {}", std::process::id(), title_suffix);
        let child = Command::new(swing_java_launcher())
            .arg(assistive_technologies)
            .arg("-cp")
            .arg(&classes)
            .arg("platynui.testapp.Main")
            .arg("--title")
            .arg(&title)
            .arg("--auto-close")
            .arg("180")
            .spawn()
            .expect("failed to launch the fixture JVM — set PLATYNUI_TEST_APP_SWING_JAVA or put `java` on PATH");
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
                .join("java")
                .join("main")
        },
        PathBuf::from,
    )
}

/// Launch JVM for the fixture: the explicit runtime selection from the
/// acceptance recipe (the provisioned Java 8), or the PATH `java` for ad-hoc
/// local runs.
fn swing_java_launcher() -> PathBuf {
    std::env::var_os("PLATYNUI_TEST_APP_SWING_JAVA").map_or_else(|| PathBuf::from("java"), PathBuf::from)
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
#[ignore = "needs a desktop, a Java runtime, and the built Swing fixture (run via just test-acceptance-windows)"]
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
    // pattern resolves to a concrete instance — except deliberate capability
    // markers, which are advertised without one (the node contract allows this;
    // see `pattern_names::TEXT_EDITABLE`) — control/item nodes validate, and the
    // identifying attributes are present.
    const CAPABILITY_MARKERS: &[&str] = &[pattern_names::TEXT_EDITABLE];
    for node in &nodes {
        if matches!(node.namespace(), Namespace::Control | Namespace::Item) {
            validate_control_or_item(node.as_ref()).expect("core node contract");
        }
        for pattern in node.supported_patterns() {
            if CAPABILITY_MARKERS.contains(&pattern.as_str()) {
                assert!(
                    node.pattern_by_name(&pattern).is_none(),
                    "capability marker {pattern} must not carry an instance on {}",
                    node.runtime_id()
                );
                continue;
            }
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

    // TextEditable is a capability marker: advertised with editability
    // metadata, deliberately without a pattern instance. Text entry itself is
    // keyboard-driven and covered by the Swing acceptance lane.
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
    assert_eq!(
        attribute_value(textfield, attribute_names::text_editable::IS_READ_ONLY),
        Some(UiValue::from(false)),
        "the editable fixture field must report IsReadOnly = false"
    );
    assert!(
        textfield.pattern_by_name(&PatternName::from(pattern_names::TEXT_EDITABLE)).is_none(),
        "TextEditable must stay a marker — no programmatic set-text action"
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

    // Interface-attribute projection (jab-interface-attributes): the two-tier
    // split — container-level `native:<Interface>.*` in the enumeration, the
    // expensive per-cell `TableCell.*` tier only via targeted lookup, so the
    // full walks above never issued a per-cell call.
    let native_names = |node: &Arc<dyn UiNode>| -> Vec<String> {
        node.attributes()
            .filter(|attr| attr.namespace() == Namespace::Native)
            .map(|attr| attr.name().to_string())
            .collect()
    };
    let native_value =
        |node: &Arc<dyn UiNode>, name: &str| node.attribute(Namespace::Native, name).map(|attr| attr.value());

    let table = find_by_name(&nodes, "main-table");
    assert_eq!(table.role(), "Table");
    let table_attrs = native_names(table);
    for expected in ["Table.RowCount", "Table.ColumnCount", "Table.SelectedRowCount", "Table.SelectedColumnCount"] {
        assert!(table_attrs.iter().any(|name| name == expected), "table must enumerate {expected}: {table_attrs:?}");
    }
    assert!(
        table_attrs.iter().all(|name| !name.starts_with("TableCell.")),
        "the table is not a table child itself and must not list TableCell.*: {table_attrs:?}"
    );
    assert_eq!(native_value(table, "Table.RowCount"), Some(UiValue::from(4i64)), "fixture table is 4x3");
    assert_eq!(native_value(table, "Table.ColumnCount"), Some(UiValue::from(3i64)));
    assert_eq!(native_value(table, "Table.SelectedRowCount"), Some(UiValue::from(1i64)), "row 2 is preselected");

    // A data cell answers the per-cell tier through attribute() only. Cells
    // are addressed by enumeration position, never by name: the JDK bridge
    // aliases every JTable cell to the shared renderer component, so cell
    // names are volatile (last-configured-wins) — the coordinate-based
    // TableCell.* attributes are the stable identity.
    let cells: Vec<Arc<dyn UiNode>> = table.children().collect();
    assert_eq!(cells.len(), 12, "4x3 fixture table must expose one child per cell");
    let cell = &cells[5]; // row-major: index 5 = (row 1, column 2), holds "r1c2"
    // Cells LIST their per-cell attributes (so enumeration consumers like the
    // Inspector's attribute panel see them); the values still resolve lazily.
    let cell_attrs = native_names(cell);
    for expected in ["TableCell.Row", "TableCell.Column", "TableCell.IsSelected"] {
        assert!(cell_attrs.iter().any(|name| name == expected), "cell must list {expected}: {cell_attrs:?}");
    }
    assert_eq!(native_value(cell, "TableCell.Row"), Some(UiValue::from(1i64)));
    assert_eq!(native_value(cell, "TableCell.Column"), Some(UiValue::from(2i64)));
    assert_eq!(native_value(cell, "TableCell.Index"), Some(UiValue::from(5i64)));
    assert_eq!(native_value(cell, "TableCell.IsSelected"), Some(UiValue::from(false)));
    let selected_cell = &cells[7]; // (row 2, column 1) — row 2 is preselected
    assert_eq!(native_value(selected_cell, "TableCell.Row"), Some(UiValue::from(2i64)));
    assert_eq!(native_value(selected_cell, "TableCell.IsSelected"), Some(UiValue::from(true)));

    // Bitfield gate: a plain label supports none of Table/Value/Text, so none
    // of those attributes exist on it — neither enumerated nor by lookup —
    // and TableCell.* is omitted because its parent is not a table.
    let label = find_by_name(&nodes, "stage1-status-clicks-0");
    let label_attrs = native_names(label);
    for prefix in ["Table.", "TableCell.", "Value.", "Text."] {
        assert!(
            label_attrs.iter().all(|name| !name.starts_with(prefix)),
            "label must not enumerate {prefix}*: {label_attrs:?}"
        );
    }
    assert!(label.attribute(Namespace::Native, "Table.RowCount").is_none(), "gate must veto the targeted lookup");
    assert!(label.attribute(Namespace::Native, "TableCell.Row").is_none(), "parent is not a table");

    // The slider's native Value.* mirrors the StatefulValue readings above.
    assert_eq!(native_value(slider, "Value.Current"), Some(UiValue::from(50.0)), "fixture slider default");
    assert_eq!(native_value(slider, "Value.Minimum"), Some(UiValue::from(0.0)));
    assert_eq!(native_value(slider, "Value.Maximum"), Some(UiValue::from(100.0)));

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
#[ignore = "needs a desktop, a Java runtime, and the built Swing fixture; may be run manually if flaky in CI"]
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

    // Captured while the JVM is still responsive: the window's desktop bounds
    // feed the hit-test points below, and raising the window makes its center
    // actually resolve to it under `WindowFromPoint`.
    let Some(UiValue::Rect(bounds)) = attribute_value(&window, attribute_names::element::BOUNDS) else {
        panic!("fixture window must expose Rect bounds");
    };
    let center = bounds.center();
    use platynui_core::ui::ActivatablePattern as _;
    if let Some(activatable) = window.pattern::<platynui_core::ui::ActivatableAction>() {
        let _ = activatable.activate();
    }

    // Captured while the JVM is still responsive: the slider answers its
    // native interface attribute normally — the same lookup must degrade to
    // absence (bounded) once the JVM freezes.
    let mut live_nodes = Vec::new();
    walk(&window, &mut live_nodes, 0);
    let slider = Arc::clone(find_by_name(&live_nodes, "stage2-slider"));
    drop(live_nodes);
    assert!(
        matches!(
            slider.attribute(Namespace::Native, "Value.Current").map(|attr| attr.value()),
            Some(UiValue::Number(_))
        ),
        "live slider must answer native:Value.Current"
    );

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

        // Hit-testing during the freeze is bounded the same way
        // (add-jab-hit-test): `element_at_point` runs its bridge calls on the
        // wedged pump under the per-call deadline, so the picker gets a prompt
        // error (or no JAB hit) instead of a hang.
        let start = Instant::now();
        let hit = provider.element_at_point(center);
        assert!(!matches!(hit, Ok(Some(_))), "a frozen JVM must not produce a hit node");
        let elapsed = start.elapsed();
        assert!(
            elapsed < CALL_TIMEOUT * 4 + Duration::from_secs(1),
            "hit-test against a frozen JVM must return within the deadline margin, took {elapsed:?}"
        );

        // Interface attributes degrade to absence, not to a hang
        // (jab-interface-attributes): the targeted lookup gates on a live
        // info snapshot the frozen JVM cannot answer — with the vm already
        // degraded it fails fast, well inside the deadline margin.
        let start = Instant::now();
        let value_attr = slider.attribute(Namespace::Native, "Value.Current");
        assert!(value_attr.is_none(), "a frozen JVM must not surface interface attributes");
        let elapsed = start.elapsed();
        assert!(
            elapsed < CALL_TIMEOUT * 4 + Duration::from_secs(1),
            "interface-attribute lookup against a frozen JVM must be bounded, took {elapsed:?}"
        );

        // Other providers stay usable while the JAB pump is wedged: a UIA
        // desktop enumeration answers normally.
        let uia =
            platynui_provider_windows_uia::WindowsUiaFactory.create(&RuntimeConfig::default()).expect("uia provider");
        let start = Instant::now();
        let count = uia.get_nodes(desktop_stub()).expect("uia get_nodes").count();
        assert!(count > 0, "UIA must still see desktop windows");
        assert!(start.elapsed() < Duration::from_secs(20), "UIA enumeration must not hang");

        // …and a concurrent UIA hit-test elsewhere (outside the claimed Java
        // window) completes normally while the JAB pump is wedged. Off-screen
        // coordinates degrade to `Ok(None)`, which still counts as prompt
        // completion.
        let outside = platynui_core::types::Point::new(bounds.right() + 40.0, bounds.bottom() + 40.0);
        let start = Instant::now();
        let result = uia.element_at_point(outside);
        assert!(result.is_ok(), "UIA hit-test outside the Java window must not fail: {:?}", result.as_ref().err());
        assert!(start.elapsed() < Duration::from_secs(10), "UIA hit-test must not hang");
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

// ---------------------------------------------------------------------------
// java-app-classifier task 4.1: classification facts + the shared diagnostic

/// The fixture window's HWND, resolved via `FindWindowW` by its unique title,
/// as the raw claim/diagnostic key (`window_claims` / `platform::java` semantics).
#[allow(unsafe_code)]
fn wait_for_native_window(title: &str) -> u64 {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    use windows::core::{HSTRING, PCWSTR};

    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    loop {
        // SAFETY: read-only top-level window lookup by title.
        if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), &HSTRING::from(title)) }
            && !hwnd.is_invalid()
        {
            return hwnd.0 as u64;
        }
        assert!(Instant::now() < deadline, "fixture window {title:?} did not appear on the native desktop");
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn native_value(node: &Arc<dyn UiNode>, name: &str) -> Option<UiValue> {
    node.attribute(Namespace::Native, name).map(|attr| attr.value())
}

#[test]
#[ignore = "needs a desktop, a Java runtime, and the built Swing fixture (run via just test-acceptance-windows)"]
fn live_jvm_classification_facts_and_diagnostic() {
    use platynui_core::platform::java::{
        IS_JVM_ATTRIBUTE, JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE, JVM_TOOLKIT_ATTRIBUTE,
        jvm_unreachable_diagnostic_emitted,
    };

    // Bridge on: the JAB window node carries the JVM+Swing+reachable facts.
    {
        let app = FixtureApp::launch("classify-on");
        let provider = build_provider(&RuntimeConfig::default());
        let parent = desktop_stub();
        let window = wait_for_window(&provider, &parent, &app.title);
        let hwnd = wait_for_native_window(&app.title);
        assert_eq!(native_value(&window, IS_JVM_ATTRIBUTE), Some(UiValue::from(true)));
        assert_eq!(native_value(&window, JVM_TOOLKIT_ATTRIBUTE), Some(UiValue::from("Swing/AWT")));
        assert_eq!(native_value(&window, JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE), Some(UiValue::from(true)));
        assert!(
            !jvm_unreachable_diagnostic_emitted(hwnd),
            "a bridge-served window must not trigger the enablement diagnostic"
        );
        provider.shutdown();
    }

    // Bridge off: the JAB provider surfaces no node for the window, fires the
    // shared "absent from native accessibility" diagnostic (at most once —
    // the loop below enumerates repeatedly), and the UIA shell carries the
    // JVM+Swing+not-reachable facts through the platform classifier.
    let app = FixtureApp::launch_with_bridge("classify-off", false);
    let provider = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();
    let hwnd = wait_for_native_window(&app.title);

    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    loop {
        let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("jab get_nodes").collect();
        assert!(
            nodes.iter().all(|node| node.name() != app.title),
            "a bridge-less Swing window must not surface as a JAB node"
        );
        if jvm_unreachable_diagnostic_emitted(hwnd) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the enablement diagnostic did not fire for the bridge-less window within {DISCOVERY_DEADLINE:?}"
        );
        std::thread::sleep(Duration::from_millis(250));
    }

    // The UIA provider with the platform classifier injected (the runtime
    // does this wiring from the platform bundle; mirrored here).
    let uia = platynui_provider_windows_uia::WindowsUiaFactory.create(&RuntimeConfig::default()).expect("uia provider");
    let bundle = platform_factories()
        .find(|factory| factory.id() == "windows")
        .expect("windows platform factory")
        .create(&RuntimeConfig::default())
        .expect("windows platform bundle");
    uia.set_java_classifier(bundle.java_classifier.expect("the Windows bundle carries a Java classifier"));

    let uia_window = wait_for_window(&uia, &parent, &app.title);
    assert_eq!(native_value(&uia_window, IS_JVM_ATTRIBUTE), Some(UiValue::from(true)), "jvm.dll module signal");
    assert_eq!(native_value(&uia_window, JVM_TOOLKIT_ATTRIBUTE), Some(UiValue::from("Swing/AWT")));
    assert_eq!(
        native_value(&uia_window, JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE),
        Some(UiValue::from(false)),
        "an unclaimed Swing window is not reachable through native accessibility"
    );

    uia.shutdown();
    provider.shutdown();
}
