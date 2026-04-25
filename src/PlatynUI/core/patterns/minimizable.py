# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Minimizable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Minimizable']


class Minimizable(PatternBase):
    """An element that can be minimized."""

    pattern_name = 'org.platynui.patterns.Minimizable'

    @property
    @abstractmethod
    def is_minimized(self) -> bool:
        """Whether the element is currently in the minimized state."""

    @property
    @abstractmethod
    def can_minimize(self) -> bool:
        """Whether `minimize` is currently available."""

    @abstractmethod
    def minimize(self) -> None:
        """Move the element into the minimized state."""
