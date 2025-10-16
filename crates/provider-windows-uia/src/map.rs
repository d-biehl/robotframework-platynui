#![cfg(target_os = "windows")]
use once_cell::sync::OnceCell;
use platynui_core::types::Point as UiPoint;
use platynui_core::types::Rect;
use platynui_core::ui::UiValue;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::POINT;
use windows::Win32::Foundation::{DECIMAL, FILETIME, VARIANT_BOOL};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Ole::VarR8FromDec;
use windows::Win32::System::Ole::{SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
use windows::Win32::System::SystemInformation::{
    GetNativeSystemInfo, PROCESSOR_ARCHITECTURE, PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
    PROCESSOR_ARCHITECTURE_INTEL, SYSTEM_INFO,
};
use windows::Win32::System::Threading::{GetProcessTimes, OpenProcessToken};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    QueryFullProcessImageNameW,
};
use windows::Win32::System::Time::FileTimeToSystemTime;
use windows::Win32::System::Variant::{VARIANT, VariantClear};
use windows::Win32::System::Variant::{
    VT_ARRAY, VT_BOOL, VT_BSTR, VT_BYREF, VT_DATE, VT_DECIMAL, VT_EMPTY, VT_I2, VT_I4, VT_I8, VT_R4, VT_R8,
    VT_TYPEMASK, VT_UI2, VT_UI4, VT_UI8, VT_UNKNOWN,
};
use windows::Win32::UI::Accessibility::*;
use windows::core::BSTR;
use windows::core::Interface;
use windows::core::PWSTR;

// Use VARENUM constants from the windows crate instead of redefining magic numbers

/// Maps UIA ControlType IDs to PlatynUI role names.
/// Namespace wird an anderer Stelle bestimmt (IsControlElement/IsContentElement),
/// daher liefert diese Funktion nur die Role.
pub fn control_type_to_role(control_type: i32) -> &'static str {
    match control_type {
        x if x == UIA_ButtonControlTypeId.0 => "Button",
        x if x == UIA_CalendarControlTypeId.0 => "Calendar",
        x if x == UIA_CheckBoxControlTypeId.0 => "CheckBox",
        x if x == UIA_ComboBoxControlTypeId.0 => "ComboBox",
        x if x == UIA_EditControlTypeId.0 => "Edit",
        x if x == UIA_HyperlinkControlTypeId.0 => "Hyperlink",
        x if x == UIA_ImageControlTypeId.0 => "Image",
        x if x == UIA_ListItemControlTypeId.0 => "ListItem",
        x if x == UIA_ListControlTypeId.0 => "List",
        x if x == UIA_MenuControlTypeId.0 => "Menu",
        x if x == UIA_MenuBarControlTypeId.0 => "MenuBar",
        x if x == UIA_MenuItemControlTypeId.0 => "MenuItem",
        x if x == UIA_ProgressBarControlTypeId.0 => "ProgressBar",
        x if x == UIA_RadioButtonControlTypeId.0 => "RadioButton",
        x if x == UIA_ScrollBarControlTypeId.0 => "ScrollBar",
        x if x == UIA_SliderControlTypeId.0 => "Slider",
        x if x == UIA_SpinnerControlTypeId.0 => "Spinner",
        x if x == UIA_StatusBarControlTypeId.0 => "StatusBar",
        x if x == UIA_TabControlTypeId.0 => "Tab",
        x if x == UIA_TabItemControlTypeId.0 => "TabItem",
        x if x == UIA_TextControlTypeId.0 => "Text",
        x if x == UIA_ToolBarControlTypeId.0 => "ToolBar",
        x if x == UIA_ToolTipControlTypeId.0 => "ToolTip",
        x if x == UIA_TreeControlTypeId.0 => "Tree",
        x if x == UIA_TreeItemControlTypeId.0 => "TreeItem",
        x if x == UIA_CustomControlTypeId.0 => "Custom",
        x if x == UIA_GroupControlTypeId.0 => "Group",
        x if x == UIA_ThumbControlTypeId.0 => "Thumb",
        x if x == UIA_DataGridControlTypeId.0 => "DataGrid",
        x if x == UIA_DataItemControlTypeId.0 => "DataItem",
        x if x == UIA_DocumentControlTypeId.0 => "Document",
        x if x == UIA_SplitButtonControlTypeId.0 => "SplitButton",
        x if x == UIA_WindowControlTypeId.0 => "Window",
        x if x == UIA_PaneControlTypeId.0 => "Pane",
        x if x == UIA_HeaderControlTypeId.0 => "Header",
        x if x == UIA_HeaderItemControlTypeId.0 => "HeaderItem",
        x if x == UIA_TableControlTypeId.0 => "Table",
        x if x == UIA_TitleBarControlTypeId.0 => "TitleBar",
        x if x == UIA_SeparatorControlTypeId.0 => "Separator",
        x if x == UIA_SemanticZoomControlTypeId.0 => "SemanticZoom",
        x if x == UIA_AppBarControlTypeId.0 => "AppBar",
        _ => "Element",
    }
}

