# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal state (predicates, lazy caches, proxy slots)
# and pass private predicates to ``ensure_that``. Pytest fixtures look
# unused to pyright; the ``_ui_helpers`` import only needs the ignore
# under mypy.

"""Unit tests for ``PlatynUI.ui.element``.

Covers adapter pass-through properties, tree navigation
(`top_level_parent`, `parent_window`, `_resolve_application`), the six
predicates, the convenience methods (`bring_to_view`, `highlight`,
`get_screenshot`, `save_screenshot`), and lazy mouse/keyboard proxy
construction.
"""

from collections.abc import Iterator
from pathlib import Path
from unittest.mock import MagicMock

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ActivatableStub,
    ElementStub,
    ReadableStub,
    ResponsiveStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.runtime import runtime
from PlatynUI.core.settings import Settings
from PlatynUI.core.types import Rect
from PlatynUI.ui.application import Application
from PlatynUI.ui.element import Element
from PlatynUI.ui.window import Window

# ---------------------------------------------------------------------------
# Common fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _fast_settings() -> Iterator[None]:
    """Shrink ensure/exists timeouts so failing predicates do not stall the suite."""
    with Settings(
        ensure_timeout=0.05,
        ensure_delay=0.0,
        exists_timeout=0.05,
        wait_for_timeout=0.05,
        wait_for_delay=0.0,
    ):
        yield


@pytest.fixture
def native_runtime() -> Iterator[MagicMock]:
    fake = MagicMock(name='FakeNativeRuntime')
    with runtime.override(lambda: fake):
        yield fake


# ---------------------------------------------------------------------------
# Adapter pass-through properties
# ---------------------------------------------------------------------------


def test_bounds_visible_in_view_enabled_pass_through_element_pattern() -> None:
    elem_pattern = ElementStub(
        bounds=Rect(10.0, 20.0, 100.0, 50.0),
        is_visible=True,
        is_in_view=False,
        is_enabled=True,
    )
    adapter = make_adapter(pattern_map={patterns.Element: elem_pattern})
    e = Element(adapter=adapter)

    assert e.bounds == Rect(10.0, 20.0, 100.0, 50.0)
    assert e.is_visible is True
    assert e.is_in_view is False
    assert e.is_enabled is True


def test_is_readonly_defaults_false_when_readable_missing() -> None:
    adapter = make_adapter(pattern_map={patterns.Element: ElementStub()})
    assert Element(adapter=adapter).is_readonly is False


def test_is_readonly_uses_readable_pattern_when_present() -> None:
    adapter = make_adapter(
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Readable: ReadableStub(is_readonly=True),
        },
    )
    assert Element(adapter=adapter).is_readonly is True


# ---------------------------------------------------------------------------
# Tree navigation
# ---------------------------------------------------------------------------


def test_top_level_parent_returns_self_when_already_top_level() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    win_adapter = make_adapter(role='Window', parent=desktop_adapter)
    win = Window(adapter=win_adapter)
    assert win.top_level_parent is win


def test_top_level_parent_walks_up_to_direct_child_of_desktop() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    win_adapter = make_adapter(role='Window', parent=desktop_adapter)
    inner_adapter = make_adapter(
        role='Button',
        parent=win_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )
    inner = Element(adapter=inner_adapter)

    top = inner.top_level_parent
    assert top.adapter is win_adapter


def test_parent_window_returns_first_window_ancestor() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    win_adapter = make_adapter(role='Window', parent=desktop_adapter)
    pane_adapter = make_adapter(role='Pane', parent=win_adapter)
    btn_adapter = make_adapter(
        role='Button',
        parent=pane_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )
    btn = Element(adapter=btn_adapter)

    pw = btn.parent_window
    assert pw is not None
    assert pw.adapter is win_adapter
    assert isinstance(pw, Window)


def test_parent_window_returns_none_when_no_window_ancestor() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    pane_adapter = make_adapter(role='Pane', parent=desktop_adapter)
    btn_adapter = make_adapter(
        role='Button',
        parent=pane_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )
    assert Element(adapter=btn_adapter).parent_window is None


def test_resolve_application_walks_up_to_application_node() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    app_adapter = make_adapter(role='Application', parent=desktop_adapter)
    win_adapter = make_adapter(role='Window', parent=app_adapter)
    btn_adapter = make_adapter(
        role='Button',
        parent=win_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )

    e = Element(adapter=btn_adapter)
    app = e._resolve_application()
    assert isinstance(app, Application)
    assert app.adapter is app_adapter


def test_resolve_application_returns_none_when_no_application_in_chain() -> None:
    desktop_adapter = make_adapter(role='Desktop')
    win_adapter = make_adapter(role='Window', parent=desktop_adapter)
    btn_adapter = make_adapter(
        role='Button',
        parent=win_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )
    assert Element(adapter=btn_adapter)._resolve_application() is None


# ---------------------------------------------------------------------------
# Predicates
# ---------------------------------------------------------------------------


def test_application_is_ready_true_without_responsive_pattern() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(role='Window', parent=desktop, pattern_map={patterns.Element: ElementStub()})
    e = Element(adapter=win)
    assert e._application_is_ready() is True


