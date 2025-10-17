use pyo3::prelude::*;

use platynui_link::platynui_link_providers;

mod core;
mod runtime;

platynui_link_providers!();

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
