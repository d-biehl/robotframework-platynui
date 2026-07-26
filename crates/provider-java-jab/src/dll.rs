//! Discovery and binding of the JAB client DLL (`WindowsAccessBridge-64.dll`).
//!
//! Discovery order (first hit wins):
//! 1. `providers.java.jab.dll_path` from the runtime config,
//! 2. the `PLATYNUI_JAB_DLL` environment variable,
//! 3. `%JAVA_HOME%\jre\bin` then `%JAVA_HOME%\bin` (JDK 8 keeps the client DLL
//!    in `jre\bin`, JDK 9+ in `bin`),
//! 4. every `PATH` entry — the DLL directly, plus the JDK 8 quirk that `PATH`
//!    usually holds `<jdk>\bin` (where `java.exe` lives) while the DLL sits in
//!    `<jdk>\jre\bin` next to it.
//!
//! The DLL is only ever loaded on the pump thread (see `pump.rs`).

use crate::ffi;
use std::path::{Path, PathBuf};

pub(crate) const DLL_NAME: &str = "WindowsAccessBridge-64.dll";

/// Inputs to DLL discovery, separated from the live environment so the order
/// is unit-testable with fake directory layouts.
pub(crate) struct DiscoveryInputs {
    /// `providers.java.jab.dll_path` from the runtime config.
    pub config_dll_path: Option<PathBuf>,
    /// `PLATYNUI_JAB_DLL` environment override.
    pub env_dll_path: Option<PathBuf>,
    /// `%JAVA_HOME%`.
    pub java_home: Option<PathBuf>,
    /// Parsed `PATH` entries.
    pub path_dirs: Vec<PathBuf>,
}

impl DiscoveryInputs {
    pub(crate) fn from_environment(config_dll_path: Option<PathBuf>) -> Self {
        Self {
            config_dll_path,
            env_dll_path: std::env::var_os("PLATYNUI_JAB_DLL").map(PathBuf::from),
            java_home: std::env::var_os("JAVA_HOME").map(PathBuf::from),
            path_dirs: std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default(),
        }
    }
}

/// Where a discovery attempt looked and failed; feeds the one actionable
/// diagnostic the provider logs when no DLL is present.
#[derive(Debug)]
pub(crate) struct DiscoveryFailure {
    pub tried: Vec<String>,
}

impl std::fmt::Display for DiscoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{DLL_NAME} not found. Set providers.java.jab.dll_path (or PLATYNUI_JAB_DLL), set JAVA_HOME, or put a JDK on PATH. Tried: {}",
            if self.tried.is_empty() { "<nothing>".to_string() } else { self.tried.join(", ") }
        )
    }
}

/// Resolve the client DLL according to the documented discovery order.
pub(crate) fn discover_dll(inputs: &DiscoveryInputs) -> Result<PathBuf, DiscoveryFailure> {
    let mut tried: Vec<String> = Vec::new();

    for explicit in [&inputs.config_dll_path, &inputs.env_dll_path].into_iter().flatten() {
        if explicit.is_file() {
            return Ok(explicit.clone());
        }
        tried.push(explicit.display().to_string());
    }

    if let Some(java_home) = &inputs.java_home {
        for sub in ["jre\\bin", "bin"] {
            let candidate = java_home.join(sub).join(DLL_NAME);
            if candidate.is_file() {
                return Ok(candidate);
            }
            tried.push(candidate.display().to_string());
        }
    }

    for dir in &inputs.path_dirs {
        let direct = dir.join(DLL_NAME);
        if direct.is_file() {
            return Ok(direct);
        }
        if dir.join("java.exe").is_file()
            && let Some(jdk_root) = dir.parent()
        {
            let jre = jdk_root.join("jre").join("bin").join(DLL_NAME);
            if jre.is_file() {
                return Ok(jre);
            }
            tried.push(jre.display().to_string());
        }
    }

    Err(DiscoveryFailure { tried })
}

