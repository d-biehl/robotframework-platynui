# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Table pattern — tabular geometry and row access."""

from abc import abstractmethod
from typing import TYPE_CHECKING, TypeVar

from .base import PatternBase

if TYPE_CHECKING:
    from ..context import ContextBase
    from ..locator import Locator, LocatorScope

__all__ = ['Table']

T = TypeVar('T', bound='ContextBase')


class Table(PatternBase):
    """A container exposing rows arranged in a 2D grid."""

    pattern_name = 'org.platynui.patterns.Table'

    @property
    @abstractmethod
    def row_count(self) -> int:
        """The total number of rows."""

    @property
    @abstractmethod
    def column_count(self) -> int:
        """The total number of columns."""

    @abstractmethod
    def get_row(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> T:
        """Resolve a single row as an instance of ``ctx``.

        Locator/scope follow `ContextBase.get` merge semantics.
        """

    @abstractmethod
    def get_rows(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> list[T]:
        """Resolve every row as a list of ``ctx`` instances."""
