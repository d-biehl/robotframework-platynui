//! Signal handling — SIGTERM/SIGINT for graceful shutdown, watchdog timer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared flag set by signal handlers to request shutdown.
pub struct ShutdownFlag {
    flag: Arc<AtomicBool>,
}

impl ShutdownFlag {
    /// Register signal handlers for SIGTERM and SIGINT.
    ///
    /// Returns a `ShutdownFlag` that can be polled to check if shutdown was requested.
    pub fn register() -> Result<Self, Box<dyn std::error::Error>> {
        let flag = Arc::new(AtomicBool::new(false));

        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&flag))?;
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&flag))?;

        Ok(Self { flag })
    }

    /// Check if a shutdown signal has been received.
    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

/// Register a watchdog timer that shuts down the compositor after `timeout`.
///
/// This prevents indefinite hangs in CI environments.
pub fn register_watchdog(
    loop_handle: &calloop::LoopHandle<'static, crate::state::State>,
    timeout: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let timer = calloop::timer::Timer::from_duration(timeout);
    let timeout_secs = timeout.as_secs();
    loop_handle.insert_source(timer, move |_deadline, _metadata, state| {
        tracing::info!(elapsed_secs = timeout_secs, "watchdog timeout reached, shutting down");
        state.running = false;
        calloop::timer::TimeoutAction::Drop
    })?;
    Ok(())
}
