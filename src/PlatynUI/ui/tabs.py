# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`TabList` container and `TabItem` for selectable tabs."""

from .control import ItemContainer
from .item import Item

__all__ = ['TabItem', 'TabList']


class TabItem(Item):
    """A selectable tab inside a `TabList`."""


class TabList(ItemContainer[TabItem]):
    """A container of `TabItem` entries.

    `select` / `deselect` / `add_to_selection` /
    `remove_from_selection` are inherited from `ItemContainer`.
    """
