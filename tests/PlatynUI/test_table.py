# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.table``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    FocusableStub,
    ResponsiveStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.table import Cell, Row, Table


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
        return self.results[0] if self.results else None

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


def _table_adapter(*, extra: dict[type, object] | None = None) -> Adapter:
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
        role='Table',
        parent=window,
        pattern_map=pmap,
    )


def _row_adapter(*, runtime_id: str = 'r1') -> Adapter:
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    return make_adapter(  # type: ignore[no-any-return]
        role='Row',
        runtime_id=runtime_id,
        pattern_map=pmap,
    )


def _cell_adapter(*, runtime_id: str = 'c1') -> Adapter:
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    return make_adapter(  # type: ignore[no-any-return]
        role='Cell',
        runtime_id=runtime_id,
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def test_table_registered_with_role_table() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_table_adapter())
    assert cls is Table


def test_row_registered_with_role_row() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_row_adapter())
    assert cls is Row


def test_cell_registered_with_role_cell() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_cell_adapter())
    assert cls is Cell


# ---------------------------------------------------------------------------
# Container traversal
# ---------------------------------------------------------------------------


def test_table_get_rows_uses_children_scope() -> None:
    adapter = _table_adapter()
    rows = [_row_adapter(runtime_id=f'r{i}') for i in range(3)]
    stub = _StubFactory(results=rows)

    with adapter_factory.override(lambda: stub):
        result = Table(adapter=adapter).get_rows()
        assert len(result) == 3
        _, locator = stub.find_all_calls[0]
        assert locator.scope == 'children'


def test_table_iter_rows_yields_rows() -> None:
    adapter = _table_adapter()
    rows = [_row_adapter(runtime_id=f'r{i}') for i in range(2)]
    stub = _StubFactory(results=rows)

    with adapter_factory.override(lambda: stub):
        result = list(Table(adapter=adapter).iter_rows())
        assert all(isinstance(r, Row) for r in result)


def test_table_get_row_returns_single() -> None:
    adapter = _table_adapter()
    only = _row_adapter(runtime_id='only')
    stub = _StubFactory(results=[only])

    with adapter_factory.override(lambda: stub):
        result = Table(adapter=adapter).get_row()
        assert isinstance(result, Row)
        assert result.adapter is only


def test_row_get_items_uses_children_scope() -> None:
    parent_adapter = _table_adapter()
    row = _row_adapter()
    cell = _cell_adapter()
    stub = _StubFactory(results=[row])

    with adapter_factory.override(lambda: stub):
        row_ctx = Table(adapter=parent_adapter).get_row()
        stub.results = [cell]
        cells = row_ctx.get_items()
        assert len(cells) == 1
        assert isinstance(cells[0], Cell)
        _, locator = stub.find_all_calls[-1]
        assert locator.scope == 'children'
