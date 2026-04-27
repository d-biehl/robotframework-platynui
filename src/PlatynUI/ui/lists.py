# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`List` container and `ListItem` for selectable list entries."""

from collections.abc import Iterator

from ..core import patterns
from ..core.locator import Locator
from .control import Control
from .item import SelectableItem

__all__ = ['List', 'ListItem']


class ListItem(SelectableItem):
    """A selectable entry inside a `List`."""


class List(Control):
    """A container of `ListItem` entries."""

    @property
    def item_count(self) -> int:
        """The total number of items in the list."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).item_count

    def get_items(self, *, locator: Locator | None = None) -> list[ListItem]:
        """Return every list item below this container."""
        return self.get_all(ListItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[ListItem]:
        """Iterate over every list item below this container."""
        return self.iter_all(ListItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> ListItem:
        """Resolve a single list item, raising if zero or multiple match."""
        return self.get(ListItem, locator=locator, scope='children')

    def select(self, *, locator: Locator | None = None) -> ListItem:
        """Resolve a list item and select it."""
        item = self.get_item(locator=locator)
        item.select()
        return item
