//! `wl_output` + `xdg-output-manager-v1` handler.

// Output management in smithay is handled via `delegate_output!()` in state.rs.
// No additional handler trait is required — `OutputManagerState` handles
// the `wl_output` and `xdg_output_manager` globals automatically.
//
// This module exists as a placeholder for future output-related logic
// (e.g., dynamic output creation, hot-plug handling).
