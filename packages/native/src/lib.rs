use pyo3::prelude::*;
use platynui_link::platynui_link_os_providers;

mod core;
mod runtime;

// Link platform providers into the extension module for non-test builds, so
// Python users get OS integrations without additional linking crates.
platynui_link_os_providers!();

// Link mock providers so they're available in Python.
// Mock providers are registered explicitly in runtime.rs, not via the inventory system.
use platynui_platform_mock as _;
use platynui_provider_mock as _;

/// Native extension module `_native` installed under the `platynui_native` package.
/// All classes and functions are registered directly in the module (no submodules).
#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register all core types directly in the main module
    core::register_types(m)?;

    // Register all runtime types and functions directly in the main module
    runtime::register_types(py, m)?;

    Ok(())
}