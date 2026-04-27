# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Selectable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Selectable']


class Selectable(PatternBase):
    """An element that can be selected within its container."""

    pattern_name = 'org.platynui.patterns.Selectable'

    @property
    @abstractmethod
    def is_selected(self) -> bool:
        """Whether the element is currently selected."""

    @abstractmethod
    def select(self) -> None:
        """Select the element."""
