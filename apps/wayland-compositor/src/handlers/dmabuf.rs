//! `linux-dmabuf-v1` handler — GPU buffer sharing.
//!
//! DMA-BUF allows clients (Chromium, Firefox, Electron, Vulkan apps) to share
//! GPU buffers directly with the compositor without copies. The compositor
//! advertises supported formats and handles buffer imports.

use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};

use crate::state::State;

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        _dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: ImportNotifier,
    ) {
        // For the test compositor, we accept all dmabufs.
        // A real compositor would attempt renderer import here.
        let _ = notifier.successful::<State>();
    }
}
