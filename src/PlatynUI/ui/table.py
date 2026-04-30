# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false

"""`Table`, `Row`, and `Cell` containers for tabular data."""

from collections.abc import Iterator
from typing import cast

from ..core.locator import Locator
from .control import Control, ItemContainer
from .item import Item

__all__ = ['Cell', 'Row', 'Table']


class Cell(Item):
    """A table cell."""


class Row(Item, ItemContainer[Cell]):
    """A table row containing `Cell` entries.

    Selection actions accept an optional ``locator``: without it the
    row itself is the target and ``self`` is returned; with it a
    child cell is resolved, the action delegates to that cell, and
    the cell is returned.
    """

    # Dual-role overrides intentionally widen the return type to
    # ``Row | Cell`` — neither parent's signature alone fits both
    # paths. The type-ignores acknowledge this LSP relaxation.

    def select(  # type: ignore[override]
        self,
        *,
        locator: Locator | None = None,
    ) -> 'Row | Cell':
        """Select this row, or a resolved cell when ``locator`` is given."""
        if locator is None:
            return cast('Row', Item.select(self))  # type: ignore[redundant-cast]
        return ItemContainer.select(self, locator=locator)

    def deselect(  # type: ignore[override]
        self,
        *,
        locator: Locator | None = None,
    ) -> 'Row | Cell':
        """Deselect this row, or a resolved cell when ``locator`` is given."""
        if locator is None:
            return cast('Row', Item.deselect(self))  # type: ignore[redundant-cast]
        return ItemContainer.deselect(self, locator=locator)

    def add_to_selection(  # type: ignore[override]
        self,
        *,
        locator: Locator | None = None,
    ) -> 'Row | Cell':
        """Add this row to the selection, or a resolved cell when ``locator`` is given."""
        if locator is None:
            return cast('Row', Item.add_to_selection(self))  # type: ignore[redundant-cast]
        return ItemContainer.add_to_selection(self, locator=locator)

    def remove_from_selection(  # type: ignore[override]
        self,
        *,
        locator: Locator | None = None,
    ) -> 'Row | Cell':
        """Remove this row from the selection, or a resolved cell when ``locator`` is given."""
        if locator is None:
            return cast('Row', Item.remove_from_selection(self))  # type: ignore[redundant-cast]
        return ItemContainer.remove_from_selection(self, locator=locator)


class Table(Control):
    """A container of `Row` entries.

    `Table` is intentionally not an `ItemContainer[Row]` — row geometry
    (`row_count`, `column_count`, header sequences) lives in the `Table`
    pattern, so `get_rows`/`get_row` stay on the context directly.
    """

    def get_rows(self, *, locator: Locator | None = None) -> list[Row]:
        """Return every row in the table."""
        return self.get_all(Row, locator=locator, scope='children')

    def iter_rows(self, *, locator: Locator | None = None) -> Iterator[Row]:
        """Iterate over every row in the table."""
        return self.iter_all(Row, locator=locator, scope='children')

    def get_row(self, *, locator: Locator | None = None) -> Row:
        """Resolve a single row in the table."""
        return self.get(Row, locator=locator, scope='children')
