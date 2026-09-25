#![cfg(target_os = "windows")]
// COM/Win32 FFI module: nearly every call it makes is `unsafe` by signature.
#![allow(unsafe_code)]

use platynui_core::types::Point as UiPoint;
use platynui_core::types::Rect;
use platynui_core::ui::UiValue;
use std::sync::OnceLock;
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
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationValuePattern,
    UIA_AnnotationAnnotationTypeIdPropertyId, UIA_AnnotationAnnotationTypeNamePropertyId,
    UIA_AnnotationAuthorPropertyId, UIA_AnnotationDateTimePropertyId, UIA_AnnotationTargetPropertyId,
    UIA_AppBarControlTypeId, UIA_ButtonControlTypeId, UIA_CalendarControlTypeId, UIA_CheckBoxControlTypeId,
    UIA_ComboBoxControlTypeId, UIA_CustomControlTypeId, UIA_DataGridControlTypeId, UIA_DataItemControlTypeId,
    UIA_DockDockPositionPropertyId, UIA_DocumentControlTypeId, UIA_DragDropEffectPropertyId,
    UIA_DragDropEffectsPropertyId, UIA_DragGrabbedItemsPropertyId, UIA_DragIsGrabbedPropertyId,
    UIA_DropTargetDropTargetEffectPropertyId, UIA_DropTargetDropTargetEffectsPropertyId, UIA_EditControlTypeId,
    UIA_ExpandCollapseExpandCollapseStatePropertyId, UIA_FullDescriptionPropertyId, UIA_GridColumnCountPropertyId,
    UIA_GridItemColumnPropertyId, UIA_GridItemColumnSpanPropertyId, UIA_GridItemContainingGridPropertyId,
    UIA_GridItemRowPropertyId, UIA_GridItemRowSpanPropertyId, UIA_GridRowCountPropertyId, UIA_GroupControlTypeId,
    UIA_HeaderControlTypeId, UIA_HeaderItemControlTypeId, UIA_HyperlinkControlTypeId, UIA_ImageControlTypeId,
    UIA_IsAnnotationPatternAvailablePropertyId, UIA_IsDockPatternAvailablePropertyId,
    UIA_IsDragPatternAvailablePropertyId, UIA_IsDropTargetPatternAvailablePropertyId,
    UIA_IsExpandCollapsePatternAvailablePropertyId, UIA_IsGridItemPatternAvailablePropertyId,
    UIA_IsGridPatternAvailablePropertyId, UIA_IsKeyboardFocusablePropertyId,
    UIA_IsLegacyIAccessiblePatternAvailablePropertyId, UIA_IsMultipleViewPatternAvailablePropertyId,
    UIA_IsRangeValuePatternAvailablePropertyId, UIA_IsScrollPatternAvailablePropertyId,
    UIA_IsSelectionItemPatternAvailablePropertyId, UIA_IsSelectionPattern2AvailablePropertyId,
    UIA_IsSelectionPatternAvailablePropertyId, UIA_IsSpreadsheetItemPatternAvailablePropertyId,
    UIA_IsStylesPatternAvailablePropertyId, UIA_IsTableItemPatternAvailablePropertyId,
    UIA_IsTablePatternAvailablePropertyId, UIA_IsTextPatternAvailablePropertyId,
    UIA_IsTogglePatternAvailablePropertyId, UIA_IsTransformPattern2AvailablePropertyId,
    UIA_IsTransformPatternAvailablePropertyId, UIA_IsValuePatternAvailablePropertyId,
    UIA_IsWindowPatternAvailablePropertyId, UIA_LegacyIAccessibleChildIdPropertyId,
    UIA_LegacyIAccessibleDefaultActionPropertyId, UIA_LegacyIAccessibleDescriptionPropertyId,
    UIA_LegacyIAccessibleHelpPropertyId, UIA_LegacyIAccessibleKeyboardShortcutPropertyId,
    UIA_LegacyIAccessibleNamePropertyId, UIA_LegacyIAccessibleRolePropertyId, UIA_LegacyIAccessibleSelectionPropertyId,
    UIA_LegacyIAccessibleStatePropertyId, UIA_LegacyIAccessibleValuePropertyId, UIA_ListControlTypeId,
    UIA_ListItemControlTypeId, UIA_MenuBarControlTypeId, UIA_MenuControlTypeId, UIA_MenuItemControlTypeId,
    UIA_MultipleViewCurrentViewPropertyId, UIA_MultipleViewSupportedViewsPropertyId, UIA_PATTERN_ID, UIA_PROPERTY_ID,
    UIA_PaneControlTypeId, UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId,
    UIA_RangeValueIsReadOnlyPropertyId, UIA_RangeValueLargeChangePropertyId, UIA_RangeValueMaximumPropertyId,
    UIA_RangeValueMinimumPropertyId, UIA_RangeValueSmallChangePropertyId, UIA_RangeValueValuePropertyId,
    UIA_ScrollBarControlTypeId, UIA_ScrollHorizontalScrollPercentPropertyId, UIA_ScrollHorizontalViewSizePropertyId,
    UIA_ScrollHorizontallyScrollablePropertyId, UIA_ScrollVerticalScrollPercentPropertyId,
    UIA_ScrollVerticalViewSizePropertyId, UIA_ScrollVerticallyScrollablePropertyId,
    UIA_Selection2CurrentSelectedItemPropertyId, UIA_Selection2FirstSelectedItemPropertyId,
    UIA_Selection2ItemCountPropertyId, UIA_Selection2LastSelectedItemPropertyId,
    UIA_SelectionCanSelectMultiplePropertyId, UIA_SelectionIsSelectionRequiredPropertyId,
    UIA_SelectionItemIsSelectedPropertyId, UIA_SelectionItemSelectionContainerPropertyId,
    UIA_SelectionSelectionPropertyId, UIA_SemanticZoomControlTypeId, UIA_SeparatorControlTypeId,
    UIA_SliderControlTypeId, UIA_SpinnerControlTypeId, UIA_SplitButtonControlTypeId,
    UIA_SpreadsheetItemAnnotationObjectsPropertyId, UIA_SpreadsheetItemAnnotationTypesPropertyId,
    UIA_SpreadsheetItemFormulaPropertyId, UIA_StatusBarControlTypeId, UIA_StylesExtendedPropertiesPropertyId,
    UIA_StylesFillColorPropertyId, UIA_StylesFillPatternColorPropertyId, UIA_StylesFillPatternStylePropertyId,
    UIA_StylesShapePropertyId, UIA_StylesStyleIdPropertyId, UIA_StylesStyleNamePropertyId, UIA_TabControlTypeId,
    UIA_TabItemControlTypeId, UIA_TableColumnHeadersPropertyId, UIA_TableControlTypeId,
    UIA_TableItemColumnHeaderItemsPropertyId, UIA_TableItemRowHeaderItemsPropertyId, UIA_TableRowHeadersPropertyId,
    UIA_TableRowOrColumnMajorPropertyId, UIA_TextControlTypeId, UIA_TextPatternId, UIA_ThumbControlTypeId,
    UIA_TitleBarControlTypeId, UIA_ToggleToggleStatePropertyId, UIA_ToolBarControlTypeId, UIA_ToolTipControlTypeId,
    UIA_Transform2CanZoomPropertyId, UIA_Transform2ZoomLevelPropertyId, UIA_Transform2ZoomMaximumPropertyId,
    UIA_Transform2ZoomMinimumPropertyId, UIA_TransformCanMovePropertyId, UIA_TransformCanResizePropertyId,
    UIA_TransformCanRotatePropertyId, UIA_TreeControlTypeId, UIA_TreeItemControlTypeId, UIA_ValueIsReadOnlyPropertyId,
    UIA_ValuePatternId, UIA_ValueValuePropertyId, UIA_WindowCanMaximizePropertyId, UIA_WindowCanMinimizePropertyId,
    UIA_WindowControlTypeId, UIA_WindowIsModalPropertyId, UIA_WindowIsTopmostPropertyId,
    UIA_WindowWindowInteractionStatePropertyId, UIA_WindowWindowVisualStatePropertyId,
    UiaGetReservedMixedAttributeValue, UiaGetReservedNotSupportedValue,
};
use windows::core::BSTR;
use windows::core::Interface;
use windows::core::PWSTR;

