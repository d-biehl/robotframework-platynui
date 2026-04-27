# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`TabList` container and `TabItem` for selectable tabs."""

from collections.abc import Iterator

from ..core import patterns
from ..core.locator import Locator
from .control import Control
from .item import SelectableItem

__all__ = ['TabItem', 'TabList']


class TabItem(SelectableItem):
    """A selectable tab inside a `TabList`."""


class TabList(Control):
    """A container of `TabItem` entries."""

    @property
    def item_count(self) -> int:
        """The total number of tabs in the list."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).item_count

    def get_items(self, *, locator: Locator | None = None) -> list[TabItem]:
        """Return every tab below this container."""
        return self.get_all(TabItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[TabItem]:
        """Iterate over every tab below this container."""
        return self.iter_all(TabItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> TabItem:
        """Resolve a single tab, raising if zero or multiple match."""
        return self.get(TabItem, locator=locator, scope='children')

    def select(self, *, locator: Locator | None = None) -> TabItem:
        """Resolve a tab and select it."""
        item = self.get_item(locator=locator)
        item.select()
        return item
