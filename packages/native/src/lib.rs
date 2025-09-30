use pyo3::prelude::*;
use platynui_link::platynui_link_os_providers;

mod core;
mod runtime;

// Link platform providers into the extension module for non-test builds, so
// Python users get OS integrations without additional linking crates.
platynui_link_os_providers!();

/// Internal native module `_native` installed under the `platynui_native` package.
/// Registers the `core` and `runtime` submodules.
#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let core_mod = PyModule::new(py, "core")?;
    core::init_submodule(py, &core_mod)?;
    m.add_submodule(&core_mod)?;

    let runtime_mod = PyModule::new(py, "runtime")?;
    runtime::init_submodule(py, &runtime_mod, &core_mod)?;
    m.add_submodule(&runtime_mod)?;

    // __all__ = ("core", "runtime")
    m.add("__all__", ("core", "runtime"))?;
    Ok(())
}