// Use VARENUM constants from the windows crate instead of redefining magic numbers

/// Maps UIA `ControlType` IDs to `PlatynUI` role names.
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

/// Reads the strict accessible description from `UIA_FullDescriptionPropertyId`.
///
/// Returns `None` when the property is empty, unset, or unsupported (it is a
/// Win10 1703+ property; older targets simply yield nothing). Deliberately does
/// NOT fall back to `HelpText` or `LegacyIAccessible.Description` — those remain
/// reachable only under the `native:` namespace. The value is returned
/// unmodified so `control:Description` matches the platform string exactly.
pub fn get_description(elem: &IUIAutomationElement) -> Option<String> {
    match read_uia_property(elem, UIA_FullDescriptionPropertyId) {
        Some(UiValue::String(s)) if !s.is_empty() => Some(s),
        _ => None,
    }
}

pub fn get_control_type(elem: &IUIAutomationElement) -> Result<i32, crate::error::UiaError> {
    unsafe { crate::error::uia_api("IUIAutomationElement::CurrentControlType", elem.CurrentControlType()).map(|v| v.0) }
}

pub fn get_bounding_rect(elem: &IUIAutomationElement) -> Result<Rect, crate::error::UiaError> {
    unsafe {
        let r =
            crate::error::uia_api("IUIAutomationElement::CurrentBoundingRectangle", elem.CurrentBoundingRectangle())?;

        let left = f64::from(r.left);
        let top = f64::from(r.top);
        let width = f64::from((r.right - r.left).max(0));
        let height = f64::from((r.bottom - r.top).max(0));
        Ok(Rect::new(left, top, width, height))
    }
}
pub fn get_clickable_point(elem: &IUIAutomationElement) -> Result<UiPoint, crate::error::UiaError> {
    unsafe {
        // UIA returns a POINT in desktop coordinates and a BOOL as return value indicating success
        let mut pt = POINT { x: 0, y: 0 };

        let got_clickable =
            crate::error::uia_api("IUIAutomationElement::GetClickablePoint", elem.GetClickablePoint(&raw mut pt))?;

        // Check if a clickable point was actually found
        if got_clickable.as_bool() {
            Ok(UiPoint::new(f64::from(pt.x), f64::from(pt.y)))
        } else {
            Err(crate::error::UiaError::NoClickablePoint)
        }
    }
}

