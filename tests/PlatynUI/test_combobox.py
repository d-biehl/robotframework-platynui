# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.combobox``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    ExpandableStub,
    FocusableStub,
    HasUserInputStub,
    ReadableStub,
    SelectableStub,
    TextContentStub,
    TextEditableStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.locator import Locator
from PlatynUI.core.settings import Settings
from PlatynUI.ui.combobox import ComboBox
from PlatynUI.ui.lists import ListItem


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


def _combo_adapter(*, extra: dict[type, object] | None = None) -> Adapter:
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
    pmap: dict[type, object] = {
        patterns.Element: ElementStub(),
        patterns.Focusable: FocusableStub(is_focused=True),
    }
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role='ComboBox', parent=window, pattern_map=pmap,
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
# Registration + expand/collapse + can_expand/is_expanded
# ---------------------------------------------------------------------------


def test_combobox_registered_with_role_combobox() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_combo_adapter())
    assert cls is ComboBox


def test_combobox_can_expand_returns_pattern_value() -> None:
    adapter = _combo_adapter(extra={patterns.Expandable: ExpandableStub(can_expand=True)})
    assert ComboBox(adapter=adapter).can_expand is True


def test_combobox_is_expanded_returns_pattern_value() -> None:
    adapter = _combo_adapter(extra={patterns.Expandable: ExpandableStub(is_expanded=True)})
    assert ComboBox(adapter=adapter).is_expanded is True


def test_combobox_expand_calls_pattern() -> None:
    exp = ExpandableStub(is_expanded=False)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    assert ComboBox(adapter=adapter).expand() is True
    assert exp.expand_calls == 1


def test_combobox_expand_no_op_when_already_expanded() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    assert ComboBox(adapter=adapter).expand() is False
    assert exp.expand_calls == 0


def test_combobox_collapse_calls_pattern() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    assert ComboBox(adapter=adapter).collapse() is True
    assert exp.collapse_calls == 1


def test_combobox_expand_raises_when_pattern_missing() -> None:
    with pytest.raises(PatternNotSupportedError):
        _ = ComboBox(adapter=_combo_adapter()).is_expanded


# ---------------------------------------------------------------------------
# Text / set_text
# ---------------------------------------------------------------------------


def test_combobox_text_returns_text_content_value() -> None:
    adapter = _combo_adapter(extra={patterns.TextContent: TextContentStub('foo')})
    assert ComboBox(adapter=adapter).text == 'foo'


def test_combobox_set_text_invokes_text_editable_pattern() -> None:
    text = TextEditableStub()
    adapter = _combo_adapter(
        extra={
            patterns.TextEditable: text,
            patterns.Readable: ReadableStub(is_readonly=False),
        },
    )
    ComboBox(adapter=adapter).set_text('hello')
    assert text.set_text_calls == ['hello']


def test_combobox_set_text_blocks_when_readonly() -> None:
    from PlatynUI.core.exceptions import CannotEnsureError

    text = TextEditableStub()
    adapter = _combo_adapter(
        extra={
            patterns.TextEditable: text,
            patterns.Readable: ReadableStub(is_readonly=True),
        },
    )
    with pytest.raises(CannotEnsureError):
        ComboBox(adapter=adapter).set_text('x')
    assert text.set_text_calls == []


# ---------------------------------------------------------------------------
# Item retrieval — auto-expand context, scope='descendants'
# ---------------------------------------------------------------------------


def test_combobox_get_items_auto_expands_and_collapses() -> None:
    """Auto-expand fires expand() before lookup and collapse() after."""
    exp = ExpandableStub(is_expanded=False)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    items = [_list_item_adapter(runtime_id=f'li{i}') for i in range(2)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        result = ComboBox(adapter=adapter).get_items()

    assert len(result) == 2
    assert exp.expand_calls == 1
    assert exp.collapse_calls == 1
    _, locator = stub.find_all_calls[0]
    assert locator.scope == 'descendants'


def test_combobox_get_items_does_not_collapse_if_already_expanded() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    stub = _StubFactory(results=[])

    with adapter_factory.override(lambda: stub):
        ComboBox(adapter=adapter).get_items()

    assert exp.expand_calls == 0
    assert exp.collapse_calls == 0


def test_combobox_iter_items_holds_dropdown_open_during_iteration() -> None:
    """Generator must keep the dropdown expanded until consumer is done."""
    exp = ExpandableStub(is_expanded=False)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    items = [_list_item_adapter(runtime_id=f'li{i}') for i in range(3)]
    stub = _StubFactory(results=items)

    with adapter_factory.override(lambda: stub):
        gen = ComboBox(adapter=adapter).iter_items()
        first = next(gen)
        # While iteration is in flight the dropdown must still be open.
        assert exp.is_expanded is True
        assert exp.collapse_calls == 0
        rest = list(gen)
        assert isinstance(first, ListItem)
        assert len(rest) == 2

    # Generator is fully consumed → collapse fires.
    assert exp.collapse_calls == 1


def test_combobox_select_resolves_and_selects() -> None:
    exp = ExpandableStub(is_expanded=False)
    adapter = _combo_adapter(extra={patterns.Expandable: exp})
    selectable = SelectableStub(is_selected=False)
    item_adapter = make_adapter(
        role='ListItem',
        pattern_map={patterns.Element: ElementStub(), patterns.Selectable: selectable},
    )
    stub = _StubFactory(results=[item_adapter])

    with adapter_factory.override(lambda: stub):
        item = ComboBox(adapter=adapter).select()

    assert isinstance(item, ListItem)
    assert selectable.select_calls == 1
    assert exp.expand_calls == 1
    assert exp.collapse_calls == 1
