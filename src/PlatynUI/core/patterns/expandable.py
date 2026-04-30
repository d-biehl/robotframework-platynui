# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Expandable patterns (Read/Action split, Rev. 46).

The expand/collapse capability is split into a Read-pattern
(`IsExpandable`) carrying state from the native adapter and an
Action-pattern (`Expandable`) synthesised by default proxies via
mouse double-click or keyboard arrows. See
`docs/python-library-design.md` §5a.4 and §A.14.18.
"""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Expandable', 'IsExpandable']


class IsExpandable(PatternBase):
    """Read-pattern: expansion state of an element."""

    pattern_name = 'org.platynui.patterns.IsExpandable'

    @property
    @abstractmethod
    def can_expand(self) -> bool:
        """Whether the element has anything to expand into."""

    @property
    @abstractmethod
    def is_expanded(self) -> bool:
        """Whether the element is currently expanded."""


class Expandable(PatternBase):
    """Action-pattern: expand/collapse an element."""

    pattern_name = 'org.platynui.patterns.Expandable'

    @abstractmethod
    def expand(self) -> None:
        """Expand the element."""

    @abstractmethod
    def collapse(self) -> None:
        """Collapse the element."""