/// Internal helper: returns the hex-dotted `RuntimeId` body without any scheme/prefix.
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
        // SAFEARRAY bounds satisfy ub >= lb - 1 (an empty array has ub == lb - 1), so
        // the element count is never negative.
        #[allow(clippy::cast_sign_loss)]
        let count = (ub - lb + 1) as usize;
        let mut data: *mut i32 = std::ptr::null_mut();
        crate::error::uia_api("SafeArrayAccessData", SafeArrayAccessData(psa, (&raw mut data).cast()))?;
        let slice = std::slice::from_raw_parts(data, count);
        // Keep formatting identical to legacy behavior to avoid breaking changes.
        let body = slice.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(".");
        crate::error::uia_api("SafeArrayUnaccessData", SafeArrayUnaccessData(psa))?;
        Ok(body)
    }
}

// Note: legacy unscoped formatter removed; use `format_scoped_runtime_id` instead.

/// Scope for composing unique, view-aware `RuntimeId` URIs within our combined trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiaIdScope {
    /// Desktop `TopLevel` view
    Desktop,
    /// Application-grouped view; disambiguate by process id
    App { pid: i32 },
}

/// Compose a scoped `RuntimeId` URI that stays unique across multiple views in our trees.
/// Examples:
///  - Desktop: `uia://desktop/<rid>`
///  - App:     `uia://app/<pid>/<rid>`
pub fn format_scoped_runtime_id(
    elem: &IUIAutomationElement,
    scope: UiaIdScope,
) -> Result<String, crate::error::UiaError> {
    let body = runtime_id_hex_body(elem)?;
    let s = match scope {
        UiaIdScope::Desktop => format!("uia://desktop/{body}"),
        UiaIdScope::App { pid } => format!("uia://app/{pid}/{body}"),
    };
    Ok(s)
}

pub fn get_is_enabled(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsEnabled", elem.CurrentIsEnabled())
            .map(windows::core::BOOL::as_bool)
    }
}

pub fn get_is_in_view(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsOffscreen", elem.CurrentIsOffscreen())
            .map(|b| !b.as_bool())
    }
}

pub fn get_process_id(elem: &IUIAutomationElement) -> Result<i32, crate::error::UiaError> {
    unsafe { crate::error::uia_api("IUIAutomationElement::CurrentProcessId", elem.CurrentProcessId()) }
}

pub fn open_process_query(pid: i32) -> Option<HANDLE> {
    unsafe {
        // Prefer broader rights to allow module queries (base module name), then fall back.
        let full = PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_INFORMATION.0 | PROCESS_VM_READ.0);
        // UIA reports the process id as i32, Win32 takes it as u32: same bits.
        let pid = pid.cast_unsigned();
        OpenProcess(full, false, pid)
            .ok()
            .or_else(|| OpenProcess(PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0), false, pid).ok())
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
                &raw mut size,
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
                if hr.0.cast_unsigned() == ERROR_INSUFFICIENT_BUFFER.0 {
                    // If API updated 'size' with the required length, use it; otherwise double
                    let next = size.max(cap.saturating_mul(2)).min(max_cap);
                    if next <= cap || next > max_cap {
                        return None;
                    }
                    cap = next;
                    continue;
                }
                return None;
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
                buf.as_mut_ptr().cast(),
                cap,
                &raw mut ret_len,
            );
            if status == STATUS_SUCCESS {
                // Interpret start of buffer as UNICODE_STRING
                #[repr(C)]
                struct UnicodeString {
                    length: u16,
                    max_length: u16,
                    buffer: *const u16,
                }
                // A `Vec<u8>` does not guarantee the pointer alignment of
                // UNICODE_STRING, so copy the header out with an unaligned read.
                let us = std::ptr::read_unaligned(buf.as_ptr().cast::<UnicodeString>());
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
        if OpenProcessToken(handle, TOKEN_QUERY, &raw mut token).is_err() {
            return None;
        }
        // Query size first
        let mut needed: u32 = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &raw mut needed);
        if needed == 0 {
            let _ = CloseHandle(token);
            return None;
        }
        let mut buf = vec![0u8; needed as usize];
        if GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), needed, &raw mut needed).is_err() {
            let _ = CloseHandle(token);
            return None;
        }
        // A `Vec<u8>` does not guarantee the pointer alignment of TOKEN_USER, so
        // copy the header out with an unaligned read; its SID still points into `buf`.
        let tu: TOKEN_USER = std::ptr::read_unaligned(buf.as_ptr().cast::<TOKEN_USER>());
        let sid = tu.User.Sid;
        // Lookup account name
        let mut name_len: u32 = 0;
        let mut domain_len: u32 = 0;
        let mut use_: SID_NAME_USE = SID_NAME_USE(0);
        // First call with None to query required buffer sizes
        let _ = LookupAccountSidW(None, sid, None, &raw mut name_len, None, &raw mut domain_len, &raw mut use_);
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
            &raw mut name_len,
            if domain_len > 0 { Some(PWSTR(domain_buf.as_mut_ptr())) } else { None },
            &raw mut domain_len,
            &raw mut use_,
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
        if domain.is_empty() { Some(name) } else { Some(format!("{domain}\\{name}")) }
    }
}

