# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.lists``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    FocusableStub,
    HasUserInputStub,
    ItemContainerStub,
    SelectableStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.lists import List, ListItem


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


def _list_adapter(
    *,
    extra: dict[type, object] | None = None,
) -> Adapter:
    desktop = make_adapter(role='Desktop')
    window = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.HasUserInput: HasUserInputStub(True),
            patterns.Focusable: FocusableStub(is_focused=True),
        },
    )
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role='List', parent=window, pattern_map=pmap,
    )


def _list_item_adapter(*, runtime_id: str = 'li1', selected: bool = False) -> Adapter:
    return make_adapter(  # type: ignore[no-any-return]
        role='ListItem',
        runtime_id=runtime_id,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Selectable: SelectableStub(is_selected=selected),
        },
    )


# ---------------------------------------------------------------------------
# List registration + item_count
# ---------------------------------------------------------------------------


def test_list_registered_with_role_list() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_list_adapter())
    assert cls is List


def test_list_item_registered_with_role_listitem() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_list_item_adapter())
    assert cls is ListItem


def test_list_item_count_returns_pattern_value() -> None:
    adapter = _list_adapter(extra={patterns.ItemContainer: ItemContainerStub(item_count=7)})
    assert List(adapter=adapter).item_count == 7


def test_list_item_count_raises_when_pattern_missing() -> None:
    with pytest.raises(PatternNotSupportedError):
        _ = List(adapter=_list_adapter()).item_count


# ---------------------------------------------------------------------------
# get_items / iter_items / get_item / select — scope='children'
# ---------------------------------------------------------------------------


def test_get_items_uses_children_scope() -> None:
    adapter = _list_adapter()
    items = [_list_item_adapter(runtime_id=f'li{i}') for i in range(3)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        result = List(adapter=adapter).get_items()

    assert len(result) == 3
    assert all(isinstance(r, ListItem) for r in result)
    _, locator = stub.find_all_calls[0]
    assert locator.scope == 'children'


def test_iter_items_yields_list_items() -> None:
    adapter = _list_adapter()
    items = [_list_item_adapter(runtime_id=f'li{i}') for i in range(2)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        result = list(List(adapter=adapter).iter_items())

    assert {r.adapter.runtime_id for r in result} == {'li0', 'li1'}


def test_get_item_returns_single_match() -> None:
    adapter = _list_adapter()
    only = _list_item_adapter(runtime_id='only')
    stub = _StubFactory(results=[only])

    with adapter_factory.override(lambda: stub):
        result = List(adapter=adapter).get_item()
        assert isinstance(result, ListItem)
        assert result.adapter is only


def test_select_resolves_and_selects() -> None:
    adapter = _list_adapter()
    selectable = SelectableStub(is_selected=False)
    item_adapter = make_adapter(
        role='ListItem',
        pattern_map={patterns.Element: ElementStub(), patterns.Selectable: selectable},
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        item = List(adapter=adapter).select()

    assert isinstance(item, ListItem)
    assert selectable.select_calls == 1


def test_get_items_passes_locator_filter() -> None:
    adapter = _list_adapter()
    stub = _StubFactory(results=[])

    with adapter_factory.override(lambda: stub):
        List(adapter=adapter).get_items(locator=Locator(name='target'))

    _, locator = stub.find_all_calls[0]
    assert locator.name == 'target'
    assert locator.scope == 'children'