def test_application_is_ready_false_when_responsive_says_no() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(False),
        },
    )
    assert Element(adapter=win)._application_is_ready() is False


def test_application_is_ready_true_when_responsive_returns_none() -> None:
    """``None`` from the platform is treated as "do not know" → not blocking."""
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(None),
        },
    )
    assert Element(adapter=win)._application_is_ready() is True


def test_application_is_ready_delegates_to_top_level_parent() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(False),
        },
    )
    btn = make_adapter(role='Button', parent=win, pattern_map={patterns.Element: ElementStub()})
    inner = Element(adapter=btn)
    assert inner._application_is_ready() is False


def test_element_is_visible_returns_true_for_visible_element() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(role='Window', parent=desktop, pattern_map={patterns.Element: ElementStub()})
    assert Element(adapter=win)._element_is_visible() is True


def test_element_is_in_view_reflects_pattern_state() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub(is_in_view=False)},
    )
    assert Element(adapter=win)._element_is_in_view() is False


def test_element_is_enabled_reflects_pattern_state() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub(is_enabled=False)},
    )
    assert Element(adapter=win)._element_is_enabled() is False


def test_element_is_not_readonly_returns_false_when_readable_says_readonly() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Readable: ReadableStub(is_readonly=True),
        },
    )
    assert Element(adapter=win)._element_is_not_readonly() is False


def test_toplevel_parent_is_active_returns_true_at_desktop_top_level() -> None:
    """A plain Element under Desktop has no activation contract → assumed active."""
    desktop = make_adapter(role='Desktop')
    el_adapter = make_adapter(
        role='Pane',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    assert Element(adapter=el_adapter)._toplevel_parent_is_active() is True


# ---------------------------------------------------------------------------
# Convenience methods
# ---------------------------------------------------------------------------


def test_highlight_delegates_to_runtime_with_bounds_and_duration_ms(
    native_runtime: MagicMock,
) -> None:
    desktop = make_adapter(role='Desktop')
    bounds = Rect(5.0, 6.0, 7.0, 8.0)
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub(bounds=bounds)},
    )

    Element(adapter=win).highlight(duration=2.5)

    native_runtime.highlight.assert_called_once_with(rects=[bounds], duration_ms=2500)


def test_get_screenshot_delegates_to_runtime_with_bounds(
    native_runtime: MagicMock,
) -> None:
    native_runtime.screenshot.return_value = b'PNG-bytes'
    desktop = make_adapter(role='Desktop')
    bounds = Rect(0.0, 0.0, 10.0, 10.0)
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub(bounds=bounds)},
    )

    data = Element(adapter=win).get_screenshot()

    assert data == b'PNG-bytes'
    native_runtime.screenshot.assert_called_once_with(rect=bounds)


def test_save_screenshot_writes_bytes_to_path(
    native_runtime: MagicMock,
    tmp_path: Path,
) -> None:
    native_runtime.screenshot.return_value = b'PNG-bytes'
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )

    target = tmp_path / 'shot.png'
    out = Element(adapter=win).save_screenshot(target)

    assert out == target
    assert target.read_bytes() == b'PNG-bytes'


def test_bring_to_view_returns_true_when_predicates_pass(
    native_runtime: MagicMock,
) -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    assert Element(adapter=win).bring_to_view() is True


def test_activate_parent_window_calls_window_activate() -> None:
    """`activate_parent_window` resolves to the Window context's `activate()`."""
    desktop = make_adapter(role='Desktop')
    activ = ActivatableStub()
    # `is_active` reads `WindowState.is_active`; start inactive so
    # `Window.activate()` actually invokes the Activatable pattern,
    # then flip the active flag so the post-condition can succeed.
    window_state = WindowStateStub(is_active=False)

    def fake_activate() -> None:
        activ.activate_calls += 1
        window_state._active = True

    activ.activate = fake_activate

    win_adapter = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Activatable: activ,
            patterns.WindowState: window_state,
        },
    )
    btn_adapter = make_adapter(
        role='Button',
        parent=win_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )

    Element(adapter=btn_adapter).activate_parent_window()
    assert activ.activate_calls == 1


def test_activate_parent_window_no_op_when_no_window_ancestor() -> None:
    desktop = make_adapter(role='Desktop')
    btn = make_adapter(
        role='Button',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    # Must not raise.
    Element(adapter=btn).activate_parent_window()


# ---------------------------------------------------------------------------
# Lazy proxy construction
# ---------------------------------------------------------------------------


def test_mouse_proxy_is_cached_per_instance() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    e = Element(adapter=win)
    assert e.mouse is e.mouse


def test_keyboard_proxy_is_cached_per_instance() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    e = Element(adapter=win)
    assert e.keyboard is e.keyboard


def test_invalidate_resets_proxy_caches() -> None:
    desktop = make_adapter(role='Desktop')
    win = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub()},
    )
    e = Element(adapter=win)
    m1 = e.mouse
    k1 = e.keyboard
    e.invalidate()
    # Re-set adapter so subsequent property access does not need re-resolution.
    e._adapter = win
    assert e.mouse is not m1
    assert e.keyboard is not k1
