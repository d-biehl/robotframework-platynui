# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Maximizable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Maximizable']


class Maximizable(PatternBase):
    """An element that can be maximized."""

    pattern_name = 'org.platynui.patterns.Maximizable'

    @property
    @abstractmethod
    def is_maximized(self) -> bool:
        """Whether the element is currently in the maximized state."""

    @property
    @abstractmethod
    def can_maximize(self) -> bool:
        """Whether `maximize` is currently available."""

    @abstractmethod
    def maximize(self) -> None:
        """Move the element into the maximized state."""
