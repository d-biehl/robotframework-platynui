use super::{ProviderDescriptor, ProviderError, ProviderEventListener};
use crate::platform::WindowManager;
use crate::types::Point;
use crate::ui::UiNode;
use std::sync::Arc;

/// Core interface implemented by every UI tree provider.
pub trait UiTreeProvider: Send + Sync {
    /// Returns static metadata describing this provider.
    fn descriptor(&self) -> &ProviderDescriptor;

    /// Returns an iterator over nodes that should be attached to the given
    /// parent (typically the runtime-managed desktop or an application node).
    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError>;

    /// Resolves the deepest, topmost UI node at the given desktop (screen) point.
    ///
    /// The point is interpreted in the same desktop coordinate space the
    /// platform's cursor-position reporting produces. The result respects
    /// window and layer z-order (occlusion): when nodes overlap, the visually
    /// frontmost one wins. The returned node is equivalent — same runtime
    /// identity and parent chain — to the node top-down traversal would reach,
    /// so callers can feed it into ancestor-walking consumers (tree reveal).
    ///
    /// Returns `Ok(Some(node))` for a hit, `Ok(None)` when the provider
    /// supports hit-testing but nothing is at the point, and
    /// `Err(ProviderError::UnsupportedOperation { .. })` when this provider (or
    /// the current platform) cannot hit-test at all — the default. Callers use
    /// the distinct `UnsupportedOperation` to disable point-based features
    /// rather than treating an unsupported provider as an empty result.
    fn element_at_point(&self, _point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        Err(ProviderError::UnsupportedOperation { operation: "element_at_point", details: None })
    }

    /// Registers a listener for provider-originated events. The default
    /// implementation does nothing so providers without event support can
    /// ignore this call.
    fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Injects the runtime's window manager (from its platform bundle) so the
    /// provider's window nodes can drive window operations against this
    /// runtime's session — instead of reaching a process-global. Called once by
    /// the runtime after the platform bundle is built. The default ignores it;
    /// providers that expose no window surfaces need no window manager.
    fn set_window_manager(&self, _window_manager: Arc<dyn WindowManager>) {}

    /// Allows the runtime to signal shutdown so the provider can release resources.
    fn shutdown(&self) {}
}
