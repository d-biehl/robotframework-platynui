#![cfg(target_os = "windows")]
use platynui_core::types::Point as UiPoint;
use platynui_core::types::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Accessibility::*;
use platynui_core::ui::UiValue;
use once_cell::sync::OnceCell;
use windows::core::Interface;
use windows::Win32::System::Variant::{VARIANT, VariantClear};
use windows::Win32::System::Ole::VarR8FromDec;
use windows::Win32::System::Ole::{SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
use windows::Win32::Foundation::{DECIMAL, VARIANT_BOOL};
use windows::core::BSTR;
use windows::Win32::System::Variant::{
    VT_ARRAY, VT_BSTR, VT_BYREF, VT_DATE, VT_DECIMAL, VT_EMPTY, VT_BOOL, VT_I2, VT_I4, VT_I8,
    VT_R4, VT_R8, VT_TYPEMASK, VT_UI2, VT_UI4, VT_UI8, VT_UNKNOWN,
};

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
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentName", elem.CurrentName())
            .map(|b| b.to_string())
    }
}

pub fn get_control_type(elem: &IUIAutomationElement) -> Result<i32, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentControlType", elem.CurrentControlType())
            .map(|v| v.0)
    }
}

pub fn get_bounding_rect(elem: &IUIAutomationElement) -> Result<Rect, crate::error::UiaError> {
    unsafe {
        let r = crate::error::uia_api(
            "IUIAutomationElement::CurrentBoundingRectangle",
            elem.CurrentBoundingRectangle(),
        )?;
        // Treat it as Foundation::RECT with integer fields
        let left = (r.left) as f64;
        let top = (r.top) as f64;
        let width = (r.right - r.left).max(0) as f64;
        let height = (r.bottom - r.top).max(0) as f64;
        Ok(Rect::new(left, top, width, height))
    }
}

pub fn get_clickable_point(
    elem: &IUIAutomationElement,
) -> Result<UiPoint, crate::error::UiaError> {
    unsafe {
        // UIA returns a POINT in desktop coordinates; call may fail with UIA_E_NOCLICKABLEPOINT
        let mut pt = POINT { x: 0, y: 0 };
        crate::error::uia_api("IUIAutomationElement::GetClickablePoint", elem.GetClickablePoint(&mut pt))
            .map(|_| UiPoint::new(pt.x as f64, pt.y as f64))
    }
}

pub fn format_runtime_id(elem: &IUIAutomationElement) -> Result<String, crate::error::UiaError> {
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
        let body = slice.iter().map(|v| format!("{:x}", v)).collect::<Vec<_>>().join(".");
        crate::error::uia_api("SafeArrayUnaccessData", SafeArrayUnaccessData(psa))?;
        Ok(format!("uia://{}", body))
    }
}

pub fn get_is_enabled(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsEnabled", elem.CurrentIsEnabled())
            .map(|b| b.as_bool())
    }
}

pub fn get_is_offscreen(elem: &IUIAutomationElement) -> Result<bool, crate::error::UiaError> {
    unsafe {
        crate::error::uia_api("IUIAutomationElement::CurrentIsOffscreen", elem.CurrentIsOffscreen())
            .map(|b| b.as_bool())
    }
}

// get_activation_point and is_visible moved into attribute-level caching logic.

/// Collects a curated set of native UIA properties and returns only those
/// that appear to be supported (non-empty / meaningful values).
///
/// Hinweis: Die UIA COM‑API bietet keine direkte Enumeration aller unterstützten
/// Properties eines Elements. Wir ermitteln Properties daher über einen
/// Programmatic‑Name‑Katalog (IDs im typischen UIA‑Bereich) und lesen pro ID
/// den aktuellen Wert via `GetCurrentPropertyValueEx(id, true)` aus. Nicht
/// unterstützte oder gemischte Werte werden über die UIA‑Sentinels gefiltert.
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
        let vt = unsafe { var.Anonymous.Anonymous.vt.0 as u16 };
        if vt == VT_EMPTY.0 as u16 {
            continue;
        }
        if vt == VT_UNKNOWN.0 as u16 {
            // Compare against UIA reserved sentinels if available
            let mut skip = false;
            unsafe {
                if let Ok(ns) = UiaGetReservedNotSupportedValue() {
                    let p = var.Anonymous.Anonymous.Anonymous.punkVal.clone();
                    if let Some(u) = p.as_ref() {
                        if u.as_raw() == ns.as_raw() {
                            skip = true;
                        }
                    }
                }
                if !skip {
                    if let Ok(mx) = UiaGetReservedMixedAttributeValue() {
                        let p = var.Anonymous.Anonymous.Anonymous.punkVal.clone();
                        if let Some(u) = p.as_ref() {
                            if u.as_raw() == mx.as_raw() {
                                skip = true;
                            }
                        }
                    }
                }
            }
            if skip {
                unsafe { let _ = VariantClear(&mut var); }
                continue;
            }
        }

        if let Some(value) = unsafe { variant_to_ui_value(&var) } {
            out.push((name.clone(), value));
        }
        unsafe { let _ = VariantClear(&mut var); }
    }
    out
}