pub fn get_name(elem: &IUIAutomationElement) -> Result<String, crate::error::UiaError> {
    unsafe { crate::error::uia_api("IUIAutomationElement::CurrentName", elem.CurrentName()).map(|b| b.to_string()) }
}

pub fn get_control_type(elem: &IUIAutomationElement) -> Result<i32, crate::error::UiaError> {
    unsafe { crate::error::uia_api("IUIAutomationElement::CurrentControlType", elem.CurrentControlType()).map(|v| v.0) }
}

pub fn get_bounding_rect(elem: &IUIAutomationElement) -> Result<Rect, crate::error::UiaError> {
    unsafe {
        let r =
            crate::error::uia_api("IUIAutomationElement::CurrentBoundingRectangle", elem.CurrentBoundingRectangle())?;

        let left = r.left as f64;
        let top = r.top as f64;
        let width = (r.right - r.left).max(0) as f64;
        let height = (r.bottom - r.top).max(0) as f64;
        Ok(Rect::new(left, top, width, height))
    }
}
pub fn get_clickable_point(elem: &IUIAutomationElement) -> Result<UiPoint, crate::error::UiaError> {
    unsafe {
        // UIA returns a POINT in desktop coordinates and a BOOL as return value indicating success
        let mut pt = POINT { x: 0, y: 0 };

        let got_clickable =
            crate::error::uia_api("IUIAutomationElement::GetClickablePoint", elem.GetClickablePoint(&mut pt))?;

        // Check if a clickable point was actually found
        if got_clickable.as_bool() {
            Ok(UiPoint::new(pt.x as f64, pt.y as f64))
        } else {
            Err(crate::error::UiaError::NoClickablePoint)
        }
    }
}

/// Internal helper: returns the hex-dotted RuntimeId body without any scheme/prefix.
fn runtime_id_hex_body(elem: &IUIAutomationElement) -> Result<String, crate::error::UiaError> {
    use windows::Win32::System::Ole::{
        SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
    };
    unsafe {
        let psa = crate::error::uia_api("IUIAutomationElement::GetRuntimeId", elem.GetRuntimeId())?;
        if psa.is_null() {
            return Err(crate::error::UiaError::Null("GetRuntimeId"));
        }
        let lb = crate::error::uia_api("SafeArrayGetLBound", SafeArrayGetLBound(psa, 1))?;
        let ub = crate::error::uia_api("SafeArrayGetUBound", SafeArrayGetUBound(psa, 1))?;
        let count = (ub - lb + 1) as usize;
        let mut data: *mut i32 = std::ptr::null_mut();
        crate::error::uia_api("SafeArrayAccessData", SafeArrayAccessData(psa, &mut data as *mut _ as *mut _))?;
        let slice = std::slice::from_raw_parts(data, count);
        // Keep formatting identical to legacy behavior to avoid breaking changes.
        let body = slice.iter().map(|v| format!("{:x}", v)).collect::<Vec<_>>().join(".");
        crate::error::uia_api("SafeArrayUnaccessData", SafeArrayUnaccessData(psa))?;
        Ok(body)
    }
}

