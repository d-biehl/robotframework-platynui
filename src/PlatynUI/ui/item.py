# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Item` base class for elements inside a container.

`Item` carries the capabilities every container entry can plausibly
have: selection, inline editing, and activation.  Each capability
raises `PatternNotSupportedError` when the underlying pattern is
missing.

Hierarchical capabilities (`expand`/`collapse`) live on `TreeItem`
only, since flat container entries (`ListItem`, `TabItem`, `Cell`,
`Row`) cannot expand.
"""

from typing import Self

from ..core import patterns
from .element import Element

__all__ = ['Item']


class Item(Element, register=False):
    """Marker base for elements inside a container."""

    default_prefix = 'item'

    # ----- TextContent ------------------------------------------------

    @property
    def text(self) -> str:
        """The item's display text."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).text

    # ----- Selection (Read: IsSelectable / Action: Selectable, MultiSelectable) ---

    @property
    def is_selected(self) -> bool:
        """Whether the item is currently selected."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.IsSelectable).is_selected

    def select(self) -> Self:
        """Select the item if it is not already selected.

        Single-select semantics: any current selection in the container
        is replaced. Reads state via ``IsSelectable``; the action runs
        through ``Selectable`` (default proxy: mouse click). Returns
        ``self`` for chaining.
        """
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        if not self.is_selected:
            self.adapter.get_pattern(patterns.Selectable).select()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return self

    def add_to_selection(self) -> Self:
        """Add the item to the current selection (multi-select).

        Idempotent if the item is already selected. Requires the
        container to support multi-selection — otherwise the
        ``MultiSelectable`` pattern is missing and
        ``PatternNotSupportedError`` is raised. Returns ``self`` for
        chaining.
        """
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        if not self.is_selected:
            self.adapter.get_pattern(patterns.MultiSelectable).add_to_selection()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return self

    def remove_from_selection(self) -> Self:
        """Remove the item from a multi-selection.

        Other selected items remain selected. Requires
        ``MultiSelectable``; raises ``PatternNotSupportedError``
        when the container is single-select only. Returns ``self`` for
        chaining.
        """
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        if self.is_selected:
            self.adapter.get_pattern(patterns.MultiSelectable).remove_from_selection()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return self

    def deselect(self) -> Self:
        """Clear a single-select item's selection.

        Inverse of ``select()``. Requires ``Deselectable``, which is
        an optional pattern — many toolkits do not expose
        single-select deselect. Raises ``PatternNotSupportedError``
        when not registered. Returns ``self`` for chaining.
        """
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        if self.is_selected:
            self.adapter.get_pattern(patterns.Deselectable).deselect()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return self

    # ----- HasEditor + TextEditable / Clearable -----------------------

    def set_text(self, value: str) -> None:
        """Replace the item's content with ``value`` via the editor lifecycle."""
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
        """Remove the item's content via the editor lifecycle."""
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

    # ----- Activatable ------------------------------------------------

    def activate(self) -> None:
        """Trigger the item's default action."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)
