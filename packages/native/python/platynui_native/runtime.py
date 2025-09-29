"""Shim module for Pylance/pyright to resolve module source.

At runtime, all symbols are implemented in the native extension
`platynui_native._native.runtime`. This shim simply re-exports them so that
editors that prefer a .py source file (e.g. Pylance's reportMissingModuleSource)
can resolve the module while still using `runtime.pyi` for type information.
"""

from ._native.runtime import *  # noqa: F401,F403

