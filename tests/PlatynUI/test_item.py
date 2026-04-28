# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.item`` capability mixins."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ClearableStub,
    ElementStub,
    ExpandableStub,
    FocusableStub,
    HasEditorStub,
    ResponsiveStub,
    SelectableStub,
    TextContentStub,
    TextEditableStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import CannotEnsureError, PatternNotSupportedError
from PlatynUI.core.settings import Settings
from PlatynUI.ui.item import EditableItem, ExpandableItem, Item, SelectableItem


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


def _item_adapter(
    *,
    role: str = 'ListItem',
    extra: dict[type, object] | None = None,
) -> Adapter:
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
        role=role,
        parent=window,
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# Item base — register=False, default_prefix, text
# ---------------------------------------------------------------------------


def test_item_base_classes_are_not_auto_registered() -> None:
    """All Item-mixin bases stay out of the context registry."""
    from PlatynUI.core.context import ContextFactory

    factory = ContextFactory()
    for role in ('Item', 'SelectableItem', 'ExpandableItem', 'EditableItem'):
        cls = factory.find_context_class_for(make_adapter(role=role))
        assert cls not in (Item, SelectableItem, ExpandableItem, EditableItem), role


def test_item_default_prefix_is_item() -> None:
    assert Item.default_prefix == 'item'


def test_item_text_returns_text_content_value() -> None:
    class _ConcreteItem(Item):
        pass

    adapter = _item_adapter(role='ConcreteItem', extra={patterns.TextContent: TextContentStub('hello')})
    assert _ConcreteItem(adapter=adapter).text == 'hello'


# ---------------------------------------------------------------------------
# SelectableItem
# ---------------------------------------------------------------------------


class _Selectable(SelectableItem):
    """Concrete SelectableItem for direct testing."""


def test_selectable_is_selected_returns_pattern_value() -> None:
    sel = SelectableStub(is_selected=True)
    adapter = _item_adapter(role='Selectable', extra={patterns.Selectable: sel})
    assert _Selectable(adapter=adapter).is_selected is True


def test_selectable_select_calls_pattern_when_not_selected() -> None:
    sel = SelectableStub(is_selected=False)
    adapter = _item_adapter(role='Selectable', extra={patterns.Selectable: sel})
    _Selectable(adapter=adapter).select()
    assert sel.select_calls == 1


def test_selectable_select_skips_pattern_when_already_selected() -> None:
    sel = SelectableStub(is_selected=True)
    adapter = _item_adapter(role='Selectable', extra={patterns.Selectable: sel})
    _Selectable(adapter=adapter).select()
    assert sel.select_calls == 0


def test_selectable_select_blocks_when_disabled() -> None:
    sel = SelectableStub(is_selected=False)
    adapter = _item_adapter(
        role='Selectable',
        extra={
            patterns.Element: ElementStub(is_enabled=False),
            patterns.Selectable: sel,
        },
    )
    with pytest.raises(CannotEnsureError):
        _Selectable(adapter=adapter).select()
    assert sel.select_calls == 0


def test_selectable_raises_when_pattern_missing() -> None:
    adapter = _item_adapter(role='Selectable')
    with pytest.raises(PatternNotSupportedError):
        _ = _Selectable(adapter=adapter).is_selected


# ---------------------------------------------------------------------------
# ExpandableItem
# ---------------------------------------------------------------------------


class _Expandable(ExpandableItem):
    """Concrete ExpandableItem for direct testing."""


def test_expandable_is_expanded_returns_pattern_value() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).is_expanded is True


def test_expandable_can_expand_returns_pattern_value() -> None:
    exp = ExpandableStub(can_expand=False)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).can_expand is False


def test_expandable_expand_returns_true_and_calls_pattern() -> None:
    exp = ExpandableStub(is_expanded=False)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).expand() is True
    assert exp.expand_calls == 1


def test_expandable_expand_no_op_when_already_expanded() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).expand() is False
    assert exp.expand_calls == 0


def test_expandable_expand_no_op_when_cannot_expand() -> None:
    exp = ExpandableStub(can_expand=False)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).expand() is False
    assert exp.expand_calls == 0


def test_expandable_collapse_returns_true_and_calls_pattern() -> None:
    exp = ExpandableStub(is_expanded=True)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).collapse() is True
    assert exp.collapse_calls == 1


def test_expandable_collapse_no_op_when_already_collapsed() -> None:
    exp = ExpandableStub(is_expanded=False)
    adapter = _item_adapter(role='Expandable', extra={patterns.Expandable: exp})
    assert _Expandable(adapter=adapter).collapse() is False
    assert exp.collapse_calls == 0


# ---------------------------------------------------------------------------
# EditableItem — set_text/clear lifecycle
# ---------------------------------------------------------------------------


class _Editable(EditableItem):
    """Concrete EditableItem for direct testing."""


def test_editable_set_text_runs_open_set_accept_sequence() -> None:
    editor = HasEditorStub()
    text = TextEditableStub()
    adapter = _item_adapter(
        role='Editable',
        extra={patterns.HasEditor: editor, patterns.TextEditable: text},
    )
    _Editable(adapter=adapter).set_text('xyz')
    assert editor.open_calls == 1
    assert text.set_text_calls == ['xyz']
    assert editor.accept_calls == 1
    assert editor.cancel_calls == 0


def test_editable_set_text_accepts_even_when_set_text_raises() -> None:
    """Editor must be closed via accept() in finally even on TextEditable failure."""
    editor = HasEditorStub()

    class BoomTextEditable(TextEditableStub):
        def set_text(self, value: str) -> None:
            del value
            raise RuntimeError('boom')

    text = BoomTextEditable()
    adapter = _item_adapter(
        role='Editable',
        extra={patterns.HasEditor: editor, patterns.TextEditable: text},
    )
    with pytest.raises(RuntimeError, match='boom'):
        _Editable(adapter=adapter).set_text('x')
    assert editor.open_calls == 1
    assert editor.accept_calls == 1


def test_editable_clear_runs_open_clear_accept_sequence() -> None:
    editor = HasEditorStub()
    clearable = ClearableStub()
    adapter = _item_adapter(
        role='Editable',
        extra={patterns.HasEditor: editor, patterns.Clearable: clearable},
    )
    _Editable(adapter=adapter).clear()
    assert editor.open_calls == 1
    assert clearable.clear_calls == 1
    assert editor.accept_calls == 1


def test_editable_set_text_raises_when_editor_pattern_missing() -> None:
    adapter = _item_adapter(
        role='Editable',
        extra={patterns.TextEditable: TextEditableStub()},
    )
    with pytest.raises(PatternNotSupportedError):
        _Editable(adapter=adapter).set_text('x')
