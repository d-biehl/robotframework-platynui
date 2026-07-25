//! FFI surface of `WindowsAccessBridge-64.dll`, transcribed from the JDK
//! headers (`%JAVA_HOME%\include\win32\bridge`).
//!
//! `AccessBridgePackages.h` defines the structs — natural MSVC alignment, **no
//! packing pragmas**, fixed UTF-16 buffers with silent truncation.
//! `AccessBridgeCalls.c` documents the real export names: lowercase camelCase
//! plus `Windows_run`, undecorated cdecl (the capitalized names in
//! `AccessBridgeCalls.h` are wrapper shims, not exports).

use windows::Win32::Foundation::HWND;
use windows::core::BOOL;

/// `JOBJECT64` — a `jlong` in both the `-32` and `-64` APIs (the wire format is
/// bitness-neutral; only the legacy unsuffixed DLL uses pointer-sized
/// `jobject`). Opaque JVM-side reference that must be released via
/// `releaseJavaObject`, or the **target JVM** leaks.
pub(crate) type JObject64 = i64;

/// `vmID` is a C `long`, i.e. 32-bit on Windows (LLP64).
pub(crate) type VmId = i32;

/// Fixed buffer sizes from `AccessBridgePackages.h` (UTF-16 code units).
pub(crate) const MAX_STRING_SIZE: usize = 1024;
pub(crate) const SHORT_STRING_SIZE: usize = 256;
/// Large transfer buffer size (`MAX_BUFFER_SIZE`), used for text ranges.
pub(crate) const MAX_BUFFER_SIZE: usize = 10240;

/// Bit values of the `accessibleInterfaces` bitfield. Note: the header's
/// `// 1 << n` comments are off by one — these are the actual values.
pub(crate) const INTERFACE_VALUE: i32 = 1;
pub(crate) const INTERFACE_ACTION: i32 = 2;
pub(crate) const INTERFACE_COMPONENT: i32 = 4;
pub(crate) const INTERFACE_SELECTION: i32 = 8;
pub(crate) const INTERFACE_TABLE: i32 = 16;
pub(crate) const INTERFACE_TEXT: i32 = 32;
pub(crate) const INTERFACE_HYPERTEXT: i32 = 64;

/// `AccessibleContextInfo` from `AccessBridgePackages.h`. The trailing field
/// was historically `BOOL accessibleValue` and is today the
/// `accessibleInterfaces` bitfield (same offset).
#[repr(C)]
pub(crate) struct AccessibleContextInfo {
    pub name: [u16; MAX_STRING_SIZE],
    pub description: [u16; MAX_STRING_SIZE],
    pub role: [u16; SHORT_STRING_SIZE],
    pub role_en_us: [u16; SHORT_STRING_SIZE],
    pub states: [u16; SHORT_STRING_SIZE],
    pub states_en_us: [u16; SHORT_STRING_SIZE],
    pub index_in_parent: i32,
    pub children_count: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub accessible_component: i32,
    pub accessible_action: i32,
    pub accessible_selection: i32,
    pub accessible_text: i32,
    pub accessible_interfaces: i32,
}

impl AccessibleContextInfo {
    /// All-zero instance for out-parameter use (plain-old-data, no invariants).
    pub(crate) fn zeroed() -> Box<Self> {
        Box::new(Self {
            name: [0; MAX_STRING_SIZE],
            description: [0; MAX_STRING_SIZE],
            role: [0; SHORT_STRING_SIZE],
            role_en_us: [0; SHORT_STRING_SIZE],
            states: [0; SHORT_STRING_SIZE],
            states_en_us: [0; SHORT_STRING_SIZE],
            index_in_parent: 0,
            children_count: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            accessible_component: 0,
            accessible_action: 0,
            accessible_selection: 0,
            accessible_text: 0,
            accessible_interfaces: 0,
        })
    }
}