pub fn query_process_start_time_iso8601(handle: HANDLE) -> Option<String> {
    unsafe {
        let mut creation: FILETIME = FILETIME::default();
        let mut exit: FILETIME = FILETIME::default();
        let mut kernel: FILETIME = FILETIME::default();
        let mut user: FILETIME = FILETIME::default();
        if GetProcessTimes(handle, &raw mut creation, &raw mut exit, &raw mut kernel, &raw mut user).is_err() {
            return None;
        }
        // Convert to SYSTEMTIME in UTC
        let mut st = windows::Win32::Foundation::SYSTEMTIME::default();
        if FileTimeToSystemTime(&raw const creation, &raw mut st).is_err() {
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

pub fn process_architecture(_handle: HANDLE) -> String {
    // Fallback: report native system architecture
    let mut info: SYSTEM_INFO = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&raw mut info) };
    let a: PROCESSOR_ARCHITECTURE = unsafe { info.Anonymous.Anonymous.wProcessorArchitecture };
    let sys_arch = match a {
        x if x == PROCESSOR_ARCHITECTURE_AMD64 => "x64",
        x if x == PROCESSOR_ARCHITECTURE_ARM64 => "arm64",
        x if x == PROCESSOR_ARCHITECTURE_INTEL => "x86",
        _ => "unknown",
    };
    sys_arch.to_string()
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

/// Reads a single UIA property value from an element, handling COM fallback
/// and UIA sentinel filtering (`NotSupported` / `MixedAttribute`).
fn read_uia_property(elem: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> Option<UiValue> {
    let mut var: VARIANT = match unsafe { elem.GetCurrentPropertyValueEx(id, true) } {
        Ok(v) => v,
        Err(_) => match unsafe { elem.GetCurrentPropertyValue(id) } {
            Ok(v) => v,
            Err(_) => return None,
        },
    };

    let vt = unsafe { var.Anonymous.Anonymous.vt.0 };
    if vt == VT_EMPTY.0 {
        return None;
    }
    if vt == VT_UNKNOWN.0 {
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
                let _ = VariantClear(&raw mut var);
            }
            return None;
        }
    }

    let result = unsafe { variant_to_ui_value(&var) };
    unsafe {
        let _ = VariantClear(&raw mut var);
    }
    result
}

/// Whether this element exposes readable text content for the canonical
/// `control:Text` attribute (`TextContent`) — i.e. it supports the UIA
/// `TextPattern` or `ValuePattern`. Used to gate the attribute so it is
/// absent (not empty) on elements with neither pattern. Never considers the
/// accessible name.
pub fn supports_text_content(elem: &IUIAutomationElement) -> bool {
    is_pattern_available(elem, UIA_IsTextPatternAvailablePropertyId)
        || is_pattern_available(elem, UIA_IsValuePatternAvailablePropertyId)
}

fn is_pattern_available(elem: &IUIAutomationElement, id: UIA_PROPERTY_ID) -> bool {
    matches!(read_uia_property(elem, id), Some(UiValue::Bool(true)))
}

/// What UIA reports for `IsKeyboardFocusable`, keeping "the provider says no"
/// apart from "the provider says nothing".
///
/// This distinction is invisible through `IUIAutomationElement::CurrentIsKeyboardFocusable`,
/// whose documented default value is `FALSE` — an unimplemented property and an
/// explicit denial come back identically. Reading with `ignoreDefaultValue` set
/// yields the `NotSupported` sentinel instead, which [`read_uia_property`] maps to
/// `None`. Measured: static `Text` labels report an explicit `false`, buttons and
/// edits an explicit `true`, while an Electron window (VS Code, accessibility not
/// switched on) and several plain Win32 panes supply nothing at all.
pub fn keyboard_focusable(elem: &IUIAutomationElement) -> Option<bool> {
    match read_uia_property(elem, UIA_IsKeyboardFocusablePropertyId) {
        Some(UiValue::Bool(value)) => Some(value),
        _ => None,
    }
}

/// Whether the text content of this element is read-only, for `control:IsReadOnly`
/// and the `TextEditable` capability marker.
///
/// `ValuePattern.CurrentIsReadOnly` is the authoritative signal; an element that
/// exposes only a `TextPattern` (document viewers, read-only rich text) has no
/// write surface at all and therefore counts as read-only. Callers must gate on
/// [`supports_text_content`] first — for an element with neither pattern the
/// answer is meaningless, and this returns `true` rather than claiming the
/// element is editable.
pub fn is_text_read_only(elem: &IUIAutomationElement) -> bool {
    resolve_read_only(value_pattern_read_only(elem))
}

/// What the element's `ValuePattern` reports for `IsReadOnly`, or `None` when
/// there is no `ValuePattern` to ask — a `TextPattern`-only element such as a
/// document viewer — or when the read fails.
fn value_pattern_read_only(elem: &IUIAutomationElement) -> Option<bool> {
    let pattern = unsafe {
        elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_ValuePatternId.0))
            .ok()
            .and_then(|unk| unk.cast::<IUIAutomationValuePattern>().ok())
    }?;
    unsafe { pattern.CurrentIsReadOnly() }.map(windows::core::BOOL::as_bool).ok()
}

