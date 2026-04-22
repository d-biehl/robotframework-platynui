# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Base element capability — bounds, visibility and enabled state.

Mirrors the Rust ``element`` attribute group
(``crates/core/src/ui/attributes.rs``) one-to-one. Adapters that surface
any kind of on-screen widget implement this single pattern instead of the
finer-grained ``HasBounds`` / ``Visibility`` / ``HasIsEnabled`` triple
that earlier revisions exposed.
"""

from __future__ import annotations

from abc import abstractmethod

from ..types import Point, Rect
from .base import PatternBase

__all__ = ['Element']


class Element(PatternBase):
    """An on-screen UI element with geometry and visibility state.

    Properties map directly onto the Rust ``element`` attribute names:

    - :attr:`bounds` ↔ ``Bounds``
    - :attr:`is_visible` ↔ ``IsVisible``
    - :attr:`is_in_view` ↔ ``IsInView`` (positive form of UIA ``IsOffscreen``)
    - :attr:`is_enabled` ↔ ``IsEnabled``

    :attr:`default_click_position` is a Python-side convenience that
    defaults to the centre of :attr:`bounds`; adapters may override to
    surface a more accurate hit point (UIA ``ClickablePoint``,
    AT-SPI ``GetExtents`` mid-point, etc.).
    """

    pattern_name = 'org.platynui.patterns.Element'

    @property
    @abstractmethod
    def bounds(self) -> Rect: ...

    @property
    @abstractmethod
    def is_visible(self) -> bool: ...

    @property
    @abstractmethod
    def is_in_view(self) -> bool: ...

    @property
    @abstractmethod
    def is_enabled(self) -> bool: ...

    @property
    def default_click_position(self) -> Point:
        """Default interaction point — centre of :attr:`bounds`."""
        return self.bounds.center()
