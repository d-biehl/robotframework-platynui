# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Expandable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Expandable']


class Expandable(PatternBase):
    """An element that can be expanded and collapsed."""

    pattern_name = 'org.platynui.patterns.Expandable'

    @property
    @abstractmethod
    def can_expand(self) -> bool:
        """Whether the element has anything to expand into."""

    @property
    @abstractmethod
    def is_expanded(self) -> bool:
        """Whether the element is currently expanded."""

    @abstractmethod
    def expand(self) -> None:
        """Expand the element."""

    @abstractmethod
    def collapse(self) -> None:
        """Collapse the element."""
