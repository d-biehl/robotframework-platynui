# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Shared type aliases and geometry primitives for the PlatynUI core.

Re-exports `Point`, `Rect` and `PointerButton` from
the native module so Python code, adapters and the runtime share a single
representation without conversion shims.
"""

from __future__ import annotations

from typing import TypeAlias

from platynui_native import Point, PointerButton, Rect

__all__ = [
    'FrameworkId',
    'MouseButton',
    'PatternName',
    'Point',
    'PointerButton',
    'Rect',
    'RoleName',
    'TechnologyName',
]


#: Reverse-DNS pattern identifier, e.g. ``"org.platynui.patterns.Activatable"``.
PatternName: TypeAlias = str

#: UI role name in PascalCase, e.g. ``"Button"``, ``"Window"``, ``"Application"``.
RoleName: TypeAlias = str

#: Adapter technology name, e.g. ``"UiNode"``, ``"mock"``.
TechnologyName: TypeAlias = str

#: UI framework identifier reported by the platform, e.g. ``"WPF"``, ``"Qt"``, ``"Gtk"``.
FrameworkId: TypeAlias = str

#: Mouse button identifier. Alias for `PointerButton`
#: with members ``LEFT``, ``RIGHT``, ``MIDDLE``, ``X1``, ``X2``.
MouseButton: TypeAlias = PointerButton
