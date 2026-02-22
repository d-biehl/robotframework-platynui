//! COM bootstrap and thread-local UIA singletons.
//!
//! - `ensure_com_mta()` calls `CoInitializeEx(nullptr, COINIT_MULTITHREADED)` once per thread.
//! - `uia()` returns a thread-local `IUIAutomation` instance (created once via `CoCreateInstance`).
//! - `raw_walker()` returns a thread-local RawView `IUIAutomationTreeWalker`.
//!
//! This avoids repeatedly creating COM objects and keeps all UIA calls on the
//! same MTA thread when used from iterator code.

use std::cell::{Cell, RefCell};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationTreeWalker, UIA_AutomationIdPropertyId,
    UIA_ControlTypePropertyId, UIA_IsContentElementPropertyId, UIA_IsControlElementPropertyId, UIA_ProcessIdPropertyId,
    UIA_RuntimeIdPropertyId,
};

thread_local! {
    static COM_INIT: Cell<bool> = const { Cell::new(false) };
    static UIA_SINGLETON: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
    static RAW_WALKER: RefCell<Option<IUIAutomationTreeWalker>> = const { RefCell::new(None) };
    static TRAVERSAL_CACHE: RefCell<Option<IUIAutomationCacheRequest>> = const { RefCell::new(None) };
}

pub fn ensure_com_mta() {
    COM_INIT.with(|flag| {
        if !flag.get() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            flag.set(true);
        }
    });
}

pub fn uia() -> Result<IUIAutomation, crate::error::UiaError> {
    ensure_com_mta();
    UIA_SINGLETON.with(|cell| {
        if let Some(existing) = cell.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let created: IUIAutomation = unsafe {
            crate::error::uia_api(
                "CoCreateInstance(CUIAutomation)",
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER),
            )?
        };
        *cell.borrow_mut() = Some(created.clone());
        Ok(created)
    })
}

pub fn raw_walker() -> Result<IUIAutomationTreeWalker, crate::error::UiaError> {
    let uia = uia()?;
    RAW_WALKER.with(|cell| {
        if let Some(existing) = cell.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let walker: IUIAutomationTreeWalker =
            unsafe { crate::error::uia_api("IUIAutomation::RawViewWalker", uia.RawViewWalker())? };
        *cell.borrow_mut() = Some(walker.clone());
        Ok(walker)
    })
}

/// Returns a cached `IUIAutomationCacheRequest` pre-loaded with properties needed during tree
/// traversal: ProcessId, ControlType, IsControlElement, IsContentElement, AutomationId,
/// and RuntimeId. Using BuildCache walker methods with this request fetches all properties
/// in a single cross-process call per element rather than one call per property.
/// Clears all thread-local COM singletons on the calling thread.
///
/// This releases the UIA handle, walker, and cache request so that COM
/// resources are freed promptly during provider shutdown.  Subsequent calls
/// to [`uia()`], [`raw_walker()`], or [`traversal_cache_request()`] will
/// lazily re-create the singletons.
pub fn clear_thread_local_singletons() {
    UIA_SINGLETON.with(|cell| {
        *cell.borrow_mut() = None;
    });
    RAW_WALKER.with(|cell| {
        *cell.borrow_mut() = None;
    });
    TRAVERSAL_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub fn traversal_cache_request() -> Result<IUIAutomationCacheRequest, crate::error::UiaError> {
    let uia = uia()?;
    TRAVERSAL_CACHE.with(|cell| {
        if let Some(existing) = cell.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let req: IUIAutomationCacheRequest =
            unsafe { crate::error::uia_api("IUIAutomation::CreateCacheRequest", uia.CreateCacheRequest())? };
        unsafe {
            let _ = req.AddProperty(UIA_ProcessIdPropertyId);
            let _ = req.AddProperty(UIA_ControlTypePropertyId);
            let _ = req.AddProperty(UIA_IsControlElementPropertyId);
            let _ = req.AddProperty(UIA_IsContentElementPropertyId);
            let _ = req.AddProperty(UIA_AutomationIdPropertyId);
            let _ = req.AddProperty(UIA_RuntimeIdPropertyId);
        }
        *cell.borrow_mut() = Some(req.clone());
        Ok(req)
    })
}