// Note: legacy unscoped formatter removed; use `format_scoped_runtime_id` instead.

/// Scope for composing unique, view-aware RuntimeId URIs within our combined trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiaIdScope {
    /// Desktop TopLevel view
    Desktop,
    /// Application-grouped view; disambiguate by process id
    App { pid: i32 },
}

/// Compose a scoped RuntimeId URI that stays unique across multiple views in our trees.
/// Examples:
///  - Desktop: `uia://desktop/<rid>`
///  - App:     `uia://app/<pid>/<rid>`
pub fn format_scoped_runtime_id(
    elem: &IUIAutomationElement,
    scope: UiaIdScope,
) -> Result<String, crate::error::UiaError> {
    let body = runtime_id_hex_body(elem)?;
    let s = match scope {
        UiaIdScope::Desktop => format!("uia://desktop/{}", body),
        UiaIdScope::App { pid } => format!("uia://app/{}/{}", pid, body),
    };
    Ok(s)
}

pub fn get_is_enabled(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsEnabled", elem.CurrentIsEnabled()).map(|b| b.as_bool())
    }
}

pub fn get_is_offscreen(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsOffscreen", elem.CurrentIsOffscreen())
            .map(|b| b.as_bool())
    }
}

pub fn get_process_id(elem: &IUIAutomationElement) -> Result<i32, crate::error::UiaError> {
    unsafe { crate::error::uia_api("IUIAutomationElement::CurrentProcessId", elem.CurrentProcessId()) }
}

pub fn open_process_query(pid: i32) -> Option<HANDLE> {
    unsafe {
        // Prefer broader rights to allow module queries (base module name), then fall back.
        let full = PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_INFORMATION.0 | PROCESS_VM_READ.0);
        OpenProcess(full, false, pid as u32)
            .ok()
            .or_else(|| OpenProcess(PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0), false, pid as u32).ok())
    }
}

