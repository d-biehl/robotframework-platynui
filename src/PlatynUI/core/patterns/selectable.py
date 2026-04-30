# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Selectable patterns (Read/Action split, Rev. 46/47).

The selection capability is split into orthogonal Read- and Action-
patterns. Read-patterns expose state from the native adapter; action-
patterns are synthesised by default proxies via mouse and keyboard.
See `docs/python-library-design.md` §5a.4 and §A.14.17.
"""

from abc import abstractmethod
from collections.abc import Sequence
from typing import TYPE_CHECKING

from .base import PatternBase

if TYPE_CHECKING:
    from ..adapter import Adapter

__all__ = [
    'Deselectable',
    'IsMultiSelectable',
    'IsSelectable',
    'MultiSelectable',
    'Selectable',
    'Selection',
]


class IsSelectable(PatternBase):
    """Read-pattern: the selection state of an item."""

    pattern_name = 'org.platynui.patterns.IsSelectable'

    @property
    @abstractmethod
    def is_selected(self) -> bool:
        """Whether the element is currently selected."""


class Selectable(PatternBase):
    """Action-pattern: single-select an item.

    Replaces any current selection in the container. The deselect
    counterpart is `Deselectable.deselect()` (single-select clear) or
    `MultiSelectable.remove_from_selection()` (additive removal).
    """

    pattern_name = 'org.platynui.patterns.Selectable'

    @abstractmethod
    def select(self) -> None:
        """Select the element, replacing any current selection."""


class Deselectable(PatternBase):
    """Action-pattern: clear a single-select item's selection.

    Inverse of `Selectable.select()` for containers that allow "no
    selection" as a valid state. Optional — many toolkits do not
    expose single-select deselect, so the default item proxy does
    not register this pattern. `Item.deselect()` raises
    `PatternNotSupportedError` when not present.
    """

    pattern_name = 'org.platynui.patterns.Deselectable'

    @abstractmethod
    def deselect(self) -> None:
        """Deselect the element (clear the container's single selection)."""


class IsMultiSelectable(PatternBase):
    """Read-pattern: multi-selection capability of an item's container."""

    pattern_name = 'org.platynui.patterns.IsMultiSelectable'

    @property
    @abstractmethod
    def can_select_multiple(self) -> bool:
        """Whether the container allows selecting multiple items."""

    @property
    @abstractmethod
    def is_selection_required(self) -> bool:
        """Whether at least one item must remain selected."""


class MultiSelectable(PatternBase):
    """Action-pattern: additive selection of an item.

    Both methods are synthesised in the default proxy via Ctrl+Click
    (modifier hold + `mouse.click`). Native action APIs (UIA
    `AddToSelection`, AT-SPI `Selection.SelectChild`, …) are not used.
    """

    pattern_name = 'org.platynui.patterns.MultiSelectable'

    @abstractmethod
    def add_to_selection(self) -> None:
        """Add the element to the current selection."""

    @abstractmethod
    def remove_from_selection(self) -> None:
        """Remove the element from the current selection."""


class Selection(PatternBase):
    """Read-pattern at the container exposing the current selection."""

    pattern_name = 'org.platynui.patterns.Selection'

    @property
    @abstractmethod
    def can_select_multiple(self) -> bool:
        """Whether the container allows multi-selection."""

    @property
    @abstractmethod
    def is_selection_required(self) -> bool:
        """Whether at least one item must remain selected."""

    @abstractmethod
    def get_selected_adapters(self) -> Sequence['Adapter']:
        """Return the adapters of the currently selected items."""
