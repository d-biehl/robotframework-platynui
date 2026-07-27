//! Live real-provider checks against the Swing fixture app, through the Java
//! provider and its JAB backend.
//!
//! These tests need a desktop session, a Java runtime (the explicit selection
//! in `PLATYNUI_TEST_APP_SWING_JAVA`, or `java` on `PATH`), and the built
//! fixture app (`just build-test-app-swing`); they are `#[ignore]`d so the
//! plain `just test` lane stays desktop-free. The Windows acceptance recipe
//! runs them explicitly:
//!
//! ```text
//! cargo nextest run -p platynui-provider-java --run-ignored ignored-only
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
use platynui_provider_java::JavaFactory;
// Force-link the Windows platform crate so its inventory-registered platform
// factory (id "windows") is available to `build_provider`.
use platynui_platform_windows as _;
use std::path::PathBuf;

// Crate dependencies of the library that this integration-test target does
// not use directly (`unused_crate_dependencies` is target-scoped).
use inventory as _;
use platynui_java_agent as _;
use platynui_provider_java_jab as _;
use serde as _;
use serde_json as _;
use std::process::{Child, Command};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
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
        Self::launch_with(title_suffix, bridge, &[])
    }

    /// Launch the fixture JVM carrying the `PlatynUI` agent — the state the
    /// agent-presence classification fact exists to report.
    fn launch_with_agent(title_suffix: &str) -> Self {
        let jar = std::env::var_os("PLATYNUI_JAVA_AGENT_JAR").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("java")
                    .join("agent")
                    .join("build")
                    .join("libs")
                    .join("platynui-agent.jar")
            },
            PathBuf::from,
        );
        assert!(jar.is_file(), "agent JAR not found at {} — run `just build-java-agent` first", jar.display());
        Self::launch_with(title_suffix, true, &[format!("-javaagent:{}", jar.display())])
    }

    fn launch_with(title_suffix: &str, bridge: bool, extra_jvm_args: &[String]) -> Self {
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
            .args(extra_jvm_args)
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

    /// Ends the JVM and waits for it, so a test that asserts on the aftermath is
    /// not racing the process teardown.
    fn kill_and_wait(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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

/// Config for a test that is *about the Access Bridge*.
///
/// Automatic attachment is on by default, and on purpose: a Java window whose JVM
/// has no agent gets one, because the agent's representation is the better one.
/// That makes "no agent" a state which does not persist — so a suite that wants
/// to verify the bridge has to say so, rather than relying on an absence the
/// provider is actively working to remove.
fn jab_only() -> RuntimeConfig {
    let providers =
        ConfigMap::new().with("java", ConfigMap::new().with("agent", ConfigMap::new().with("enabled", false)));
    RuntimeConfig::new(ConfigMap::new(), providers)
}

fn build_provider(config: &RuntimeConfig) -> Arc<dyn UiTreeProvider> {
    let provider = JavaFactory.create(config).expect("provider construction is infallible");
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
    let provider = build_provider(&jab_only());
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
    assert_eq!(native_value(table, "Table.RowCount"), Some(UiValue::from(100i64)), "fixture table is 100x6");
    assert_eq!(native_value(table, "Table.ColumnCount"), Some(UiValue::from(6i64)));
    assert_eq!(native_value(table, "Table.SelectedRowCount"), Some(UiValue::from(1i64)), "row 2 is preselected");

    // A data cell answers the per-cell tier through attribute() only. Cells
    // are addressed by enumeration position, never by name: the JDK bridge
    // aliases every JTable cell to the shared renderer component, so cell
    // names are volatile (last-configured-wins) — the coordinate-based
    // TableCell.* attributes are the stable identity.
    let cells: Vec<Arc<dyn UiNode>> = table.children().collect();
    // Flat and complete: the bridge reports every cell of the model as a direct
    // child, scrolled into view or not. That is the shape this backend has and
    // must keep having — the row level exists only through the agent.
    assert_eq!(cells.len(), 600, "100x6 fixture table must expose one child per cell");
    let cell = &cells[8]; // row-major: index 1*6 + 2 = (row 1, column 2), holds "r1c2"
    // Cells LIST their per-cell attributes (so enumeration consumers like the
    // Inspector's attribute panel see them); the values still resolve lazily.
    let cell_attrs = native_names(cell);
    for expected in ["TableCell.Row", "TableCell.Column", "TableCell.IsSelected"] {
        assert!(cell_attrs.iter().any(|name| name == expected), "cell must list {expected}: {cell_attrs:?}");
    }
    assert_eq!(native_value(cell, "TableCell.Row"), Some(UiValue::from(1i64)));
    assert_eq!(native_value(cell, "TableCell.Column"), Some(UiValue::from(2i64)));
    assert_eq!(native_value(cell, "TableCell.Index"), Some(UiValue::from(8i64)));
    assert_eq!(native_value(cell, "TableCell.IsSelected"), Some(UiValue::from(false)));
    let selected_cell = &cells[13]; // 2*6 + 1 = (row 2, column 1) — row 2 is preselected
    assert_eq!(native_value(selected_cell, "TableCell.Row"), Some(UiValue::from(2i64)));
    assert_eq!(native_value(selected_cell, "TableCell.IsSelected"), Some(UiValue::from(true)));
    // Far below the fold: the bridge answers from the model, so a cell nobody
    // scrolled to still knows where it sits.
    let offscreen = &cells[90 * 6 + 3];
    assert_eq!(native_value(offscreen, "TableCell.Row"), Some(UiValue::from(90i64)));
    assert_eq!(native_value(offscreen, "TableCell.Column"), Some(UiValue::from(3i64)));

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
    // Bridge-only, like every other JAB scenario here (see `jab_only`), plus a
    // short per-call deadline so a frozen JVM surfaces quickly. The agent has its
    // own containment mechanism — a degraded session that fails fast — and mixing
    // the two would test neither.
    let providers = ConfigMap::new().with(
        platynui_provider_java::PROVIDER_ID,
        ConfigMap::new()
            .with(
                "jab",
                ConfigMap::new().with("call_timeout_ms", i64::try_from(CALL_TIMEOUT.as_millis()).expect("fits")),
            )
            .with("agent", ConfigMap::new().with("enabled", false)),
    );
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

/// The other half of the agent-presence fact: a JVM that *does* carry an agent
/// must say so. Checked against the platform classifier directly — the fact is
/// the classifier's, not any one provider's.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR"]
fn live_agent_presence_is_reported_for_an_instrumented_jvm() {
    use platynui_core::platform::WindowId;

    let app = FixtureApp::launch_with_agent("agent-present");
    let hwnd = wait_for_native_window(&app.title);
    let classifier = platform_factories()
        .find(|factory| factory.id() == "windows")
        .expect("windows platform factory")
        .create(&RuntimeConfig::default())
        .expect("windows platform bundle")
        .java_classifier
        .expect("the Windows bundle carries a Java classifier");

    // The agent publishes its handshake file a moment after the window exists,
    // so this is "eventually", not "immediately".
    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    loop {
        let classification = classifier.classify(WindowId::new(hwnd), app.pid()).expect("classify");
        assert!(classification.is_jvm);
        if classification.agent_present == Some(true) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "an agent-carrying JVM never reported agent-present within {DISCOVERY_DEADLINE:?} \
             (last: {:?})",
            classification.agent_present
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "needs a desktop, a Java runtime, and the built Swing fixture (run via just test-acceptance-windows)"]
fn live_jvm_classification_facts_and_diagnostic() {
    use platynui_core::platform::java::{
        IS_JVM_ATTRIBUTE, JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE, JVM_AGENT_PRESENT_ATTRIBUTE, JVM_TOOLKIT_ATTRIBUTE,
        jvm_unreachable_diagnostic_emitted,
    };

    // Bridge on: the JAB window node carries the JVM+Swing+reachable facts.
    {
        let app = FixtureApp::launch("classify-on");
        let provider = build_provider(&jab_only());
        let parent = desktop_stub();
        let window = wait_for_window(&provider, &parent, &app.title);
        let hwnd = wait_for_native_window(&app.title);
        assert_eq!(native_value(&window, IS_JVM_ATTRIBUTE), Some(UiValue::from(true)));
        assert_eq!(native_value(&window, JVM_TOOLKIT_ATTRIBUTE), Some(UiValue::from("Swing/AWT")));
        assert_eq!(native_value(&window, JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE), Some(UiValue::from(true)));
        // The fixture is launched without `-javaagent`, so "no agent here" is
        // the correct fact — and it must be *reported*, not left out: that is
        // what turns an empty window into an actionable diagnostic.
        assert_eq!(native_value(&window, JVM_AGENT_PRESENT_ATTRIBUTE), Some(UiValue::from(false)));
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
    let provider = build_provider(&jab_only());
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
    assert_eq!(
        native_value(&uia_window, JVM_AGENT_PRESENT_ATTRIBUTE),
        Some(UiValue::from(false)),
        "the same fact must be observable no matter which provider serves the window"
    );

    uia.shutdown();
    provider.shutdown();
}

// ---------------------------------------------------------------------------
// The agent backend end to end (tasks 4.1 and 4.3)

/// The change's whole reason to exist, proven through the provider rather than
/// against the wire: a `JTable` cell served by the agent backend has its own
/// name, its own bounds, its own selection state, and an identity-stable
/// `RuntimeId` — none of which the Access Bridge can give it, because the JDK
/// aliases every cell to one shared renderer component.
///
/// Also proves the routing: the same window would be reachable through JAB (the
/// fixture runs with the bridge on), and it must surface **once**, through the
/// stronger backend.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR"]
fn live_agent_serves_table_cells_the_bridge_cannot() {
    let app = FixtureApp::launch_with_agent("agent-cells");
    let provider = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();

    // Poll: the agent publishes its handshake file, connects and finds a window
    // a moment after the JVM is up.
    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    let window = loop {
        let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
        let agent_windows: Vec<_> = nodes
            .iter()
            .filter(|node| {
                node.name() == app.title && attribute_value(node, "Technology") == Some(UiValue::from("JavaAgent"))
            })
            .cloned()
            .collect();
        if let Some(window) = agent_windows.first() {
            // One window, one representation: the bridge can reach this window
            // too, and must not have contributed a second node for it.
            let same_title = nodes.iter().filter(|node| node.name() == app.title).count();
            assert_eq!(
                same_title, 1,
                "an agent-served window must appear exactly once, not once per backend that can reach it"
            );
            break window.clone();
        }
        assert!(Instant::now() < deadline, "no agent-served window for {:?} within {DISCOVERY_DEADLINE:?}", app.title);
        std::thread::sleep(Duration::from_millis(250));
    };

    // The core node contract, over the agent's whole tree.
    //
    // This is the check that was missing: the contract testkit only ever ran
    // against JAB nodes, so nothing verified that agent nodes carry what every
    // `control:`/`item:` node must — `SupportedPatterns` above all, which a
    // consumer uses to find out what a node can do before trying it.
    {
        const CAPABILITY_MARKERS: &[&str] = &[pattern_names::TEXT_EDITABLE];
        let mut all = Vec::new();
        walk(&window, &mut all, 0);
        assert!(all.len() >= 20, "expected the full fixture tree, got {} nodes", all.len());

        // An empty text field still *is* a text field. `control:Text` is the
        // sentinel the client layer derives the TextContent capability from
        // (`_ATTRIBUTE_ONLY_PATTERNS`), so it has to be present with an empty
        // value rather than omitted — otherwise an empty input is indistinguishable
        // from a label, and `supports_pattern(TextContent)` answers false on
        // something the user can type into.
        let field = find_by_name(&all, "stage1-textfield");
        assert_eq!(
            attribute_value(field, "Text"),
            Some(UiValue::from("")),
            "an empty text field must still expose control:Text"
        );
        assert_eq!(
            attribute_value(field, "IsReadOnly"),
            Some(UiValue::from(false)),
            "and pair it with the editability marker"
        );

        // A cell's name is its *model value*, which is content — not a
        // developer-provided identifier. Publishing it as `control:Id` would
        // promise a stability that editing the data breaks.
        let cell = find_by_name(&all, "r2c0");
        assert_eq!(cell.id(), None, "a table cell's model value must not be published as control:Id");

        // A table's children are its **rows**, and each row holds its cells.
        // The flat, row-major cell list the bridge reports is not a model, it is
        // what `AccessibleContext.getAccessibleChild(i)` happens to offer; the
        // agent reads the toolkit's own model, where a row is a first-class
        // thing — as it already is for `provider-atspi` and `provider-windows-uia`.
        let table = find_by_name(&all, "main-table");
        let rows: Vec<Arc<dyn UiNode>> = table.children().collect();
        assert_eq!(rows.len(), 100, "the 100x6 fixture table has one child per row, not 600 direct cells");
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.role(), "TableRow", "a table's children are rows");
            assert_eq!(row.namespace(), Namespace::Item);
            let position = i64::try_from(index).expect("fixture row index");
            assert_eq!(native_value(row, "TableRow.Index"), Some(UiValue::Integer(position)));
            // A row carries no label of its own, and joining its cells' values
            // would invent an identifier that changes when any cell does.
            assert_eq!(row.id(), None, "a row has no developer-provided identifier");
        }

        // Cells in detail on three rows: the first, the preselected one, and one
        // far below the fold. Six hundred of them would prove nothing the three
        // do not — except how long a walk takes.
        for index in [0usize, 2, 90] {
            let position = i64::try_from(index).expect("fixture row index");
            let cells: Vec<Arc<dyn UiNode>> = rows[index].children().collect();
            assert_eq!(cells.len(), 6, "row {index} must hold one cell per column");
            for (column, cell) in cells.iter().enumerate() {
                let column_position = i64::try_from(column).expect("fixture column index");
                assert_eq!(cell.role(), "TableCell", "a row's children are cells");
                assert_eq!(cell.name(), format!("r{index}c{column}"), "the model value, in model order");
                assert_eq!(native_value(cell, "TableCell.Row"), Some(UiValue::Integer(position)));
                assert_eq!(native_value(cell, "TableCell.Column"), Some(UiValue::Integer(column_position)));
                assert_eq!(
                    native_value(cell, "TableCell.IsSelected"),
                    Some(UiValue::from(index == 2)),
                    "row 2 is preselected, and cell-level selection stays on the cell"
                );
            }
        }

        // The fixture table does not fit its viewport, and that is the point.
        // A row scrolled out of view must report **no** bounds and must not
        // claim to be in view: its model rectangle is real but sits far below
        // the window, so publishing it would aim the pointer at whatever
        // happens to be there — the same reason an unlaid-out menu item
        // reports nothing rather than an empty rectangle at its owner's corner.
        let below_the_fold = &rows[90];
        assert_eq!(attribute_value(below_the_fold, "Bounds"), None, "a row nobody scrolled to has no place on screen");
        assert_eq!(attribute_value(below_the_fold, "ActivationPoint"), None, "and therefore nothing to aim at");
        assert_eq!(attribute_value(below_the_fold, "IsInView"), Some(UiValue::from(false)));
        let hidden_cell = &below_the_fold.children().next().expect("a scrolled-out row still has its cells");
        assert_eq!(attribute_value(hidden_cell, "Bounds"), None, "nor do its cells");
        assert_eq!(attribute_value(hidden_cell, "IsInView"), Some(UiValue::from(false)));
        // It is still *there*, though — name, coordinates and identity all hold.
        assert_eq!(hidden_cell.name(), "r90c0");
        assert_eq!(native_value(hidden_cell, "TableCell.Row"), Some(UiValue::Integer(90)));

        // A row has an on-screen rectangle spanning the cells it contains, so it
        // is something a user could point at rather than a bookkeeping node.
        let selected_row = &rows[2];
        let UiValue::Rect(row_rect) = attribute_value(selected_row, "Bounds").expect("a row has bounds") else {
            panic!("Bounds must be a rectangle");
        };
        let row_cells: Vec<Arc<dyn UiNode>> = selected_row.children().collect();
        let cell_rect = |node: &Arc<dyn UiNode>| match attribute_value(node, "Bounds") {
            Some(UiValue::Rect(rect)) => rect,
            other => panic!("a cell must have rectangular bounds, got {other:?}"),
        };
        // Row and cells alike are clipped to the viewport, so the span holds
        // over the columns that are in view — and the ones past the right edge
        // have no rectangle at all, for the same reason rows below the fold do not.
        let first = cell_rect(&row_cells[0]);
        let last = cell_rect(&row_cells[3]);
        assert!(
            row_rect.x() <= first.x() && row_rect.right() >= last.right(),
            "the row must span its cells horizontally: row {row_rect:?}, cells {first:?}..{last:?}"
        );
        assert!(row_rect.height() >= first.height(), "and be at least as tall as a cell: {row_rect:?}");
        assert_eq!(
            attribute_value(&row_cells[5], "Bounds"),
            None,
            "a column scrolled out to the right has no rectangle either"
        );

        // Row selection, on the row that has it.
        assert_eq!(native_value(selected_row, "TableRow.IsSelected"), Some(UiValue::from(true)));
        // …and on the normalised Selectable surface, not only the native one. In
        // the node's own namespace, which for a row is `item:` — the convention
        // this backend already follows for cells.
        assert_eq!(
            selected_row.attribute(Namespace::Item, "IsSelected").map(|attr| attr.value()),
            Some(UiValue::from(true))
        );
        assert_eq!(native_value(&rows[0], "TableRow.IsSelected"), Some(UiValue::from(false)));

        // Identity: the same row is the same node across enumerations, which is
        // what an interned `(table, row)` key buys over a positional scheme.
        let row_ids: Vec<String> = rows.iter().map(|row| row.runtime_id().as_str().to_owned()).collect();
        let again: Vec<String> = table.children().map(|row| row.runtime_id().as_str().to_owned()).collect();
        assert_eq!(again, row_ids, "a row must keep its identity across enumerations of an unchanged table");

        // `SelectedItems` has to name nodes that exist. Ids assembled from a
        // parallel scheme look like an answer and resolve to nothing — and with a
        // row level the accessible child index no longer addresses a direct
        // child, so the selection is re-derived from the table's own model.
        let UiValue::Array(selected) = attribute_value(table, "SelectedItems").expect("SelectedItems") else {
            panic!("SelectedItems must be a list");
        };
        assert!(!selected.is_empty(), "row 2 is preselected in the fixture");
        let known: Vec<String> = all.iter().map(|node| node.runtime_id().as_str().to_owned()).collect();
        for entry in &selected {
            let UiValue::String(id) = entry else { panic!("a SelectedItems entry must be a RuntimeId string") };
            assert!(known.contains(id), "SelectedItems names {id}, which is no node of this tree");
        }
        assert_eq!(
            selected,
            vec![UiValue::from(row_ids[2].clone())],
            "row selection names the selected row, not its cells"
        );

        // A column header is one of the most clickable things in a table — sorting,
        // resizing and reordering all happen there — and Swing's accessible view
        // reports it as an unlaid-out `label` with a zero-height rectangle. The
        // header component knows better, and that is what has to travel.
        let header = find_by_name(&all, "col-1");
        assert_eq!(header.role(), "ColumnHeader", "a header is not a label");
        assert_eq!(header.namespace(), Namespace::Item);
        let UiValue::Rect(header_rect) = attribute_value(header, "Bounds").expect("a header has bounds") else {
            panic!("Bounds must be a rectangle");
        };
        assert!(
            header_rect.width() > 0.0 && header_rect.height() > 0.0,
            "the header's real rectangle, not the renderer's empty one: {header_rect:?}"
        );
        assert_eq!(native_value(header, "ColumnHeader.Column"), Some(UiValue::Integer(1)));
        assert_eq!(
            native_value(header, "ColumnHeader.ModelIndex"),
            Some(UiValue::Integer(1)),
            "the model index is what survives the user reordering columns"
        );

        // An element that has never been laid out (a menu item whose popup was
        // never opened) must report *no* bounds, not an empty rectangle at its
        // owner's corner — that would aim the pointer at the menu bar and let the
        // Element capability resolve on something with no place on screen.
        let hidden = find_by_name(&all, "menu-file-exit");
        assert_eq!(attribute_value(hidden, "Bounds"), None, "an unlaid-out element has no bounds");
        assert_eq!(attribute_value(hidden, "ActivationPoint"), None, "and therefore nothing to aim at");

        for node in &all {
            if matches!(node.namespace(), Namespace::Control | Namespace::Item) {
                validate_control_or_item(node.as_ref()).expect("core node contract");
            }
            let issues = platynui_core::ui::contract::testkit::verify_common_attributes(node.as_ref());
            assert!(issues.is_empty(), "common-attribute contract violated on {}: {issues:?}", node.runtime_id());
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
        }
    }

    // The JVM classification facts, on the same `native:` names the bridge and the
    // UIA shell publish them under — so "which Java toolkit is this?" has one
    // answer regardless of who served the window. The agent's answer is the
    // authoritative one: it reads the loaded classes from inside the process,
    // where the platform classifier can only infer from a window class.
    {
        use platynui_core::platform::java::{IS_JVM_ATTRIBUTE, JVM_AGENT_PRESENT_ATTRIBUTE, JVM_TOOLKIT_ATTRIBUTE};

        assert_eq!(native_value(&window, IS_JVM_ATTRIBUTE), Some(UiValue::from(true)));
        assert_eq!(
            native_value(&window, JVM_TOOLKIT_ATTRIBUTE),
            Some(UiValue::from("Swing/AWT")),
            "the shared label, not the agent's wire spelling"
        );
        assert_eq!(
            native_value(&window, JVM_AGENT_PRESENT_ATTRIBUTE),
            Some(UiValue::from(true)),
            "we are the agent, so this is certain rather than probed"
        );
        // `@Technology` keeps naming the *channel*: it is what tells an
        // agent-served window from a bridge-served one, which the dedup and
        // routing suites depend on.
        assert_eq!(attribute_value(&window, "Technology"), Some(UiValue::from("JavaAgent")));
    }

    // The native handle came from inside the JVM, so the window patterns resolve
    // exactly rather than through the platform's PID guess.
    let handle = native_value(&window, "NativeWindowHandle");
    assert!(matches!(handle, Some(UiValue::Integer(raw)) if raw != 0), "a window handle, got {handle:?}");
    assert_eq!(
        native_value(&window, "WindowHandleSource"),
        Some(UiValue::from("sun.awt.windows.WComponentPeer#getHWnd")),
        "the in-JVM strategy is the one that should have answered"
    );

    let mut nodes = Vec::new();
    walk(&window, &mut nodes, 0);

    // Cell (2, 0) of the fixture's 4x3 table. Row 2 is preselected and the
    // fixture never changes it.
    let cell = find_by_name(&nodes, "r2c0");
    assert_eq!(cell.role(), "TableCell", "a cell is an item of its table, not the renderer's label");
    assert_eq!(cell.namespace(), Namespace::Item);
    assert_eq!(native_value(cell, "TableCell.Row"), Some(UiValue::Integer(2)), "the cell knows its own coordinates");
    assert_eq!(native_value(cell, "TableCell.Column"), Some(UiValue::Integer(0)));
    assert_eq!(
        native_value(cell, "TableCell.IsSelected"),
        Some(UiValue::from(true)),
        "row 2 is preselected in the fixture"
    );

    // Bounds: the JAB gap in one assertion. Every cell of a row must have a
    // distinct, non-empty rectangle — the bridge reports none at all.
    let bounds = attribute_value(cell, "Bounds");
    let UiValue::Rect(rect) = bounds.clone().expect("a cell must have bounds") else {
        panic!("Bounds must be a rectangle, got {bounds:?}");
    };
    assert!(rect.width() > 0.0 && rect.height() > 0.0, "an empty rectangle is not bounds: {rect:?}");
    let neighbour = find_by_name(&nodes, "r2c1");
    let UiValue::Rect(neighbour_rect) = attribute_value(neighbour, "Bounds").expect("neighbour bounds") else {
        panic!("neighbour Bounds must be a rectangle");
    };
    assert!(
        neighbour_rect.x() > rect.x(),
        "the next column must be to the right; the bridge cannot tell these apart at all"
    );

    // Hit-testing passes through the row. In-process this is a walk over
    // rectangles the toolkit already knows, so it needs no physical pointer —
    // and it must hand back the *same* objects the enumeration does, which is
    // what lets the Inspector reveal a pick in the tree it already shows.
    let picked = provider
        .element_at_point(rect.center())
        .expect("hit-test over a cell")
        .expect("a node under the cell's centre");
    assert_eq!(picked.role(), "TableCell", "the pick reaches the cell, not the table");
    assert_eq!(
        picked.runtime_id(),
        cell.runtime_id(),
        "the picked cell must be the very node the enumeration produced"
    );
    let picked_row = picked.parent().and_then(|parent| parent.upgrade()).expect("a picked cell has a parent");
    assert_eq!(picked_row.role(), "TableRow", "the chain reaches the cell by way of its row");
    assert_eq!(native_value(&picked_row, "TableRow.Index"), Some(UiValue::Integer(2)), "and it is the right row");

    // Identity stability: a second walk must hand out the same RuntimeId for the
    // same cell. This is what the enumeration-index scheme cannot promise.
    let first_id = cell.runtime_id().clone();
    let mut again = Vec::new();
    walk(&window, &mut again, 0);
    assert_eq!(
        find_by_name(&again, "r2c0").runtime_id(),
        &first_id,
        "the same cell must keep its identity across enumerations"
    );

    // And it is honest about being alive, which is what lets a scoped root be
    // reused instead of re-resolved on every access.
    assert!(cell.is_valid(), "a cell of a live window is valid");

    provider.shutdown();
}

/// The other side of the routing rule: a JVM with no agent stays with the Access
/// Bridge. Nothing about the JAB path may change just because a stronger backend
/// now exists.
#[test]
#[ignore = "needs a desktop, a Java runtime, and the built Swing fixture (run via just test-acceptance-windows)"]
fn live_automatic_attachment_off_leaves_the_window_to_the_bridge() {
    // `auto_attach = false` is the documented way to keep the agent to
    // `-javaagent`-launched targets. The backend stays enabled, so this also
    // proves the two switches are independent: no attachment, but the agent would
    // still serve a JVM that already carries one.
    let providers =
        ConfigMap::new().with("java", ConfigMap::new().with("agent", ConfigMap::new().with("auto_attach", false)));
    let app = FixtureApp::launch("no-auto-attach");
    let provider = build_provider(&RuntimeConfig::new(ConfigMap::new(), providers));
    let parent = desktop_stub();
    let window = wait_for_window(&provider, &parent, &app.title);
    assert_eq!(
        attribute_value(&window, "Technology"),
        Some(UiValue::from("JAB")),
        "with attachment off the bridge serves the window, exactly as before"
    );

    // And it stays the bridge's: nothing is injected behind the flag's back.
    std::thread::sleep(Duration::from_secs(3));
    let still: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
    let ours = still.iter().find(|node| node.name() == app.title).expect("the window is still served");
    assert_eq!(attribute_value(ours, "Technology"), Some(UiValue::from("JAB")));
    provider.shutdown();
}

/// Task 4.2: a Swing application started by its own script, with no `PlatynUI`
/// arguments, is served through the agent **without being restarted**.
///
/// This is the premise of the whole design rather than a convenience: Java
/// applications are launched by scripts, installers and Web Start, so the launch
/// line is typically not `PlatynUI`'s to change, and the Inspector's core use is
/// looking into something that is *already running* — where `-javaagent` is
/// impossible by definition.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR"]
fn live_a_running_jvm_is_attached_and_served_without_a_restart() {
    // No `-javaagent`: the fixture is launched exactly as its own script would.
    let mut app = FixtureApp::launch("auto-attach");
    let launched_pid = app.pid();
    let provider = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();

    // **The first enumeration that shows the window already shows the agent.**
    //
    // That is the property, not a nicety. The pass which discovers an agent-less
    // Java window is also the pass that injects into it, and it waits for the
    // agent to become reachable and then enumerates again — so a caller never
    // sees the weaker backend for a window that is about to be taken over. Before
    // this, the Inspector needed two refreshes: one to trigger the attach and one
    // to see its effect.
    let deadline = Instant::now() + Duration::from_secs(45);
    let window = loop {
        let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
        let ours: Vec<_> = nodes.iter().filter(|node| node.name() == app.title).collect();
        // Whatever happens, the window must never appear twice: the takeover
        // changes which backend serves it, not how many representations exist.
        assert!(ours.len() <= 1, "the window appeared {} times during the backend takeover", ours.len());
        if let Some(node) = ours.first() {
            assert_eq!(
                attribute_value(node, "Technology"),
                Some(UiValue::from("JavaAgent")),
                "the pass that first surfaces the window must already serve it through the agent — \
                 seeing JAB here means the in-pass attach did not take effect and a second \
                 enumeration would be needed"
            );
            break (*node).clone();
        }
        assert!(
            !app.has_exited(),
            "the fixture must be served in place — it exited, so something restarted or killed it"
        );
        assert!(Instant::now() < deadline, "the fixture window never appeared within 45s");
        std::thread::sleep(Duration::from_millis(250));
    };

    // The proof that nothing was restarted: same process, all along.
    assert_eq!(app.pid(), launched_pid, "the application was never restarted");
    // And the agent's fidelity is really there, not just its label.
    let mut nodes = Vec::new();
    walk(&window, &mut nodes, 0);
    let cell = find_by_name(&nodes, "r2c0");
    assert_eq!(native_value(cell, "TableCell.IsSelected"), Some(UiValue::from(true)));
    provider.shutdown();
}

/// Task 4.4: a killed JVM must not leave nodes reporting valid.
///
/// `UiNode::is_valid`'s `true` default is the trap this guards: the Robot
/// Framework library reuses the element a scoped root resolved to for exactly as
/// long as it answers `true`, so a node that stays optimistically valid pins a
/// dead element forever and every later step acts on nothing.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR"]
fn live_a_killed_jvm_leaves_no_valid_nodes() {
    let mut app = FixtureApp::launch_with_agent("lifetime");
    let provider = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();

    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    let window = loop {
        let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
        if let Some(node) = nodes.iter().find(|node| {
            node.name() == app.title && attribute_value(node, "Technology") == Some(UiValue::from("JavaAgent"))
        }) {
            break node.clone();
        }
        assert!(Instant::now() < deadline, "no agent-served window within {DISCOVERY_DEADLINE:?}");
        std::thread::sleep(Duration::from_millis(250));
    };

    // A node of a live window is valid, and something inside it too — the check
    // has to be per element, not per process.
    let mut nodes = Vec::new();
    walk(&window, &mut nodes, 0);
    let cell = find_by_name(&nodes, "r2c0").clone();
    assert!(window.is_valid() && cell.is_valid(), "a live window and its cells are valid");

    app.kill_and_wait();

    // Bounded: the answer has to arrive, not hang on a socket to a dead process.
    let started = Instant::now();
    let deadline = started + Duration::from_secs(30);
    loop {
        if !window.is_valid() && !cell.is_valid() {
            break;
        }
        assert!(Instant::now() < deadline, "nodes of a killed JVM still reported valid after 30s");
        std::thread::sleep(Duration::from_millis(200));
    }
    // Each individual answer must be prompt, whatever the loop above took: a
    // consumer asks this on every scoped-root access.
    let one_answer = Instant::now();
    assert!(!window.is_valid());
    assert!(one_answer.elapsed() < Duration::from_secs(10), "an invalid answer must be bounded, not a full deadline");

    provider.shutdown();
}

/// Task 4.6, agent half: a wedged JVM stays bounded and does not take the run
/// with it.
///
/// The JAB backend has had this coverage since `add-jab-provider`; the agent's
/// own containment — a session that degrades after consecutive bounded failures
/// and then fails fast until a rate-limited probe recovers it — had none, so it
/// was correct only by construction. A frozen JVM is the honest test: the agent
/// is *there*, its socket accepts, and nothing behind it will ever answer.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR; may be run manually if flaky in CI"]
fn live_frozen_agent_stays_contained() {
    const CALL_TIMEOUT: Duration = Duration::from_millis(750);

    let app = FixtureApp::launch_with_agent("frozen-agent");
    // Short per-call deadline so the freeze surfaces quickly; the agent backend
    // is otherwise at its defaults, degradation included.
    let providers = ConfigMap::new().with(
        platynui_provider_java::PROVIDER_ID,
        ConfigMap::new().with(
            "agent",
            ConfigMap::new().with("call_timeout_ms", i64::try_from(CALL_TIMEOUT.as_millis()).expect("fits")),
        ),
    );
    let provider = build_provider(&RuntimeConfig::new(ConfigMap::new(), providers));
    let parent = desktop_stub();

    let deadline = Instant::now() + DISCOVERY_DEADLINE;
    let window = loop {
        let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
        if let Some(node) = nodes.iter().find(|node| {
            node.name() == app.title && attribute_value(node, "Technology") == Some(UiValue::from("JavaAgent"))
        }) {
            break node.clone();
        }
        assert!(Instant::now() < deadline, "no agent-served window within {DISCOVERY_DEADLINE:?}");
        std::thread::sleep(Duration::from_millis(250));
    };
    assert!(window.is_valid(), "a live agent-served window is valid");

    // Freeze every thread of the JVM: the socket still accepts, the toolkit
    // thread behind it never answers again.
    set_process_frozen(app.pid(), true);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Validity is answered `false` rather than optimistically `true`, and it
        // is answered *within the deadline* rather than hanging on the socket.
        let start = Instant::now();
        window.invalidate();
        assert!(!window.is_valid(), "a frozen JVM must not report a valid node");
        let elapsed = start.elapsed();
        assert!(
            elapsed < CALL_TIMEOUT * 4 + Duration::from_secs(1),
            "the first answer must be bounded by the configured deadline, took {elapsed:?}"
        );

        // After a streak of bounded failures the session is degraded and stops
        // paying the deadline at all — which is what keeps one sick application
        // from making a whole run look hung.
        for _ in 0..4 {
            window.invalidate();
            let _ = window.is_valid();
        }
        let start = Instant::now();
        window.invalidate();
        let _ = window.is_valid();
        assert!(start.elapsed() < CALL_TIMEOUT / 2, "a degraded session must fail fast, took {:?}", start.elapsed());

        // Enumeration keeps working: the frozen JVM contributes nothing instead
        // of stalling the pass, so other Java windows (and other providers)
        // continue to be served.
        let start = Instant::now();
        let _ = provider.get_nodes(Arc::clone(&parent)).expect("enumeration must not fail").count();
        assert!(
            start.elapsed() < CALL_TIMEOUT * 6 + Duration::from_secs(2),
            "an enumeration pass with a frozen agent must stay bounded, took {:?}",
            start.elapsed()
        );

        // And another provider is entirely unaffected.
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

    // Recovery: the rate-limited probe finds the agent answering again, the
    // degraded flag clears, and the node becomes usable without a re-enumeration.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        window.invalidate();
        if window.is_valid() {
            break;
        }
        assert!(Instant::now() < deadline, "the session did not recover after thawing");
        std::thread::sleep(Duration::from_millis(500));
    }

    provider.shutdown();
}

/// Two `PlatynUI` processes on one agent — the Inspector open while a test run is
/// going — must both work, and must agree on what they are looking at.
///
/// The transport already proves two connections coexist (`two_clients_share_one_agent`
/// in `platynui-java-agent`), but that test predates the toolkit adapter and only
/// exchanges `ping`/`agent/info`. What matters here is the tree: element ids come
/// from one registry per JVM, so the same object has to carry the same id for both
/// readers, or a `RuntimeId` would mean different things depending on who asked and
/// the Inspector could not reveal what a test run reported.
///
/// Two providers in one process are the closest faithful stand-in: separate
/// backends, separate sessions, separate connections — which is exactly what two
/// host processes have.
#[test]
#[ignore = "needs a desktop, a Java runtime, the built Swing fixture and the built agent JAR"]
fn live_two_hosts_share_one_agent_and_agree_on_identity() {
    let app = FixtureApp::launch_with_agent("two-hosts");
    let inspector = build_provider(&RuntimeConfig::default());
    let test_run = build_provider(&RuntimeConfig::default());
    let parent = desktop_stub();

    let window_for = |provider: &Arc<dyn UiTreeProvider>| -> Arc<dyn UiNode> {
        let deadline = Instant::now() + DISCOVERY_DEADLINE;
        loop {
            let nodes: Vec<_> = provider.get_nodes(Arc::clone(&parent)).expect("get_nodes").collect();
            if let Some(node) = nodes.iter().find(|node| {
                node.name() == app.title && attribute_value(node, "Technology") == Some(UiValue::from("JavaAgent"))
            }) {
                return node.clone();
            }
            assert!(Instant::now() < deadline, "no agent-served window within {DISCOVERY_DEADLINE:?}");
            std::thread::sleep(Duration::from_millis(250));
        }
    };

    let inspector_window = window_for(&inspector);
    let test_run_window = window_for(&test_run);
    assert_eq!(
        inspector_window.runtime_id(),
        test_run_window.runtime_id(),
        "one element, one identity — both hosts read the same registry"
    );

    // Interleaved deep reads: a per-process agent would deadlock or time out here,
    // and ids assigned per connection would diverge.
    let mut from_inspector = Vec::new();
    walk(&inspector_window, &mut from_inspector, 0);
    let mut from_test_run = Vec::new();
    walk(&test_run_window, &mut from_test_run, 0);
    assert_eq!(
        structure_signature(&from_inspector),
        structure_signature(&from_test_run),
        "both hosts must see the same tree with the same identities"
    );

    // And they stay independent: shutting one down leaves the other working, which
    // is what makes closing the Inspector mid-run harmless.
    inspector.shutdown();
    let mut after = Vec::new();
    walk(&test_run_window, &mut after, 0);
    assert_eq!(
        structure_signature(&after),
        structure_signature(&from_test_run),
        "one host going away must not disturb the other"
    );
    assert!(test_run_window.is_valid(), "the surviving host still has a live node");

    test_run.shutdown();
}