pub fn query_executable_path(handle: HANDLE) -> Option<String> {
    use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    let mut cap: u32 = 4096;
    let max_cap: u32 = 32768; // 32K UTF-16 code units upper bound
    // Loop until success or we hit the max cap
    loop {
        let mut buf: Vec<u16> = vec![0u16; cap as usize];
        let mut size = cap;
        let res = unsafe {
            QueryFullProcessImageNameW(
                handle,
                windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
        };
        match res {
            Ok(()) => {
                let slice = &buf[..size as usize];
                return String::from_utf16(slice).ok();
            }
            Err(e) => {
                // Grow buffer if too small; prefer the returned required size when available
                let hr = e.code();
                // Map common insufficient-buffer case: on Win32 APIs this usually means last-error
                // is ERROR_INSUFFICIENT_BUFFER. Compare using raw value as a pragmatic fallback.
                if hr.0 as u32 == ERROR_INSUFFICIENT_BUFFER.0 {
                    // If API updated 'size' with the required length, use it; otherwise double
                    let next = size.max(cap.saturating_mul(2)).min(max_cap);
                    if next <= cap || next > max_cap {
                        return None;
                    }
                    cap = next;
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

/// Best-effort command line retrieval for a process. Currently not implemented
/// due to complexity of PEB inspection and WMI dependency. Returns None.
pub fn query_process_command_line(handle: HANDLE) -> Option<String> {
    // Use NtQueryInformationProcess(ProcessCommandLineInformation) from ntdll to query
    // the command line UNICODE_STRING. Requires PROCESS_QUERY_INFORMATION | PROCESS_VM_READ.
    unsafe {
        use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
        use windows::Win32::Foundation::{NTSTATUS, STATUS_INFO_LENGTH_MISMATCH, STATUS_SUCCESS};

        let mut cap: u32 = 4096; // start with 4 KB, grow as needed
        let max_cap: u32 = 1 << 20; // 1 MB upper bound for safety
        loop {
            if cap == 0 || cap > max_cap {
                return None;
            }
            let mut buf: Vec<u8> = vec![0u8; cap as usize];
            let mut ret_len: u32 = 0;
            let status: NTSTATUS = NtQueryInformationProcess(
                handle,
                ProcessCommandLineInformation,
                buf.as_mut_ptr() as *mut _,
                cap,
                &mut ret_len as *mut u32,
            );
            if status == STATUS_SUCCESS {
                // Interpret start of buffer as UNICODE_STRING
                #[repr(C)]
                struct UnicodeString {
                    length: u16,
                    max_length: u16,
                    buffer: *const u16,
                }
                let us_ptr = buf.as_ptr() as *const UnicodeString;
                if us_ptr.is_null() {
                    return None;
                }
                let us = &*us_ptr;
                let len_bytes = us.length as usize;
                if len_bytes == 0 || us.buffer.is_null() {
                    return None;
                }
                let len_chars = len_bytes / 2;
                let slice = std::slice::from_raw_parts(us.buffer, len_chars);
                let s = String::from_utf16_lossy(slice);
                return Some(s);
            }
            if status == STATUS_INFO_LENGTH_MISMATCH || ret_len > cap {
                // Grow to the required size if provided, otherwise double
                let next = ret_len.max(cap.saturating_mul(2)).min(max_cap);
                if next <= cap || next > max_cap {
                    return None;
                }
                cap = next;
                continue;
            }
            // Other status codes: give up
            return None;
        }
    }
}
// No argv-splitting; callers consume the raw command line string only.

pub fn query_process_username(handle: HANDLE) -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token).is_err() {
            return None;
        }
        // Query size first
        let mut needed: u32 = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            let _ = CloseHandle(token);
            return None;
        }
        let mut buf = vec![0u8; needed as usize];
        if GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), needed, &mut needed).is_err() {
            let _ = CloseHandle(token);
            return None;
        }
        let tu: &TOKEN_USER = &*(buf.as_ptr() as *const TOKEN_USER);
        let sid = tu.User.Sid;
        // Lookup account name
        let mut name_len: u32 = 0;
        let mut domain_len: u32 = 0;
        let mut use_: SID_NAME_USE = SID_NAME_USE(0);
        // First call with None to query required buffer sizes
        let _ = LookupAccountSidW(None, sid, None, &mut name_len, None, &mut domain_len, &mut use_);
        if name_len == 0 {
            let _ = CloseHandle(token);
            return None;
        }
        let mut name_buf: Vec<u16> = vec![0u16; name_len as usize];
        let mut domain_buf: Vec<u16> = if domain_len > 0 { vec![0u16; domain_len as usize] } else { Vec::new() };
        if LookupAccountSidW(
            None,
            sid,
            Some(PWSTR(name_buf.as_mut_ptr())),
            &mut name_len,
            if domain_len > 0 { Some(PWSTR(domain_buf.as_mut_ptr())) } else { None },
            &mut domain_len,
            &mut use_,
        )
        .is_err()
        {
            let _ = CloseHandle(token);
            return None;
        }
        let name = String::from_utf16_lossy(&name_buf[..(name_len as usize)]);
        let domain =
            if domain_len > 0 { String::from_utf16_lossy(&domain_buf[..(domain_len as usize)]) } else { String::new() };
        let _ = CloseHandle(token);
        if !domain.is_empty() { Some(format!("{}\\{}", domain, name)) } else { Some(name) }
    }
}

pub fn query_process_start_time_iso8601(handle: HANDLE) -> Option<String> {
    unsafe {
        let mut creation: FILETIME = FILETIME::default();
        let mut exit: FILETIME = FILETIME::default();
        let mut kernel: FILETIME = FILETIME::default();
        let mut user: FILETIME = FILETIME::default();
        if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).is_err() {
            return None;
        }
        // Convert to SYSTEMTIME in UTC
        let mut st = windows::Win32::Foundation::SYSTEMTIME::default();
        if FileTimeToSystemTime(&creation, &mut st).is_err() {
            return None;
        }
        // Format as ISO 8601 UTC without timezone conversion
        let s = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
        );
        Some(s)
    }
}

