# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Resizable pattern."""

from abc import abstractmethod

from ..types import Size
from .base import PatternBase

__all__ = ['Resizable']


class Resizable(PatternBase):
    """An element whose size can be changed."""

    pattern_name = 'org.platynui.patterns.Resizable'

    @property
    @abstractmethod
    def can_resize(self) -> bool:
        """Whether `resize` is currently available."""

    @abstractmethod
    def resize(self, size: Size) -> None:
        """Resize the element to `size`."""
