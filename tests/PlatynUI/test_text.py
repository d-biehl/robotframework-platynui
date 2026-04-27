# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal state (call counts on stub patterns) and pass
# private predicates to ``ensure_that``. Pytest fixtures look unused to
# pyright; the ``_ui_helpers`` import only needs the ignore under mypy.

"""Unit tests for ``PlatynUI.ui.text``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ClearableStub,
    ElementStub,
    FocusableStub,
    HasUserInputStub,
    ReadableStub,
    TextContentStub,
    TextEditableStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import CannotEnsureError, PatternNotSupportedError
from PlatynUI.core.settings import Settings
from PlatynUI.ui.text import Edit, Text

# ---------------------------------------------------------------------------
# Common fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _fast_settings() -> Iterator[None]:
    """Shrink ensure timeouts so failing predicates do not stall the suite."""
    with Settings(
        ensure_timeout=0.05,
        ensure_delay=0.0,
        exists_timeout=0.05,
        wait_for_timeout=0.05,
        wait_for_delay=0.0,
    ):
        yield


def _control_adapter(
    *,
    role: str,
    extra: dict[type, object] | None = None,
    is_focused: bool = True,
    element: ElementStub | None = None,
    focusable: FocusableStub | None = None,
) -> Adapter:
    """Build a control adapter parented to an active Window/Desktop chain."""
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
        patterns.Element: element or ElementStub(),
        patterns.Focusable: focusable or FocusableStub(is_focused=is_focused),
    }
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role=role, parent=window, pattern_map=pmap,
    )


def _text_adapter(
    *,
    text: str = '',
    locale: str = '',
    is_truncated: bool = False,
    with_text_content: bool = True,
) -> Adapter:
    extra: dict[type, object] = {}
    if with_text_content:
        extra[patterns.TextContent] = TextContentStub(
            text,
            locale=locale,
            is_truncated=is_truncated,
        )
    return _control_adapter(role='Text', extra=extra)


def _edit_adapter(
    *,
    text: str = '',
    text_editable: TextEditableStub | None = None,
    clearable: ClearableStub | None = None,
    focusable: FocusableStub | None = None,
    extra_readable: bool = False,
    is_focused: bool = True,
    element: ElementStub | None = None,
    with_text_content: bool = True,
    with_text_editable: bool = True,
    with_clearable: bool = True,
) -> Adapter:
    extra: dict[type, object] = {}
    if with_text_content:
        extra[patterns.TextContent] = TextContentStub(text)
    if with_text_editable:
        extra[patterns.TextEditable] = text_editable or TextEditableStub()
    if with_clearable:
        extra[patterns.Clearable] = clearable or ClearableStub()
    if extra_readable:
        extra[patterns.Readable] = ReadableStub(is_readonly=True)
    return _control_adapter(
        role='Edit', extra=extra, is_focused=is_focused, element=element,
        focusable=focusable,
    )


# ---------------------------------------------------------------------------
# Text — read-only properties
# ---------------------------------------------------------------------------


def test_text_returns_text_content_pattern_value() -> None:
    assert Text(adapter=_text_adapter(text='hello')).text == 'hello'


def test_text_raises_when_text_content_pattern_missing() -> None:
    """`Text` requires the provider to expose `TextContent`."""
    with pytest.raises(PatternNotSupportedError):
        _ = Text(adapter=_text_adapter(with_text_content=False)).text


def test_text_is_truncated_returns_pattern_value() -> None:
    assert Text(adapter=_text_adapter(is_truncated=True)).is_truncated is True


def test_text_locale_returns_pattern_value() -> None:
    assert Text(adapter=_text_adapter(locale='de-DE')).locale == 'de-DE'


def test_text_is_auto_registered() -> None:
    """`Text` is registered as the default context for role=\"Text\"."""
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_text_adapter())
    assert cls is Text


# ---------------------------------------------------------------------------
# Edit — text read/write
# ---------------------------------------------------------------------------