pub fn process_architecture(_handle: HANDLE) -> Option<String> {
    // Fallback: report native system architecture
    let mut info: SYSTEM_INFO = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&mut info) };
    let a: PROCESSOR_ARCHITECTURE = unsafe { info.Anonymous.Anonymous.wProcessorArchitecture };
    let sys_arch = match a {
        x if x == PROCESSOR_ARCHITECTURE_AMD64 => "x64",
        x if x == PROCESSOR_ARCHITECTURE_ARM64 => "arm64",
        x if x == PROCESSOR_ARCHITECTURE_INTEL => "x86",
        _ => "unknown",
    };
    Some(sys_arch.to_string())
}

pub fn process_architecture_from_path(path: &str) -> Option<String> {
    use std::fs::File;
    use std::io::Read;
    let mut f = File::open(path).ok()?;
    let mut header = vec![0u8; 4096];
    let n = f.read(&mut header).ok()?;
    let data = &header[..n];
    // DOS header check
    if data.len() < 0x40 {
        return None;
    }
    if &data[0..2] != b"MZ" {
        return None;
    }
    let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    if data.len() < e_lfanew + 4 + 20 {
        return None;
    }
    if &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }
    // COFF header starts after signature; Machine is WORD at offset 0
    let machine = u16::from_le_bytes([data[e_lfanew + 4], data[e_lfanew + 5]]);
    let arch = match machine {
        0x8664 => "x64",
        0x014c => "x86",
        0xAA64 | 0x1C0 => "arm64", // AA64 = ARM64, 0x1C0 = ARM (fallback to arm64/arm)
        _ => "unknown",
    };
    Some(arch.to_string())
}

// get_activation_point and is_visible moved into attribute-level caching logic.

/// Collects a curated set of native UIA properties and returns only those
/// that appear to be supported (non-empty / meaningful values).
///
/// Note: The UIA COM API does not provide a direct enumeration of all
/// supported properties for an element. We therefore use a catalog of
/// programmatic names (typical UIA property ID range) and read each ID via
/// `GetCurrentPropertyValueEx(id, true)`. Unsupported/mixed values are
/// filtered using the UIA reserved sentinels.
pub fn collect_native_properties(elem: &IUIAutomationElement) -> Vec<(String, UiValue)> {
    // Enumerate UIA property programmatic names in the common range and fetch current values.
    static PROPERTY_CATALOG: OnceCell<Vec<(UIA_PROPERTY_ID, String)>> = OnceCell::new();
    let catalog = PROPERTY_CATALOG.get_or_init(|| {
        let mut list: Vec<(UIA_PROPERTY_ID, String)> = Vec::new();
        if let Ok(uia) = crate::com::uia() {
            for id_num in 30000i32..31050i32 {
                let id = UIA_PROPERTY_ID(id_num);
                if let Ok(name_bstr) = unsafe { uia.GetPropertyProgrammaticName(id) } {
                    let name = name_bstr.to_string();
                    if !name.is_empty() {
                        list.push((id, name));
                    }
                }
            }
        }
        list
    });

    let mut out: Vec<(String, UiValue)> = Vec::new();
    for (id, name) in catalog.iter() {
        // Try Ex first (to ignore default values), fall back to plain getter.
        let mut var: VARIANT = match unsafe { elem.GetCurrentPropertyValueEx(*id, true) } {
            Ok(v) => v,
            Err(_) => match unsafe { elem.GetCurrentPropertyValue(*id) } {
                Ok(v) => v,
                Err(_) => continue,
            },
        };

        // Skip unsupported/mixed sentinels and empty values
        let vt = unsafe { var.Anonymous.Anonymous.vt.0 };
        if vt == VT_EMPTY.0 {
            continue;
        }
        if vt == VT_UNKNOWN.0 {
            // Compare against UIA reserved sentinels if available
            let mut skip = false;
            unsafe {
                if let Ok(ns) = UiaGetReservedNotSupportedValue() {
                    let p = var.Anonymous.Anonymous.Anonymous.punkVal.clone();
                    if let Some(u) = p.as_ref()
                        && u.as_raw() == ns.as_raw()
                    {
                        skip = true;
                    }
                }
                if !skip && let Ok(mx) = UiaGetReservedMixedAttributeValue() {
                    let p = var.Anonymous.Anonymous.Anonymous.punkVal.clone();
                    if let Some(u) = p.as_ref()
                        && u.as_raw() == mx.as_raw()
                    {
                        skip = true;
                    }
                }
            }
            if skip {
                unsafe {
                    let _ = VariantClear(&mut var);
                }
                continue;
            }
        }

        if let Some(value) = unsafe { variant_to_ui_value(&var) } {
            out.push((name.clone(), value));
        }
        unsafe {
            let _ = VariantClear(&mut var);
        }
    }
    out
}