/// Resolves the read-only state from what the `ValuePattern` reported.
///
/// No answer means read-only. This is the opposite default from the focusability
/// gate ([`crate::node`]), and deliberately so: there the question is whether to
/// *withdraw* a capability the element may well have, here it is whether to
/// *claim* one. An element with no write surface to ask about has none, and a
/// pattern that fails to report its state has not confirmed one — the provider
/// must not advertise editability it could not establish.
fn resolve_read_only(reported: Option<bool>) -> bool {
    reported.unwrap_or(true)
}

/// Whether this element's text content can be edited — the predicate behind the
/// `TextEditable` capability marker. Editing itself stays keyboard-driven; the
/// marker only tells clients that typing into this element is meaningful.
pub fn supports_text_editing(elem: &IUIAutomationElement) -> bool {
    // Short-circuits deliberately: `is_text_read_only` costs two cross-process
    // reads, and for an element with no text surface at all the answer cannot
    // change the outcome.
    supports_text_content(elem) && !is_text_read_only(elem)
}

/// Reads the element's textual content for the canonical `control:Text`
/// attribute: the UIA `TextPattern` document text first (the actually
/// displayed text via `DocumentRange().GetText(-1)`), falling back to the
/// `ValuePattern` value (which may be a formatted/adapted string) when there
/// is no `TextPattern`. Never falls back to the accessible name. Returns
/// `Some("")` for an empty text field and `None` only when the element
/// supports neither pattern (or both reads fail).
pub fn get_text_content(elem: &IUIAutomationElement) -> Option<String> {
    unsafe {
        if let Ok(unk) = elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TextPatternId.0))
            && let Ok(text_pattern) = unk.cast::<IUIAutomationTextPattern>()
            && let Ok(range) = text_pattern.DocumentRange()
            && let Ok(text) = range.GetText(-1)
        {
            return Some(text.to_string());
        }
        if let Ok(unk) = elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_ValuePatternId.0))
            && let Ok(value_pattern) = unk.cast::<IUIAutomationValuePattern>()
            && let Ok(value) = value_pattern.CurrentValue()
        {
            return Some(value.to_string());
        }
        None
    }
}

/// Categorized UIA property catalog: separates base element properties from
/// pattern-specific properties so we only query pattern properties when the
/// pattern is actually supported by the element.
struct CategorizedCatalog {
    /// Base element properties — always queried for every element.
    base: Vec<(UIA_PROPERTY_ID, String)>,
    /// Pattern-specific groups: (`pattern_availability_property_id`, properties).
    /// The availability ID is queried first; its properties are only read when
    /// the pattern is available.
    pattern_groups: Vec<(UIA_PROPERTY_ID, Vec<(UIA_PROPERTY_ID, String)>)>,
}

