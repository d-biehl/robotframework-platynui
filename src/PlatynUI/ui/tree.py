# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false

"""`Tree` container and `TreeItem` for hierarchical selection."""

from typing import cast

from ..core import patterns
from ..core.locator import Locator
from .control import ItemContainer
from .item import Item

__all__ = ['Tree', 'TreeItem']


class TreeItem(Item, ItemContainer['TreeItem']):
    """A tree node that may itself contain child nodes.

    Inherits both ``Item`` action methods (operating on this node)
    and ``ItemContainer`` convenience wrappers (resolving a child).
    The selection actions are unified: when no ``locator`` is given
    the action targets this node; otherwise a child is resolved and
    the action delegates to it. Returns the affected ``TreeItem``
    (this node or the resolved child) for chaining.
    """

    def select(self, *, locator: Locator | None = None) -> 'TreeItem':
        """Select this node, or a resolved child when ``locator`` is given."""
        if locator is None:
            return cast('TreeItem', Item.select(self))  # type: ignore[redundant-cast]
        return ItemContainer.select(self, locator=locator)

    def deselect(self, *, locator: Locator | None = None) -> 'TreeItem':
        """Deselect this node, or a resolved child when ``locator`` is given."""
        if locator is None:
            return cast('TreeItem', Item.deselect(self))  # type: ignore[redundant-cast]
        return ItemContainer.deselect(self, locator=locator)

    def add_to_selection(self, *, locator: Locator | None = None) -> 'TreeItem':
        """Add this node to the selection, or a resolved child when ``locator`` is given."""
        if locator is None:
            return cast('TreeItem', Item.add_to_selection(self))  # type: ignore[redundant-cast]
        return ItemContainer.add_to_selection(self, locator=locator)

    def remove_from_selection(self, *, locator: Locator | None = None) -> 'TreeItem':
        """Remove this node from the selection, or a resolved child when ``locator`` is given."""
        if locator is None:
            return cast('TreeItem', Item.remove_from_selection(self))  # type: ignore[redundant-cast]
        return ItemContainer.remove_from_selection(self, locator=locator)

    @property
    def can_expand(self) -> bool:
        """Whether the node has anything to expand into."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.IsExpandable).can_expand

    @property
    def is_expanded(self) -> bool:
        """Whether the node is currently expanded."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.IsExpandable).is_expanded

    def expand(self) -> bool:
        """Expand the node; return ``True`` if the state changed."""
        if not self.can_expand or self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(patterns.Expandable).expand()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def collapse(self) -> bool:
        """Collapse the node; return ``True`` if the state changed."""
        if not self.can_expand or not self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(patterns.Expandable).collapse()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True


class Tree(ItemContainer[TreeItem]):
    """A container of `TreeItem` nodes."""
