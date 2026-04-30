# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""Tests for the generic `ItemContainer[I: Item]` context base class."""

import pytest

from PlatynUI.ui.control import ItemContainer, _resolve_item_type
from PlatynUI.ui.item import Item
from PlatynUI.ui.lists import List, ListItem
from PlatynUI.ui.tabs import TabItem, TabList
from PlatynUI.ui.tree import Tree, TreeItem

# ---------------------------------------------------------------------------
# _resolve_item_type — concrete classes from the public API
# ---------------------------------------------------------------------------


def test_resolve_item_type_for_list_returns_list_item() -> None:
    assert _resolve_item_type(List) is ListItem


def test_resolve_item_type_for_tabs_returns_tab_item() -> None:
    assert _resolve_item_type(TabList) is TabItem


def test_resolve_item_type_for_tree_returns_tree_item() -> None:
    assert _resolve_item_type(Tree) is TreeItem


def test_resolve_item_type_handles_self_reference_via_forward_ref() -> None:
    # `TreeItem` parameterises ``ItemContainer['TreeItem']`` — exercise
    # the ForwardRef branch.
    assert _resolve_item_type(TreeItem) is TreeItem


def test_resolve_item_type_caches_on_class() -> None:
    # First call populates the cache; subsequent lookups must read it.
    _ = _resolve_item_type(List)
    assert List.__dict__.get('_item_container_item_type') is ListItem
    # Second call hits the cache (no exceptions and same object).
    assert _resolve_item_type(List) is ListItem


# ---------------------------------------------------------------------------
# _resolve_item_type — error paths
# ---------------------------------------------------------------------------


def test_resolve_item_type_rejects_non_item_argument() -> None:
    class _NotAnItem:
        pass

    class _BadContainer(ItemContainer[_NotAnItem]):  # type: ignore[type-var]
        pass

    with pytest.raises(TypeError, match='must be a subclass of Item'):
        _resolve_item_type(_BadContainer)


def test_resolve_item_type_raises_when_not_parameterised() -> None:
    with pytest.raises(TypeError, match='does not parameterise ItemContainer'):
        _resolve_item_type(Item)
