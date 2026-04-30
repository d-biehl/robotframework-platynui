# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Item-container pattern.

Counts and item lookups for any container that exposes a homogeneous
sequence of items (lists, trees, tab strips, menus, combo dropdowns,
table rows, row cells). Tabular geometry (rows/columns/headers) lives
in the dedicated `Table`, `HasRowHeaders`, and `HasColumnHeaders`
patterns.

`ItemContainer` is intentionally not provided as a native adapter
wrapper: provider-side `ItemCount` reads are not reliable across
toolkits (WPF `VirtualizingStackPanel`, HTML custom layouts), so the
default implementation lives Python-side in
`ui/proxies/.../ItemContainerProxyMixin`.
"""

from abc import abstractmethod
from typing import TYPE_CHECKING, TypeVar

from .base import PatternBase

if TYPE_CHECKING:
    from ..context import ContextBase
    from ..locator import Locator, LocatorScope

__all__ = ['ItemContainer']

T = TypeVar('T', bound='ContextBase')


class ItemContainer(PatternBase):
    """A container exposing a homogeneous sequence of typed items."""

    pattern_name = 'org.platynui.patterns.ItemContainer'

    @property
    @abstractmethod
    def item_count(self) -> int:
        """The total number of items in this container."""

    @abstractmethod
    def get_item(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> T:
        """Resolve a single item as an instance of ``ctx``.

        ``locator`` and ``scope`` follow the merge semantics of
        `ContextBase.get`: a user-supplied locator overrides the
        per-class default of ``ctx``; ``scope`` overrides the
        locator's axis for one call.
        """

    @abstractmethod
    def get_items(
        self,
        ctx: type[T],
        *,
        locator: 'Locator | None' = None,
        scope: 'LocatorScope | None' = None,
    ) -> list[T]:
        """Resolve every item as a list of ``ctx`` instances."""
