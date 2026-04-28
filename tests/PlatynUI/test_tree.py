# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.tree``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    ExpandableStub,
    FocusableStub,
    ItemContainerStub,
    ResponsiveStub,
    SelectableStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.tree import Tree, TreeItem


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


def _tree_adapter(*, extra: dict[type, object] | None = None) -> Adapter:
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
        role='Tree',
        parent=window,
        pattern_map=pmap,
    )


def _tree_item_adapter(*, runtime_id: str = 'ti1', extra: dict[type, object] | None = None) -> Adapter:
    pmap: dict[type, object] = {
        patterns.Element: ElementStub(),
        patterns.Selectable: SelectableStub(),
        patterns.Expandable: ExpandableStub(),
    }
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role='TreeItem',
        runtime_id=runtime_id,
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# Tree / TreeItem registration
# ---------------------------------------------------------------------------


def test_tree_registered_with_role_tree() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_tree_adapter())
    assert cls is Tree


def test_tree_item_registered_with_role_treeitem() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_tree_item_adapter())
    assert cls is TreeItem


# ---------------------------------------------------------------------------
# Counts
# ---------------------------------------------------------------------------


def test_tree_item_count_returns_pattern_value() -> None:
    adapter = _tree_adapter(extra={patterns.ItemContainer: ItemContainerStub(item_count=4)})
    assert Tree(adapter=adapter).item_count == 4


def test_tree_column_count_returns_pattern_value() -> None:
    adapter = _tree_adapter(extra={patterns.ItemContainer: ItemContainerStub(column_count=3)})
    assert Tree(adapter=adapter).column_count == 3


def test_tree_column_count_unsupported_raises_not_implemented() -> None:
    """ItemContainer.column_count raises NotImplementedError when the role omits it."""
    adapter = _tree_adapter(extra={patterns.ItemContainer: ItemContainerStub(item_count=4)})
    with pytest.raises(NotImplementedError):
        _ = Tree(adapter=adapter).column_count


def test_tree_item_count_raises_when_pattern_missing() -> None:
    with pytest.raises(PatternNotSupportedError):
        _ = Tree(adapter=_tree_adapter()).item_count


# ---------------------------------------------------------------------------
# Container traversal — scope='children'
# ---------------------------------------------------------------------------


def test_tree_get_items_uses_children_scope() -> None:
    adapter = _tree_adapter()
    items = [_tree_item_adapter(runtime_id=f't{i}') for i in range(2)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        result = Tree(adapter=adapter).get_items()
        assert len(result) == 2
        _, locator = stub.find_all_calls[0]
        assert locator.scope == 'children'


def test_tree_iter_items_yields_tree_items() -> None:
    adapter = _tree_adapter()
    items = [_tree_item_adapter(runtime_id=f't{i}') for i in range(2)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        result = list(Tree(adapter=adapter).iter_items())
        assert all(isinstance(r, TreeItem) for r in result)


def test_tree_get_item_returns_single() -> None:
    adapter = _tree_adapter()
    only = _tree_item_adapter(runtime_id='only')
    stub = _StubFactory(results=[only])

    with adapter_factory.override(lambda: stub):
        result = Tree(adapter=adapter).get_item()
        assert isinstance(result, TreeItem)
        assert result.adapter is only


# ---------------------------------------------------------------------------
# TreeItem nested traversal
# ---------------------------------------------------------------------------


def test_tree_item_get_items_uses_children_scope() -> None:
    """Nested ``TreeItem.get_items`` traverses one level down (children)."""
    parent_adapter = _tree_adapter()
    root_item = _tree_item_adapter(runtime_id='root')
    child = _tree_item_adapter(runtime_id='c1')
    stub = _StubFactory(results=[root_item])

    with adapter_factory.override(lambda: stub):
        root = Tree(adapter=parent_adapter).get_item()

        # Re-program stub to return children for the next call.
        stub.results = [child]
        children = root.get_items()
        assert len(children) == 1
        # Two find_all calls: first tree.get_item materialised root via find_one;
        # the children call goes through find_all.
        _, locator = stub.find_all_calls[-1]
        assert locator.scope == 'children'


def test_tree_item_item_count_uses_item_container_pattern() -> None:
    adapter = _tree_item_adapter(
        extra={patterns.ItemContainer: ItemContainerStub(item_count=2)},
    )
    assert TreeItem(adapter=adapter).item_count == 2
