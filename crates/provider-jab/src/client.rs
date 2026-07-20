//! Typed, thread-safe API over the pump thread.
//!
//! Every method marshals one bridge call onto the pump thread and waits for
//! the reply with the configured per-call deadline. A timeout returns an error
//! to the caller immediately, but the pump thread itself may still be blocked
//! inside the OS call until the JVM responds — repeated timeouts therefore
//! mark the `vmID` degraded and new calls against it fail fast until a
//! `getVersionInfo` health probe succeeds (see [`crate::pump::DegradedTracker`]).

use crate::dll::Bridge;
use crate::error::JabError;
use crate::ffi::{self, AccessBridgeVersionInfo, AccessibleContextInfo, AccessibleTextInfo, JObject64, VmId, wide_str};
use crate::handle::{JabObject, ReleaseSender};
use crate::map::{StateFlags, parse_states};
use crate::pump::{DegradedTracker, Job, PumpConnection};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::Foundation::HWND;

/// Parsed, owned copy of one `getAccessibleContextInfo` result — everything a
/// node needs, detached from the fixed-buffer FFI struct.
#[derive(Debug, Clone)]
pub(crate) struct ContextInfo {
    pub name: String,
    pub description: String,
    pub role_localized: String,
    pub role_en_us: String,
    pub states_en_us: String,
    pub states: StateFlags,
    pub index_in_parent: i32,
    pub children_count: i32,
    /// `None` when the bridge reports the hidden-element sentinel
    /// `(-1, -1, -1x-1)` (or otherwise degenerate extents).
    pub bounds: Option<(i32, i32, i32, i32)>,
    pub interfaces: i32,
}

impl ContextInfo {
    fn from_ffi(raw: &AccessibleContextInfo) -> Self {
        let bounds = if raw.width < 0 || raw.height < 0 { None } else { Some((raw.x, raw.y, raw.width, raw.height)) };
        let states_en_us = wide_str(&raw.states_en_us);
        Self {
            name: wide_str(&raw.name),
            description: wide_str(&raw.description),
            role_localized: wide_str(&raw.role),
            role_en_us: wide_str(&raw.role_en_us),
            states: parse_states(&states_en_us),
            states_en_us,
            index_in_parent: raw.index_in_parent,
            children_count: raw.children_count,
            bounds,
            interfaces: raw.accessible_interfaces,
        }
    }

    pub(crate) fn has_interface(&self, bit: i32) -> bool {
        self.interfaces & bit != 0
    }
}

/// Bridge version strings for one JVM (`getVersionInfo`).
#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)] // mirrors the header field names verbatim
pub(crate) struct VersionInfo {
    pub vm_version: String,
    pub bridge_java_class_version: String,
    pub bridge_java_dll_version: String,
    pub bridge_win_dll_version: String,
}

/// Owned copy of one `getAccessibleTableInfo` result. The caption, summary,
/// and duplicate-context handles are released before this leaves the pump
/// thread; only the `AccessibleTable` handle survives (as RAII `table`)
/// because the per-cell and selection calls take it, not the context.
#[derive(Debug)]
pub(crate) struct TableInfo {
    pub row_count: i32,
    pub column_count: i32,
    pub has_caption: bool,
    pub has_summary: bool,
    pub table: JabObject,
}

/// Owned copy of one `getAccessibleTableCellInfo` result (the cell's own
/// context handle is released after extraction).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableCellInfo {
    pub index: i32,
    pub row: i32,
    pub column: i32,
    pub row_extent: i32,
    pub column_extent: i32,
    pub is_selected: bool,
}

/// Owned copy of one `getAccessibleTextInfo` result (the probe point for
/// `index_at_point` is unused here).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextInfo {
    pub char_count: i32,
    pub caret_index: i32,
}

/// Owned copy of one `getAccessibleTextSelectionInfo` result.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextSelection {
    pub start_index: i32,
    pub end_index: i32,
}

/// One key binding from `getAccessibleKeyBindings` (raw character/modifiers;
/// formatting happens in the attribute layer).
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyBinding {
    pub character: u16,
    pub modifiers: i32,
}

/// Summary of one relation from `getAccessibleRelationSet` (target handles
/// are released after counting — only the summary survives).
#[derive(Debug, Clone)]
pub(crate) struct RelationSummary {
    pub key: String,
    pub target_count: i32,
}

pub(crate) struct JabClient {
    job_tx: mpsc::Sender<Job>,
    release_tx: ReleaseSender,
    call_timeout: Duration,
    degraded: Arc<DegradedTracker>,
}

