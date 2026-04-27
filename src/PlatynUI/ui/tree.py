# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Tree` container and `TreeItem` for hierarchical selection."""

from collections.abc import Iterator

from ..core import patterns
from ..core.locator import Locator
from .control import Control
from .item import ExpandableItem, SelectableItem

__all__ = ['Tree', 'TreeItem']


class TreeItem(SelectableItem, ExpandableItem):
    """A selectable, expandable tree node that may contain child nodes."""

    @property
    def item_count(self) -> int:
        """The total number of direct child items."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).item_count

    def get_items(self, *, locator: Locator | None = None) -> list['TreeItem']:
        """Return every direct child item."""
        return self.get_all(TreeItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator['TreeItem']:
        """Iterate over every direct child item."""
        return self.iter_all(TreeItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> 'TreeItem':
        """Resolve a single direct child item."""
        return self.get(TreeItem, locator=locator, scope='children')


class Tree(Control):
    """A container of `TreeItem` nodes."""

    @property
    def item_count(self) -> int:
        """The total number of top-level items."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).item_count

    @property
    def column_count(self) -> int:
        """The number of columns (multi-column tree views)."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).column_count

    def get_items(self, *, locator: Locator | None = None) -> list[TreeItem]:
        """Return every top-level tree item."""
        return self.get_all(TreeItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[TreeItem]:
        """Iterate over every top-level tree item."""
        return self.iter_all(TreeItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> TreeItem:
        """Resolve a single top-level tree item."""
        return self.get(TreeItem, locator=locator, scope='children')
