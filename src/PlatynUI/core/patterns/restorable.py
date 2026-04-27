# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Restorable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Restorable']


class Restorable(PatternBase):
    """An element that can be restored from a minimized or maximized state."""

    pattern_name = 'org.platynui.patterns.Restorable'

    @abstractmethod
    def restore(self) -> None:
        """Return the element to its non-minimized, non-maximized state."""
