# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Tests for the container-side ``Selection`` API on ``ItemContainer`` (Rev. 46/47)."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    DeselectableStub,
    ElementStub,
    FocusableStub,
    IsSelectableStub,
    MultiSelectableStub,
    ResponsiveStub,
    SelectableStub,
    SelectionStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.lists import List, ListItem
from PlatynUI.ui.tabs import TabList


class _StubFactory(AdapterFactory):
    """Routes ``find_one``/``find_all`` to a fixed result list."""

    def __init__(self, *, results: list[Adapter] | None = None) -> None:
        self.results = results or []

    def find_one(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> Adapter | None:
        del parent, locator, parent_is_root_like, default_role, default_prefix
        if not self.results or len(self.results) > 1:
            return None
        return self.results[0]

    def find_all(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> list[Adapter]:
        del parent, locator, parent_is_root_like, default_role, default_prefix
        return list(self.results)


@pytest.fixture(autouse=True)
def _fast_settings() -> Iterator[None]:
    with Settings(
        ensure_timeout=0.05,
        ensure_delay=0.0,
        exists_timeout=0.05,
        wait_for_timeout=0.05,
        wait_for_delay=0.0,
    ):
        yield


def _list_adapter(*, extra: dict[type, object] | None = None) -> Adapter:
    desktop = make_adapter(role='Desktop')
    window = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(True),
            patterns.WindowState: WindowStateStub(is_active=True),
            patterns.Focusable: FocusableStub(is_focused=True),
        },
    )
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role='List',
        parent=window,
        pattern_map=pmap,
    )