/// Pattern groups: (availability-check property, the pattern's property IDs).
/// Property IDs that don't appear in any group are classified as base.
const PATTERN_GROUPS: &[(UIA_PROPERTY_ID, &[UIA_PROPERTY_ID])] = &[
    // --- Classic patterns (Windows 7+) ---
    (UIA_IsDockPatternAvailablePropertyId, &[UIA_DockDockPositionPropertyId]),
    (UIA_IsExpandCollapsePatternAvailablePropertyId, &[UIA_ExpandCollapseExpandCollapseStatePropertyId]),
    (
        UIA_IsGridItemPatternAvailablePropertyId,
        &[
            UIA_GridItemRowPropertyId,
            UIA_GridItemColumnPropertyId,
            UIA_GridItemRowSpanPropertyId,
            UIA_GridItemColumnSpanPropertyId,
            UIA_GridItemContainingGridPropertyId,
        ],
    ),
    (UIA_IsGridPatternAvailablePropertyId, &[UIA_GridRowCountPropertyId, UIA_GridColumnCountPropertyId]),
    // Invoke pattern has no properties.
    (
        UIA_IsMultipleViewPatternAvailablePropertyId,
        &[UIA_MultipleViewCurrentViewPropertyId, UIA_MultipleViewSupportedViewsPropertyId],
    ),
    (
        UIA_IsRangeValuePatternAvailablePropertyId,
        &[
            UIA_RangeValueValuePropertyId,
            UIA_RangeValueIsReadOnlyPropertyId,
            UIA_RangeValueMinimumPropertyId,
            UIA_RangeValueMaximumPropertyId,
            UIA_RangeValueLargeChangePropertyId,
            UIA_RangeValueSmallChangePropertyId,
        ],
    ),
    (
        UIA_IsScrollPatternAvailablePropertyId,
        &[
            UIA_ScrollHorizontalScrollPercentPropertyId,
            UIA_ScrollHorizontalViewSizePropertyId,
            UIA_ScrollVerticalScrollPercentPropertyId,
            UIA_ScrollVerticalViewSizePropertyId,
            UIA_ScrollHorizontallyScrollablePropertyId,
            UIA_ScrollVerticallyScrollablePropertyId,
        ],
    ),
    // ScrollItem pattern has no properties.
    (
        UIA_IsSelectionItemPatternAvailablePropertyId,
        &[UIA_SelectionItemIsSelectedPropertyId, UIA_SelectionItemSelectionContainerPropertyId],
    ),
    (
        UIA_IsSelectionPatternAvailablePropertyId,
        &[
            UIA_SelectionSelectionPropertyId,
            UIA_SelectionCanSelectMultiplePropertyId,
            UIA_SelectionIsSelectionRequiredPropertyId,
        ],
    ),
    (
        UIA_IsTablePatternAvailablePropertyId,
        &[UIA_TableRowHeadersPropertyId, UIA_TableColumnHeadersPropertyId, UIA_TableRowOrColumnMajorPropertyId],
    ),
    (
        UIA_IsTableItemPatternAvailablePropertyId,
        &[UIA_TableItemRowHeaderItemsPropertyId, UIA_TableItemColumnHeaderItemsPropertyId],
    ),
    // Text pattern has no simple property IDs (uses TextRange).
    (UIA_IsTogglePatternAvailablePropertyId, &[UIA_ToggleToggleStatePropertyId]),
    (
        UIA_IsTransformPatternAvailablePropertyId,
        &[UIA_TransformCanMovePropertyId, UIA_TransformCanResizePropertyId, UIA_TransformCanRotatePropertyId],
    ),
    (UIA_IsValuePatternAvailablePropertyId, &[UIA_ValueValuePropertyId, UIA_ValueIsReadOnlyPropertyId]),
    (
        UIA_IsWindowPatternAvailablePropertyId,
        &[
            UIA_WindowCanMaximizePropertyId,
            UIA_WindowCanMinimizePropertyId,
            UIA_WindowWindowVisualStatePropertyId,
            UIA_WindowWindowInteractionStatePropertyId,
            UIA_WindowIsModalPropertyId,
            UIA_WindowIsTopmostPropertyId,
        ],
    ),
    (
        UIA_IsLegacyIAccessiblePatternAvailablePropertyId,
        &[
            UIA_LegacyIAccessibleChildIdPropertyId,
            UIA_LegacyIAccessibleNamePropertyId,
            UIA_LegacyIAccessibleValuePropertyId,
            UIA_LegacyIAccessibleDescriptionPropertyId,
            UIA_LegacyIAccessibleRolePropertyId,
            UIA_LegacyIAccessibleStatePropertyId,
            UIA_LegacyIAccessibleHelpPropertyId,
            UIA_LegacyIAccessibleKeyboardShortcutPropertyId,
            UIA_LegacyIAccessibleSelectionPropertyId,
            UIA_LegacyIAccessibleDefaultActionPropertyId,
        ],
    ),
    // --- Windows 8+ patterns ---
    (
        UIA_IsAnnotationPatternAvailablePropertyId,
        &[
            UIA_AnnotationAnnotationTypeIdPropertyId,
            UIA_AnnotationAnnotationTypeNamePropertyId,
            UIA_AnnotationAuthorPropertyId,
            UIA_AnnotationDateTimePropertyId,
            UIA_AnnotationTargetPropertyId,
        ],
    ),
    (
        UIA_IsDragPatternAvailablePropertyId,
        &[
            UIA_DragIsGrabbedPropertyId,
            UIA_DragDropEffectPropertyId,
            UIA_DragDropEffectsPropertyId,
            UIA_DragGrabbedItemsPropertyId,
        ],
    ),
    (
        UIA_IsDropTargetPatternAvailablePropertyId,
        &[UIA_DropTargetDropTargetEffectPropertyId, UIA_DropTargetDropTargetEffectsPropertyId],
    ),
    (
        UIA_IsSpreadsheetItemPatternAvailablePropertyId,
        &[
            UIA_SpreadsheetItemFormulaPropertyId,
            UIA_SpreadsheetItemAnnotationObjectsPropertyId,
            UIA_SpreadsheetItemAnnotationTypesPropertyId,
        ],
    ),
    (
        UIA_IsStylesPatternAvailablePropertyId,
        &[
            UIA_StylesStyleIdPropertyId,
            UIA_StylesStyleNamePropertyId,
            UIA_StylesFillColorPropertyId,
            UIA_StylesFillPatternStylePropertyId,
            UIA_StylesShapePropertyId,
            UIA_StylesFillPatternColorPropertyId,
            UIA_StylesExtendedPropertiesPropertyId,
        ],
    ),
    (
        UIA_IsTransformPattern2AvailablePropertyId,
        &[
            UIA_Transform2CanZoomPropertyId,
            UIA_Transform2ZoomLevelPropertyId,
            UIA_Transform2ZoomMinimumPropertyId,
            UIA_Transform2ZoomMaximumPropertyId,
        ],
    ),
    (
        UIA_IsSelectionPattern2AvailablePropertyId,
        &[
            UIA_Selection2FirstSelectedItemPropertyId,
            UIA_Selection2LastSelectedItemPropertyId,
            UIA_Selection2CurrentSelectedItemPropertyId,
            UIA_Selection2ItemCountPropertyId,
        ],
    ),
];

