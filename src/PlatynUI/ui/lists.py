# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`List` container and `ListItem` for selectable list entries."""

from .control import ItemContainer
from .item import Item

__all__ = ['List', 'ListItem']


class ListItem(Item):
    """A selectable entry inside a `List`."""


class List(ItemContainer[ListItem]):
    """A container of `ListItem` entries.

    `select` / `deselect` / `add_to_selection` /
    `remove_from_selection` are inherited from `ItemContainer`.
    """
