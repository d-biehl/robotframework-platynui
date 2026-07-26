//! COM bootstrap and thread-local UIA singletons.
//!
//! - `ensure_com_mta()` calls `CoInitializeEx(nullptr, COINIT_MULTITHREADED)` once per thread.
//! - `uia()` returns a thread-local `IUIAutomation` instance (created once via `CoCreateInstance`).
//! - `raw_walker()` returns a thread-local RawView `IUIAutomationTreeWalker`.
//!
//! This avoids repeatedly creating COM objects and keeps all UIA calls on the
//! same MTA thread when used from iterator code.

use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
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

// HRESULT for "cannot change thread mode after it is set".
const RPC_E_CHANGED_MODE_HRESULT: i32 = 0x8001_0106u32 as i32;

pub fn ensure_com_mta() {
    COM_INIT.with(|flag| {
        if !flag.get() {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_ok() {
                flag.set(true);
            } else if hr.0 == RPC_E_CHANGED_MODE_HRESULT {
                // Another library may have initialized COM on this thread
                // with a different apartment model (commonly STA).
                flag.set(true);
            } else {
                tracing::warn!(hr = ?hr, "CoInitializeEx(MTA) failed");
            }
        }
    });
}

/// Gates the UIA client library's one-time, process-wide initialization.
///
/// The library is safe to use from many threads *once it is up*, but bringing it
/// up concurrently is not: measured on Windows 11, eight threads whose first ever
/// UIA use overlaps see `CoCreateInstance` succeed everywhere and then
/// `GetRootElement` fail with `E_FAIL` on seven of the eight. Priming it once on
/// a single thread first makes all eight succeed, and it stays warm for the life
/// of the process.
///
/// So the first arrival performs the initialization while holding this lock and
/// the others wait for it. Every thread still keeps its own `IUIAutomation` in
/// [`UIA_SINGLETON`], so the lock is taken once per thread, not per call.
static FIRST_INIT: Mutex<bool> = Mutex::new(false);

pub fn uia() -> Result<IUIAutomation, crate::error::UiaError> {
    ensure_com_mta();
    UIA_SINGLETON.with(|cell| {
        if let Some(existing) = cell.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let created = create_uia_serialized()?;
        *cell.borrow_mut() = Some(created.clone());
        Ok(created)
    })
}

/// Creates this thread's `IUIAutomation`, serialized against a cold start.
fn create_uia_serialized() -> Result<IUIAutomation, crate::error::UiaError> {
    // Poisoning carries no broken invariant here: the flag only records whether
    // the library has been warmed up, so a panicking initializer just means the
    // next arrival retries.
    let mut warmed_up = FIRST_INIT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let created: IUIAutomation = unsafe {
        crate::error::uia_api(
            "CoCreateInstance(CUIAutomation)",
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER),
        )?
    };

    if !*warmed_up {
        // One real call, not just the creation, is what finishes the library's
        // initialization — measured: serializing `CoCreateInstance` alone only
        // takes the cold-start failures from 7/8 down to 1-2/8, while adding
        // this read removes them.
        //
        // A failure here is ignored on purpose: the caller's own first call will
        // report an unreadable desktop root with proper context, and a transient
        // failure must not leave the library permanently marked as cold.
        let _ = unsafe { created.GetRootElement() };
        *warmed_up = true;
    }

    Ok(created)
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

#[cfg(test)]
mod cold_start_tests {
    /// A cold concurrent start must not fail.
    ///
    /// Without the serialized first initialization in [`create_uia_serialized`],
    /// eight threads whose first ever UIA use overlaps leave seven of them with
    /// `GetRootElement` returning `E_FAIL`. That is not merely a test-harness
    /// artefact: the provider is reached from several threads (tree streaming,
    /// the Inspector, hit-testing), so a cold parallel start is reachable in
    /// production too.
    ///
    /// This test only means something on the process's *first* UIA use, so it
    /// must be the only test in this module — once the library is warm, any
    /// fan-out succeeds and the assertion proves nothing.
    #[test]
    fn concurrent_first_use_succeeds() {
        const THREADS: usize = 8;
        let handles: Vec<_> = (0..THREADS)
            .map(|i| {
                std::thread::spawn(move || match super::uia() {
                    Err(e) => format!("thread {i}: CoCreateInstance failed: {e:?}"),
                    Ok(uia) => match unsafe { uia.GetRootElement() } {
                        Ok(_) => String::new(),
                        Err(e) => format!("thread {i}: GetRootElement failed: {e:?}"),
                    },
                })
            })
            .collect();

        let failures: Vec<String> =
            handles.into_iter().map(|h| h.join().expect("worker thread")).filter(|r| !r.is_empty()).collect();

        assert!(
            failures.is_empty(),
            "cold concurrent start failed on {} of {THREADS} threads: {failures:?}",
            failures.len()
        );
    }
}
