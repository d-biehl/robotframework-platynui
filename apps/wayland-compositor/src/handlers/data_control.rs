//! Data-control protocol handlers (`wlr-data-control-v1` + `ext-data-control-v1`).
//!
//! Enables clipboard read/write without window focus — required by
//! `wl-copy`/`wl-paste` and programmatic clipboard testing in `PlatynUI`.
//!
//! Both protocols provide functionally identical clipboard management:
//! - `wlr-data-control-v1` — wlroots-originated, widely adopted (Sway, wlr-based compositors)
//! - `ext-data-control-v1` — standardized staging version (Mutter, `KWin`, future default)

use smithay::wayland::selection::ext_data_control;
use smithay::wayland::selection::wlr_data_control;

use crate::state::State;

impl wlr_data_control::DataControlHandler for State {
    fn data_control_state(&mut self) -> &mut wlr_data_control::DataControlState {
        &mut self.data_control_state
    }
}

impl ext_data_control::DataControlHandler for State {
    fn data_control_state(&mut self) -> &mut ext_data_control::DataControlState {
        &mut self.ext_data_control_state
    }
}
