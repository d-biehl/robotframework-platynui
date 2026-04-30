# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.tabs``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    FocusableStub,
    IsSelectableStub,
    ResponsiveStub,
    SelectableStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.tabs import TabItem, TabList


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


class _StubFactory(AdapterFactory):
    def __init__(self, *, results: list[Adapter] | None = None) -> None:
        self.results = results or []
        self.find_all_calls: list[tuple[Adapter, Locator]] = []
        self.find_one_calls: list[tuple[Adapter, Locator]] = []

    def find_one(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> Adapter | None:
        del parent_is_root_like, default_role, default_prefix
        self.find_one_calls.append((parent, locator))
        if not self.results:
            return None
        if len(self.results) > 1:
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
        del parent_is_root_like, default_role, default_prefix
        self.find_all_calls.append((parent, locator))
        return list(self.results)


def _tab_list_adapter(*, extra: dict[type, object] | None = None) -> Adapter:
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
        role='TabList',
        parent=window,
        pattern_map=pmap,
    )


def _tab_item_adapter(*, runtime_id: str = 't1', selected: bool = False) -> Adapter:
    return make_adapter(  # type: ignore[no-any-return]
        role='TabItem',
        runtime_id=runtime_id,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.IsSelectable: IsSelectableStub(is_selected=selected),
            patterns.Selectable: SelectableStub(),
        },
    )


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def test_tab_list_registered_with_role_tablist() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_tab_list_adapter())
    assert cls is TabList


def test_tab_item_registered_with_role_tabitem() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_tab_item_adapter())
    assert cls is TabItem


# ---------------------------------------------------------------------------
# get_items / iter_items / get_item / select — scope='children'
# ---------------------------------------------------------------------------


def test_get_items_uses_children_scope() -> None:
    adapter = _tab_list_adapter()
    tabs = [_tab_item_adapter(runtime_id=f't{i}') for i in range(3)]
    stub = _StubFactory(results=tabs)

    with adapter_factory.override(lambda: stub):
        result = TabList(adapter=adapter).get_items()

    assert len(result) == 3
    assert all(isinstance(r, TabItem) for r in result)
    _, locator = stub.find_all_calls[0]
    assert locator.scope == 'children'


def test_iter_items_yields_tab_items() -> None:
    adapter = _tab_list_adapter()
    tabs = [_tab_item_adapter(runtime_id=f't{i}') for i in range(2)]
    stub = _StubFactory(results=tabs)

    with adapter_factory.override(lambda: stub):
        result = list(TabList(adapter=adapter).iter_items())

    assert {r.adapter.runtime_id for r in result} == {'t0', 't1'}


def test_get_item_returns_single_match() -> None:
    adapter = _tab_list_adapter()
    only = _tab_item_adapter(runtime_id='only')
    stub = _StubFactory(results=[only])

    with adapter_factory.override(lambda: stub):
        result = TabList(adapter=adapter).get_item()
        assert isinstance(result, TabItem)
        assert result.adapter is only


def test_select_resolves_and_selects() -> None:
    adapter = _tab_list_adapter()
    selectable = SelectableStub()
    item_adapter = make_adapter(
        role='TabItem',
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Selectable: selectable,
        },
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        result = TabList(adapter=adapter).select()

    assert isinstance(result, TabItem)
    assert result.adapter is item_adapter
    assert selectable.select_calls == 1


def test_get_items_passes_locator_filter() -> None:
    adapter = _tab_list_adapter()
    stub = _StubFactory(results=[])

    with adapter_factory.override(lambda: stub):
        TabList(adapter=adapter).get_items(locator=Locator(name='Settings'))

    _, locator = stub.find_all_calls[0]
    assert locator.name == 'Settings'
    assert locator.scope == 'children'
