use std::sync::OnceLock;

use platynui_core::platform::{PlatformError, PlatformErrorKind, PlatformModule};
use platynui_core::register_platform_module;
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::core::HRESULT;

struct WindowsPlatform;

static WINDOWS_PLATFORM: WindowsPlatform = WindowsPlatform;

register_platform_module!(&WINDOWS_PLATFORM);

impl PlatformModule for WindowsPlatform {
    fn name(&self) -> &'static str {
        "Windows Platform"
    }

    fn initialize(&self) -> Result<(), PlatformError> {
        ensure_dpi_awareness()
    }
}

static DPI_AWARENESS: OnceLock<Result<(), PlatformError>> = OnceLock::new();

pub(crate) fn ensure_dpi_awareness() -> Result<(), PlatformError> {
    DPI_AWARENESS.get_or_init(set_dpi_awareness).clone()
}

fn set_dpi_awareness() -> Result<(), PlatformError> {
    unsafe {
        match SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            Ok(()) => Ok(()),
            Err(err) => {
                let access_denied = HRESULT::from_win32(ERROR_ACCESS_DENIED.0);
                if err.code() == access_denied {
                    Ok(())
                } else {
                    Err(PlatformError::new(
                        PlatformErrorKind::CapabilityUnavailable,
                        format!("SetProcessDpiAwarenessContext failed: {err:?}"),
                    ))
                }
            }
        }
    }
}
