//! The backend surface the Java provider routes to.

use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// One toolkit channel behind the single Java provider.
///
/// The surface deliberately mirrors what a `UiTreeProvider` already does per
/// top-level window — discover, serve a subtree, hit-test, take the window
/// manager, wind down — so a provider-shaped implementation wraps without
/// restructuring.
///
/// Nodes travel to the runtime unwrapped: `UiNode::is_valid` (load-bearing for
/// scoped-root reuse), the pattern set and `@Technology` are the backend's own
/// answers, and the router must never proxy them.
pub trait JavaBackend: Send + Sync {
    /// Stable backend id — also its config sub-map (`providers.java.<id>.*`)
    /// and the name used in diagnostics.
    fn id(&self) -> &'static str;

    /// One enumeration pass under `parent`.
    ///
    /// A backend that is disabled, unavailable, or has nothing to contribute
    /// returns an empty [`Enumeration`]; enumerating must never fail the
    /// runtime.
    fn enumerate(&self, parent: &Arc<dyn UiNode>) -> Enumeration;

    /// Hit-test a desktop point, with the same contract as
    /// `UiTreeProvider::element_at_point`: `Ok(Some)` for a hit, `Ok(None)`
    /// for "mine, but nothing there", and `UnsupportedOperation` for "not
    /// mine" — which is what makes the router try the next backend.
    ///
    /// # Errors
    ///
    /// `UnsupportedOperation` when the point is not this backend's to answer,
    /// or the backend's own error when a call against the target fails.
    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError>;

    /// Inject the runtime's window manager so the backend's window nodes drive
    /// this runtime's session.
    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>);

    /// Release whatever the backend holds. Idempotent.
    fn shutdown(&self);
}

/// What one backend contributes in a single enumeration pass.
#[derive(Default)]
pub struct Enumeration {
    /// Native handles of the top-level windows this backend can serve. These
    /// become the Java provider's window claims: a Java window is claimed
    /// exactly when a backend can serve it, so a Java window no backend
    /// reaches stays with the platform's native provider instead of being
    /// claimed and served empty.
    pub served_windows: Vec<u64>,
    /// The nodes to attach under the enumerated parent — window nodes plus
    /// whatever application-level nodes the backend groups them into.
    pub nodes: Vec<Arc<dyn UiNode>>,
    /// Java-looking top-level windows this backend recognised but cannot
    /// serve. The router turns the ones no backend claimed into the shared
    /// enablement diagnostic — which is why the backend reports them instead
    /// of diagnosing them itself: only the router knows about the others.
    pub unserved: Vec<UnservedJavaWindow>,
    /// Processes behind the Java windows this backend saw, **served or not**.
    ///
    /// Separate from [`Self::unserved`] because the two answer different
    /// questions. "Nobody can serve this window" is a diagnostic. "This JVM has
    /// no agent" is an *opportunity*: a window the Access Bridge serves
    /// perfectly well still has a higher-fidelity representation available, and
    /// automatic attachment exists to take it. Deriving the second from the
    /// first would mean only ever attaching to JVMs that are already broken.
    pub java_processes: Vec<u32>,
}

/// Which backend serves which top-level window right now.
///
/// Two backends can reach the same Swing window — the in-JVM agent and the
/// Access Bridge both see it — and exactly one may serve it, or it appears
/// twice. The router settles that by **preference rank**: a backend's position
/// in the provider's backend list, strongest first.
///
/// Ownership is recorded incrementally *during* a pass, right after each backend
/// enumerates, so the next (weaker) backend already sees it in the same pass —
/// otherwise the very first enumeration would show both trees and only later
/// passes would agree. And it is never cleared wholesale, because an
/// `app:Application` node enumerates its windows lazily, between passes, and has
/// to get a valid answer then too.
#[derive(Default)]
pub(crate) struct BackendOwnership {
    by_window: RwLock<HashMap<u64, usize>>,
}

impl BackendOwnership {
    /// Record what the backend at `rank` serves as of now, replacing whatever it
    /// served before.
    ///
    /// The strongest rank offered wins, rather than the first one recorded. Both
    /// halves of that matter: within a pass it keeps a weaker backend from
    /// overwriting a stronger one's entry, and across passes it lets a stronger
    /// backend take a window over from the incumbent — which is precisely the
    /// mid-session case of an agent appearing in an already-served JVM.
    pub(crate) fn record(&self, rank: usize, served: &[u64]) {
        let mut owners = self.by_window.write().expect("ownership map poisoned");
        owners.retain(|_, owner| *owner != rank);
        for window in served {
            owners.entry(*window).and_modify(|owner| *owner = (*owner).min(rank)).or_insert(rank);
        }
    }

    /// One backend's view of the map.
    pub(crate) fn view(self: &Arc<Self>, rank: usize) -> Arc<ForeignWindows> {
        Arc::new(ForeignWindows { ownership: Arc::clone(self), rank })
    }

    fn owner_of(&self, window: u64) -> Option<usize> {
        self.by_window.read().expect("ownership map poisoned").get(&window).copied()
    }
}

/// The windows a *stronger* backend serves, from one backend's point of view.
///
/// Preference-aware on purpose: "already owned" is not enough to exclude a
/// window, because that is exactly the mid-session case the routing has to get
/// right — when an agent appears in a JVM the Access Bridge is already serving,
/// the agent must take the window over, not defer to the incumbent.
pub(crate) struct ForeignWindows {
    ownership: Arc<BackendOwnership>,
    rank: usize,
}

impl ForeignWindows {
    /// Whether a stronger backend than this one serves `window`.
    pub(crate) fn is_foreign(&self, window: u64) -> bool {
        self.ownership.owner_of(window).is_some_and(|owner| owner < self.rank)
    }
}

/// A Java-looking top-level window a backend cannot reach — on Windows the
/// signature of a Swing application whose Access Bridge is not enabled.
pub struct UnservedJavaWindow {
    /// Native window handle, in the raw form the claims and diagnostic
    /// registries key on.
    pub window: u64,
    /// Owning process.
    pub pid: u32,
    /// Platform window class; the toolkit discriminator
    /// (`platynui_core::platform::java::JavaToolkit::from_window_class`).
    pub class_name: String,
}
