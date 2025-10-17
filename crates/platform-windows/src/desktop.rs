use std::collections::HashMap;
use std::env;
use std::mem::size_of;

use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError, PlatformErrorKind};
use platynui_core::register_desktop_info_provider;
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{LPARAM, RECT, WIN32_ERROR};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::SystemInformation::GetVersion;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows::core::BOOL;

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

        // Enumerate physical monitors; on any failure, fall back to no monitors.
        let monitors = unsafe { enumerate_monitors().unwrap_or_else(|_| Vec::new()) };

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from("windows://desktop"),
            name: "Windows Desktop".into(),
            technology: TechnologyId::from("Windows"),
            bounds: Rect::new(left, top, width, height),
            os_name: env::consts::OS.into(),
            os_version: os_version_string(),
            monitors,
        })
    }
}

unsafe fn enumerate_monitors() -> Result<Vec<MonitorInfo>, PlatformError> {
    extern "system" fn enum_proc(
        hmonitor: windows::Win32::Graphics::Gdi::HMONITOR,
        _hdc: HDC,
        _rc: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        unsafe {
            let list = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
            let mut infoex: MONITORINFOEXW = MONITORINFOEXW {
                monitorInfo: MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() },
                szDevice: [0u16; 32],
            };
            // Windows expects cbSize for MONITORINFO; using MONITORINFOEXW requires setting to its size
            infoex.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(hmonitor, &mut infoex as *mut MONITORINFOEXW as *mut MONITORINFO) == BOOL(0) {
                return BOOL(1);
            }
            let r = infoex.monitorInfo.rcMonitor;
            let bounds = Rect::new(r.left as f64, r.top as f64, (r.right - r.left) as f64, (r.bottom - r.top) as f64);
            let is_primary = (infoex.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY = 0x00000001
            let id = trim_wstr(&infoex.szDevice);
            let friendly_dc = FRIENDLY_NAMES.get_or_init(build_friendly_name_map).get(&id).cloned();
            let friendly_enum = monitor_friendly_name(&infoex);
            let friendly = friendly_dc.or(friendly_enum);
            let mut monitor = MonitorInfo::new(id.clone(), bounds);
            monitor.is_primary = is_primary;
            monitor.name = Some(friendly.unwrap_or(id));
            // Try to determine per-monitor scale factor via effective DPI
            let mut dpix: u32 = 0;
            let mut dpiy: u32 = 0;
            if GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpix, &mut dpiy).is_ok() && dpix > 0 {
                monitor.scale_factor = Some(dpix as f64 / 96.0);
            }
            list.push(monitor);
            BOOL(1)
        }
    }

    use windows::Win32::Graphics::Gdi::HDC;
    let mut list: Vec<MonitorInfo> = Vec::new();
    let lparam = LPARAM(&mut list as *mut _ as isize);
    let ok = unsafe { EnumDisplayMonitors(None, None, Some(enum_proc), lparam) };
    if !ok.as_bool() {
        return Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "EnumDisplayMonitors failed"));
    }
    Ok(list)
}

fn trim_wstr(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

unsafe fn monitor_friendly_name(infoex: &MONITORINFOEXW) -> Option<String> {
    // Try to resolve a human-friendly monitor name via EnumDisplayDevicesW
    let mut dd: DISPLAY_DEVICEW = DISPLAY_DEVICEW { cb: size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
    let ok = unsafe { EnumDisplayDevicesW(windows::core::PCWSTR(infoex.szDevice.as_ptr()), 0, &mut dd, 0) };
    if ok.as_bool() {
        let s = trim_wstr(&dd.DeviceString);
        if !s.trim().is_empty() {
            return Some(s);
        }
    }
    None
}

static FRIENDLY_NAMES: once_cell::sync::OnceCell<HashMap<String, String>> = once_cell::sync::OnceCell::new();

fn build_friendly_name_map() -> HashMap<String, String> {
    // Map from "\\\.\DISPLAYn" → "Friendly Monitor Name"
    let mut map = HashMap::new();
    unsafe {
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count) != WIN32_ERROR(0) {
            return map;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        ) != WIN32_ERROR(0)
        {
            return map;
        }
        let paths = &paths[..path_count as usize];
        for p in paths {
            // Source device name (\\.\DISPLAYn)
            let mut src: DISPLAYCONFIG_SOURCE_DEVICE_NAME = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: p.sourceInfo.adapterId,
                    id: p.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let gdi_name = trim_wstr(&src.viewGdiDeviceName);

            // Target friendly name
            let mut tgt: DISPLAYCONFIG_TARGET_DEVICE_NAME = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: p.targetInfo.adapterId,
                    id: p.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut tgt.header) != 0 {
                continue;
            }
            let friendly = trim_wstr(&tgt.monitorFriendlyDeviceName);
            if !gdi_name.is_empty() && !friendly.is_empty() {
                map.insert(gdi_name, friendly);
            }
        }
    }
    map
}

fn os_version_string() -> String {
    unsafe {
        let v = GetVersion();
        let major = v & 0xFF;
        let minor = (v >> 8) & 0xFF;
        let build = if (v & 0x8000_0000) == 0 { (v >> 16) & 0xFFFF } else { 0 };
        if build != 0 { format!("{}.{}.{}", major, minor, build) } else { format!("{}.{}", major, minor) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_wstr_stops_at_nul() {
        let buf = [65u16, 66, 0, 67, 68]; // "AB\0CD"
        assert_eq!(trim_wstr(&buf), "AB");
    }

    #[test]
    fn os_version_string_is_non_empty() {
        let s = os_version_string();
        assert!(!s.is_empty());
    }

    // This file only builds on Windows; provide a smoke test that exercises
    // the DesktopInfo provider and checks basic invariants without being too strict
    // about environment-specific layouts (RDP/headless).
    #[test]
    fn windows_desktop_info_smoke() {
        let info = WINDOWS_DESKTOP_PROVIDER.desktop_info().expect("desktop info");
        assert!(info.bounds.width() > 0.0 && info.bounds.height() > 0.0, "desktop bounds must be positive");

        let count = info.display_count();
        if count > 0 {
            // Exactly one primary monitor should be reported.
            let primary = info.monitors.iter().filter(|m| m.is_primary).count();
            assert_eq!(primary, 1, "exactly one primary monitor expected");

            // Union of monitor rects should be within (or equal to) the virtual screen bounds.
            let (l, t, r, b) = info.monitors.iter().fold(
                (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
                |acc, m| {
                    let (mut l, mut t, mut r, mut b) = acc;
                    l = l.min(m.bounds.x());
                    t = t.min(m.bounds.y());
                    r = r.max(m.bounds.right());
                    b = b.max(m.bounds.bottom());
                    (l, t, r, b)
                },
            );
            let union = Rect::new(l, t, r - l, b - t);
            let within = union.x() >= info.bounds.x()
                && union.y() >= info.bounds.y()
                && union.right() <= info.bounds.right()
                && union.bottom() <= info.bounds.bottom();
            assert!(within, "union({union}) must be within desktop bounds {}", info.bounds);

            // Names/IDs should not be empty (friendly name may still be generic depending on system).
            assert!(info.monitors.iter().all(|m| !m.id.trim().is_empty()), "device ids must be non-empty");
            assert!(
                info.monitors.iter().all(|m| !m.name.as_deref().unwrap_or("").trim().is_empty()),
                "monitor names should be present"
            );
        }
    }
}
