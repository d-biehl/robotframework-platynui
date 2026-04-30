# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Menu`, `MenuBar`, and `MenuItem` for application menu hierarchies."""

from collections.abc import Iterator

from ..core import patterns
from ..core.locator import Locator
from .control import Control

__all__ = ['Menu', 'MenuBar', 'MenuItem']


class MenuItem(Control):
    """A single menu entry that may host a submenu."""

    def activate(self) -> None:
        """Trigger this menu entry.

        Walking ancestor menus to open submenus is intentionally not
        performed: callers resolve and activate each level explicitly.
        """
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class Menu(Control):
    """A popup or submenu container of `MenuItem` entries.

    `Menu` is intentionally not an `ItemContainer[MenuItem]` — `MenuItem`
    inherits from `Control` (not `Item`), so the item-resolving methods
    live directly on the context, analogous to `Table.get_rows`.
    """

    def get_items(self, *, locator: Locator | None = None) -> list[MenuItem]:
        """Return every menu entry directly contained in this menu."""
        return self.get_all(MenuItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[MenuItem]:
        """Iterate over every menu entry directly contained in this menu."""
        return self.iter_all(MenuItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> MenuItem:
        """Resolve a single menu entry."""
        return self.get(MenuItem, locator=locator, scope='children')


class MenuBar(Menu):
    """A top-level menu strip, typically anchored to a `Window`."""
