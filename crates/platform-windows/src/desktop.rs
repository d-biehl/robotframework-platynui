#![cfg(target_os = "windows")]

use std::env;

use platynui_core::platform::{
    DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError, PlatformErrorKind,
};
use platynui_core::register_desktop_info_provider;
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

static WINDOWS_DESKTOP_PROVIDER: WindowsDesktopProvider = WindowsDesktopProvider;

register_desktop_info_provider!(&WINDOWS_DESKTOP_PROVIDER);

struct WindowsDesktopProvider;

impl DesktopInfoProvider for WindowsDesktopProvider {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } as f64;
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } as f64;
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f64;
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f64;

        if width <= 0.0 || height <= 0.0 {
            return Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "virtual screen dimensions unavailable",
            ));
        }

        let mut monitor = MonitorInfo::new("virtual-screen", Rect::new(left, top, width, height));
        monitor.is_primary = true;

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from("windows://desktop"),
            name: "Windows Desktop".into(),
            technology: TechnologyId::from("Windows"),
            bounds: Rect::new(left, top, width, height),
            os_name: env::consts::OS.into(),
            os_version: env::consts::ARCH.into(),
            monitors: vec![monitor],
        })
    }
}
