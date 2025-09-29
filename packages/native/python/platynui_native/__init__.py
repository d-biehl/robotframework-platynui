"""platynui_native root package.

This package exposes two Rust-backed submodules:
- platynui_native.core     (types, namespaces, attribute names)
- platynui_native.runtime  (Runtime orchestration, nodes)

Both submodules are implemented inside the native extension `platynui_native._native`.
To support `from platynui_native.runtime import Runtime` we alias the extension
submodules into this package's module namespace.
"""

from . import _native as _ext
import sys as _sys

# Bind convenient attributes
core = _ext.core
runtime = _ext.runtime

# Improve module identity in tracebacks/introspection
try:
    core.__name__ = __name__ + ".core"
    runtime.__name__ = __name__ + ".runtime"
except Exception:
    pass

# Register module aliases so `platynui_native.runtime` and `.core` are importable
_sys.modules[__name__ + ".core"] = core
_sys.modules[__name__ + ".runtime"] = runtime

# Note: TypedDicts for typing are declared in .pyi. At runtime, the
# native module exposes a concrete `Attribute` class for isinstance checks.

__all__ = ("core", "runtime")

# EvaluatedAttribute is a real class in the native module.
