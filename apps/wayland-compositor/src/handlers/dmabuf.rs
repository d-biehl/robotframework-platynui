//! `linux-dmabuf-v1` handler — GPU buffer sharing.
//!
//! DMA-BUF allows clients (Chromium, Firefox, Electron, Vulkan apps) to share
//! GPU buffers directly with the compositor without copies. The compositor
//! advertises supported formats and handles buffer imports.

use smithay::backend::renderer::ImportDma;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};

use crate::state::State;

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: ImportNotifier,
    ) {
        if let Some(renderer) = self.screenshot_renderer.as_mut()
            && let Err(err) = renderer.import_dmabuf(&dmabuf, None)
        {
            tracing::debug!(%err, "DMA-BUF import validation failed");
            return;
        }

        if let Err(err) = notifier.successful::<State>() {
            tracing::warn!(%err, "failed to signal successful dmabuf import");
        }
    }
}
