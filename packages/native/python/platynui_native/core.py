"""Shim module for Pylance/pyright to resolve module source.

Re-exports the native `platynui_native._native.core` symbols.
"""

from ._native.core import *  # noqa: F401,F403