unsafe fn variant_to_ui_value(variant: &VARIANT) -> Option<UiValue> {
    let vt = unsafe { variant.Anonymous.Anonymous.vt.0 as u16 };

    // Handle SAFEARRAY values
    if (vt & VT_ARRAY.0 as u16) != 0 {
        if (vt & VT_BYREF.0 as u16) != 0 {
            return None; // unsupported indirection for now
        }
        let base = vt & (VT_TYPEMASK.0 as u16);
        let psa = unsafe { variant.Anonymous.Anonymous.Anonymous.parray };
        if psa.is_null() {
            return None;
        }
        // Only support 1D arrays for now
        let dim = unsafe { SafeArrayGetDim(psa) };
        if dim != 1 { return None; }
        let lb = unsafe { SafeArrayGetLBound(psa, 1) }.ok()?;
        let ub = unsafe { SafeArrayGetUBound(psa, 1) }.ok()?;
        let mut items: Vec<UiValue> = Vec::new();
        for i in lb..=ub {
            match base {
                x if x == VT_BSTR.0 as u16 => {
                    let mut b: BSTR = BSTR::new();
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut b as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(b.to_string()));
                    }
                }
                x if x == VT_BOOL.0 as u16 => {
                    let mut v: VARIANT_BOOL = VARIANT_BOOL(0);
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v.as_bool()));
                    }
                }
                x if x == VT_I2.0 as u16 => {
                    let mut v: i16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_UI2.0 as u16 => {
                    let mut v: u16 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_I4.0 as u16 => {
                    let mut v: i32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_UI4.0 as u16 => {
                    let mut v: u32 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_I8.0 as u16 => {
                    let mut v: i64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_UI8.0 as u16 => {
                    let mut v: u64 = 0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as i64));
                    }
                }
                x if x == VT_R4.0 as u16 => {
                    let mut v: f32 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v as f64));
                    }
                }
                x if x == VT_R8.0 as u16 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DATE.0 as u16 => {
                    let mut v: f64 = 0.0;
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut v as *mut _ as *mut _) }.is_ok() {
                        items.push(UiValue::from(v));
                    }
                }
                x if x == VT_DECIMAL.0 as u16 => {
                    let mut d: DECIMAL = unsafe { std::mem::zeroed() };
                    if unsafe { SafeArrayGetElement(psa, &i as *const _ as *const i32, &mut d as *mut _ as *mut _) }.is_ok() {
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
        x if x == VT_BOOL.0 as u16 => {
            let b = unsafe { variant.Anonymous.Anonymous.Anonymous.boolVal.as_bool() };
            Some(UiValue::from(b))
        }
        x if x == VT_I2.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.iVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_I4.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.lVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_UI2.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.uiVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_UI4.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ulVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_I8.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.llVal };
            Some(UiValue::from(v))
        }
        x if x == VT_UI8.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.ullVal };
            Some(UiValue::from(v as i64))
        }
        x if x == VT_R4.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.fltVal };
            Some(UiValue::from(v as f64))
        }
        x if x == VT_R8.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.dblVal };
            Some(UiValue::from(v))
        }
        x if x == VT_DATE.0 as u16 => {
            let v = unsafe { variant.Anonymous.Anonymous.Anonymous.date };
            Some(UiValue::from(v))
        }
        x if x == VT_BSTR.0 as u16 => {
            let s = unsafe { variant.Anonymous.Anonymous.Anonymous.bstrVal.to_string() };
            if s.is_empty() { None } else { Some(UiValue::from(s)) }
        }
        x if x == VT_DECIMAL.0 as u16 => {
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
