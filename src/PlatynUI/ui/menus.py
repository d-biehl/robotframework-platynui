# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Menu`, `MenuBar`, and `MenuItem` for application menu hierarchies."""

from ..core import patterns
from .control import Control

__all__ = ['Menu', 'MenuBar', 'MenuItem']


class Menu(Control):
    """A popup or submenu container of `MenuItem` entries."""


class MenuBar(Control):
    """A top-level menu strip, typically anchored to a `Window`."""


class MenuItem(Control):
    """A single menu entry that may host a submenu."""

    def activate(self) -> None:
        """Open every ancestor menu and trigger this entry."""
        # Walk upward to collect MenuItem ancestors, stopping at the first
        # Window or DesktopBase boundary.
        from .desktopbase import DesktopBase
        from .window import Window

        ancestors: list[MenuItem] = []
        node = self.parent
        while node is not None and not isinstance(node, (Window, DesktopBase)):
            if isinstance(node, MenuItem):
                ancestors.append(node)
            node = node.parent

        # Open from outermost to innermost so that the path is visible.
        for ancestor in reversed(ancestors):
            expandable = ancestor.adapter.get_pattern(
                patterns.Expandable, raise_exception=False
            )
            if expandable is None or expandable.is_expanded:
                continue
            expandable.expand()

        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)
