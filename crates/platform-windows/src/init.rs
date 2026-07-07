use std::sync::OnceLock;

use platynui_core::platform::PlatformError;
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::core::HRESULT;

/// Memoised result of setting the process DPI-awareness context.
///
/// Configuring DPI awareness is a genuine once-per-process Win32 operation, so
/// the result is cached here and shared by every runtime bundle built in this
/// process (see [`crate::factory::create_windows_bundle`]). This is deliberately
/// process-global — unlike a session connection there is nothing per-runtime to
/// rebuild.
static DPI_AWARENESS: OnceLock<Result<(), PlatformError>> = OnceLock::new();

pub(crate) fn ensure_dpi_awareness() -> Result<(), PlatformError> {
    DPI_AWARENESS.get_or_init(set_dpi_awareness).clone()
}

fn set_dpi_awareness() -> Result<(), PlatformError> {
    unsafe {
        match SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            Ok(()) => {
                tracing::info!("DPI awareness set to PerMonitorAwareV2");
                Ok(())
            }
            Err(err) => {
                let access_denied = HRESULT::from_win32(ERROR_ACCESS_DENIED.0);
                if err.code() == access_denied {
                    tracing::debug!("DPI awareness already set (ACCESS_DENIED — process manifest)");
                    Ok(())
                } else {
                    tracing::error!(?err, "SetProcessDpiAwarenessContext failed");
                    Err(PlatformError::CapabilityUnavailable {
                        capability: "DPI awareness",
                        details: Some(format!("SetProcessDpiAwarenessContext failed: {err:?}")),
                    })
                }
            }
        }
    }
}
