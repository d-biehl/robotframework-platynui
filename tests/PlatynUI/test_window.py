# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal predicates and adapter pattern interactions.
# Pytest fixtures look unused to pyright; the ``_ui_helpers`` import
# only needs the ignore under mypy.

"""Unit tests for ``PlatynUI.ui.window``.

Covers the read-only properties (`is_active`, `is_minimized`,
`is_maximized`, `title`), the ``_window_can_*`` and ``_window_is_*``
predicates, and the seven Pre/Pattern/Post capability methods
(`activate`, `minimize`, `maximize`, `restore`, `close`, `move_to`,
`resize`). Includes a smoke test for the `Frame` marker subclass.
"""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ActivatableStub,
    CloseableStub,
    ElementStub,
    MaximizableStub,
    MinimizableStub,
    MovableStub,
    ResizableStub,
    ResponsiveStub,
    RestorableStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import CannotEnsureError, PatternNotSupportedError
from PlatynUI.core.settings import Settings
from PlatynUI.core.types import Point, Size
from PlatynUI.ui.window import Frame, Window

# ---------------------------------------------------------------------------
# Common fixtures and helpers
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
        window_close_timeout=0.05,
    ):
        yield


def _window_adapter(extra: dict[type, object] | None = None) -> Adapter:
    """Build a window adapter parented to a Desktop with `Responsive=True`."""
    desktop = make_adapter(role='Desktop')
    pmap: dict[type, object] = {
        patterns.Element: ElementStub(),
        patterns.Responsive: ResponsiveStub(True),
    }
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role='Window',
        parent=desktop,
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# Read-only properties
# ---------------------------------------------------------------------------


def test_is_active_uses_window_state_when_present() -> None:
    w = Window(adapter=_window_adapter({patterns.WindowState: WindowStateStub(is_active=True)}))
    assert w.is_active is True


def test_is_active_defaults_false_without_window_state() -> None:
    w = Window(adapter=_window_adapter())
    assert w.is_active is False


def test_is_topmost_uses_window_state_when_present() -> None:
    w = Window(adapter=_window_adapter({patterns.WindowState: WindowStateStub(is_topmost=True)}))
    assert w.is_topmost is True


def test_is_topmost_defaults_false_without_window_state() -> None:
    w = Window(adapter=_window_adapter())
    assert w.is_topmost is False


def test_is_modal_uses_window_state_when_present() -> None:
    w = Window(adapter=_window_adapter({patterns.WindowState: WindowStateStub(is_modal=True)}))
    assert w.is_modal is True


def test_is_modal_defaults_false_without_window_state() -> None:
    w = Window(adapter=_window_adapter())
    assert w.is_modal is False


def test_is_minimized_reflects_pattern_state() -> None:
    w = Window(adapter=_window_adapter({patterns.Minimizable: MinimizableStub(is_minimized=True)}))
    assert w.is_minimized is True


def test_is_minimized_defaults_false_without_pattern() -> None:
    assert Window(adapter=_window_adapter()).is_minimized is False


def test_is_maximized_reflects_pattern_state() -> None:
    w = Window(adapter=_window_adapter({patterns.Maximizable: MaximizableStub(is_maximized=True)}))
    assert w.is_maximized is True


def test_is_maximized_defaults_false_without_pattern() -> None:
    assert Window(adapter=_window_adapter()).is_maximized is False


def test_title_returns_adapter_name() -> None:
    desktop = make_adapter(role='Desktop')
    adapter = make_adapter(
        role='Window',
        name='Untitled - Notepad',
        parent=desktop,
        pattern_map={patterns.Element: ElementStub(), patterns.Responsive: ResponsiveStub(True)},
    )
    assert Window(adapter=adapter).title == 'Untitled - Notepad'


# ---------------------------------------------------------------------------
# _window_can_* predicates
# ---------------------------------------------------------------------------


def test_window_can_minimize_true_when_pattern_allows() -> None:
    w = Window(adapter=_window_adapter({patterns.Minimizable: MinimizableStub(can_minimize=True)}))
    assert w._window_can_minimize() is True


