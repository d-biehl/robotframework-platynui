# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Context base classes for container items.

`Item` is the marker for elements inside a container; the
`SelectableItem`, `ExpandableItem`, and `EditableItem` mixins add
exactly one capability each.
"""

from ..core import patterns
from .element import Element

__all__ = [
    'EditableItem',
    'ExpandableItem',
    'Item',
    'SelectableItem',
]


class Item(Element, register=False):
    """Marker base for elements inside a container."""

    default_prefix = 'item'

    @property
    def text(self) -> str:
        """The item's display text."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).text


class SelectableItem(Item, register=False):
    """An item that can be selected within its container."""

    @property
    def is_selected(self) -> bool:
        """Whether the item is currently selected."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Selectable).is_selected

    def select(self) -> None:
        """Select the item if it is not already selected."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        selectable = self.adapter.get_pattern(patterns.Selectable)
        if not selectable.is_selected:
            selectable.select()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class ExpandableItem(Item, register=False):
    """An item that can be expanded and collapsed."""

    @property
    def can_expand(self) -> bool:
        """Whether the item has anything to expand into."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Expandable).can_expand

    @property
    def is_expanded(self) -> bool:
        """Whether the item is currently expanded."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Expandable).is_expanded

    def expand(self) -> bool:
        """Expand the item; return ``True`` if the state changed."""
        if not self.can_expand or self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(patterns.Expandable).expand()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def collapse(self) -> bool:
        """Collapse the item; return ``True`` if the state changed."""
        if not self.can_expand or not self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(patterns.Expandable).collapse()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True


class EditableItem(Item, register=False):
    """An item whose value can be edited inline through a `HasEditor` lifecycle."""

    def set_text(self, value: str) -> None:
        """Replace the item's content with ``value``."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        editor = self.adapter.get_pattern(patterns.HasEditor)
        editor.open_editor()
        try:
            self.adapter.get_pattern(patterns.TextEditable).set_text(value)
        finally:
            editor.accept()
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def clear(self) -> None:
        """Remove the item's content."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        editor = self.adapter.get_pattern(patterns.HasEditor)
        editor.open_editor()
        try:
            self.adapter.get_pattern(patterns.Clearable).clear()
        finally:
            editor.accept()
        self.ensure_that(self._application_is_ready, raise_exception=False)
