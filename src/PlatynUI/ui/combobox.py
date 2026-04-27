# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`ComboBox`: expandable dropdown with selectable items and optional editable text."""

from collections.abc import Generator, Iterator
from contextlib import contextmanager

from ..core import patterns
from ..core.locator import Locator
from .control import Control
from .lists import ListItem

__all__ = ['ComboBox']


class ComboBox(Control):
    """A dropdown that can be expanded to select one of its items."""

    @property
    def can_expand(self) -> bool:
        """Whether the dropdown can be expanded."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Expandable).can_expand

    @property
    def is_expanded(self) -> bool:
        """Whether the dropdown is currently open."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Expandable).is_expanded

    def expand(self) -> bool:
        """Open the dropdown; return ``True`` if the state changed."""
        if self.is_expanded:
            return False
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._control_has_focus,
        )
        self.adapter.get_pattern(patterns.Expandable).expand()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def collapse(self) -> bool:
        """Close the dropdown; return ``True`` if the state changed."""
        if not self.is_expanded:
            return False
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._control_has_focus,
        )
        self.adapter.get_pattern(patterns.Expandable).collapse()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def get_items(self, *, locator: Locator | None = None) -> list[ListItem]:
        """Return every item in the dropdown (auto-expanding if needed)."""
        with self._expanded():
            return self.get_all(ListItem, locator=locator, scope='descendants')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[ListItem]:
        """Iterate over every item in the dropdown (auto-expanding for the duration)."""
        with self._expanded():
            yield from self.iter_all(ListItem, locator=locator, scope='descendants')

    def get_item(self, *, locator: Locator | None = None) -> ListItem:
        """Resolve a single dropdown item (auto-expanding if needed)."""
        with self._expanded():
            return self.get(ListItem, locator=locator, scope='descendants')

    def select(self, *, locator: Locator | None = None) -> ListItem:
        """Resolve a dropdown item and select it."""
        with self._expanded():
            item = self.get(ListItem, locator=locator, scope='descendants')
            item.select()
            return item

    @property
    def text(self) -> str:
        """The displayed text."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).text

    @text.setter
    def text(self, value: str) -> None:
        self.set_text(value)

    def set_text(self, value: str) -> None:
        """Replace the displayed text (editable combo boxes only)."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(patterns.TextEditable).set_text(value)
        self.ensure_that(self._application_is_ready, raise_exception=False)

    @contextmanager
    def _expanded(self) -> Generator[None, None, None]:
        was_collapsed = self.expand()
        try:
            yield
        finally:
            if was_collapsed:
                self.collapse()
