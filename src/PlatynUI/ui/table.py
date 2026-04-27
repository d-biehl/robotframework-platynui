# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Table`, `Row`, and `Cell` containers for tabular data."""

from collections.abc import Iterator

from ..core import patterns
from ..core.locator import Locator
from .control import Control
from .item import EditableItem, Item

__all__ = ['Cell', 'EditableCell', 'Row', 'Table']


class Cell(Item):
    """A read-only table cell."""


class EditableCell(Cell, EditableItem):
    """A table cell whose value can be edited inline."""


class Row(Item):
    """A table row containing `Cell` entries."""

    @property
    def column_count(self) -> int:
        """The number of cells in the row."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).column_count

    def get_cells(self, *, locator: Locator | None = None) -> list[Cell]:
        """Return every cell in the row."""
        return self.get_all(Cell, locator=locator, scope='children')

    def iter_cells(self, *, locator: Locator | None = None) -> Iterator[Cell]:
        """Iterate over every cell in the row."""
        return self.iter_all(Cell, locator=locator, scope='children')

    def get_cell(self, *, locator: Locator | None = None) -> Cell:
        """Resolve a single cell in the row."""
        return self.get(Cell, locator=locator, scope='children')


class Table(Control):
    """A container of `Row` entries."""

    @property
    def row_count(self) -> int:
        """The total number of rows."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).row_count

    @property
    def column_count(self) -> int:
        """The total number of columns."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.ItemContainer).column_count

    def get_rows(self, *, locator: Locator | None = None) -> list[Row]:
        """Return every row in the table."""
        return self.get_all(Row, locator=locator, scope='children')

    def iter_rows(self, *, locator: Locator | None = None) -> Iterator[Row]:
        """Iterate over every row in the table."""
        return self.iter_all(Row, locator=locator, scope='children')

    def get_row(self, *, locator: Locator | None = None) -> Row:
        """Resolve a single row in the table."""
        return self.get(Row, locator=locator, scope='children')