/// The bound client DLL. Function pointers stay valid exactly as long as
/// `_library` lives, so the library handle is kept in the struct and dropped
/// last (field order) when the pump thread winds down.
pub(crate) struct Bridge {
    pub windows_run: ffi::WindowsRunFn,
    pub is_java_window: ffi::IsJavaWindowFn,
    pub get_accessible_context_from_hwnd: ffi::GetAccessibleContextFromHwndFn,
    pub get_accessible_context_info: ffi::GetAccessibleContextInfoFn,
    pub get_accessible_child_from_context: ffi::GetAccessibleChildFromContextFn,
    pub get_accessible_parent_from_context: ffi::GetAccessibleParentFromContextFn,
    pub get_accessible_context_at: ffi::GetAccessibleContextAtFn,
    pub release_java_object: ffi::ReleaseJavaObjectFn,
    pub is_same_object: ffi::IsSameObjectFn,
    pub get_version_info: ffi::GetVersionInfoFn,
    pub request_focus: ffi::RequestFocusFn,
    pub get_accessible_text_info: ffi::GetAccessibleTextInfoFn,
    pub get_accessible_text_range: ffi::GetAccessibleTextRangeFn,
    pub get_current_accessible_value: ffi::GetAccessibleValueFn,
    pub get_maximum_accessible_value: ffi::GetAccessibleValueFn,
    pub get_minimum_accessible_value: ffi::GetAccessibleValueFn,
    pub get_accessible_selected_children_count: ffi::GetAccessibleSelectedChildrenCountFn,
    pub is_accessible_child_selected: ffi::IsAccessibleChildSelectedFn,
    pub get_accessible_table_info: ffi::GetAccessibleTableInfoFn,
    pub get_accessible_table_cell_info: ffi::GetAccessibleTableCellInfoFn,
    pub get_accessible_table_row_selection_count: ffi::GetAccessibleTableSelectionCountFn,
    pub get_accessible_table_column_selection_count: ffi::GetAccessibleTableSelectionCountFn,
    pub get_accessible_text_selection_info: ffi::GetAccessibleTextSelectionInfoFn,
    pub get_accessible_actions: ffi::GetAccessibleActionsFn,
    pub get_accessible_hypertext_ext: ffi::GetAccessibleHypertextExtFn,
    pub get_accessible_key_bindings: ffi::GetAccessibleKeyBindingsFn,
    pub get_accessible_relation_set: ffi::GetAccessibleRelationSetFn,
    _library: libloading::Library,
}

impl Bridge {
    #[allow(unsafe_code)]
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        // SAFETY: loading the vendor DLL and resolving its documented cdecl
        // exports. The transmuted function-pointer types in `ffi` are
        // transcribed from the JDK headers; the `Library` stays alive in the
        // struct so every pointer remains valid for the struct's lifetime.
        unsafe {
            let library =
                libloading::Library::new(path).map_err(|e| format!("loading {} failed: {e}", path.display()))?;
            macro_rules! bind {
                ($name:literal) => {
                    *library.get($name).map_err(|e| {
                        format!("export {} not found: {e}", String::from_utf8_lossy($name).trim_end_matches('\0'))
                    })?
                };
            }
            Ok(Self {
                windows_run: bind!(b"Windows_run\0"),
                is_java_window: bind!(b"isJavaWindow\0"),
                get_accessible_context_from_hwnd: bind!(b"getAccessibleContextFromHWND\0"),
                get_accessible_context_info: bind!(b"getAccessibleContextInfo\0"),
                get_accessible_child_from_context: bind!(b"getAccessibleChildFromContext\0"),
                get_accessible_parent_from_context: bind!(b"getAccessibleParentFromContext\0"),
                get_accessible_context_at: bind!(b"getAccessibleContextAt\0"),
                release_java_object: bind!(b"releaseJavaObject\0"),
                is_same_object: bind!(b"isSameObject\0"),
                get_version_info: bind!(b"getVersionInfo\0"),
                request_focus: bind!(b"requestFocus\0"),
                get_accessible_text_info: bind!(b"getAccessibleTextInfo\0"),
                get_accessible_text_range: bind!(b"getAccessibleTextRange\0"),
                get_current_accessible_value: bind!(b"getCurrentAccessibleValueFromContext\0"),
                get_maximum_accessible_value: bind!(b"getMaximumAccessibleValueFromContext\0"),
                get_minimum_accessible_value: bind!(b"getMinimumAccessibleValueFromContext\0"),
                get_accessible_selected_children_count: bind!(b"getAccessibleSelectionCountFromContext\0"),
                is_accessible_child_selected: bind!(b"isAccessibleChildSelectedFromContext\0"),
                get_accessible_table_info: bind!(b"getAccessibleTableInfo\0"),
                get_accessible_table_cell_info: bind!(b"getAccessibleTableCellInfo\0"),
                get_accessible_table_row_selection_count: bind!(b"getAccessibleTableRowSelectionCount\0"),
                get_accessible_table_column_selection_count: bind!(b"getAccessibleTableColumnSelectionCount\0"),
                get_accessible_text_selection_info: bind!(b"getAccessibleTextSelectionInfo\0"),
                get_accessible_actions: bind!(b"getAccessibleActions\0"),
                get_accessible_hypertext_ext: bind!(b"getAccessibleHypertextExt\0"),
                get_accessible_key_bindings: bind!(b"getAccessibleKeyBindings\0"),
                get_accessible_relation_set: bind!(b"getAccessibleRelationSet\0"),
                _library: library,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, b"").expect("touch");
    }

    fn inputs() -> DiscoveryInputs {
        DiscoveryInputs { config_dll_path: None, env_dll_path: None, java_home: None, path_dirs: Vec::new() }
    }

    #[test]
    fn config_path_wins_over_everything() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_dll = dir.path().join("custom").join(DLL_NAME);
        touch(&config_dll);
        let java_home = dir.path().join("jdk");
        touch(&java_home.join("bin").join(DLL_NAME));

        let mut inputs = inputs();
        inputs.config_dll_path = Some(config_dll.clone());
        inputs.java_home = Some(java_home);
        assert_eq!(discover_dll(&inputs).expect("found"), config_dll);
    }

