//! Where a node's on-screen geometry comes from, and how a substituted
//! top-level geometry shows up in the log.
//!
//! A real top-level window's position on screen is known only to the window
//! manager. When the window manager cannot answer for a window — it cannot
//! resolve the window, or cannot report its bounds — the window still answers
//! with the geometry its toolkit reports, as a successful read: real screen
//! coordinates on X11, but relative to the window itself on Wayland, where a
//! client does not know its position. Nothing in that rectangle says it is a
//! substitute, so the substitution is logged instead: a warning the first
//! time for each window, `debug` for as long as the window manager keeps
//! failing for it, and a warning again once it has answered in between.
//!
//! Nodes inside a window are placed relative to their ancestors and are
//! covered by their window's warning; grafted popups keep the window
//! manager's popup geometry. Neither is a substitute, so neither is logged.

use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Arc, Mutex, PoisonError};

use platynui_core::platform::{PlatformError, WindowId, WindowManager};
use platynui_core::types::Rect;
use platynui_core::ui::UiNode;
use tracing::{debug, warn};

/// The runtime's window manager as the provider holds it: together with the
/// record of the top-levels it could not answer for, so that record has the
/// window manager's own per-runtime scope. Dereferences to the window manager.
#[derive(Clone)]
pub(crate) struct InjectedWindowManager {
    window_manager: Arc<dyn WindowManager>,
    substitutions: Arc<Substitutions>,
}

impl InjectedWindowManager {
    pub(crate) fn new(window_manager: Arc<dyn WindowManager>) -> Self {
        Self { window_manager, substitutions: Arc::default() }
    }

    pub(crate) fn substitutions(&self) -> &Substitutions {
        &self.substitutions
    }
}

impl Deref for InjectedWindowManager {
    type Target = dyn WindowManager;

    fn deref(&self) -> &Self::Target {
        self.window_manager.as_ref()
    }
}

/// A window-manager call that could not answer for a top-level window.
#[derive(Debug)]
pub(crate) struct WindowManagerFailure {
    /// The call that failed: `resolve_window` or `bounds`.
    call: &'static str,
    /// The window, when the lookup got that far.
    window: Option<WindowId>,
    error: PlatformError,
}

/// Ask the window manager for a top-level's bounds: look up its window, then
/// read that window's bounds. The toolkit hint is resolved only once the
/// window is found.
pub(crate) fn window_manager_bounds(
    window_manager: &dyn WindowManager,
    node: &dyn UiNode,
    toolkit: impl FnOnce() -> Option<String>,
) -> Result<Rect, WindowManagerFailure> {
    let window = window_manager.resolve_window(node).map_err(|error| WindowManagerFailure {
        call: "resolve_window",
        window: None,
        error,
    })?;
    window_manager.bounds(window, toolkit().as_deref()).map_err(|error| WindowManagerFailure {
        call: "bounds",
        window: Some(window),
        error,
    })
}

/// A top-level's identity on the accessibility bus. Unlike a node, it
/// survives re-enumeration, so a window rebuilt on every tree refresh is
/// still the same window.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TopLevelKey {
    pub(crate) bus_name: String,
    pub(crate) path: String,
}

/// The top-levels whose last window-manager read failed. Bounded by the
/// distinct windows that failed during the runtime's lifetime, and dropped
/// with the runtime.
#[derive(Default)]
pub(crate) struct Substitutions {
    failing: Mutex<HashSet<TopLevelKey>>,
}

impl Substitutions {
    /// The window manager answered for `key`: its next failure is news again.
    fn answered(&self, key: &TopLevelKey) {
        self.failing.lock().unwrap_or_else(PoisonError::into_inner).remove(key);
    }

    /// `key`'s bounds are the toolkit's geometry `used`, because the window
    /// manager failed.
    fn substituted(
        &self,
        key: &TopLevelKey,
        describe: impl FnOnce() -> String,
        failure: &WindowManagerFailure,
        used: Option<Rect>,
    ) {
        let first = self.failing.lock().unwrap_or_else(PoisonError::into_inner).insert(key.clone());
        let window_id = failure.window.map_or_else(|| "unresolved".to_string(), |id| id.to_string());
        let used = used.map_or_else(|| "none".to_string(), |rect| rect.to_string());
        if first {
            warn!(
                window = %describe(),
                bus_name = %key.bus_name,
                path = %key.path,
                window_id,
                call = failure.call,
                error = %failure.error,
                toolkit_bounds = %used,
                "window manager cannot answer for this top-level window; its bounds are the toolkit's own \
                 geometry instead, which on Wayland is relative to the window, not its position on screen"
            );
        } else {
            debug!(
                bus_name = %key.bus_name,
                path = %key.path,
                window_id,
                call = failure.call,
                error = %failure.error,
                "window manager still cannot answer for this top-level window; bounds stay the toolkit's geometry"
            );
        }
    }
}