// Layout guards derived from the header (natural alignment, x64). A mismatch
// here means the transcription drifted from `AccessBridgePackages.h` and every
// field read past the drift point would be garbage.
const _: () = assert!(std::mem::size_of::<AccessibleContextInfo>() == 6188);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, role_en_us) == 4608);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, states_en_us) == 5632);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, index_in_parent) == 6144);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, children_count) == 6148);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, x) == 6152);
const _: () = assert!(std::mem::offset_of!(AccessibleContextInfo, accessible_interfaces) == 6184);

#[repr(C)]
#[allow(clippy::struct_field_names)] // mirrors the header field names verbatim
pub(crate) struct AccessBridgeVersionInfo {
    pub vm_version: [u16; SHORT_STRING_SIZE],
    pub bridge_java_class_version: [u16; SHORT_STRING_SIZE],
    pub bridge_java_dll_version: [u16; SHORT_STRING_SIZE],
    pub bridge_win_dll_version: [u16; SHORT_STRING_SIZE],
}

impl AccessBridgeVersionInfo {
    pub(crate) fn zeroed() -> Box<Self> {
        Box::new(Self {
            vm_version: [0; SHORT_STRING_SIZE],
            bridge_java_class_version: [0; SHORT_STRING_SIZE],
            bridge_java_dll_version: [0; SHORT_STRING_SIZE],
            bridge_win_dll_version: [0; SHORT_STRING_SIZE],
        })
    }
}

const _: () = assert!(std::mem::size_of::<AccessBridgeVersionInfo>() == 2048);

/// `AccessibleTextInfo`: character count, caret index, index at point.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub(crate) struct AccessibleTextInfo {
    pub char_count: i32,
    pub caret_index: i32,
    pub index_at_point: i32,
}

const _: () = assert!(std::mem::size_of::<AccessibleTextInfo>() == 12);

// ---------------------------------------------------------------------------
// Interface-getter out-parameter structs (`AccessBridgePackages.h`). All are
// plain-old-data (integers, handles, fixed UTF-16 buffers); the all-zero bit
// pattern is a valid value for every one of them. Out-parameters start from
// `zeroed()` — small structs by value, large ones heap-boxed (`boxed_zeroed`).

/// Fixed array sizes from `AccessBridgePackages.h`.
pub(crate) const MAX_KEY_BINDINGS: usize = 10;
pub(crate) const MAX_RELATION_TARGETS: usize = 25;
pub(crate) const MAX_RELATIONS: usize = 5;
pub(crate) const MAX_HYPERLINKS: usize = 64;
pub(crate) const MAX_ACTION_INFO: usize = 256;

/// All-zero heap allocation for the POD out-parameter structs below. Kept
/// private so the "all-zero is valid" argument stays local to this module.
#[allow(unsafe_code)]
fn boxed_zeroed<T>() -> Box<T> {
    // SAFETY: only instantiated for the `#[repr(C)]` structs in this module,
    // which consist solely of integers, raw handles, and fixed arrays thereof
    // — all-zero is a valid, fully initialized value for each of them.
    unsafe { Box::new_zeroed().assume_init() }
}

/// `AccessibleTableInfo`: table dimensions plus JVM-side references (caption,
/// summary, the table's own context, and the `AccessibleTable` object that the
/// per-cell and selection calls take). Every non-null handle must be released.
#[repr(C)]
pub(crate) struct AccessibleTableInfo {
    pub caption: JObject64,
    pub summary: JObject64,
    pub row_count: i32,
    pub column_count: i32,
    pub accessible_context: JObject64,
    pub accessible_table: JObject64,
}