/// Builds the categorized catalog once and returns a static reference.
///
/// Pattern-specific property IDs are mapped to their owning pattern's
/// availability-check property. Any property ID not covered by a known
/// pattern group is treated as a base property.
fn categorized_catalog() -> &'static CategorizedCatalog {
    static INSTANCE: OnceLock<CategorizedCatalog> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        use std::collections::HashMap;

        let groups = PATTERN_GROUPS;

        // Build reverse map: property ID → group index
        let mut prop_to_group: HashMap<i32, usize> = HashMap::new();
        for (idx, (_, props)) in groups.iter().enumerate() {
            for prop_id in *props {
                prop_to_group.insert(prop_id.0, idx);
            }
        }

        let mut base = Vec::new();
        let mut pattern_props: Vec<Vec<(UIA_PROPERTY_ID, String)>> = vec![Vec::new(); groups.len()];

        if let Ok(uia) = crate::com::uia() {
            for id_num in 30000i32..31050i32 {
                let id = UIA_PROPERTY_ID(id_num);
                if let Ok(name_bstr) = unsafe { uia.GetPropertyProgrammaticName(id) } {
                    let name = name_bstr.to_string();
                    if !name.is_empty() {
                        if let Some(&group_idx) = prop_to_group.get(&id_num) {
                            pattern_props[group_idx].push((id, name));
                        } else {
                            base.push((id, name));
                        }
                    }
                }
            }
        }

        let pattern_groups = groups
            .iter()
            .zip(pattern_props)
            .map(|((avail_id, _), props)| (*avail_id, props))
            .filter(|(_, props)| !props.is_empty())
            .collect();

        CategorizedCatalog { base, pattern_groups }
    })
}

/// Collects native UIA properties, querying only pattern-specific properties
/// when the corresponding pattern is actually supported by the element.
///
/// This avoids the cost of blindly probing all ~1050 property IDs in the UIA
/// range. Instead we query ~50 base element properties unconditionally plus
/// one boolean availability check per pattern group (~20 cheap COM calls),
/// then only read the properties of patterns the element actually supports.
pub fn collect_native_properties(elem: &IUIAutomationElement) -> Vec<(String, UiValue)> {
    let catalog = categorized_catalog();
    let mut out: Vec<(String, UiValue)> = Vec::new();

    // 1. Always query base element properties.
    for (id, name) in &catalog.base {
        if let Some(value) = read_uia_property(elem, *id) {
            out.push((name.clone(), value));
        }
    }

    // 2. For each pattern group, check availability first (single boolean
    //    COM call), then read pattern-specific properties only if supported.
    for (avail_id, props) in &catalog.pattern_groups {
        let available = unsafe { elem.GetCurrentPropertyValue(*avail_id) }
            .ok()
            .and_then(|var| {
                let vt = unsafe { var.Anonymous.Anonymous.vt.0 };
                if vt == VT_BOOL.0 {
                    Some(unsafe { var.Anonymous.Anonymous.Anonymous.boolVal.as_bool() })
                } else {
                    None
                }
            })
            .unwrap_or(false);

        if available {
            for (id, name) in props {
                if let Some(value) = read_uia_property(elem, *id) {
                    out.push((name.clone(), value));
                }
            }
        }
    }

    out
}

