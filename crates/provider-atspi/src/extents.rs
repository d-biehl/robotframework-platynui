//! Where a node's on-screen geometry comes from, and how a substituted
//! top-level geometry shows up in the log.
//!
//! **A node is placed relative to its window**, never by adding up its
//! ancestors. Its *reference window* is its nearest ancestor that is a real
//! top-level window or a popup the platform shows as a surface of its own. The
//! reference window gives an origin on the desktop; the node adds its own
//! position relative to that window, read in a coordinate type its toolkit
//! implements consistently:
//!
//! - GTK 4 reports no screen positions, so its nodes are read in window
//!   coordinates.
//! - Every other toolkit is read in screen coordinates, unless a node's screen
//!   position equals its window's while its window position does not — the
//!   mark of a toolkit that reports no screen positions either.
//! - Where the platform says that toolkit screen coordinates are desktop
//!   coordinates (X11 with a window manager), a screen position is the answer
//!   as it is. Elsewhere it is re-based: the window's origin plus the node's
//!   offset from its window's own screen position.
//!
//! A top-level window's own bounds are its visible geometry from the window
//! manager. Its descendants count from the surface origin their toolkit
//! measures from: the visible frame for GTK 4, the window surface for every
//! other toolkit. A popup is matched to the platform's popup list by the size
//! of its surface; its own bounds are the visible rectangle, and its
//! descendants count from the surface's origin.
//!
//! When the window manager cannot answer for a window — it cannot resolve the
//! window, or cannot report its geometry — the window still answers with the
//! geometry its toolkit reports, as a successful read: real screen coordinates
//! on X11, but relative to the window itself on Wayland, where a client does
//! not know its position. Nothing in that rectangle says it is a substitute,
//! so the substitution is logged instead: a warning the first time for each
//! window, `debug` for as long as the window manager keeps failing for it, one
//! `debug` when it answers again, and a warning again when it fails after
//! that. Nodes inside the window are placed from the substitute and are
//! covered by their window's warning; popups keep the platform's popup
//! geometry. Neither is a substitute of its own, so neither is logged.

use std::ops::Deref;
use std::sync::Arc;

use platynui_core::diagnostics::{Episode, Transitions};
use platynui_core::platform::{PlatformError, PopupGeometry, WindowGeometry, WindowId, WindowManager};
use platynui_core::types::{Point, Rect};
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
    /// The call that failed: `resolve_window` or `geometry`.
    call: &'static str,
    /// The window, when the lookup got that far.
    window: Option<WindowId>,
    error: PlatformError,
}

