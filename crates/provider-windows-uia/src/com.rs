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
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTreeWalker};

thread_local! {
    static COM_INIT: Cell<bool> = const { Cell::new(false) };
    static UIA_SINGLETON: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
    static RAW_WALKER: RefCell<Option<IUIAutomationTreeWalker>> = const { RefCell::new(None) };
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