impl AccessibleTableInfo {
    pub(crate) fn zeroed() -> Self {
        Self { caption: 0, summary: 0, row_count: 0, column_count: 0, accessible_context: 0, accessible_table: 0 }
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleTableInfo>() == 40);
const _: () = assert!(std::mem::offset_of!(AccessibleTableInfo, row_count) == 16);
const _: () = assert!(std::mem::offset_of!(AccessibleTableInfo, accessible_context) == 24);
const _: () = assert!(std::mem::offset_of!(AccessibleTableInfo, accessible_table) == 32);

/// `AccessibleTableCellInfo`: one cell's coordinates, extents, and selection
/// state. `accessible_context` (the cell's own context) must be released.
#[repr(C)]
pub(crate) struct AccessibleTableCellInfo {
    pub accessible_context: JObject64,
    pub index: i32,
    pub row: i32,
    pub column: i32,
    pub row_extent: i32,
    pub column_extent: i32,
    /// `jboolean` — a single byte, not a Win32 `BOOL`.
    pub is_selected: u8,
}

impl AccessibleTableCellInfo {
    pub(crate) fn zeroed() -> Self {
        Self { accessible_context: 0, index: 0, row: 0, column: 0, row_extent: 0, column_extent: 0, is_selected: 0 }
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleTableCellInfo>() == 32);
const _: () = assert!(std::mem::offset_of!(AccessibleTableCellInfo, index) == 8);
const _: () = assert!(std::mem::offset_of!(AccessibleTableCellInfo, is_selected) == 28);

/// `AccessibleTextSelectionInfo`: selection bounds plus the selected text.
#[repr(C)]
pub(crate) struct AccessibleTextSelectionInfo {
    pub selection_start_index: i32,
    pub selection_end_index: i32,
    pub selected_text: [u16; MAX_STRING_SIZE],
}

impl AccessibleTextSelectionInfo {
    pub(crate) fn zeroed() -> Box<Self> {
        boxed_zeroed()
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleTextSelectionInfo>() == 2056);
const _: () = assert!(std::mem::offset_of!(AccessibleTextSelectionInfo, selected_text) == 8);

/// `AccessibleActionInfo` / `AccessibleActions`: the action-name list. No
/// embedded handles. ~128 KiB — always heap-allocated.
#[repr(C)]
pub(crate) struct AccessibleActionInfo {
    pub name: [u16; SHORT_STRING_SIZE],
}

#[repr(C)]
pub(crate) struct AccessibleActions {
    pub actions_count: i32,
    pub action_info: [AccessibleActionInfo; MAX_ACTION_INFO],
}

impl AccessibleActions {
    pub(crate) fn zeroed() -> Box<Self> {
        boxed_zeroed()
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleActionInfo>() == 512);
const _: () = assert!(std::mem::size_of::<AccessibleActions>() == 131_076);
const _: () = assert!(std::mem::offset_of!(AccessibleActions, action_info) == 4);

/// `AccessibleKeyBindingInfo` / `AccessibleKeyBindings`: key bindings
/// associated with a component. No embedded handles.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct AccessibleKeyBindingInfo {
    /// `jchar` — a UTF-16 code unit; under `ACCESSIBLE_FKEY_KEYSTROKE` it
    /// holds the F-key number, under `ACCESSIBLE_CONTROLCODE_KEYSTROKE` a
    /// control-code constant.
    pub character: u16,
    pub modifiers: i32,
}

#[repr(C)]
pub(crate) struct AccessibleKeyBindings {
    pub key_bindings_count: i32,
    pub key_binding_info: [AccessibleKeyBindingInfo; MAX_KEY_BINDINGS],
}

impl AccessibleKeyBindings {
    pub(crate) fn zeroed() -> Self {
        Self {
            key_bindings_count: 0,
            key_binding_info: [AccessibleKeyBindingInfo { character: 0, modifiers: 0 }; MAX_KEY_BINDINGS],
        }
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleKeyBindingInfo>() == 8);
const _: () = assert!(std::mem::offset_of!(AccessibleKeyBindingInfo, modifiers) == 4);
const _: () = assert!(std::mem::size_of::<AccessibleKeyBindings>() == 84);
const _: () = assert!(std::mem::offset_of!(AccessibleKeyBindings, key_binding_info) == 4);

/// `AccessibleRelationInfo` / `AccessibleRelationSetInfo`: relation keys with
/// target references. Every non-null target handle must be released.
#[repr(C)]
pub(crate) struct AccessibleRelationInfo {
    pub key: [u16; SHORT_STRING_SIZE],
    pub target_count: i32,
    pub targets: [JObject64; MAX_RELATION_TARGETS],
}

#[repr(C)]
pub(crate) struct AccessibleRelationSetInfo {
    pub relation_count: i32,
    pub relations: [AccessibleRelationInfo; MAX_RELATIONS],
}

impl AccessibleRelationSetInfo {
    pub(crate) fn zeroed() -> Box<Self> {
        boxed_zeroed()
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleRelationInfo>() == 720);
const _: () = assert!(std::mem::offset_of!(AccessibleRelationInfo, target_count) == 512);
const _: () = assert!(std::mem::offset_of!(AccessibleRelationInfo, targets) == 520);
const _: () = assert!(std::mem::size_of::<AccessibleRelationSetInfo>() == 3608);
const _: () = assert!(std::mem::offset_of!(AccessibleRelationSetInfo, relations) == 8);

/// `AccessibleHyperlinkInfo` / `AccessibleHypertextInfo`: hyperlink summary.
/// Each filled link carries an `accessibleHyperlink` handle, and the struct an
/// `accessibleHypertext` handle — all must be released. ~33 KiB.
#[repr(C)]
pub(crate) struct AccessibleHyperlinkInfo {
    pub text: [u16; SHORT_STRING_SIZE],
    pub start_index: i32,
    pub end_index: i32,
    pub accessible_hyperlink: JObject64,
}

#[repr(C)]
pub(crate) struct AccessibleHypertextInfo {
    pub link_count: i32,
    pub links: [AccessibleHyperlinkInfo; MAX_HYPERLINKS],
    pub accessible_hypertext: JObject64,
}

impl AccessibleHypertextInfo {
    pub(crate) fn zeroed() -> Box<Self> {
        boxed_zeroed()
    }
}

const _: () = assert!(std::mem::size_of::<AccessibleHyperlinkInfo>() == 528);
const _: () = assert!(std::mem::offset_of!(AccessibleHyperlinkInfo, accessible_hyperlink) == 520);
const _: () = assert!(std::mem::size_of::<AccessibleHypertextInfo>() == 33808);
const _: () = assert!(std::mem::offset_of!(AccessibleHypertextInfo, links) == 8);
const _: () = assert!(std::mem::offset_of!(AccessibleHypertextInfo, accessible_hypertext) == 33800);

// Key-binding modifier bits (`ACCESSIBLE_*_KEYSTROKE`).
pub(crate) const KEYSTROKE_SHIFT: i32 = 1;
pub(crate) const KEYSTROKE_CONTROL: i32 = 2;
pub(crate) const KEYSTROKE_META: i32 = 4;
pub(crate) const KEYSTROKE_ALT: i32 = 8;
pub(crate) const KEYSTROKE_ALT_GRAPH: i32 = 16;
pub(crate) const KEYSTROKE_BUTTON1: i32 = 32;
pub(crate) const KEYSTROKE_BUTTON2: i32 = 64;
pub(crate) const KEYSTROKE_BUTTON3: i32 = 128;
/// F key pressed; `character` contains 1–24.
pub(crate) const KEYSTROKE_FKEY: i32 = 256;
/// Control-code key pressed; `character` contains the `ACCESSIBLE_VK_*` code.
pub(crate) const KEYSTROKE_CONTROLCODE: i32 = 512;

// Function-pointer types for the lowercase cdecl exports actually present in
// the DLL. Every call is synchronous blocking IPC (`SendMessage` + shared
// memory) into the target JVM — a hung JVM blocks the calling thread, which is
// why all of these run exclusively on the pump thread.
pub(crate) type WindowsRunFn = unsafe extern "C" fn();
pub(crate) type IsJavaWindowFn = unsafe extern "C" fn(HWND) -> BOOL;
pub(crate) type GetAccessibleContextFromHwndFn = unsafe extern "C" fn(HWND, *mut VmId, *mut JObject64) -> BOOL;
pub(crate) type GetAccessibleContextInfoFn = unsafe extern "C" fn(VmId, JObject64, *mut AccessibleContextInfo) -> BOOL;
pub(crate) type GetAccessibleChildFromContextFn = unsafe extern "C" fn(VmId, JObject64, i32) -> JObject64;
pub(crate) type GetAccessibleParentFromContextFn = unsafe extern "C" fn(VmId, JObject64) -> JObject64;
/// Native hit-test: deepest accessible context at desktop point `(x, y)`
/// within `acParent` (`getAccessibleContextAt`).
pub(crate) type GetAccessibleContextAtFn = unsafe extern "C" fn(VmId, JObject64, i32, i32, *mut JObject64) -> BOOL;
pub(crate) type ReleaseJavaObjectFn = unsafe extern "C" fn(VmId, JObject64);
pub(crate) type IsSameObjectFn = unsafe extern "C" fn(VmId, JObject64, JObject64) -> BOOL;
pub(crate) type GetVersionInfoFn = unsafe extern "C" fn(VmId, *mut AccessBridgeVersionInfo) -> BOOL;
pub(crate) type RequestFocusFn = unsafe extern "C" fn(VmId, JObject64) -> BOOL;
pub(crate) type GetAccessibleTextInfoFn =
    unsafe extern "C" fn(VmId, JObject64, *mut AccessibleTextInfo, i32, i32) -> BOOL;
pub(crate) type GetAccessibleTextRangeFn = unsafe extern "C" fn(VmId, JObject64, i32, i32, *mut u16, i16) -> BOOL;
/// Shared by `getCurrentAccessibleValueFromContext`, `getMaximumAccessibleValueFromContext`,
/// and `getMinimumAccessibleValueFromContext` (value returned as a string).
pub(crate) type GetAccessibleValueFn = unsafe extern "C" fn(VmId, JObject64, *mut u16, i16) -> BOOL;
pub(crate) type GetAccessibleSelectedChildrenCountFn = unsafe extern "C" fn(VmId, JObject64) -> i32;
pub(crate) type IsAccessibleChildSelectedFn = unsafe extern "C" fn(VmId, JObject64, i32) -> BOOL;
/// `getAccessibleTableInfo(vmID, accessibleContext, *out)`.
pub(crate) type GetAccessibleTableInfoFn = unsafe extern "C" fn(VmId, JObject64, *mut AccessibleTableInfo) -> BOOL;
/// `getAccessibleTableCellInfo(vmID, accessibleTable, row, column, *out)` —
/// takes the `AccessibleTable` handle from `AccessibleTableInfo`, not the
/// table's accessible context.
pub(crate) type GetAccessibleTableCellInfoFn =
    unsafe extern "C" fn(VmId, JObject64, i32, i32, *mut AccessibleTableCellInfo) -> BOOL;
/// Shared by `getAccessibleTableRowSelectionCount` and
/// `getAccessibleTableColumnSelectionCount` (both take the `AccessibleTable`
/// handle and answer a count, `-1` on error).
pub(crate) type GetAccessibleTableSelectionCountFn = unsafe extern "C" fn(VmId, JObject64) -> i32;
pub(crate) type GetAccessibleTextSelectionInfoFn =
    unsafe extern "C" fn(VmId, JObject64, *mut AccessibleTextSelectionInfo) -> BOOL;
pub(crate) type GetAccessibleActionsFn = unsafe extern "C" fn(VmId, JObject64, *mut AccessibleActions) -> BOOL;
/// `getAccessibleHypertextExt(vmID, accessibleContext, nStartIndex, *out)`.
pub(crate) type GetAccessibleHypertextExtFn =
    unsafe extern "C" fn(VmId, JObject64, i32, *mut AccessibleHypertextInfo) -> BOOL;
pub(crate) type GetAccessibleKeyBindingsFn = unsafe extern "C" fn(VmId, JObject64, *mut AccessibleKeyBindings) -> BOOL;
pub(crate) type GetAccessibleRelationSetFn =
    unsafe extern "C" fn(VmId, JObject64, *mut AccessibleRelationSetInfo) -> BOOL;

/// Decode the `accessibleInterfaces` bitfield into stable interface names
/// (used for `native:Interfaces`).
pub(crate) fn interface_names(bits: i32) -> Vec<&'static str> {
    const NAMES: [(i32, &str); 7] = [
        (INTERFACE_VALUE, "value"),
        (INTERFACE_ACTION, "action"),
        (INTERFACE_COMPONENT, "component"),
        (INTERFACE_SELECTION, "selection"),
        (INTERFACE_TABLE, "table"),
        (INTERFACE_TEXT, "text"),
        (INTERFACE_HYPERTEXT, "hypertext"),
    ];
    NAMES.iter().filter(|(bit, _)| bits & bit != 0).map(|(_, name)| *name).collect()
}

/// Decode a fixed-size, NUL-terminated UTF-16 buffer.
pub(crate) fn wide_str(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_info_layout_matches_header() {
        // The compile-time asserts above are the real guard; this test keeps
        // the numbers visible in test output and fails loudly if the consts
        // are ever relaxed.
        assert_eq!(std::mem::size_of::<AccessibleContextInfo>(), 6188);
        assert_eq!(std::mem::size_of::<AccessBridgeVersionInfo>(), 2048);
        assert_eq!(std::mem::size_of::<AccessibleTextInfo>(), 12);
        assert_eq!(std::mem::offset_of!(AccessibleContextInfo, index_in_parent), 6144);
    }

    #[test]
    fn interface_struct_layouts_match_header() {
        assert_eq!(std::mem::size_of::<AccessibleTableInfo>(), 40);
        assert_eq!(std::mem::size_of::<AccessibleTableCellInfo>(), 32);
        assert_eq!(std::mem::size_of::<AccessibleTextSelectionInfo>(), 2056);
        assert_eq!(std::mem::size_of::<AccessibleActions>(), 131_076);
        assert_eq!(std::mem::size_of::<AccessibleKeyBindings>(), 84);
        assert_eq!(std::mem::size_of::<AccessibleRelationSetInfo>(), 3608);
        assert_eq!(std::mem::size_of::<AccessibleHypertextInfo>(), 33808);
    }

    #[test]
    fn boxed_zeroed_out_params_start_all_zero() {
        let table = AccessibleTableInfo::zeroed();
        assert_eq!((table.row_count, table.column_count), (0, 0));
        assert_eq!(table.accessible_table, 0);
        let actions = AccessibleActions::zeroed();
        assert_eq!(actions.actions_count, 0);
        assert!(actions.action_info[MAX_ACTION_INFO - 1].name.iter().all(|&c| c == 0));
        let relations = AccessibleRelationSetInfo::zeroed();
        assert_eq!(relations.relation_count, 0);
        assert!(relations.relations[MAX_RELATIONS - 1].targets.iter().all(|&t| t == 0));
    }

    #[test]
    fn wide_str_stops_at_nul() {
        let mut buf = [0u16; 8];
        for (i, c) in "abc".encode_utf16().enumerate() {
            buf[i] = c;
        }
        assert_eq!(wide_str(&buf), "abc");
    }

    #[test]
    fn wide_str_without_nul_takes_whole_buffer() {
        let buf: Vec<u16> = "full".encode_utf16().collect();
        assert_eq!(wide_str(&buf), "full");
    }

    #[test]
    fn interface_names_decode_bits() {
        assert_eq!(interface_names(0), Vec::<&str>::new());
        assert_eq!(interface_names(INTERFACE_VALUE | INTERFACE_TEXT), vec!["value", "text"]);
        assert_eq!(
            interface_names(INTERFACE_COMPONENT | INTERFACE_SELECTION | INTERFACE_HYPERTEXT),
            vec!["component", "selection", "hypertext"]
        );
    }
}