def _list_item_adapter(
    *,
    runtime_id: str = 'li',
    is_selected: bool = True,
    multi: MultiSelectableStub | None = None,
) -> Adapter:
    pmap: dict[type, object] = {
        patterns.Element: ElementStub(),
        patterns.IsSelectable: IsSelectableStub(is_selected=is_selected),
    }
    if multi is not None:
        pmap[patterns.MultiSelectable] = multi
    return make_adapter(  # type: ignore[no-any-return]
        role='ListItem',
        runtime_id=runtime_id,
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# can_select_multiple / is_selection_required
# ---------------------------------------------------------------------------


def test_can_select_multiple_returns_pattern_value() -> None:
    sel = SelectionStub(can_select_multiple=True)
    adapter = _list_adapter(extra={patterns.Selection: sel})
    assert List(adapter=adapter).can_select_multiple is True


def test_is_selection_required_returns_pattern_value() -> None:
    sel = SelectionStub(is_selection_required=True)
    adapter = _list_adapter(extra={patterns.Selection: sel})
    assert List(adapter=adapter).is_selection_required is True


def test_can_select_multiple_raises_when_pattern_missing() -> None:
    adapter = _list_adapter()
    with pytest.raises(PatternNotSupportedError):
        _ = List(adapter=adapter).can_select_multiple


# ---------------------------------------------------------------------------
# get_selected_items
# ---------------------------------------------------------------------------


def test_get_selected_items_returns_empty_when_no_selection() -> None:
    sel = SelectionStub([])
    adapter = _list_adapter(extra={patterns.Selection: sel})
    assert List(adapter=adapter).get_selected_items() == []


def test_get_selected_items_wraps_adapters_via_context_factory() -> None:
    item1 = _list_item_adapter(runtime_id='li1')
    item2 = _list_item_adapter(runtime_id='li2')
    sel = SelectionStub([item1, item2])
    adapter = _list_adapter(extra={patterns.Selection: sel})

    result = List(adapter=adapter).get_selected_items()

    assert len(result) == 2
    assert all(isinstance(item, ListItem) for item in result)
    assert {item.adapter.runtime_id for item in result} == {'li1', 'li2'}


# ---------------------------------------------------------------------------
# clear_selection
# ---------------------------------------------------------------------------


def test_clear_selection_calls_remove_on_each_selected_item() -> None:
    multi1 = MultiSelectableStub()
    multi2 = MultiSelectableStub()
    item1 = _list_item_adapter(runtime_id='li1', multi=multi1)
    item2 = _list_item_adapter(runtime_id='li2', multi=multi2)
    sel = SelectionStub([item1, item2])
    adapter = _list_adapter(extra={patterns.Selection: sel})

    List(adapter=adapter).clear_selection()

    assert multi1.remove_calls == 1
    assert multi2.remove_calls == 1


def test_clear_selection_is_noop_when_nothing_selected() -> None:
    sel = SelectionStub([])
    adapter = _list_adapter(extra={patterns.Selection: sel})
    List(adapter=adapter).clear_selection()  # must not raise


def test_clear_selection_raises_when_item_lacks_multi_selectable() -> None:
    item = _list_item_adapter(runtime_id='li1')  # no MultiSelectable
    sel = SelectionStub([item])
    adapter = _list_adapter(extra={patterns.Selection: sel})

    with pytest.raises(PatternNotSupportedError):
        List(adapter=adapter).clear_selection()


# ---------------------------------------------------------------------------
# Convenience wrappers: select / deselect / add_to_selection /
# remove_from_selection (Rev. 47)
# ---------------------------------------------------------------------------


def _single_item(item_extra: dict[type, object]) -> Adapter:
    """Build a single ListItem adapter exposing the given patterns."""
    return make_adapter(  # type: ignore[no-any-return]
        role='ListItem',
        runtime_id='only',
        pattern_map={patterns.Element: ElementStub(), **item_extra},
    )


def test_container_select_resolves_and_selects() -> None:
    selectable = SelectableStub()
    list_adapter = _list_adapter()
    item_adapter = _single_item(
        {
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Selectable: selectable,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = List(adapter=list_adapter).select()

    assert isinstance(result, ListItem)
    assert result.adapter is item_adapter
    assert selectable.select_calls == 1


def test_container_deselect_calls_deselectable() -> None:
    deselectable = DeselectableStub()
    list_adapter = _list_adapter()
    item_adapter = _single_item(
        {
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.Deselectable: deselectable,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = List(adapter=list_adapter).deselect()

    assert isinstance(result, ListItem)
    assert result.adapter is item_adapter
    assert deselectable.deselect_calls == 1


def test_container_deselect_raises_when_deselectable_missing() -> None:
    list_adapter = _list_adapter()
    item_adapter = _single_item(
        {patterns.IsSelectable: IsSelectableStub(is_selected=True)},
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub), pytest.raises(PatternNotSupportedError):
        List(adapter=list_adapter).deselect()


def test_container_add_to_selection_calls_multi_selectable() -> None:
    multi = MultiSelectableStub()
    list_adapter = _list_adapter()
    item_adapter = _single_item(
        {
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.MultiSelectable: multi,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = List(adapter=list_adapter).add_to_selection()

    assert isinstance(result, ListItem)
    assert result.adapter is item_adapter
    assert multi.add_calls == 1


def test_container_remove_from_selection_calls_multi_selectable() -> None:
    multi = MultiSelectableStub()
    list_adapter = _list_adapter()
    item_adapter = _single_item(
        {
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.MultiSelectable: multi,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = List(adapter=list_adapter).remove_from_selection()

    assert isinstance(result, ListItem)
    assert result.adapter is item_adapter
    assert multi.remove_calls == 1


def test_tablist_inherits_full_selection_api() -> None:
    """Sanity: TabList exposes select/deselect/add/remove via inheritance."""
    selectable = SelectableStub()
    desktop = make_adapter(role='Desktop')
    window = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(True),
            patterns.WindowState: WindowStateStub(is_active=True),
            patterns.Focusable: FocusableStub(is_focused=True),
        },
    )
    tablist_adapter = make_adapter(
        role='TabList',
        parent=window,
        pattern_map={patterns.Element: ElementStub()},
    )
    item_adapter = make_adapter(
        role='TabItem',
        runtime_id='tab1',
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Selectable: selectable,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = TabList(adapter=tablist_adapter).select()

    assert result.adapter is item_adapter
    assert selectable.select_calls == 1
