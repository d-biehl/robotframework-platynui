# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Item-container size pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['ItemContainer']


class ItemContainer(PatternBase):
    """A container that exposes typed item, row, and column counts.

    Concrete container roles only implement the properties that are
    meaningful for them; unsupported properties raise
    `NotImplementedError`.
    """

    pattern_name = 'org.platynui.patterns.ItemContainer'

    @property
    @abstractmethod
    def item_count(self) -> int:
        """The total number of items."""

    @property
    @abstractmethod
    def row_count(self) -> int:
        """The total number of rows."""

    @property
    @abstractmethod
    def column_count(self) -> int:
        """The total number of columns."""
