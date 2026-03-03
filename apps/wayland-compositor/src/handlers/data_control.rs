//! Data-control protocol handlers (`wlr-data-control-v1` + `ext-data-control-v1`).
//!
//! Enables clipboard read/write without window focus — required by
//! `wl-copy`/`wl-paste` and programmatic clipboard testing in `PlatynUI`.

use smithay::wayland::selection::wlr_data_control::DataControlHandler;

use crate::state::State;

impl DataControlHandler for State {
    fn data_control_state(&self) -> &smithay::wayland::selection::wlr_data_control::DataControlState {
        &self.data_control_state
    }
}