/// The geometry sources a node's bounds are composed from. The AT-SPI node
/// reads them from the bus and the window manager; tests answer directly.
pub(crate) trait ExtentSources {
    /// A real platform top-level window, whose position only the window
    /// manager knows.
    fn is_real_toplevel(&self) -> bool;
    /// A grafted transient popup, placed by the window manager's popup geometry.
    fn is_transient_popup(&self) -> bool;
    /// The window manager's bounds for this node; `None` when there is no
    /// window manager to ask.
    fn window_manager_bounds(&self) -> Option<Result<Rect, WindowManagerFailure>>;
    /// Where this node's substitutions are recorded, and under which key.
    fn substitutions(&self) -> Option<(&Substitutions, TopLevelKey)>;
    /// How the log names this node.
    fn describe(&self) -> String;
    /// Absolute extents summed from parent-relative positions up the tree.
    fn parent_chain_extents(&self) -> Option<Rect>;
    /// The extents the toolkit reports in screen coordinates.
    fn screen_extents(&self) -> Option<Rect>;
    /// The window manager's rect for a popup of this size.
    fn popup_bounds(&self, size: (f64, f64)) -> Option<Rect>;
}

/// Compose a node's on-screen bounds from its geometry sources.
pub(crate) fn resolve_extents(node: &impl ExtentSources) -> Option<Rect> {
    // Step 1: real platform top-level → WM bounds.
    let failure = if node.is_real_toplevel() {
        match node.window_manager_bounds() {
            Some(Ok(bounds)) => {
                if let Some((substitutions, key)) = node.substitutions() {
                    substitutions.answered(&key);
                }
                return Some(bounds);
            }
            Some(Err(failure)) => Some(failure),
            None => None,
        }
    } else {
        None
    };

    let extents = toolkit_extents(node);
    if let Some(failure) = failure
        && let Some((substitutions, key)) = node.substitutions()
    {
        substitutions.substituted(&key, || node.describe(), &failure, extents);
    }
    extents
}

