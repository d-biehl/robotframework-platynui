# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Shared type aliases for the PlatynUI core layer.

Reverse-DNS pattern identifiers (``PatternName``) and the role/technology
naming conventions are defined here so that both pattern ABCs and adapter
implementations share a single source of truth.

Geometry primitives ``Point`` and ``Rect`` are re-exported from the Rust
native module so that pure-Python code, the native adapter, and the runtime
all share a single representation. Keeping them in one place avoids
conversion shims at the FFI boundary.
"""

from __future__ import annotations

from typing import TypeAlias

from platynui_native import Point, Rect

__all__ = [
    'FrameworkId',
    'PatternName',
    'Point',
    'Rect',
    'RoleName',
    'TechnologyName',
]


#: Reverse-DNS pattern identifier, e.g. ``"org.platynui.patterns.Activatable"``.
PatternName: TypeAlias = str

#: UI role name, e.g. ``"Button"``, ``"Window"``, ``"Application"``.
RoleName: TypeAlias = str

#: Adapter technology name, e.g. ``"rust"``, ``"jsonrpc"``, ``"mock"``.
TechnologyName: TypeAlias = str

#: UI framework identifier as reported by the platform (e.g. ``"WPF"``,
#: ``"Qt"``, ``"Gtk"``).
FrameworkId: TypeAlias = str