def test_edit_text_returns_text_content_pattern_value() -> None:
    assert Edit(adapter=_edit_adapter(text='abc')).text == 'abc'


def test_edit_set_text_invokes_text_editable_pattern() -> None:
    editable = TextEditableStub()
    Edit(adapter=_edit_adapter(text_editable=editable)).set_text('new')
    assert editable.set_text_calls == ['new']


def test_edit_text_setter_delegates_to_set_text() -> None:
    editable = TextEditableStub()
    Edit(adapter=_edit_adapter(text_editable=editable)).text = 'via setter'
    assert editable.set_text_calls == ['via setter']


def test_edit_set_text_blocks_when_element_disabled() -> None:
    editable = TextEditableStub()
    edit = Edit(adapter=_edit_adapter(text_editable=editable, element=ElementStub(is_enabled=False)))
    with pytest.raises(CannotEnsureError):
        edit.set_text('x')
    assert editable.set_text_calls == []


def test_edit_set_text_blocks_when_field_readonly() -> None:
    """Read-only is signalled via the `Readable` pattern at the Element level."""
    editable = TextEditableStub()
    edit = Edit(adapter=_edit_adapter(text_editable=editable, extra_readable=True))
    with pytest.raises(CannotEnsureError):
        edit.set_text('x')
    assert editable.set_text_calls == []


def test_edit_set_text_focuses_when_not_yet_focused() -> None:
    """`_control_has_focus` is self-healing — calls `focus()` if needed."""
    editable = TextEditableStub()
    focusable = FocusableStub(is_focused=False)
    adapter = _edit_adapter(text_editable=editable, focusable=focusable)
    Edit(adapter=adapter).set_text('x')
    assert focusable.focus_calls >= 1
    assert editable.set_text_calls == ['x']


def test_edit_set_text_raises_when_text_editable_missing() -> None:
    adapter = _edit_adapter(with_text_editable=False)
    with pytest.raises(PatternNotSupportedError):
        Edit(adapter=adapter).set_text('x')


# ---------------------------------------------------------------------------
# Edit — clear
# ---------------------------------------------------------------------------


def test_edit_clear_invokes_clearable_pattern() -> None:
    clearable = ClearableStub()
    Edit(adapter=_edit_adapter(clearable=clearable)).clear()
    assert clearable.clear_calls == 1


def test_edit_clear_blocks_when_field_readonly() -> None:
    clearable = ClearableStub()
    edit = Edit(adapter=_edit_adapter(clearable=clearable, extra_readable=True))
    with pytest.raises(CannotEnsureError):
        edit.clear()
    assert clearable.clear_calls == 0


def test_edit_clear_raises_when_clearable_missing() -> None:
    adapter = _edit_adapter(with_clearable=False)
    with pytest.raises(PatternNotSupportedError):
        Edit(adapter=adapter).clear()


# ---------------------------------------------------------------------------
# Edit — read-only properties
# ---------------------------------------------------------------------------


def test_edit_max_length_returns_pattern_value() -> None:
    editable = TextEditableStub(max_length=42)
    assert Edit(adapter=_edit_adapter(text_editable=editable)).max_length == 42
    assert Edit(adapter=_edit_adapter()).max_length is None


def test_edit_supports_password_mode_returns_pattern_value() -> None:
    editable = TextEditableStub(supports_password_mode=True)
    assert Edit(adapter=_edit_adapter(text_editable=editable)).supports_password_mode is True


def test_edit_is_multi_line_returns_pattern_value() -> None:
    multi = TextEditableStub(is_multi_line=True)
    assert Edit(adapter=_edit_adapter(text_editable=multi)).is_multi_line is True
    single = TextEditableStub(is_multi_line=False)
    assert Edit(adapter=_edit_adapter(text_editable=single)).is_multi_line is False


def test_edit_is_auto_registered() -> None:
    """`Edit` is registered as the default context for role=\"Edit\"."""
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_edit_adapter())
    assert cls is Edit
