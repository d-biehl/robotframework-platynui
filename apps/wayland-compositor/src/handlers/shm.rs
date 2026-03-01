//! `wl_shm` buffer handler.

use smithay::wayland::{
    buffer::BufferHandler,
    shm::{ShmHandler, ShmState},
};

use crate::state::State;

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {
        // Renderers handle buffer cleanup internally
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
