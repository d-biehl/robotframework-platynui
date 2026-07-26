//! The backend surface the Java provider routes to.

use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use std::sync::Arc;

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