/// Ask the window manager for a top-level's geometry: look up its window, then
/// read that window's geometry.
pub(crate) fn window_manager_geometry(
    window_manager: &dyn WindowManager,
    node: &dyn UiNode,
) -> Result<WindowGeometry, WindowManagerFailure> {
    let window = window_manager.resolve_window(node).map_err(|error| WindowManagerFailure {
        call: "resolve_window",
        window: None,
        error,
    })?;
    window_manager.geometry(window).map_err(|error| WindowManagerFailure {
        call: "geometry",
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
    failing: Transitions<TopLevelKey>,
}

impl Substitutions {
    /// The window manager answered for `key`: the end of its episode is
    /// recorded once, and its next failure is news again.
    fn answered(&self, key: &TopLevelKey, describe: impl FnOnce() -> String) {
        if self.failing.recovered(key) {
            debug!(
                window = %describe(),
                bus_name = %key.bus_name,
                path = %key.path,
                "window manager answers for this top-level window again; its bounds come from the window manager"
            );
        }
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
        let episode = self.failing.failed(key);
        let window_id = failure.window.map_or_else(|| "unresolved".to_string(), |id| id.to_string());
        let used = used.map_or_else(|| "none".to_string(), |rect| rect.to_string());
        if episode == Episode::Started {
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

/// What the descendants of a reference window are placed from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reference {
    /// The desktop point the descendants' offsets count from.
    pub(crate) origin: Point,
    /// The reference node's own position in its toolkit's screen coordinates,
    /// when it reports one.
    pub(crate) screen: Option<Point>,
    /// The reference node's own position in its toolkit's window coordinates,
    /// when it reports one.
    pub(crate) window: Option<Point>,
    /// How many popups the reference is nested in, itself included: `0` for a
    /// top-level window, `1` for a menu, `2` for a submenu it opened.
    pub(crate) popup_depth: u32,
}

/// The geometry sources a node's placement is composed from. The AT-SPI node
/// reads them from the bus and the window manager; tests answer directly.
pub(crate) trait ExtentSources {
    /// A real platform top-level window, whose position only the window
    /// manager knows.
    fn is_real_toplevel(&self) -> bool;
    /// A popup: a transient popup grafted under its application, or a node on
    /// the AT-SPI popup layer.
    fn is_popup(&self) -> bool;
    /// The window manager's geometry for this top-level; `None` when there is
    /// no window manager to ask.
    fn window_manager_geometry(&self) -> Option<Result<WindowGeometry, WindowManagerFailure>>;
    /// Where this node's substitutions are recorded, and under which key.
    fn substitutions(&self) -> Option<(&Substitutions, TopLevelKey)>;
    /// How the log names this node: its description (`Role "Name" #Id`).
    fn describe(&self) -> String;
    /// The extents the toolkit reports in screen coordinates.
    fn screen_extents(&self) -> Option<Rect>;
    /// The extents the toolkit reports in window coordinates.
    fn window_extents(&self) -> Option<Rect>;
    /// The node's toolkit as a lowercase name and major version, such as
    /// `gtk4` or `qt6`.
    fn toolkit(&self) -> Option<String>;
    /// Whether the platform's toolkit screen coordinates are desktop
    /// coordinates.
    fn screen_is_desktop(&self) -> bool;
    /// The platform's popups for this node's process, most recently opened or
    /// topmost first; `None` when the platform cannot list them.
    fn popups(&self) -> Option<Vec<PopupGeometry>>;
    /// What this node's parent places its descendants from; `None` when no
    /// ancestor is a reference window.
    fn ancestor_reference(&self) -> Option<Reference>;
    /// The reference of a popup among this node's children that matches one
    /// of `popups`, as GTK 3 lists a menu's popup beside the items it shows,
    /// below the item that opened the menu. `parent_depth` is the popup depth
    /// this node inherits.
    fn child_popup(&self, popups: &[PopupGeometry], parent_depth: u32) -> Option<Reference>;
}

/// A node's own on-screen bounds.
pub(crate) fn resolve_bounds(node: &impl ExtentSources) -> Option<Rect> {
    let window_coordinates = reads_window_coordinates(node);
    if node.is_real_toplevel() {
        return top_level(node, window_coordinates).bounds;
    }
    let inherited = node.ancestor_reference();
    if node.is_popup()
        && let Some(popups) = node.popups()
        && let Some(popup) = matched_popup(node, window_coordinates, &popups, inherited.as_ref())
    {
        return popup.bounds;
    }
    place_in_window(node, window_coordinates, inherited.as_ref())
}

/// What a node's descendants are placed from: the node's own reference when it
/// is a reference window, its ancestors' otherwise.
///
/// A popup the platform does not list places its children from a listed popup
/// among them, when there is one: GTK 3 lists a menu's items, and the menu's
/// popup beside them, below the item that opened the menu.
pub(crate) fn resolve_reference(node: &impl ExtentSources) -> Option<Reference> {
    let window_coordinates = reads_window_coordinates(node);
    if node.is_real_toplevel() {
        return top_level(node, window_coordinates).reference;
    }
    let inherited = node.ancestor_reference();
    if node.is_popup()
        && let Some(popups) = node.popups()
    {
        if let Some(popup) = matched_popup(node, window_coordinates, &popups, inherited.as_ref()) {
            return popup.reference;
        }
        let depth = inherited.map_or(0, |reference| reference.popup_depth);
        if let Some(reference) = node.child_popup(&popups, depth) {
            return Some(reference);
        }
    }
    inherited
}

/// The reference of `node` when it is one of the listed `popups`, nested
/// `parent_depth` popups deep. `None` when it matches none of them.
pub(crate) fn listed_popup_reference(
    node: &impl ExtentSources,
    popups: &[PopupGeometry],
    parent_depth: u32,
) -> Option<Reference> {
    match_listed(node, reads_window_coordinates(node), popups, parent_depth)?.reference
}

/// A reference window's own bounds and what it places its descendants from.
struct OwnPlacement {
    bounds: Option<Rect>,
    reference: Option<Reference>,
}

/// Max per-axis difference (px) between a popup node's size and a listed
/// popup's rectangle for the two to be the same popup. Both describe one
/// surface, so the match is near-exact; the tolerance only absorbs rounding.
const POPUP_SIZE_TOLERANCE: f64 = 2.0;

/// Whether a node is read in window coordinates rather than screen ones: GTK 4
/// and later report every screen position as `(0,0)`.
fn reads_window_coordinates(node: &impl ExtentSources) -> bool {
    node.toolkit()
        .and_then(|toolkit| toolkit.strip_prefix("gtk").and_then(|major| major.parse::<u32>().ok()))
        .is_some_and(|major| major >= 4)
}

/// `rect` when it describes geometry: a positive size, and no coordinate at
/// `G_MININT`, which GTK 3 reports for widgets it does not show.
fn geometry(rect: Option<Rect>) -> Option<Rect> {
    let min = f64::from(i32::MIN);
    rect.filter(|r| r.width() > 0.0 && r.height() > 0.0 && r.x() > min && r.y() > min)
}

/// `rect` moved so that its offset from `base` counts from `origin` instead.
fn rebased(rect: Rect, base: Option<Point>, origin: Point) -> Rect {
    let base = base.unwrap_or_default();
    Rect::new(origin.x() + rect.x() - base.x(), origin.y() + rect.y() - base.y(), rect.width(), rect.height())
}

/// A real top-level window: its visible geometry from the window manager, and
/// the surface origin its toolkit counts from. When the window manager cannot
/// answer, the toolkit's own screen geometry stands in for both, and the
/// substitution is logged.
fn top_level(node: &impl ExtentSources, window_coordinates: bool) -> OwnPlacement {
    let window = geometry(node.window_extents()).map(|rect| rect.position());
    let failure = match node.window_manager_geometry() {
        Some(Ok(geometry)) => {
            if let Some((substitutions, key)) = node.substitutions() {
                substitutions.answered(&key, || node.describe());
            }
            let origin = if window_coordinates { geometry.bounds.position() } else { geometry.surface_origin };
            let screen = if window_coordinates { None } else { self::geometry(node.screen_extents()) };
            return OwnPlacement {
                bounds: Some(geometry.bounds),
                reference: Some(Reference {
                    origin,
                    screen: screen.map(|rect| rect.position()),
                    window,
                    popup_depth: 0,
                }),
            };
        }
        Some(Err(failure)) => Some(failure),
        None => None,
    };

    let screen = geometry(node.screen_extents());
    if let Some(failure) = failure
        && let Some((substitutions, key)) = node.substitutions()
    {
        substitutions.substituted(&key, || node.describe(), &failure, screen);
    }
    OwnPlacement {
        bounds: screen,
        reference: screen.map(|rect| Reference {
            origin: rect.position(),
            screen: Some(rect.position()),
            window,
            popup_depth: 0,
        }),
    }
}

/// A popup the platform lists, below `inherited`. See [`match_listed`].
///
/// GTK 3 lists a menu's popup beside the items it shows, so the item that
/// opened the menu already hands this popup's reference on to it: a popup the
/// inherited reference already stands for is not nested in itself.
fn matched_popup(
    node: &impl ExtentSources,
    window_coordinates: bool,
    popups: &[PopupGeometry],
    inherited: Option<&Reference>,
) -> Option<OwnPlacement> {
    let parent_depth = inherited.map_or(0, |reference| reference.popup_depth);
    let mut placement = match_listed(node, window_coordinates, popups, parent_depth)?;
    if let (Some(inherited), Some(own)) = (inherited, placement.reference.as_mut())
        && inherited.popup_depth > 0
        && inherited.origin == own.origin
    {
        own.popup_depth = inherited.popup_depth;
    }
    Some(placement)
}

/// A popup the platform lists: matched by its own size against each listed
/// popup's surface or visible rectangle, the first listed match winning. A
/// GTK 4 popover's node is its surface, shadow included; GTK 3's menu node is
/// the visible menu. Its bounds are the listed visible rectangle, and its
/// descendants count from the origin of the rectangle it matched, which is
/// where its own position lies.
fn match_listed(
    node: &impl ExtentSources,
    window_coordinates: bool,
    popups: &[PopupGeometry],
    parent_depth: u32,
) -> Option<OwnPlacement> {
    let screen = if window_coordinates { None } else { geometry(node.screen_extents()) };
    let window = geometry(node.window_extents());
    let size = if window_coordinates { window } else { screen.or(window) }?.size();
    let fits = |rect: &Rect| {
        (rect.width() - size.width()).abs() <= POPUP_SIZE_TOLERANCE
            && (rect.height() - size.height()).abs() <= POPUP_SIZE_TOLERANCE
    };
    let (listed, origin) = popups.iter().find_map(|popup| {
        if fits(&popup.surface) {
            Some((popup, popup.surface.position()))
        } else if fits(&popup.bounds) {
            Some((popup, popup.bounds.position()))
        } else {
            None
        }
    })?;
    Some(OwnPlacement {
        bounds: Some(listed.bounds),
        reference: Some(Reference {
            origin,
            screen: screen.map(|rect| rect.position()),
            window: window.map(|rect| rect.position()),
            popup_depth: parent_depth + 1,
        }),
    })
}

/// A node inside its reference window: the window's origin plus the node's
/// position relative to the window, read in screen coordinates unless its
/// toolkit reports none. Screen coordinates stand as they are where the
/// platform says they are desktop coordinates, and are re-based on the window
/// elsewhere. Without a reference window, the toolkit's screen extents are all
/// there is.
fn place_in_window(node: &impl ExtentSources, window_coordinates: bool, reference: Option<&Reference>) -> Option<Rect> {
    if window_coordinates {
        let reference = reference?;
        return geometry(node.window_extents()).map(|rect| rebased(rect, reference.window, reference.origin));
    }
    let screen = geometry(node.screen_extents())?;
    let Some(reference) = reference else { return Some(screen) };
    // A node whose screen position is its window's, while its window position
    // is not the window's origin, reports no screen positions of its own.
    if reference.screen == Some(screen.position())
        && let Some(window) = geometry(node.window_extents())
        && window.position() != reference.window.unwrap_or_default()
    {
        return Some(rebased(window, reference.window, reference.origin));
    }
    if node.screen_is_desktop() {
        return Some(screen);
    }
    Some(rebased(screen, reference.screen, reference.origin))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_log::{at_level, logged, warnings};
    use platynui_core::types::Size;
    use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex, Weak};

    // A node's toolkit extents need the accessibility bus, so these tests drive
    // the composition through its geometry sources: a node answers its own
    // extents, its toolkit, the platform's view and its ancestors' reference
    // directly, and a top-level asks a window manager injected for the test.

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
        Geometry,
    }

    struct StubWindowManager {
        failing: Mutex<Failing>,
        geometry: WindowGeometry,
        asked: AtomicBool,
    }

    impl StubWindowManager {
        fn injected(failing: Failing) -> (Arc<Self>, InjectedWindowManager) {
            Self::injected_with(failing, WindowGeometry::from_bounds(wm_rect()))
        }

        fn injected_with(failing: Failing, geometry: WindowGeometry) -> (Arc<Self>, InjectedWindowManager) {
            let stub = Arc::new(Self { failing: Mutex::new(failing), geometry, asked: AtomicBool::new(false) });
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
        fn geometry(&self, _id: WindowId) -> Result<WindowGeometry, PlatformError> {
            match *self.failing.lock().unwrap() {
                Failing::Geometry => Err(Self::error()),
                _ => Ok(self.geometry),
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

    /// A node as the composition sees it. Every source answers directly.
    #[derive(Clone, Default)]
    struct Node {
        top_level: bool,
        popup: bool,
        path: &'static str,
        window_manager: Option<InjectedWindowManager>,
        screen: Option<Rect>,
        window: Option<Rect>,
        toolkit: Option<&'static str>,
        desktop: bool,
        popups: Option<Vec<PopupGeometry>>,
        ancestor: Option<Reference>,
        child_popup: Option<Reference>,
    }

    impl Node {
        /// A top-level window whose toolkit reports `toolkit_rect()` as its
        /// screen extents.
        fn top_level(path: &'static str, window_manager: &InjectedWindowManager) -> Self {
            Self {
                top_level: true,
                path,
                window_manager: Some(window_manager.clone()),
                screen: Some(toolkit_rect()),
                ..Self::default()
            }
        }

        /// A node inside a window, below `ancestor`.
        fn inside(ancestor: Reference) -> Self {
            Self { ancestor: Some(ancestor), ..Self::default() }
        }
    }

    impl ExtentSources for Node {
        fn is_real_toplevel(&self) -> bool {
            self.top_level
        }
        fn is_popup(&self) -> bool {
            self.popup
        }
        fn window_manager_geometry(&self) -> Option<Result<WindowGeometry, WindowManagerFailure>> {
            self.window_manager.as_ref().map(|wm| window_manager_geometry(&**wm, &StubNode))
        }
        fn substitutions(&self) -> Option<(&Substitutions, TopLevelKey)> {
            let wm = self.window_manager.as_ref()?;
            Some((wm.substitutions(), TopLevelKey { bus_name: ":1.42".into(), path: self.path.into() }))
        }
        fn describe(&self) -> String {
            r#"Frame "Sidecar App""#.into()
        }
        fn screen_extents(&self) -> Option<Rect> {
            self.screen
        }
        fn window_extents(&self) -> Option<Rect> {
            self.window
        }
        fn toolkit(&self) -> Option<String> {
            self.toolkit.map(str::to_owned)
        }
        fn screen_is_desktop(&self) -> bool {
            self.desktop
        }
        fn popups(&self) -> Option<Vec<PopupGeometry>> {
            self.popups.clone()
        }
        fn ancestor_reference(&self) -> Option<Reference> {
            self.ancestor
        }
        fn child_popup(&self, _popups: &[PopupGeometry], _parent_depth: u32) -> Option<Reference> {
            self.child_popup
        }
    }

    /// A window's reference: its origin, and its own screen and window
    /// positions.
    fn window_reference(origin: (f64, f64), screen: Option<(f64, f64)>) -> Reference {
        Reference {
            origin: Point::new(origin.0, origin.1),
            screen: screen.map(|(x, y)| Point::new(x, y)),
            window: Some(Point::new(0.0, 0.0)),
            popup_depth: 0,
        }
    }

    // --- substitution: *A substituted window geometry is visible in the log* ---

    #[test]
    fn a_top_level_the_window_manager_cannot_find_keeps_the_toolkit_geometry_and_says_so() {
        let (_, wm) = StubWindowManager::injected(Failing::Lookup);
        let (bounds, log) = logged(|| resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

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
    fn a_top_level_whose_geometry_cannot_be_read_names_its_window() {
        let (_, wm) = StubWindowManager::injected(Failing::Geometry);
        let (bounds, log) = logged(|| resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

        assert_eq!(bounds, Some(toolkit_rect()));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["call=\"geometry\"", "WindowId(0x2a)", "/run/sidecar/wl-0.control"] {
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
            [resolve_bounds(&node), resolve_bounds(&node), resolve_bounds(&rebuilt)]
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
            let failed = resolve_bounds(&node);
            stub.fail(Failing::Nothing);
            let answered = resolve_bounds(&node);
            stub.fail(Failing::Geometry);
            let failed_again = resolve_bounds(&node);
            [failed, answered, failed_again]
        });

        assert_eq!(reads, [Some(toolkit_rect()), Some(wm_rect()), Some(toolkit_rect())]);
        assert_eq!(warnings(&log).len(), 2, "{log}");
    }

    #[test]
    fn a_window_the_window_manager_answers_for_again_is_recorded_once() {
        let (stub, wm) = StubWindowManager::injected(Failing::Lookup);
        let node = Node::top_level("/org/a11y/atspi/accessible/1", &wm);
        let ((), log) = logged(|| {
            resolve_bounds(&node);
            stub.fail(Failing::Nothing);
            resolve_bounds(&node);
            resolve_bounds(&node);
        });

        let recovered: Vec<_> = at_level(&log, "DEBUG").into_iter().filter(|line| line.contains("again")).collect();
        assert_eq!(recovered.len(), 1, "one record ends the episode: {log}");
        for expected in [r#"Frame "Sidecar App""#, ":1.42", "/org/a11y/atspi/accessible/1"] {
            assert!(recovered[0].contains(expected), "the record names `{expected}`: {}", recovered[0]);
        }
    }

    #[test]
    fn a_window_the_window_manager_always_answered_for_records_no_recovery() {
        let (_, wm) = StubWindowManager::injected(Failing::Nothing);
        let (_, log) = logged(|| resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));
        assert!(log.is_empty(), "{log}");
    }

    #[test]
    fn each_failing_top_level_gets_its_own_warning() {
        let (_, wm) = StubWindowManager::injected(Failing::Lookup);
        let ((), log) = logged(|| {
            resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &wm));
            resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/2", &wm));
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
            resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &first));
            resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &second));
        });

        assert_eq!(warnings(&log).len(), 2, "{log}");
    }

    #[test]
    fn a_window_manager_that_answers_gives_its_bounds_without_a_warning() {
        let (_, wm) = StubWindowManager::injected(Failing::Nothing);
        let (bounds, log) = logged(|| resolve_bounds(&Node::top_level("/org/a11y/atspi/accessible/1", &wm)));

        assert_eq!(bounds, Some(wm_rect()));
        assert!(warnings(&log).is_empty(), "{log}");
    }

    /// Spec: *Geometry that never came from the window manager is unchanged*.
    /// The window's substitute is its toolkit's own screen geometry, so a
    /// control placed from it keeps its own screen position; a popup keeps the
    /// platform's popup geometry. Neither asks the window manager or warns.
    #[test]
    fn geometry_that_never_came_from_the_window_manager_is_unchanged() {
        let (stub, wm) = StubWindowManager::injected(Failing::Lookup);
        let substitute = resolve_reference(&Node::top_level("/org/a11y/atspi/accessible/1", &wm))
            .expect("a substituted window still places its descendants");
        assert_eq!(substitute.origin, toolkit_rect().position());
        stub.asked.store(false, Ordering::SeqCst);

        let control = Node { screen: Some(Rect::new(20.0, 70.0, 80.0, 24.0)), ..Node::inside(substitute) };
        let listed = PopupGeometry::from_bounds(Rect::new(30.0, 90.0, 120.0, 90.0));
        let popup = Node {
            popup: true,
            screen: Some(Rect::new(0.0, 0.0, 120.0, 90.0)),
            popups: Some(vec![listed]),
            ..Node::default()
        };
        let (bounds, log) = logged(|| [resolve_bounds(&control), resolve_bounds(&popup)]);

        assert_eq!(bounds, [Some(Rect::new(20.0, 70.0, 80.0, 24.0)), Some(listed.bounds)]);
        assert!(!stub.asked.load(Ordering::SeqCst), "neither asks the window manager for a window");
        assert!(warnings(&log).is_empty(), "{log}");
    }

    // --- *A window's own bounds are its visible geometry* ---

    /// A GTK 4 window counts its elements from its visible frame; every other
    /// toolkit from its surface.
    #[test]
    fn a_top_level_places_its_elements_from_the_origin_its_toolkit_counts_from() {
        let geometry =
            WindowGeometry { bounds: Rect::new(24.0, 22.0, 420.0, 305.0), surface_origin: Point::new(10.0, 10.0) };
        let (_, wm) = StubWindowManager::injected_with(Failing::Nothing, geometry);
        let gtk4 = Node { toolkit: Some("gtk4"), ..Node::top_level("/org/a11y/atspi/accessible/1", &wm) };
        let gtk3 = Node { toolkit: Some("gtk3"), ..Node::top_level("/org/a11y/atspi/accessible/1", &wm) };

        assert_eq!(resolve_bounds(&gtk4), Some(geometry.bounds));
        assert_eq!(resolve_reference(&gtk4).map(|r| r.origin), Some(Point::new(24.0, 22.0)));
        assert_eq!(resolve_bounds(&gtk3), Some(geometry.bounds));
        assert_eq!(resolve_reference(&gtk3).map(|r| r.origin), Some(Point::new(10.0, 10.0)));
        assert_eq!(resolve_reference(&gtk3).map(|r| r.popup_depth), Some(0));
    }

    // --- *A toolkit is read in the coordinate type it implements consistently* ---

    #[test]
    fn a_gtk4_element_is_read_in_window_coordinates() {
        let node = Node {
            toolkit: Some("gtk4"),
            screen: Some(Rect::new(0.0, 0.0, 80.0, 24.0)),
            window: Some(Rect::new(30.0, 40.0, 80.0, 24.0)),
            ..Node::inside(window_reference((100.0, 200.0), Some((0.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(130.0, 240.0, 80.0, 24.0)));
    }

    /// GTK 3 answers parent-relative extents with screen ones; on X11 a
    /// control's screen extents are its bounds, not its ancestors' sum.
    #[test]
    fn a_toolkit_that_reports_screen_values_as_parent_relative_ones_is_not_added_up() {
        let node = Node {
            toolkit: Some("gtk3"),
            desktop: true,
            screen: Some(Rect::new(140.0, 90.0, 60.0, 20.0)),
            window: Some(Rect::new(40.0, 40.0, 60.0, 20.0)),
            ..Node::inside(window_reference((100.0, 50.0), Some((100.0, 50.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(140.0, 90.0, 60.0, 20.0)));
    }

    #[test]
    fn an_unknown_toolkit_without_screen_positions_falls_back_to_window_coordinates() {
        let node = Node {
            screen: Some(Rect::new(0.0, 0.0, 50.0, 20.0)),
            window: Some(Rect::new(40.0, 60.0, 50.0, 20.0)),
            ..Node::inside(window_reference((10.0, 40.0), Some((0.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(50.0, 100.0, 50.0, 20.0)));
    }

    #[test]
    fn a_node_with_consistent_screen_coordinates_keeps_them() {
        let node = Node {
            screen: Some(Rect::new(140.0, 160.0, 50.0, 20.0)),
            window: Some(Rect::new(5.0, 5.0, 50.0, 20.0)),
            ..Node::inside(window_reference((300.0, 300.0), Some((100.0, 100.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(340.0, 360.0, 50.0, 20.0)));
    }

    /// A node at its window's origin reports the window's screen position and a
    /// window position of `(0,0)`: its screen coordinates stand.
    #[test]
    fn a_node_at_its_windows_origin_keeps_its_screen_coordinates() {
        let node = Node {
            desktop: true,
            screen: Some(Rect::new(100.0, 100.0, 50.0, 20.0)),
            window: Some(Rect::new(0.0, 0.0, 50.0, 20.0)),
            ..Node::inside(window_reference((100.0, 100.0), Some((100.0, 100.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(100.0, 100.0, 50.0, 20.0)));
    }

    // --- *Screen coordinates are re-based where they are window-relative* ---

    #[test]
    fn a_window_whose_toolkit_pins_it_to_its_outputs_origin_keeps_its_elements() {
        let node = Node {
            screen: Some(Rect::new(1940.0, 30.0, 80.0, 24.0)),
            ..Node::inside(window_reference((2000.0, 100.0), Some((1920.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(2020.0, 130.0, 80.0, 24.0)));
    }

    /// Measured with egui on X11 without a window manager: after a move the
    /// toolkit keeps counting from the old origin.
    #[test]
    fn an_element_of_a_window_moved_without_a_window_manager_is_placed_where_it_is_drawn() {
        let node = Node {
            toolkit: Some("accesskit"),
            screen: Some(Rect::new(8.0, 57.0, 67.0, 20.0)),
            window: Some(Rect::new(8.0, 57.0, 67.0, 20.0)),
            ..Node::inside(window_reference((310.0, 210.0), Some((0.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(318.0, 267.0, 67.0, 20.0)));
    }

    #[test]
    fn on_x11_screen_extents_are_the_bounds() {
        let node = Node {
            toolkit: Some("qt6"),
            desktop: true,
            screen: Some(Rect::new(323.0, 291.0, 67.0, 20.0)),
            ..Node::inside(window_reference((315.0, 234.0), Some((315.0, 234.0))))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(323.0, 291.0, 67.0, 20.0)));
    }

    // --- *A container without extents does not break placement* ---

    #[test]
    fn a_node_below_a_container_without_geometry_is_placed() {
        let window = window_reference((10.0, 40.0), Some((0.0, 0.0)));
        let container = Node::inside(window);
        assert_eq!(resolve_bounds(&container), None, "the container has no bounds of its own");

        let node = Node {
            screen: Some(Rect::new(30.0, 20.0, 80.0, 24.0)),
            ..Node::inside(resolve_reference(&container).expect("the container passes its window on"))
        };
        assert_eq!(resolve_bounds(&node), Some(Rect::new(40.0, 60.0, 80.0, 24.0)));
    }

    /// GTK 3 reports `G_MININT` for widgets it does not show, and a zero size
    /// is no geometry either.
    #[test]
    fn rectangles_at_g_minint_or_without_size_are_no_geometry() {
        let window = window_reference((10.0, 40.0), Some((0.0, 0.0)));
        let offscreen = Node {
            desktop: true,
            screen: Some(Rect::new(f64::from(i32::MIN), f64::from(i32::MIN), 1.0, 1.0)),
            ..Node::inside(window)
        };
        let empty = Node { desktop: true, screen: Some(Rect::new(20.0, 20.0, 0.0, 16.0)), ..Node::inside(window) };
        assert_eq!(resolve_bounds(&offscreen), None);
        assert_eq!(resolve_bounds(&empty), None);
    }

    // --- *A popup is placed where the platform shows it* ---

    /// The measured GTK 4 submenu under the `PlatynUI` compositor.
    fn gtk4_submenu() -> PopupGeometry {
        PopupGeometry { bounds: Rect::new(265.0, 179.0, 158.0, 82.0), surface: Rect::new(242.0, 158.0, 204.0, 128.0) }
    }

    fn gtk4_popover(ancestor: Reference, popups: Vec<PopupGeometry>) -> Node {
        Node {
            popup: true,
            toolkit: Some("gtk4"),
            screen: Some(Rect::new(0.0, 0.0, 204.0, 128.0)),
            window: Some(Rect::new(127.0, 60.0, 204.0, 128.0)),
            popups: Some(popups),
            ..Node::inside(ancestor)
        }
    }

    #[test]
    fn a_popup_with_a_shadow_is_matched_by_its_surface() {
        let menu = Reference { popup_depth: 1, ..window_reference((24.0, 22.0), None) };
        let popover = gtk4_popover(menu, vec![gtk4_submenu()]);
        assert_eq!(resolve_bounds(&popover), Some(gtk4_submenu().bounds));

        let reference = resolve_reference(&popover).expect("a matched popup places its descendants");
        assert_eq!(reference.popup_depth, 2, "a submenu is nested in the menu that opened it");
        let item = Node {
            toolkit: Some("gtk4"),
            window: Some(Rect::new(150.0, 81.0, 158.0, 25.0)),
            ..Node::inside(reference)
        };
        assert_eq!(resolve_bounds(&item).map(|rect| rect.position()), Some(Point::new(265.0, 179.0)));
        assert_eq!(resolve_reference(&item).map(|r| r.popup_depth), Some(2), "an item inherits its popup");
    }

    /// A Qt context menu cascade (measured): root menu, submenu and nested
    /// submenu, listed most recent first, told apart by size, within rounding.
    #[test]
    fn grafted_popups_are_told_apart_by_size() {
        let listed = [
            PopupGeometry::from_bounds(Rect::new(715.0, 490.0, 136.0, 26.0)),
            PopupGeometry::from_bounds(Rect::new(581.0, 438.0, 134.0, 78.0)),
            PopupGeometry::from_bounds(Rect::new(461.0, 360.0, 120.0, 104.0)),
        ];
        for (size, expected) in [((121.0, 103.0), 2), ((134.0, 78.0), 1), ((136.0, 26.0), 0)] {
            let menu = Node {
                popup: true,
                screen: Some(Rect::new(0.0, 0.0, size.0, size.1)),
                popups: Some(listed.to_vec()),
                ..Node::default()
            };
            assert_eq!(resolve_bounds(&menu), Some(listed[expected].bounds), "size {size:?}");
        }
    }

    /// The measured GTK 3 `menu-file` menu under the `PlatynUI` compositor.
    fn gtk3_menu() -> PopupGeometry {
        PopupGeometry { bounds: Rect::new(34.0, 105.0, 177.0, 93.0), surface: Rect::new(28.0, 100.0, 189.0, 105.0) }
    }

    /// GTK 3's menu node has the visible size of its popup, not the size of
    /// the surface around its shadow; it counts from the visible origin.
    #[test]
    fn a_popup_of_its_visible_size_counts_from_its_visible_origin() {
        let menu = Node {
            popup: true,
            toolkit: Some("gtk3"),
            screen: Some(Rect::new(6.0, 32.0, 177.0, 93.0)),
            window: Some(Rect::new(6.0, 5.0, 177.0, 93.0)),
            popups: Some(vec![gtk3_menu()]),
            ..Node::inside(window_reference((10.0, 10.0), Some((0.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&menu), Some(gtk3_menu().bounds));
        let reference = resolve_reference(&menu).expect("a matched popup places its descendants");
        assert_eq!(reference.origin, gtk3_menu().bounds.position());
        assert_eq!(reference.screen, Some(Point::new(6.0, 32.0)));
    }

    /// GTK 3 lists a menu's items, and the menu's popup beside them, below the
    /// menu-bar item that opened it: that item places its children from the
    /// popup. Measured: `menu-file-new` is drawn at `(40,111)`.
    #[test]
    fn a_gtk3_menu_places_the_items_listed_beside_it() {
        let popup = Reference {
            origin: gtk3_menu().bounds.position(),
            screen: Some(Point::new(6.0, 32.0)),
            window: Some(Point::new(6.0, 5.0)),
            popup_depth: 1,
        };
        let opener = Node {
            popup: true,
            toolkit: Some("gtk3"),
            screen: Some(Rect::new(24.0, 68.0, 75.0, 27.0)),
            popups: Some(vec![gtk3_menu()]),
            child_popup: Some(popup),
            ..Node::inside(window_reference((10.0, 10.0), Some((0.0, 0.0))))
        };
        assert_eq!(resolve_bounds(&opener), Some(Rect::new(34.0, 78.0, 75.0, 27.0)), "the opener stays in its window");
        assert_eq!(resolve_reference(&opener), Some(popup));

        let item = Node {
            popup: true,
            toolkit: Some("gtk3"),
            screen: Some(Rect::new(12.0, 38.0, 165.0, 27.0)),
            window: Some(Rect::new(12.0, 11.0, 165.0, 27.0)),
            popups: Some(vec![gtk3_menu()]),
            ..Node::inside(popup)
        };
        assert_eq!(resolve_bounds(&item), Some(Rect::new(40.0, 111.0, 165.0, 27.0)));
    }

    #[test]
    fn popups_of_equal_size_resolve_to_the_first_one_listed() {
        let first = gtk4_submenu();
        let second = PopupGeometry {
            bounds: Rect::new(600.0, 300.0, 158.0, 82.0),
            surface: Rect::new(577.0, 279.0, 204.0, 128.0),
        };
        let popover = gtk4_popover(window_reference((24.0, 22.0), None), vec![first, second]);
        assert_eq!(resolve_bounds(&popover), Some(first.bounds));
    }

    #[test]
    fn a_popup_the_platform_does_not_list_keeps_its_window_placement() {
        let window = window_reference((24.0, 22.0), None);
        let unlisted = PopupGeometry::from_bounds(Rect::new(500.0, 500.0, 80.0, 40.0));
        let in_window = Rect::new(151.0, 82.0, 204.0, 128.0);
        for popups in [Some(vec![unlisted]), None] {
            let popover = Node { popups, ..gtk4_popover(window, Vec::new()) };
            assert_eq!(resolve_bounds(&popover), Some(in_window));
            assert_eq!(resolve_reference(&popover), Some(window), "an unmatched popup is no reference window");
        }
    }
}
