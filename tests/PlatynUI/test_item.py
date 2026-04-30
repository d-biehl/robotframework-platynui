# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.item`` flat-capability ``Item``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ActivatableStub,
    ClearableStub,
    DeselectableStub,
    ElementStub,
    FocusableStub,
    HasEditorStub,
    IsMultiSelectableStub,
    IsSelectableStub,
    MultiSelectableStub,
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
from PlatynUI.ui.item import Item


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


class _ConcreteItem(Item):
    """Concrete subclass to exercise the `Item` capability surface."""


# ---------------------------------------------------------------------------
# Item base — register=False, default_prefix
# ---------------------------------------------------------------------------


def test_item_base_is_not_auto_registered() -> None:
    """The `Item` base stays out of the context registry."""
    from PlatynUI.core.context import ContextFactory

    factory = ContextFactory()
    cls = factory.find_context_class_for(make_adapter(role='Item'))
    assert cls is not Item


def test_item_default_prefix_is_item() -> None:
    assert Item.default_prefix == 'item'


# ---------------------------------------------------------------------------
# TextContent
# ---------------------------------------------------------------------------


def test_item_text_returns_text_content_value() -> None:
    adapter = _item_adapter(extra={patterns.TextContent: TextContentStub('hello')})
    assert _ConcreteItem(adapter=adapter).text == 'hello'


def test_item_text_raises_when_pattern_missing() -> None:
    adapter = _item_adapter()
    with pytest.raises(PatternNotSupportedError):
        _ = _ConcreteItem(adapter=adapter).text


# ---------------------------------------------------------------------------
# Selection (IsSelectable / Selectable / MultiSelectable, Rev. 46)
# ---------------------------------------------------------------------------


def test_is_selected_returns_pattern_value() -> None:
    adapter = _item_adapter(extra={patterns.IsSelectable: IsSelectableStub(is_selected=True)})
    assert _ConcreteItem(adapter=adapter).is_selected is True


def test_select_calls_pattern_when_not_selected() -> None:
    sel = SelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Selectable: sel,
        },
    )
    _ConcreteItem(adapter=adapter).select()
    assert sel.select_calls == 1


def test_select_skips_pattern_when_already_selected() -> None:
    sel = SelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.Selectable: sel,
        },
    )
    _ConcreteItem(adapter=adapter).select()
    assert sel.select_calls == 0


def test_select_blocks_when_disabled() -> None:
    sel = SelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.Element: ElementStub(is_enabled=False),
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Selectable: sel,
        },
    )
    with pytest.raises(CannotEnsureError):
        _ConcreteItem(adapter=adapter).select()
    assert sel.select_calls == 0


def test_is_selected_raises_when_pattern_missing() -> None:
    adapter = _item_adapter()
    with pytest.raises(PatternNotSupportedError):
        _ = _ConcreteItem(adapter=adapter).is_selected


def test_select_raises_when_pattern_missing() -> None:
    adapter = _item_adapter(extra={patterns.IsSelectable: IsSelectableStub(is_selected=False)})
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).select()


def test_add_to_selection_calls_pattern_when_not_selected() -> None:
    multi = MultiSelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.MultiSelectable: multi,
        },
    )
    _ConcreteItem(adapter=adapter).add_to_selection()
    assert multi.add_calls == 1
    assert multi.remove_calls == 0


def test_add_to_selection_is_idempotent_when_already_selected() -> None:
    multi = MultiSelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.MultiSelectable: multi,
        },
    )
    _ConcreteItem(adapter=adapter).add_to_selection()
    assert multi.add_calls == 0


def test_add_to_selection_raises_when_multi_pattern_missing() -> None:
    adapter = _item_adapter(extra={patterns.IsSelectable: IsSelectableStub(is_selected=False)})
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).add_to_selection()


def test_remove_from_selection_calls_pattern_when_selected() -> None:
    multi = MultiSelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.MultiSelectable: multi,
        },
    )
    _ConcreteItem(adapter=adapter).remove_from_selection()
    assert multi.remove_calls == 1
    assert multi.add_calls == 0


def test_remove_from_selection_is_idempotent_when_not_selected() -> None:
    multi = MultiSelectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.MultiSelectable: multi,
        },
    )
    _ConcreteItem(adapter=adapter).remove_from_selection()
    assert multi.remove_calls == 0