def test_window_can_minimize_false_when_pattern_missing() -> None:
    assert Window(adapter=_window_adapter())._window_can_minimize() is False


def test_window_can_minimize_false_when_pattern_disallows() -> None:
    w = Window(adapter=_window_adapter({patterns.Minimizable: MinimizableStub(can_minimize=False)}))
    assert w._window_can_minimize() is False


def test_window_can_maximize_true_when_pattern_allows() -> None:
    w = Window(adapter=_window_adapter({patterns.Maximizable: MaximizableStub(can_maximize=True)}))
    assert w._window_can_maximize() is True


def test_window_can_maximize_false_when_pattern_missing() -> None:
    assert Window(adapter=_window_adapter())._window_can_maximize() is False


def test_window_can_close_true_when_pattern_allows() -> None:
    w = Window(adapter=_window_adapter({patterns.Closeable: CloseableStub(can_close=True)}))
    assert w._window_can_close() is True


def test_window_can_close_false_when_pattern_missing() -> None:
    assert Window(adapter=_window_adapter())._window_can_close() is False


def test_window_can_move_true_when_pattern_allows() -> None:
    w = Window(adapter=_window_adapter({patterns.Movable: MovableStub(can_move=True)}))
    assert w._window_can_move() is True


def test_window_can_move_false_when_pattern_missing() -> None:
    assert Window(adapter=_window_adapter())._window_can_move() is False


def test_window_can_resize_true_when_pattern_allows() -> None:
    w = Window(adapter=_window_adapter({patterns.Resizable: ResizableStub(can_resize=True)}))
    assert w._window_can_resize() is True


def test_window_can_resize_false_when_pattern_missing() -> None:
    assert Window(adapter=_window_adapter())._window_can_resize() is False


# ---------------------------------------------------------------------------
# activate()
# ---------------------------------------------------------------------------


def test_activate_short_circuits_when_already_active() -> None:
    activatable = ActivatableStub()
    window_state = WindowStateStub(is_active=True)
    w = Window(
        adapter=_window_adapter(
            {
                patterns.Activatable: activatable,
                patterns.WindowState: window_state,
            }
        )
    )
    w.activate()
    assert activatable.activate_calls == 0


def test_activate_calls_activatable_then_waits_for_active_state() -> None:
    window_state = WindowStateStub(is_active=False)

    class FlippingActivatable(ActivatableStub):
        def __init__(self, window_state: WindowStateStub) -> None:
            super().__init__()
            self._window_state = window_state

        def activate(self) -> None:
            super().activate()
            self._window_state._active = True

    activatable = FlippingActivatable(window_state)
    w = Window(
        adapter=_window_adapter(
            {
                patterns.Activatable: activatable,
                patterns.WindowState: window_state,
            }
        )
    )
    w.activate()
    assert activatable.activate_calls == 1
    assert w.is_active is True


def test_activate_raises_pattern_not_supported_when_activatable_missing() -> None:
    w = Window(adapter=_window_adapter())
    with pytest.raises(PatternNotSupportedError):
        w.activate()


# ---------------------------------------------------------------------------
# minimize() / maximize() / restore()
# ---------------------------------------------------------------------------


def test_minimize_calls_pattern_then_waits_for_minimized() -> None:
    minimizable = MinimizableStub()
    w = Window(adapter=_window_adapter({patterns.Minimizable: minimizable}))
    w.minimize()
    assert minimizable.minimize_calls == 1
    assert w.is_minimized is True


def test_minimize_raises_when_pattern_disallows() -> None:
    minimizable = MinimizableStub(can_minimize=False)
    w = Window(adapter=_window_adapter({patterns.Minimizable: minimizable}))
    with pytest.raises(CannotEnsureError):
        w.minimize()
    assert minimizable.minimize_calls == 0


def test_minimize_raises_when_pattern_missing() -> None:
    w = Window(adapter=_window_adapter())
    with pytest.raises(CannotEnsureError):
        w.minimize()