unsafe fn variant_to_ui_value(variant: &VARIANT) -> Option<UiValue> {
    let vt = unsafe { variant.Anonymous.Anonymous.vt.0 };

    // Handle SAFEARRAY values
    if (vt & VT_ARRAY.0) != 0 {
        if (vt & VT_BYREF.0) != 0 {
            return None; // unsupported indirection for now
        }
        let base = vt & VT_TYPEMASK.0;
        let psa = unsafe { variant.Anonymous.Anonymous.Anonymous.parray };
        if psa.is_null() {
            return None;
        }
        // Only support 1D arrays for now
        let dim = unsafe { SafeArrayGetDim(psa) };
        if dim != 1 {
            return None;
        }
        let lb = unsafe { SafeArrayGetLBound(psa, 1) }.ok()?;
        let ub = unsafe { SafeArrayGetUBound(psa, 1) }.ok()?;
        let mut items: Vec<UiValue> = Vec::new();
        for i in lb..=ub {
            match base {
                x if x == VT_BSTR.0 => {
                    let mut b: BSTR = BSTR::new();
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut b as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(b.to_string()));
                    }
                }
                x if x == VT_BOOL.0 => {
                    let mut v: VARIANT_BOOL = VARIANT_BOOL(0);
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v.as_bool()));
                    }
                }
                x if x == VT_I2.0 => {
                    let mut v: i16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_UI2.0 => {
                    let mut v: u16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_I4.0 => {
                    let mut v: i32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_UI4.0 => {
                    let mut v: u32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_I8.0 => {
                    let mut v: i64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_UI8.0 => {
                    let mut v: u64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_R4.0 => {
                    let mut v: f32 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as f64));
                    }
                }
                x if x == VT_R8.0 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DATE.0 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DECIMAL.0 => {
                    let mut d: DECIMAL = unsafe { std::mem::zeroed() };
                    if unsafe { SafeArrayGetElement(psa, &i as *const _, &mut d as *mut _ as *mut _) }.is_ok() {
                        if let Ok(v) = unsafe { VarR8FromDec(&d) } {
                            items.push(UiValue::from(v));
                        } else {
                            items.push(UiValue::from("DECIMAL(..)".to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
        return Some(UiValue::Array(items));
    }

    match vt {
        x if x == VT_BOOL.0 => {
            let b = unsafe { variant.Anonymous.Anonymous.Anonymous.boolVal.as_bool() };
            Some(UiValue::from(b))
        }
        x if x == VT_I2.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.iVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_I4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.lVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_UI2.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.uiVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_UI4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ulVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_I8.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.llVal };
            Some(UiValue::from(v))
        }
        x if x == VT_UI8.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ullVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_R4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.fltVal };
            Some(UiValue::from(v as f64))
        }
        x if x == VT_R8.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.dblVal };
            Some(UiValue::from(v))
        }
        x if x == VT_DATE.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.date };
            Some(UiValue::from(v))
        }
        x if x == VT_BSTR.0 => {
            let s = unsafe { variant.Anonymous.Anonymous.Anonymous.bstrVal.to_string() };
            if s.is_empty() { None } else { Some(UiValue::from(s)) }
        }
        x if x == VT_DECIMAL.0 => {
            let dec = unsafe { &variant.Anonymous.decVal };
            if let Ok(v) = unsafe { VarR8FromDec(dec) } {
                Some(UiValue::from(v))
            } else {
                Some(UiValue::from("DECIMAL(..)".to_string()))
            }
        }
        _ => None,
    }
}