/// The bounds a node gets without the window manager's window geometry.
fn toolkit_extents(node: &impl ExtentSources) -> Option<Rect> {
    // Step 2: walk up via CoordType::Parent.  We deliberately do
    // **not** use CoordType::Window: at least Qt's AT-SPI bridge
    // treats embedded surfaces (QMdiSubWindow, popup widgets …) as
    // window boundaries, so window-relative extents for everything
    // underneath are reported relative to that embedded surface
    // rather than the real toolkit top-level window — which makes
    // window-relative coordinates unsafe to combine with the
    // top-level's WM bounds.  Parent-relative coordinates are
    // unambiguous, so we sum them up the parent chain instead.
    if let Some(rect) = node.parent_chain_extents() {
        return Some(rect);
    }

    // Fallback: Screen extents (works on X11 where AT-SPI reports
    // real screen coordinates; on Wayland clients return 0,0).
    let screen_extents = node.screen_extents();

    // Step 3 (grafted popups only): the window manager's popup
    // geometry. On Wayland the Screen extents above are client-local
    // — only their size is trustworthy — while the PlatynUI
    // compositor knows every popup's real global rect. Match the two
    // by process and size. Backends without the popup query (X11,
    // Windows, mock) answer "unavailable", keeping the extents
    // fallback authoritative there.
    if node.is_transient_popup()
        && let Some(extents) = &screen_extents
        && let Some(rect) = node.popup_bounds((extents.width(), extents.height()))
    {
        return Some(rect);
    }

    screen_extents
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::{Point, Size};
    use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Weak};

    // Spec: *A substituted window geometry is visible in the log*. A node's
    // toolkit extents need the accessibility bus, so these tests drive the
    // composition through its geometry sources, with a real window-manager
    // call against an injected window manager that fails on demand.

    fn toolkit_rect() -> Rect {
        Rect::new(0.0, 0.0, 600.0, 500.0)
    }
    fn wm_rect() -> Rect {
        Rect::new(10.0, 40.0, 600.0, 500.0)
    }
    const SOCKET_ERROR: &str = "failed to connect to control socket /run/sidecar/wl-0.control: Connection refused";

    /// Which window-manager call fails, if any.
    #[derive(Clone, Copy)]
    enum Failing {
        Nothing,
        Lookup,
        Bounds,
    }

    struct StubWindowManager {
        failing: Mutex<Failing>,
        asked: AtomicBool,
    }

    impl StubWindowManager {
        fn injected(failing: Failing) -> (Arc<Self>, InjectedWindowManager) {
            let stub = Arc::new(Self { failing: Mutex::new(failing), asked: AtomicBool::new(false) });
            (Arc::clone(&stub), InjectedWindowManager::new(stub))
        }

        fn fail(&self, failing: Failing) {
            *self.failing.lock().unwrap() = failing;
        }

        fn error() -> PlatformError {
            PlatformError::OperationFailed { operation: "Wayland window manager", details: Some(SOCKET_ERROR.into()) }
        }
    }

    impl WindowManager for StubWindowManager {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn resolve_window(&self, _node: &dyn UiNode) -> Result<WindowId, PlatformError> {
            self.asked.store(true, Ordering::SeqCst);
            match *self.failing.lock().unwrap() {
                Failing::Lookup => Err(Self::error()),
                _ => Ok(WindowId::new(0x2a)),
            }
        }
        fn bounds(&self, _id: WindowId, _toolkit_hint: Option<&str>) -> Result<Rect, PlatformError> {
            match *self.failing.lock().unwrap() {
                Failing::Bounds => Err(Self::error()),
                _ => Ok(wm_rect()),
            }
        }
        fn is_active(&self, _id: WindowId) -> Result<bool, PlatformError> {
            unreachable!()
        }
        fn activate(&self, _id: WindowId) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn close(&self, _id: WindowId) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn minimize(&self, _id: WindowId) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn maximize(&self, _id: WindowId) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn restore(&self, _id: WindowId) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn move_to(&self, _id: WindowId, _position: Point) -> Result<(), PlatformError> {
            unreachable!()
        }
        fn resize(&self, _id: WindowId, _size: Size) -> Result<(), PlatformError> {
            unreachable!()
        }
    }

    /// What the window manager is handed to look the window up.
    struct StubNode;

    static STUB_RUNTIME_ID: LazyLock<RuntimeId> = LazyLock::new(|| RuntimeId::from("stub"));

    impl UiNode for StubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &'static str {
            "Frame"
        }
        fn name(&self) -> String {
            "Sidecar App".into()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &STUB_RUNTIME_ID
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

    /// A node as `resolve_extents` sees it.
    struct Node {
        kind: Kind,
        path: &'static str,
        window_manager: InjectedWindowManager,
    }

    #[derive(Clone, Copy)]
    enum Kind {
        TopLevel,
        Inner,
        Popup,
    }

    fn inner_rect() -> Rect {
        Rect::new(20.0, 70.0, 80.0, 24.0)
    }
    fn popup_extents() -> Rect {
        Rect::new(0.0, 0.0, 120.0, 90.0)
    }
    fn popup_rect() -> Rect {
        Rect::new(30.0, 90.0, 120.0, 90.0)
    }

    impl Node {
        fn top_level(path: &'static str, window_manager: &InjectedWindowManager) -> Self {
            Self { kind: Kind::TopLevel, path, window_manager: window_manager.clone() }
        }
    }

    impl ExtentSources for Node {
        fn is_real_toplevel(&self) -> bool {
            matches!(self.kind, Kind::TopLevel)
        }
        fn is_transient_popup(&self) -> bool {
            matches!(self.kind, Kind::Popup)
        }
        fn window_manager_bounds(&self) -> Option<Result<Rect, WindowManagerFailure>> {
            Some(window_manager_bounds(&*self.window_manager, &StubNode, || Some("egui".into())))
        }
        fn substitutions(&self) -> Option<(&Substitutions, TopLevelKey)> {
            let key = TopLevelKey { bus_name: ":1.42".into(), path: self.path.into() };
            Some((self.window_manager.substitutions(), key))
        }
        fn describe(&self) -> String {
            r#"Frame "Sidecar App""#.into()
        }
        fn parent_chain_extents(&self) -> Option<Rect> {
            matches!(self.kind, Kind::Inner).then_some(inner_rect())
        }
        fn screen_extents(&self) -> Option<Rect> {
            Some(if matches!(self.kind, Kind::Popup) { popup_extents() } else { toolkit_rect() })
        }
        fn popup_bounds(&self, size: (f64, f64)) -> Option<Rect> {
            (size == (popup_rect().width(), popup_rect().height())).then_some(popup_rect())
        }
    }

    /// Run `f` with its tracing output captured.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }

    fn warnings(log: &str) -> Vec<&str> {
        log.lines().filter(|line| line.contains(" WARN ")).collect()
    }

    #[test]
    fn a_top_level_the_window_manager_cannot_find_keeps_the_toolkit_geometry_and_says_so() {
        let (_, wm) = StubWindowManager::injected(Failing::Lookup);
        let (bounds, log) = logged(|| resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

        assert_eq!(bounds, Some(toolkit_rect()), "the read still succeeds with the toolkit's rectangle");
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        let warning = warnings[0];
        for expected in [
            r#"Frame "Sidecar App""#,
            ":1.42",
            "/org/a11y/atspi/accessible/1",
            "call=\"resolve_window\"",
            "window_id=\"unresolved\"",
            "/run/sidecar/wl-0.control",
            "toolkit's own geometry",
        ] {
            assert!(warning.contains(expected), "the warning names `{expected}`: {warning}");
        }
    }

    #[test]
    fn a_top_level_whose_bounds_cannot_be_read_names_its_window() {
        let (_, wm) = StubWindowManager::injected(Failing::Bounds);
        let (bounds, log) = logged(|| resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

        assert_eq!(bounds, Some(toolkit_rect()));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["call=\"bounds\"", "WindowId(0x2a)", "/run/sidecar/wl-0.control"] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {}", warnings[0]);
        }
    }

    #[test]
    fn a_window_that_keeps_failing_is_reported_once_across_reads_and_rebuilt_nodes() {
        let (_, wm) = StubWindowManager::injected(Failing::Lookup);
        let node = Node::top_level("/org/a11y/atspi/accessible/1", &wm);
        let (reads, log) = logged(|| {
            // The same node read again, and the window rebuilt by a re-enumeration.
            let rebuilt = Node::top_level("/org/a11y/atspi/accessible/1", &wm);
            [resolve_extents(&node), resolve_extents(&node), resolve_extents(&rebuilt)]
        });

        assert_eq!(reads, [Some(toolkit_rect()); 3], "every read returns the same fallback rectangle");
        assert_eq!(warnings(&log).len(), 1, "{log}");
        assert!(log.contains("still cannot answer"), "the repeats are debug records: {log}");
    }

    #[test]
    fn a_window_that_recovers_and_fails_again_is_reported_again() {
        let (stub, wm) = StubWindowManager::injected(Failing::Lookup);
        let node = Node::top_level("/org/a11y/atspi/accessible/1", &wm);
        let (reads, log) = logged(|| {
            let failed = resolve_extents(&node);
            stub.fail(Failing::Nothing);
            let answered = resolve_extents(&node);
            stub.fail(Failing::Bounds);
            let failed_again = resolve_extents(&node);
            [failed, answered, failed_again]
        });

        assert_eq!(reads, [Some(toolkit_rect()), Some(wm_rect()), Some(toolkit_rect())]);
        assert_eq!(warnings(&log).len(), 2, "{log}");
    }

    #[test]
    fn each_failing_top_level_gets_its_own_warning() {
        let (_, wm) = StubWindowManager::injected(Failing::Lookup);
        let ((), log) = logged(|| {
            resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &wm));
            resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/2", &wm));
        });

        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        assert!(warnings[0].contains("accessible/1") && warnings[1].contains("accessible/2"), "{log}");
    }

    #[test]
    fn a_second_runtime_reports_its_own_substitutions() {
        let (_, first) = StubWindowManager::injected(Failing::Lookup);
        let (_, second) = StubWindowManager::injected(Failing::Lookup);
        let ((), log) = logged(|| {
            resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &first));
            resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &second));
        });

        assert_eq!(warnings(&log).len(), 2, "{log}");
    }

    #[test]
    fn a_window_manager_that_answers_gives_its_bounds_without_a_warning() {
        let (_, wm) = StubWindowManager::injected(Failing::Nothing);
        let (bounds, log) = logged(|| resolve_extents(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

        assert_eq!(bounds, Some(wm_rect()));
        assert!(warnings(&log).is_empty(), "{log}");
    }

    #[test]
    fn geometry_that_never_came_from_the_window_manager_is_not_reported() {
        let (stub, wm) = StubWindowManager::injected(Failing::Lookup);
        let inner = Node { kind: Kind::Inner, path: "/org/a11y/atspi/accessible/7", window_manager: wm.clone() };
        let popup = Node { kind: Kind::Popup, path: "/org/a11y/atspi/accessible/9", window_manager: wm };
        let (bounds, log) = logged(|| [resolve_extents(&inner), resolve_extents(&popup)]);

        assert_eq!(bounds, [Some(inner_rect()), Some(popup_rect())], "each keeps its own geometry source");
        assert!(!stub.asked.load(Ordering::SeqCst), "neither asks the window manager for a window");
        assert!(warnings(&log).is_empty(), "{log}");
    }
}