    #[test]
    fn env_override_beats_java_home() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env_dll = dir.path().join("env").join(DLL_NAME);
        touch(&env_dll);
        let java_home = dir.path().join("jdk");
        touch(&java_home.join("bin").join(DLL_NAME));

        let mut inputs = inputs();
        inputs.env_dll_path = Some(env_dll.clone());
        inputs.java_home = Some(java_home);
        assert_eq!(discover_dll(&inputs).expect("found"), env_dll);
    }

    #[test]
    fn missing_config_path_falls_through_to_java_home() {
        let dir = tempfile::tempdir().expect("tempdir");
        let java_home = dir.path().join("jdk");
        let jdk_dll = java_home.join("bin").join(DLL_NAME);
        touch(&jdk_dll);

        let mut inputs = inputs();
        inputs.config_dll_path = Some(dir.path().join("missing").join(DLL_NAME));
        inputs.java_home = Some(java_home);
        assert_eq!(discover_dll(&inputs).expect("found"), jdk_dll);
    }

    #[test]
    fn jdk8_jre_bin_wins_over_bin() {
        let dir = tempfile::tempdir().expect("tempdir");
        let java_home = dir.path().join("jdk8");
        let jre_dll = java_home.join("jre").join("bin").join(DLL_NAME);
        touch(&jre_dll);
        touch(&java_home.join("bin").join(DLL_NAME));

        let mut inputs = inputs();
        inputs.java_home = Some(java_home);
        assert_eq!(discover_dll(&inputs).expect("found"), jre_dll);
    }

    #[test]
    fn path_dir_direct_hit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("some-bin");
        let dll = bin.join(DLL_NAME);
        touch(&dll);

        let mut inputs = inputs();
        inputs.path_dirs = vec![dir.path().join("empty"), bin];
        assert_eq!(discover_dll(&inputs).expect("found"), dll);
    }

    #[test]
    fn path_java_exe_derives_jdk8_jre_layout() {
        // PATH holds <jdk>\bin (java.exe), the DLL sits in <jdk>\jre\bin.
        let dir = tempfile::tempdir().expect("tempdir");
        let jdk = dir.path().join("jdk8");
        touch(&jdk.join("bin").join("java.exe"));
        let jre_dll = jdk.join("jre").join("bin").join(DLL_NAME);
        touch(&jre_dll);

        let mut inputs = inputs();
        inputs.path_dirs = vec![jdk.join("bin")];
        assert_eq!(discover_dll(&inputs).expect("found"), jre_dll);
    }

    #[test]
    fn failure_lists_tried_locations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut inputs = inputs();
        inputs.config_dll_path = Some(dir.path().join("nope").join(DLL_NAME));
        inputs.java_home = Some(dir.path().join("jdk"));

        let err = discover_dll(&inputs).expect_err("must fail");
        assert_eq!(err.tried.len(), 3, "config + jre\\bin + bin: {:?}", err.tried);
        let message = err.to_string();
        assert!(message.contains("providers.java.jab.dll_path"), "{message}");
        assert!(message.contains("JAVA_HOME"), "{message}");
    }
}
