# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Shared type aliases and geometry primitives for the PlatynUI core.

Re-exports `Point`, `Rect` and `PointerButton` from
the native module so Python code, adapters and the runtime share a single
representation without conversion shims.
"""

from platynui_native import Point, PointerButton, Rect, Size

__all__ = [
    'FrameworkId',
    'MouseButton',
    'PatternName',
    'Point',
    'PointerButton',
    'Rect',
    'RoleName',
    'Size',
]


#: Reverse-DNS pattern identifier, e.g. ``"org.platynui.patterns.Activatable"``.
type PatternName = str

#: UI role name in PascalCase, e.g. ``"Button"``, ``"Window"``, ``"Application"``.
type RoleName = str

#: UI framework identifier reported by the platform, e.g. ``"WPF"``, ``"Qt"``, ``"Gtk"``.
type FrameworkId = str

#: Mouse button identifier. Alias for `PointerButton`
#: with members ``LEFT``, ``RIGHT``, ``MIDDLE``, ``X1``, ``X2``.
type MouseButton = PointerButton
