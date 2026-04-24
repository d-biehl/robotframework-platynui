# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Base element capability: bounds, visibility, and enabled state.

Adapters that surface any kind of on-screen widget implement this
pattern to expose its geometry and basic interaction state.
"""

from abc import abstractmethod

from ..types import Point, Rect
from .base import PatternBase

__all__ = ['Element']


class Element(PatternBase):
    """An on-screen UI element with geometry and visibility state.

    Exposes the four core properties every visible widget shares:

    - `bounds`: screen rectangle in absolute pixels.
    - `is_visible`: the element is rendered (not hidden or
      collapsed).
    - `is_in_view`: the element lies within the viewport (not
      scrolled off-screen).
    - `is_enabled`: the element accepts user input.

    `default_click_position` is a convenience point used by
    high-level actions like mouse click and hover. It defaults to the
    centre of `bounds`; adapters may override it to return a
    more accurate hit point when the platform exposes one.
    """

    pattern_name = 'org.platynui.patterns.Element'

    @property
    @abstractmethod
    def bounds(self) -> Rect:
        """The element's screen rectangle in absolute pixels."""

    @property
    @abstractmethod
    def is_visible(self) -> bool:
        """Whether the element is rendered (not hidden or collapsed)."""

    @property
    @abstractmethod
    def is_in_view(self) -> bool:
        """Whether the element lies within the viewport."""

    @property
    @abstractmethod
    def is_enabled(self) -> bool:
        """Whether the element accepts user input."""

    @property
    def default_click_position(self) -> Point:
        """The default interaction point; centre of ``bounds``."""
        return self.bounds.center()
