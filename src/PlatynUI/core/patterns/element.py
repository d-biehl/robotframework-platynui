# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Element` pattern: geometry and visibility state of an on-screen widget."""

from abc import abstractmethod

from ..types import Rect
from .base import PatternBase

__all__ = ['Element']


class Element(PatternBase):
    """Geometry and visibility state of an on-screen widget."""

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
