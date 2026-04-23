# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Keyboard focus pattern."""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Focusable']


class Focusable(PatternBase):
    """An element that can receive keyboard focus.

    Combines the focus state (`is_focused`) with the focus
    action (`focus`) so callers see a single capability.
    """

    pattern_name = 'org.platynui.patterns.Focusable'

    @property
    @abstractmethod
    def is_focused(self) -> bool:
        """Whether this element currently holds keyboard focus."""

    @abstractmethod
    def focus(self) -> None:
        """Move keyboard focus to this element."""