def test_maximize_calls_pattern_then_waits_for_maximized() -> None:
    maximizable = MaximizableStub()
    w = Window(adapter=_window_adapter({patterns.Maximizable: maximizable}))
    w.maximize()
    assert maximizable.maximize_calls == 1
    assert w.is_maximized is True


def test_maximize_raises_when_pattern_missing() -> None:
    with pytest.raises(CannotEnsureError):
        Window(adapter=_window_adapter()).maximize()


def test_restore_calls_pattern_then_waits_until_restored() -> None:
    minimizable = MinimizableStub(is_minimized=True)
    restorable = RestorableStub()

    # Restorable.restore() in our stub does not flip is_minimized; emulate
    # the real behaviour by patching restore() to clear the minimized flag.
    original_restore = restorable.restore

    def _restore_and_clear() -> None:
        original_restore()
        minimizable._is_minimized = False

    restorable.restore = _restore_and_clear

    w = Window(
        adapter=_window_adapter(
            {
                patterns.Minimizable: minimizable,
                patterns.Restorable: restorable,
            }
        )
    )
    w.restore()
    assert restorable.restore_calls == 1
    assert w.is_minimized is False


def test_restore_raises_pattern_not_supported_when_restorable_missing() -> None:
    w = Window(adapter=_window_adapter())
    with pytest.raises(PatternNotSupportedError):
        w.restore()


# ---------------------------------------------------------------------------
# close()
# ---------------------------------------------------------------------------


def test_close_calls_pattern_then_waits_for_window_gone() -> None:
    closeable = CloseableStub()
    adapter = _window_adapter({patterns.Closeable: closeable})
    state = {'closed': False}

    original_close = closeable.close

    def _close_and_invalidate() -> None:
        original_close()
        state['closed'] = True

    closeable.close = _close_and_invalidate

    w = Window(adapter=adapter)
    w.close()
    assert closeable.close_calls == 1
    assert state['closed'] is True


def test_close_raises_when_closeable_missing() -> None:
    with pytest.raises(CannotEnsureError):
        Window(adapter=_window_adapter()).close()


def test_close_raises_when_window_does_not_disappear() -> None:
    closeable = CloseableStub()
    w = Window(adapter=_window_adapter({patterns.Closeable: closeable}))
    # Window never disappears: exists() always returns True.
    w.exists = lambda *, timeout=None, raise_exception=False: True  # type: ignore[method-assign]
    with pytest.raises(CannotEnsureError):
        w.close()
    assert closeable.close_calls == 1


# ---------------------------------------------------------------------------
# move_to() / resize()
# ---------------------------------------------------------------------------


def test_move_to_calls_movable_with_target_point() -> None:
    movable = MovableStub()
    w = Window(adapter=_window_adapter({patterns.Movable: movable}))
    w.move_to(Point(120.0, 80.0))
    assert movable.move_calls == [Point(120.0, 80.0)]


def test_move_to_raises_when_movable_missing() -> None:
    with pytest.raises(CannotEnsureError):
        Window(adapter=_window_adapter()).move_to(Point(0.0, 0.0))


def test_resize_calls_resizable_with_target_size() -> None:
    resizable = ResizableStub()
    w = Window(adapter=_window_adapter({patterns.Resizable: resizable}))
    w.resize(Size(640.0, 480.0))
    assert resizable.resize_calls == [Size(640.0, 480.0)]


def test_resize_raises_when_resizable_missing() -> None:
    with pytest.raises(CannotEnsureError):
        Window(adapter=_window_adapter()).resize(Size(1.0, 1.0))


# ---------------------------------------------------------------------------
# Frame
# ---------------------------------------------------------------------------


def test_frame_inherits_window_behaviour() -> None:
    f = Frame(adapter=_window_adapter())
    # Frame is a marker subclass — same surface as Window.
    assert isinstance(f, Window)
    assert Frame.default_role == 'Frame'
    assert f.is_active is False
