# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Has-column-headers pattern — optional column-header sequence on tables."""

from abc import abstractmethod
from collections.abc import Sequence
from typing import TYPE_CHECKING, TypeVar

from .base import PatternBase

if TYPE_CHECKING:
    from ..adapter import Adapter
    from ..context import ContextBase
    from ..locator import Locator, LocatorScope

__all__ = ['HasColumnHeaders']

T = TypeVar('T', bound='ContextBase')


class HasColumnHeaders(PatternBase):
    """A table that exposes a column-header sequence."""

    pattern_name = 'org.platynui.patterns.HasColumnHeaders'

    @property
    @abstractmethod
    def column_headers(self) -> 'Sequence[Adapter]':
        """Column-header adapters in column order."""

    @abstractmethod
    def get_column_headers(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> list[T]:
        """Resolve column-headers as a list of ``ctx`` instances.

        Locator/scope follow `ContextBase.get` merge semantics.
        """
