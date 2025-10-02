#![cfg(target_os = "windows")]
use platynui_core::types::Point as UiPoint;
use platynui_core::types::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Accessibility::*;

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

pub fn get_name(elem: &IUIAutomationElement) -> Result<String, String> {
    unsafe { elem.CurrentName().map(|b| b.to_string()).map_err(|e| e.to_string()) }
}

pub fn get_control_type(elem: &IUIAutomationElement) -> Result<i32, String> {
    unsafe { elem.CurrentControlType().map(|v| v.0).map_err(|e| e.to_string()) }
}

pub fn get_bounding_rect(elem: &IUIAutomationElement) -> Result<Rect, String> {
    unsafe {
        let r = elem.CurrentBoundingRectangle().map_err(|e| e.to_string())?;
        // Treat it as Foundation::RECT with integer fields
        let left = (r.left) as f64;
        let top = (r.top) as f64;
        let width = (r.right - r.left).max(0) as f64;
        let height = (r.bottom - r.top).max(0) as f64;
        Ok(Rect::new(left, top, width, height))
    }
}

pub fn get_clickable_point(elem: &IUIAutomationElement) -> Result<UiPoint, String> {
    unsafe {
        // UIA returns a POINT in desktop coordinates; call may fail with UIA_E_NOCLICKABLEPOINT
        let mut pt = POINT { x: 0, y: 0 };
        // Some windows-rs bindings expose GetClickablePoint(&mut x, &mut y) or &mut POINT; try POINT
        // The generated method signature accepts *mut POINT internally; windows handles marshalling.
        elem.GetClickablePoint(&mut pt)
            .map_err(|e| e.to_string())
            .map(|_| UiPoint::new(pt.x as f64, pt.y as f64))
    }
}

pub fn format_runtime_id(elem: &IUIAutomationElement) -> Result<String, String> {
    use windows::Win32::System::Ole::{
        SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
    };
    unsafe {
        let psa = elem.GetRuntimeId().map_err(|e| e.to_string())?;
        if psa.is_null() {
            return Err("runtime id null".into());
        }
        let lb = SafeArrayGetLBound(psa, 1).map_err(|e| e.to_string())?;
        let ub = SafeArrayGetUBound(psa, 1).map_err(|e| e.to_string())?;
        let count = (ub - lb + 1) as usize;
        let mut data: *mut i32 = std::ptr::null_mut();
        SafeArrayAccessData(psa, &mut data as *mut _ as *mut _).map_err(|e| e.to_string())?;
        let slice = std::slice::from_raw_parts(data, count);
        let body = slice.iter().map(|v| format!("{:x}", v)).collect::<Vec<_>>().join(".");
        SafeArrayUnaccessData(psa).map_err(|e| e.to_string())?;
        Ok(format!("uia://{}", body))
    }
}

pub fn get_is_enabled(elem: &IUIAutomationElement) -> Result<bool, String> {
    unsafe { elem.CurrentIsEnabled().map(|b| b.as_bool()).map_err(|e| e.to_string()) }
}

pub fn get_is_offscreen(elem: &IUIAutomationElement) -> Result<bool, String> {
    unsafe { elem.CurrentIsOffscreen().map(|b| b.as_bool()).map_err(|e| e.to_string()) }
}

// get_activation_point and is_visible moved into attribute-level caching logic.