impl JabClient {
    pub(crate) fn new(connection: PumpConnection, call_timeout: Duration, degraded: Arc<DegradedTracker>) -> Self {
        Self { job_tx: connection.job_tx, release_tx: connection.release_tx, call_timeout, degraded }
    }

    /// Sink dropped `JabObject`s feed; also used to wrap raw handles inside
    /// pump closures so a reply nobody waits for still releases its handle.
    fn release_sender(&self) -> ReleaseSender {
        self.release_tx.clone()
    }

    /// Enqueue `f` on the pump thread and await its reply within the deadline.
    ///
    /// `vm = None` for calls that do not target a specific JVM (window checks,
    /// initial context lookup). Timing out does not cancel the underlying OS
    /// call — the pump may finish it later; results that arrive after the
    /// deadline are dropped (any `JabObject` inside them releases itself).
    fn call<T>(
        &self,
        vm: Option<VmId>,
        op: &'static str,
        f: impl FnOnce(&Bridge) -> T + Send + 'static,
    ) -> Result<T, JabError>
    where
        T: Send + 'static,
    {
        if let Some(vm) = vm {
            self.ensure_vm_usable(vm)?;
        }
        let result = self.call_unchecked(op, f);
        if let Some(vm) = vm {
            match &result {
                Ok(_) => self.degraded.record_success(vm),
                Err(JabError::Timeout { .. }) => self.degraded.record_timeout(vm),
                Err(_) => {}
            }
        }
        result
    }

