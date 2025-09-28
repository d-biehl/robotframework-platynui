"""platynui_native root package.

This package exposes two Rust-backed submodules:
- platynui_native.core     (types, namespaces, attribute names)
- platynui_native.runtime  (Runtime orchestration, nodes)
"""

from ._native import core, runtime  # noqa: F401

__all__ = ("core", "runtime")
