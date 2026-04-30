# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Has-row-headers pattern — optional row-header sequence on tables."""

from abc import abstractmethod
from collections.abc import Sequence
from typing import TYPE_CHECKING, TypeVar

from .base import PatternBase

if TYPE_CHECKING:
    from ..adapter import Adapter
    from ..context import ContextBase
    from ..locator import Locator, LocatorScope

__all__ = ['HasRowHeaders']

T = TypeVar('T', bound='ContextBase')


class HasRowHeaders(PatternBase):
    """A table that exposes a row-header sequence."""

    pattern_name = 'org.platynui.patterns.HasRowHeaders'

    @property
    @abstractmethod
    def row_headers(self) -> 'Sequence[Adapter]':
        """Row-header adapters in row order."""

    @abstractmethod
    def get_row_headers(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> list[T]:
        """Resolve row-headers as a list of ``ctx`` instances.

        Locator/scope follow `ContextBase.get` merge semantics.
        """