def test_remove_from_selection_raises_when_multi_pattern_missing() -> None:
    adapter = _item_adapter(extra={patterns.IsSelectable: IsSelectableStub(is_selected=True)})
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).remove_from_selection()


def test_deselect_calls_deselectable_when_selected() -> None:
    deselectable = DeselectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=True),
            patterns.Deselectable: deselectable,
        },
    )
    _ConcreteItem(adapter=adapter).deselect()
    assert deselectable.deselect_calls == 1


def test_deselect_is_idempotent_when_not_selected() -> None:
    deselectable = DeselectableStub()
    adapter = _item_adapter(
        extra={
            patterns.IsSelectable: IsSelectableStub(is_selected=False),
            patterns.Deselectable: deselectable,
        },
    )
    _ConcreteItem(adapter=adapter).deselect()
    assert deselectable.deselect_calls == 0


def test_deselect_raises_when_deselectable_pattern_missing() -> None:
    """Single-select deselect is optional; pattern absence surfaces."""
    adapter = _item_adapter(extra={patterns.IsSelectable: IsSelectableStub(is_selected=True)})
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).deselect()


def test_is_multi_selectable_stub_referenced() -> None:
    """Sanity check: `IsMultiSelectableStub` is wired in helpers."""
    stub = IsMultiSelectableStub(can_select_multiple=False, is_selection_required=True)
    assert stub.can_select_multiple is False
    assert stub.is_selection_required is True


# ---------------------------------------------------------------------------
# HasEditor + TextEditable / Clearable lifecycle
# ---------------------------------------------------------------------------


def test_set_text_runs_open_set_accept_sequence() -> None:
    editor = HasEditorStub()
    text = TextEditableStub()
    adapter = _item_adapter(
        extra={patterns.HasEditor: editor, patterns.TextEditable: text},
    )
    _ConcreteItem(adapter=adapter).set_text('xyz')
    assert editor.open_calls == 1
    assert text.set_text_calls == ['xyz']
    assert editor.accept_calls == 1
    assert editor.cancel_calls == 0


def test_set_text_accepts_even_when_set_text_raises() -> None:
    """Editor must be closed via accept() in finally even on TextEditable failure."""
    editor = HasEditorStub()

    class BoomTextEditable(TextEditableStub):
        def set_text(self, value: str) -> None:
            del value
            raise RuntimeError('boom')

    text = BoomTextEditable()
    adapter = _item_adapter(
        extra={patterns.HasEditor: editor, patterns.TextEditable: text},
    )
    with pytest.raises(RuntimeError, match='boom'):
        _ConcreteItem(adapter=adapter).set_text('x')
    assert editor.open_calls == 1
    assert editor.accept_calls == 1


def test_clear_runs_open_clear_accept_sequence() -> None:
    editor = HasEditorStub()
    clearable = ClearableStub()
    adapter = _item_adapter(
        extra={patterns.HasEditor: editor, patterns.Clearable: clearable},
    )
    _ConcreteItem(adapter=adapter).clear()
    assert editor.open_calls == 1
    assert clearable.clear_calls == 1
    assert editor.accept_calls == 1


def test_set_text_raises_when_editor_pattern_missing() -> None:
    adapter = _item_adapter(
        extra={patterns.TextEditable: TextEditableStub()},
    )
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).set_text('x')


def test_clear_raises_when_editor_pattern_missing() -> None:
    adapter = _item_adapter(
        extra={patterns.Clearable: ClearableStub()},
    )
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).clear()


# ---------------------------------------------------------------------------
# Activatable
# ---------------------------------------------------------------------------


def test_activate_calls_pattern() -> None:
    act = ActivatableStub()
    adapter = _item_adapter(extra={patterns.Activatable: act})
    _ConcreteItem(adapter=adapter).activate()
    assert act.activate_calls == 1


def test_activate_blocks_when_disabled() -> None:
    act = ActivatableStub()
    adapter = _item_adapter(
        extra={
            patterns.Element: ElementStub(is_enabled=False),
            patterns.Activatable: act,
        },
    )
    with pytest.raises(CannotEnsureError):
        _ConcreteItem(adapter=adapter).activate()
    assert act.activate_calls == 0


def test_activate_raises_when_pattern_missing() -> None:
    adapter = _item_adapter()
    with pytest.raises(PatternNotSupportedError):
        _ConcreteItem(adapter=adapter).activate()