    /// `call` without degraded bookkeeping — used by the health probe itself.
    fn call_unchecked<T>(&self, op: &'static str, f: impl FnOnce(&Bridge) -> T + Send + 'static) -> Result<T, JabError>
    where
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = mpsc::channel::<T>();
        let job = Job {
            run: Box::new(move |bridge| {
                let _ = reply_tx.send(f(bridge));
            }),
        };
        self.job_tx.send(job).map_err(|_| JabError::PumpUnavailable)?;
        match reply_rx.recv_timeout(self.call_timeout) {
            Ok(value) => Ok(value),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(JabError::Timeout { op }),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(JabError::PumpUnavailable),
        }
    }

    /// Fail fast for degraded JVMs; when a probe is due, one `getVersionInfo`
    /// round-trip decides whether the JVM is usable again.
    #[allow(unsafe_code)]
    fn ensure_vm_usable(&self, vm: VmId) -> Result<(), JabError> {
        if !self.degraded.is_degraded(vm) {
            return Ok(());
        }
        if !self.degraded.probe_due(vm) {
            return Err(JabError::VmDegraded { vm });
        }
        let probe = self.call_unchecked("getVersionInfo (health probe)", move |bridge| {
            let mut info = AccessBridgeVersionInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread is the JAB thread.
            unsafe { (bridge.get_version_info)(vm, &raw mut *info).as_bool() }
        });
        match probe {
            Ok(true) => {
                self.degraded.record_success(vm);
                Ok(())
            }
            _ => Err(JabError::VmDegraded { vm }),
        }
    }

    #[allow(unsafe_code)]
    pub(crate) fn is_java_window(&self, hwnd: isize) -> Result<bool, JabError> {
        self.call(None, "isJavaWindow", move |bridge| {
            // SAFETY: plain window-handle query on the pump thread.
            unsafe { (bridge.is_java_window)(hwnd_from(hwnd)).as_bool() }
        })
    }

    /// Root accessible context of a Java top-level window. `Ok(None)` when the
    /// bridge answers FALSE (window vanished, bridge not ready for it yet).
    #[allow(unsafe_code)]
    pub(crate) fn context_from_hwnd(&self, hwnd: isize) -> Result<Option<(VmId, JabObject)>, JabError> {
        let release = self.release_sender();
        self.call(None, "getAccessibleContextFromHWND", move |bridge| {
            let mut vm: VmId = 0;
            let mut ctx: JObject64 = 0;
            // SAFETY: valid out-parameters; pump thread.
            let ok = unsafe { (bridge.get_accessible_context_from_hwnd)(hwnd_from(hwnd), &raw mut vm, &raw mut ctx) };
            (ok.as_bool() && ctx != 0).then(|| (vm, JabObject::new(vm, ctx, release)))
        })
    }

    #[allow(unsafe_code)]
    pub(crate) fn version_info(&self, vm: VmId) -> Result<VersionInfo, JabError> {
        self.call(Some(vm), "getVersionInfo", move |bridge| {
            let mut info = AccessBridgeVersionInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_version_info)(vm, &raw mut *info).as_bool() };
            ok.then(|| VersionInfo {
                vm_version: wide_str(&info.vm_version),
                bridge_java_class_version: wide_str(&info.bridge_java_class_version),
                bridge_java_dll_version: wide_str(&info.bridge_java_dll_version),
                bridge_win_dll_version: wide_str(&info.bridge_win_dll_version),
            })
        })?
        .ok_or(JabError::CallFailed { op: "getVersionInfo" })
    }

    #[allow(unsafe_code)]
    pub(crate) fn context_info(&self, obj: &JabObject) -> Result<ContextInfo, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleContextInfo", move |bridge| {
            let mut info = AccessibleContextInfo::zeroed();
            // SAFETY: valid out-parameter (heap-boxed, 6188 bytes); pump thread.
            let ok = unsafe { (bridge.get_accessible_context_info)(vm, handle, &raw mut *info).as_bool() };
            ok.then(|| ContextInfo::from_ffi(&info))
        })?
        .ok_or(JabError::CallFailed { op: "getAccessibleContextInfo" })
    }

    /// Child context at `index`. `Ok(None)` when the bridge returns a null
    /// handle (child vanished between the count and the fetch).
    #[allow(unsafe_code)]
    pub(crate) fn child(&self, obj: &JabObject, index: i32) -> Result<Option<JabObject>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        let release = self.release_sender();
        self.call(Some(vm), "getAccessibleChildFromContext", move |bridge| {
            // SAFETY: pump thread; returns 0 on failure.
            let child = unsafe { (bridge.get_accessible_child_from_context)(vm, handle, index) };
            (child != 0).then(|| JabObject::new(vm, child, release))
        })
    }

    /// Parent context. `Ok(None)` when the bridge returns a null handle (the
    /// context is the top of its accessible tree, or it vanished).
    #[allow(unsafe_code)]
    pub(crate) fn parent(&self, obj: &JabObject) -> Result<Option<JabObject>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        let release = self.release_sender();
        self.call(Some(vm), "getAccessibleParentFromContext", move |bridge| {
            // SAFETY: pump thread; returns 0 when there is no parent.
            let parent = unsafe { (bridge.get_accessible_parent_from_context)(vm, handle) };
            (parent != 0).then(|| JabObject::new(vm, parent, release))
        })
    }

    /// Native hit-test (`getAccessibleContextAt`): deepest accessible context
    /// at desktop point `(x, y)` within `window`. `Ok(None)` when the bridge
    /// reports no context at the point.
    #[allow(unsafe_code)]
    pub(crate) fn context_at(&self, window: &JabObject, x: i32, y: i32) -> Result<Option<JabObject>, JabError> {
        let (vm, handle) = (window.vm(), window.handle());
        let release = self.release_sender();
        self.call(Some(vm), "getAccessibleContextAt", move |bridge| {
            let mut ctx: JObject64 = 0;
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_context_at)(vm, handle, x, y, &raw mut ctx) };
            (ok.as_bool() && ctx != 0).then(|| JabObject::new(vm, ctx, release))
        })
    }

    /// JVM-side identity check — raw handle equality is meaningless.
    #[allow(unsafe_code)]
    pub(crate) fn is_same(&self, a: &JabObject, b: &JabObject) -> Result<bool, JabError> {
        if a.vm() != b.vm() {
            return Ok(false);
        }
        let (vm, ha, hb) = (a.vm(), a.handle(), b.handle());
        self.call(Some(vm), "isSameObject", move |bridge| {
            // SAFETY: pump thread.
            unsafe { (bridge.is_same_object)(vm, ha, hb).as_bool() }
        })
    }

    #[allow(unsafe_code)]
    pub(crate) fn request_focus(&self, obj: &JabObject) -> Result<(), JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        let ok = self.call(Some(vm), "requestFocus", move |bridge| {
            // SAFETY: pump thread.
            unsafe { (bridge.request_focus)(vm, handle).as_bool() }
        })?;
        if ok { Ok(()) } else { Err(JabError::CallFailed { op: "requestFocus" }) }
    }

    /// Character count of an accessible-text element.
    #[allow(unsafe_code)]
    pub(crate) fn text_char_count(&self, obj: &JabObject) -> Result<Option<i32>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleTextInfo", move |bridge| {
            let mut info = AccessibleTextInfo::default();
            // SAFETY: valid out-parameter; pump thread. (0, 0) is the probe
            // point for the unused index-at-point field.
            let ok = unsafe { (bridge.get_accessible_text_info)(vm, handle, &raw mut info, 0, 0).as_bool() };
            ok.then_some(info.char_count)
        })
    }

    /// One chunk of text content (`end` inclusive, per the JAB contract).
    /// Chunk sizes are capped by the caller at `MAX_BUFFER_SIZE - 1`.
    #[allow(unsafe_code)]
    pub(crate) fn text_range(&self, obj: &JabObject, start: i32, end: i32) -> Result<Option<String>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleTextRange", move |bridge| {
            let mut buffer = vec![0u16; ffi::MAX_BUFFER_SIZE];
            let len = i16::try_from(buffer.len()).unwrap_or(i16::MAX);
            // SAFETY: buffer sized to MAX_BUFFER_SIZE and the requested range
            // is capped below it; pump thread.
            let ok = unsafe { (bridge.get_accessible_text_range)(vm, handle, start, end, buffer.as_mut_ptr(), len) };
            ok.as_bool().then(|| {
                let wanted = usize::try_from(end - start + 1).unwrap_or(0).min(buffer.len());
                String::from_utf16_lossy(&buffer[..wanted])
            })
        })
    }

    /// Replace the whole text content. The bridge transports at most
    /// `MAX_STRING_SIZE - 1` UTF-16 units per write — longer texts fail
    /// instead of being silently truncated.
    #[allow(unsafe_code)]
    pub(crate) fn set_text_contents(&self, obj: &JabObject, text: &str) -> Result<(), JabError> {
        let mut encoded: Vec<u16> = text.encode_utf16().collect();
        if encoded.len() > ffi::MAX_STRING_SIZE - 1 {
            return Err(JabError::TextTooLong { limit: ffi::MAX_STRING_SIZE - 1 });
        }
        encoded.push(0);
        let (vm, handle) = (obj.vm(), obj.handle());
        let ok = self.call(Some(vm), "setTextContents", move |bridge| {
            // SAFETY: NUL-terminated UTF-16 buffer outlives the call; pump thread.
            unsafe { (bridge.set_text_contents)(vm, handle, encoded.as_ptr()).as_bool() }
        })?;
        if ok { Ok(()) } else { Err(JabError::CallFailed { op: "setTextContents" }) }
    }

    pub(crate) fn current_value(&self, obj: &JabObject) -> Result<Option<String>, JabError> {
        self.value_string(obj, "getCurrentAccessibleValueFromContext", |bridge| bridge.get_current_accessible_value)
    }

    pub(crate) fn maximum_value(&self, obj: &JabObject) -> Result<Option<String>, JabError> {
        self.value_string(obj, "getMaximumAccessibleValueFromContext", |bridge| bridge.get_maximum_accessible_value)
    }

    pub(crate) fn minimum_value(&self, obj: &JabObject) -> Result<Option<String>, JabError> {
        self.value_string(obj, "getMinimumAccessibleValueFromContext", |bridge| bridge.get_minimum_accessible_value)
    }

    #[allow(unsafe_code)]
    fn value_string(
        &self,
        obj: &JabObject,
        op: &'static str,
        select: impl Fn(&Bridge) -> ffi::GetAccessibleValueFn + Send + 'static,
    ) -> Result<Option<String>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), op, move |bridge| {
            let mut buffer = [0u16; ffi::SHORT_STRING_SIZE];
            let len = i16::try_from(buffer.len()).unwrap_or(i16::MAX);
            // SAFETY: fixed-size out buffer; pump thread.
            let ok = unsafe { (select(bridge))(vm, handle, buffer.as_mut_ptr(), len).as_bool() };
            ok.then(|| wide_str(&buffer))
        })
    }

    #[allow(unsafe_code)]
    pub(crate) fn selected_children_count(&self, obj: &JabObject) -> Result<i32, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleSelectionCountFromContext", move |bridge| {
            // SAFETY: pump thread.
            unsafe { (bridge.get_accessible_selected_children_count)(vm, handle) }
        })
    }

    #[allow(unsafe_code)]
    pub(crate) fn is_child_selected(&self, obj: &JabObject, index: i32) -> Result<bool, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "isAccessibleChildSelectedFromContext", move |bridge| {
            // SAFETY: pump thread.
            unsafe { (bridge.is_accessible_child_selected)(vm, handle, index).as_bool() }
        })
    }

    // -----------------------------------------------------------------------
    // Interface getters (jab-interface-attributes). Each wraps one bridge
    // call on the pump thread, extracts owned Rust values, and releases every
    // embedded `JOBJECT64` it does not hand back.

    /// Container-level table info (`getAccessibleTableInfo`) for a context
    /// supporting `AccessibleTable`. `Ok(None)` when the bridge answers FALSE.
    #[allow(unsafe_code)]
    pub(crate) fn table_info(&self, obj: &JabObject) -> Result<Option<TableInfo>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        let release = self.release_sender();
        self.call(Some(vm), "getAccessibleTableInfo", move |bridge| {
            let mut info = ffi::AccessibleTableInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_table_info)(vm, handle, &raw mut info).as_bool() };
            ok.then(|| {
                // Caption/summary/duplicate-context handles are not needed
                // beyond presence — release them here, on the pump thread.
                for unused in [info.caption, info.summary, info.accessible_context] {
                    if unused != 0 {
                        // SAFETY: releasing a handle the bridge just returned.
                        unsafe { (bridge.release_java_object)(vm, unused) };
                    }
                }
                TableInfo {
                    row_count: info.row_count,
                    column_count: info.column_count,
                    has_caption: info.caption != 0,
                    has_summary: info.summary != 0,
                    table: JabObject::new(vm, info.accessible_table, release),
                }
            })
        })
    }

    /// Per-cell info (`getAccessibleTableCellInfo`). Takes the RAII
    /// `AccessibleTable` handle from [`TableInfo::table`]. `Ok(None)` when the
    /// bridge answers FALSE (coordinate out of range, table gone).
    #[allow(unsafe_code)]
    pub(crate) fn table_cell_info(
        &self,
        table: &JabObject,
        row: i32,
        column: i32,
    ) -> Result<Option<TableCellInfo>, JabError> {
        let (vm, handle) = (table.vm(), table.handle());
        self.call(Some(vm), "getAccessibleTableCellInfo", move |bridge| {
            let mut info = ffi::AccessibleTableCellInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok =
                unsafe { (bridge.get_accessible_table_cell_info)(vm, handle, row, column, &raw mut info) }.as_bool();
            ok.then(|| {
                if info.accessible_context != 0 {
                    // SAFETY: releasing the cell context handle after extraction.
                    unsafe { (bridge.release_java_object)(vm, info.accessible_context) };
                }
                TableCellInfo {
                    index: info.index,
                    row: info.row,
                    column: info.column,
                    row_extent: info.row_extent,
                    column_extent: info.column_extent,
                    is_selected: info.is_selected != 0,
                }
            })
        })
    }

    /// Selected-row count of a table (`getAccessibleTableRowSelectionCount`);
    /// takes the `AccessibleTable` handle. `Ok(None)` on the bridge's `-1`
    /// error convention.
    #[allow(unsafe_code)]
    pub(crate) fn table_row_selection_count(&self, table: &JabObject) -> Result<Option<i32>, JabError> {
        let (vm, handle) = (table.vm(), table.handle());
        self.call(Some(vm), "getAccessibleTableRowSelectionCount", move |bridge| {
            // SAFETY: pump thread.
            let count = unsafe { (bridge.get_accessible_table_row_selection_count)(vm, handle) };
            (count >= 0).then_some(count)
        })
    }

    /// Selected-column count of a table; see [`Self::table_row_selection_count`].
    #[allow(unsafe_code)]
    pub(crate) fn table_column_selection_count(&self, table: &JabObject) -> Result<Option<i32>, JabError> {
        let (vm, handle) = (table.vm(), table.handle());
        self.call(Some(vm), "getAccessibleTableColumnSelectionCount", move |bridge| {
            // SAFETY: pump thread.
            let count = unsafe { (bridge.get_accessible_table_column_selection_count)(vm, handle) };
            (count >= 0).then_some(count)
        })
    }

    /// Character count and caret index (`getAccessibleTextInfo`).
    #[allow(unsafe_code)]
    pub(crate) fn text_info(&self, obj: &JabObject) -> Result<Option<TextInfo>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleTextInfo", move |bridge| {
            let mut info = AccessibleTextInfo::default();
            // SAFETY: valid out-parameter; pump thread. (0, 0) is the probe
            // point for the unused index-at-point field.
            let ok = unsafe { (bridge.get_accessible_text_info)(vm, handle, &raw mut info, 0, 0).as_bool() };
            ok.then_some(TextInfo { char_count: info.char_count, caret_index: info.caret_index })
        })
    }

    /// Selection bounds (`getAccessibleTextSelectionInfo`).
    #[allow(unsafe_code)]
    pub(crate) fn text_selection(&self, obj: &JabObject) -> Result<Option<TextSelection>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleTextSelectionInfo", move |bridge| {
            let mut info = ffi::AccessibleTextSelectionInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_text_selection_info)(vm, handle, &raw mut *info).as_bool() };
            ok.then_some(TextSelection { start_index: info.selection_start_index, end_index: info.selection_end_index })
        })
    }

    /// Available action names (`getAccessibleActions`).
    #[allow(unsafe_code)]
    pub(crate) fn action_names(&self, obj: &JabObject) -> Result<Option<Vec<String>>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleActions", move |bridge| {
            let mut actions = ffi::AccessibleActions::zeroed();
            // SAFETY: valid (heap-boxed, ~128 KiB) out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_actions)(vm, handle, &raw mut *actions).as_bool() };
            ok.then(|| {
                let count = usize::try_from(actions.actions_count).unwrap_or(0).min(ffi::MAX_ACTION_INFO);
                actions.action_info[..count].iter().map(|action| wide_str(&action.name)).collect()
            })
        })
    }

    /// Hyperlink count (`getAccessibleHypertextExt` at start index 0). The
    /// per-link and hypertext handles embedded in the result are released
    /// immediately — only the count survives. `Ok(None)` when the bridge
    /// answers FALSE (no hypertext on the element).
    #[allow(unsafe_code)]
    pub(crate) fn hypertext_link_count(&self, obj: &JabObject) -> Result<Option<i32>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleHypertextExt", move |bridge| {
            let mut info = ffi::AccessibleHypertextInfo::zeroed();
            // SAFETY: valid (heap-boxed, ~33 KiB) out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_hypertext_ext)(vm, handle, 0, &raw mut *info).as_bool() };
            ok.then(|| {
                let filled = usize::try_from(info.link_count).unwrap_or(0).min(ffi::MAX_HYPERLINKS);
                for link in &info.links[..filled] {
                    if link.accessible_hyperlink != 0 {
                        // SAFETY: releasing a handle the bridge just returned.
                        unsafe { (bridge.release_java_object)(vm, link.accessible_hyperlink) };
                    }
                }
                if info.accessible_hypertext != 0 {
                    // SAFETY: as above.
                    unsafe { (bridge.release_java_object)(vm, info.accessible_hypertext) };
                }
                info.link_count
            })
        })
    }

    /// Key bindings of a component (`getAccessibleKeyBindings`).
    #[allow(unsafe_code)]
    pub(crate) fn key_bindings(&self, obj: &JabObject) -> Result<Option<Vec<KeyBinding>>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleKeyBindings", move |bridge| {
            let mut bindings = ffi::AccessibleKeyBindings::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_key_bindings)(vm, handle, &raw mut bindings).as_bool() };
            ok.then(|| {
                let count = usize::try_from(bindings.key_bindings_count).unwrap_or(0).min(ffi::MAX_KEY_BINDINGS);
                bindings.key_binding_info[..count]
                    .iter()
                    .map(|info| KeyBinding { character: info.character, modifiers: info.modifiers })
                    .collect()
            })
        })
    }

    /// Relation summaries (`getAccessibleRelationSet`). All target handles are
    /// released after counting.
    #[allow(unsafe_code)]
    pub(crate) fn relation_summaries(&self, obj: &JabObject) -> Result<Option<Vec<RelationSummary>>, JabError> {
        let (vm, handle) = (obj.vm(), obj.handle());
        self.call(Some(vm), "getAccessibleRelationSet", move |bridge| {
            let mut set = ffi::AccessibleRelationSetInfo::zeroed();
            // SAFETY: valid out-parameter; pump thread.
            let ok = unsafe { (bridge.get_accessible_relation_set)(vm, handle, &raw mut *set).as_bool() };
            ok.then(|| {
                let count = usize::try_from(set.relation_count).unwrap_or(0).min(ffi::MAX_RELATIONS);
                set.relations[..count]
                    .iter()
                    .map(|relation| {
                        let filled = usize::try_from(relation.target_count).unwrap_or(0).min(ffi::MAX_RELATION_TARGETS);
                        for &target in &relation.targets[..filled] {
                            if target != 0 {
                                // SAFETY: releasing a handle the bridge just returned.
                                unsafe { (bridge.release_java_object)(vm, target) };
                            }
                        }
                        RelationSummary { key: wide_str(&relation.key), target_count: relation.target_count }
                    })
                    .collect()
            })
        })
    }
}

fn hwnd_from(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}