/// Looks up a single native UIA property by its programmatic name, avoiding
/// the cost of collecting all properties.
///
/// Uses a lazily-built reverse map (name → property ID) so subsequent lookups
/// are O(1) hash-table probes plus a single COM property read.
pub fn get_native_property_by_name(elem: &IUIAutomationElement, prop_name: &str) -> Option<(String, UiValue)> {
    use std::collections::HashMap;

    static NAME_TO_ID: OnceLock<HashMap<String, UIA_PROPERTY_ID>> = OnceLock::new();
    let map = NAME_TO_ID.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(uia) = crate::com::uia() {
            for id_num in 30000i32..31050i32 {
                let id = UIA_PROPERTY_ID(id_num);
                if let Ok(name_bstr) = unsafe { uia.GetPropertyProgrammaticName(id) } {
                    let name = name_bstr.to_string();
                    if !name.is_empty() {
                        m.insert(name, id);
                    }
                }
            }
        }
        m
    });

    let id = map.get(prop_name)?;
    read_uia_property(elem, *id).map(|v| (prop_name.to_string(), v))
}

// One dispatch arm per VARIANT element type, for arrays and scalars alike.
#[allow(clippy::too_many_lines)]
unsafe fn variant_to_ui_value(variant: &VARIANT) -> Option<UiValue> {
    let vt = unsafe { variant.Anonymous.Anonymous.vt.0 };

    // Handle SAFEARRAY values
    if (vt & VT_ARRAY.0) != 0 {
        if (vt & VT_BYREF.0) != 0 {
            return None; // unsupported indirection for now
        }
        let base = vt & VT_TYPEMASK.0;
        // Element-reference arrays (VT_UNKNOWN base) hold pointers to other UI
        // elements (e.g. ControllerFor, DescribedBy, FlowsTo/FlowsFrom,
        // Selection, table header items). Those name elements to navigate to
        // via XPath axes, not values to match in a predicate, and cannot be
        // decoded here anyway, so drop them like the scalar VT_UNKNOWN case.
        if base == VT_UNKNOWN.0 {
            return None;
        }
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
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut b).cast()) }.is_ok() {
                        items.push(UiValue::from(b.to_string()));
                    }
                }
                x if x == VT_BOOL.0 => {
                    let mut v: VARIANT_BOOL = VARIANT_BOOL(0);
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(v.as_bool()));
                    }
                }
                x if x == VT_I2.0 => {
                    let mut v: i16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(i64::from(v)));
                    }
                }
                x if x == VT_UI2.0 => {
                    let mut v: u16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(i64::from(v)));
                    }
                }
                x if x == VT_I4.0 => {
                    let mut v: i32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(i64::from(v)));
                    }
                }
                x if x == VT_UI4.0 => {
                    let mut v: u32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(i64::from(v)));
                    }
                }
                x if x == VT_I8.0 => {
                    let mut v: i64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_UI8.0 => {
                    let mut v: u64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        // UiValue has no unsigned integer; values above i64::MAX wrap, as before.
                        items.push(UiValue::from(v.cast_signed()));
                    }
                }
                x if x == VT_R4.0 => {
                    let mut v: f32 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(f64::from(v)));
                    }
                }
                x if x == VT_R8.0 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DATE.0 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut v).cast()) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DECIMAL.0 => {
                    let mut d: DECIMAL = unsafe { std::mem::zeroed() };
                    if unsafe { SafeArrayGetElement(psa, &raw const i, (&raw mut d).cast()) }.is_ok() {
                        if let Ok(v) = unsafe { VarR8FromDec(&raw const d) } {
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
            Some(UiValue::from(i64::from(v)))
        }
        x if x == VT_I4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.lVal };
            Some(UiValue::from(i64::from(v)))
        }
        x if x == VT_UI2.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.uiVal };
            Some(UiValue::from(i64::from(v)))
        }
        x if x == VT_UI4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ulVal };
            Some(UiValue::from(i64::from(v)))
        }
        x if x == VT_I8.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.llVal };
            Some(UiValue::from(v))
        }
        x if x == VT_UI8.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ullVal };
            // UiValue has no unsigned integer; values above i64::MAX wrap, as before.
            Some(UiValue::from(v.cast_signed()))
        }
        x if x == VT_R4.0 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.fltVal };
            Some(UiValue::from(f64::from(v)))
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

#[cfg(test)]
mod text_editability_tests {
    use super::resolve_read_only;

    #[test]
    fn a_writable_value_pattern_is_not_read_only() {
        assert!(!resolve_read_only(Some(false)));
    }

    #[test]
    fn an_explicitly_read_only_value_pattern_is_read_only() {
        assert!(resolve_read_only(Some(true)));
    }

    /// A `TextPattern`-only element (document viewer) has no write surface to
    /// ask about, and a `ValuePattern` that fails to report has confirmed
    /// nothing — both count as read-only rather than as editable.
    #[test]
    fn no_answer_counts_as_read_only() {
        assert!(resolve_read_only(None));
    }
}
