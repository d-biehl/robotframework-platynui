# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal state (predicates, proxy slots) and pass private
# predicates to ``ensure_that``. Pytest fixtures look unused to pyright;
# the ``_ui_helpers`` import only needs the ignore under mypy.

"""Unit tests for ``PlatynUI.ui.control``.

Covers the focus contract added on top of `Element`:
``has_focus``, ``focus``, the ``_control_has_focus`` predicate, and the
focus-aware keyboard proxy installed by ``_create_keyboard_proxy``.
"""

from __future__ import annotations

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    FocusableStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.devices import KeyboardAction
from PlatynUI.core.exceptions import CannotEnsureError
from PlatynUI.core.settings import Settings
from PlatynUI.ui.control import Control, _ControlKeyboardProxy
from PlatynUI.ui.element import _ElementKeyboardProxy

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
    is_focused: bool = False,
    with_focusable: bool = True,
) -> Adapter:
    """Build an adapter exposing `Element` and optionally `Focusable`."""
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if with_focusable:
        pmap[patterns.Focusable] = FocusableStub(is_focused=is_focused)
    return make_adapter(role='Button', pattern_map=pmap)  # type: ignore[no-any-return]


# ---------------------------------------------------------------------------
# has_focus
# ---------------------------------------------------------------------------


def test_has_focus_true_when_focusable_reports_focused() -> None:
    c = Control(adapter=_control_adapter(is_focused=True))
    assert c.has_focus is True


def test_has_focus_false_when_focusable_reports_unfocused() -> None:
    c = Control(adapter=_control_adapter(is_focused=False))
    assert c.has_focus is False


def test_has_focus_defaults_false_when_focusable_missing() -> None:
    c = Control(adapter=_control_adapter(with_focusable=False))
    assert c.has_focus is False


# ---------------------------------------------------------------------------
# focus()
# ---------------------------------------------------------------------------


def test_focus_calls_focusable_focus_when_pattern_present() -> None:
    focusable = FocusableStub(is_focused=False)
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub(), patterns.Focusable: focusable},
    )
    Control(adapter=adapter).focus()
    assert focusable.focus_calls == 1
    assert focusable.is_focused is True


def test_focus_is_silent_no_op_when_focusable_missing() -> None:
    # No exception even though Focusable is absent.
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub()},
    )
    Control(adapter=adapter).focus()  # must not raise


def test_focus_raises_when_element_not_in_view() -> None:
    focusable = FocusableStub()
    adapter = make_adapter(
        role='Button',
        pattern_map={
            patterns.Element: ElementStub(is_visible=False, is_in_view=False),
            patterns.Focusable: focusable,
        },
    )
    with pytest.raises(CannotEnsureError):
        Control(adapter=adapter).focus()
    assert focusable.focus_calls == 0


def test_focus_raises_when_element_disabled() -> None:
    focusable = FocusableStub()
    adapter = make_adapter(
        role='Button',
        pattern_map={
            patterns.Element: ElementStub(is_enabled=False),
            patterns.Focusable: focusable,
        },
    )
    with pytest.raises(CannotEnsureError):
        Control(adapter=adapter).focus()
    assert focusable.focus_calls == 0


# ---------------------------------------------------------------------------
# _control_has_focus predicate
# ---------------------------------------------------------------------------


def test_control_has_focus_returns_true_when_already_focused() -> None:
    focusable = FocusableStub(is_focused=True)
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub(), patterns.Focusable: focusable},
    )
    c = Control(adapter=adapter)
    assert c._control_has_focus() is True
    # Already-focused short-circuit must not call focus().
    assert focusable.focus_calls == 0


def test_control_has_focus_calls_focus_then_returns_true() -> None:
    focusable = FocusableStub(is_focused=False)
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub(), patterns.Focusable: focusable},
    )
    c = Control(adapter=adapter)
    assert c._control_has_focus() is True
    assert focusable.focus_calls == 1


def test_control_has_focus_returns_false_when_focus_does_not_take() -> None:
    # Focusable.focus() that never flips ``is_focused`` to True.
    class StuckFocusable(FocusableStub):
        def focus(self) -> None:
            self.focus_calls += 1
            # Intentionally do NOT set ``self._focused = True``.

    focusable = StuckFocusable(is_focused=False)
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub(), patterns.Focusable: focusable},
    )
    c = Control(adapter=adapter)
    assert c._control_has_focus() is False
    assert focusable.focus_calls == 1


# ---------------------------------------------------------------------------
# Keyboard proxy override
# ---------------------------------------------------------------------------


def test_create_keyboard_proxy_returns_control_keyboard_proxy() -> None:
    c = Control(adapter=_control_adapter())
    proxy = c.keyboard
    assert isinstance(proxy, _ControlKeyboardProxy)
    # _ControlKeyboardProxy extends the element keyboard proxy.
    assert isinstance(proxy, _ElementKeyboardProxy)


def test_keyboard_before_action_focuses_then_calls_super() -> None:
    focusable = FocusableStub(is_focused=False)
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub(), patterns.Focusable: focusable},
    )
    c = Control(adapter=adapter)
    proxy = c.keyboard
    assert isinstance(proxy, _ControlKeyboardProxy)

    proxy.before_action(KeyboardAction.TYPE)
    # Focus was acquired exactly once via the _control_has_focus predicate.
    assert focusable.focus_calls == 1
    assert focusable.is_focused is True


def test_keyboard_before_action_raises_when_focus_unavailable() -> None:
    # No Focusable pattern → has_focus stays False forever → predicate fails.
    adapter = make_adapter(
        role='Button',
        pattern_map={patterns.Element: ElementStub()},
    )
    proxy = Control(adapter=adapter).keyboard
    assert isinstance(proxy, _ControlKeyboardProxy)
    with pytest.raises(CannotEnsureError):
        proxy.before_action(KeyboardAction.TYPE)
