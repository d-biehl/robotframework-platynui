# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false, reportUnknownLambdaType=false, reportUnknownArgumentType=false

"""Unit tests for ``PlatynUI.ui.application``.

Covers `Application.process_id` / `process_name` (with type-error
fallbacks), `is_ready` default, `_top_level_windows`, `_request_exit`
(graceful close per top-level window), `_force_exit` (poll loop and
kill), and `exit` (orchestration of the two stages plus invalidation).
"""

from collections.abc import Iterator
from unittest.mock import patch

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    CloseableStub,
    ElementStub,
    ResponsiveStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.settings import Settings
from PlatynUI.ui.application import Application
from PlatynUI.ui.window import Window

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
        application_exit_timeout=0.2,
    ):
        yield


def _app_adapter(
    *,
    attributes: dict[tuple[str, str], object] | None = None,
    children: list[Adapter] | None = None,
) -> Adapter:
    """Build an Application adapter with the requested attributes and children."""
    return make_adapter(  # type: ignore[no-any-return]
        role='Application',
        attributes=attributes or {},
        children=children or [],
    )


def _window_child_adapter(parent: Adapter, *, closeable: CloseableStub | None = None) -> Adapter:
    """Build a child Window adapter with the patterns required by `Window.close`."""
    pmap: dict[type, object] = {
        patterns.Element: ElementStub(),
        patterns.Responsive: ResponsiveStub(True),
    }
    if closeable is not None:
        pmap[patterns.Closeable] = closeable
    return make_adapter(role='Window', parent=parent, pattern_map=pmap)  # type: ignore[no-any-return]


# ---------------------------------------------------------------------------
# Process attributes
# ---------------------------------------------------------------------------


def test_process_id_returns_app_namespaced_attribute() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 4321}))
    assert a.process_id == 4321


def test_process_id_raises_type_error_when_attribute_missing() -> None:
    a = Application(adapter=_app_adapter())
    with pytest.raises(TypeError, match='expected int for ProcessId'):
        _ = a.process_id


def test_process_id_raises_type_error_on_wrong_type() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 'oops'}))
    with pytest.raises(TypeError, match='expected int for ProcessId'):
        _ = a.process_id


def test_process_name_returns_app_namespaced_attribute() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessName', 'app'): 'notepad.exe'}))
    assert a.process_name == 'notepad.exe'


def test_process_name_raises_type_error_when_attribute_missing() -> None:
    a = Application(adapter=_app_adapter())
    with pytest.raises(TypeError, match='expected str for ProcessName'):
        _ = a.process_name


# ---------------------------------------------------------------------------
# is_ready default
# ---------------------------------------------------------------------------


def test_is_ready_default_returns_true() -> None:
    a = Application(adapter=_app_adapter())
    assert a.is_ready() is True


# ---------------------------------------------------------------------------
# default_role / default_prefix
# ---------------------------------------------------------------------------


def test_default_role_and_prefix() -> None:
    assert Application.default_role == 'Application'
    assert Application.default_prefix == 'app'


# ---------------------------------------------------------------------------
# _top_level_windows
# ---------------------------------------------------------------------------


def test_top_level_windows_returns_only_window_children() -> None:
    app_adapter = _app_adapter()
    win1 = _window_child_adapter(app_adapter)
    win2 = _window_child_adapter(app_adapter)
    other = make_adapter(role='Pane', parent=app_adapter, pattern_map={patterns.Element: ElementStub()})
    app_adapter.children = [win1, other, win2]  # type: ignore[misc]

    a = Application(adapter=app_adapter)
    windows = a._top_level_windows()
    assert len(windows) == 2
    assert all(isinstance(w, Window) for w in windows)


def test_top_level_windows_empty_when_no_window_children() -> None:
    a = Application(adapter=_app_adapter())
    assert a._top_level_windows() == []


# ---------------------------------------------------------------------------
# _request_exit
# ---------------------------------------------------------------------------


def test_request_exit_closes_each_top_level_window() -> None:
    app_adapter = _app_adapter()
    closeables = [CloseableStub(), CloseableStub()]
    children = [_window_child_adapter(app_adapter, closeable=c) for c in closeables]
    app_adapter.children = list(children)  # type: ignore[misc]

    a = Application(adapter=app_adapter)
    # Bypass Window.close()'s post-condition (`exists()` re-resolves through
    # the mocked factory which is non-functional). We only care that close()
    # is invoked on each pattern.
    with patch.object(Window, 'close', autospec=True) as mock_close:
        a._request_exit()
    assert mock_close.call_count == 2


def test_request_exit_swallows_close_failures() -> None:
    app_adapter = _app_adapter()
    app_adapter.children = [  # type: ignore[misc]
        _window_child_adapter(app_adapter),
        _window_child_adapter(app_adapter),
    ]
    a = Application(adapter=app_adapter)

    def _boom(self: Window) -> None:
        raise RuntimeError('refused')

    with patch.object(Window, 'close', _boom):
        # Must not raise — failures are logged at debug level and swallowed.
        a._request_exit()


# ---------------------------------------------------------------------------
# _force_exit
# ---------------------------------------------------------------------------


def test_force_exit_returns_immediately_when_process_id_unavailable() -> None:
    a = Application(adapter=_app_adapter())  # no ProcessId attribute
    with (
        patch('PlatynUI.ui.application._process_alive') as alive,
        patch('PlatynUI.ui.application._kill_process') as kill,
    ):
        a._force_exit(timeout=0.1)
    alive.assert_not_called()
    kill.assert_not_called()


def test_force_exit_returns_when_pid_non_positive() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 0}))
    with (
        patch('PlatynUI.ui.application._process_alive') as alive,
        patch('PlatynUI.ui.application._kill_process') as kill,
    ):
        a._force_exit(timeout=0.1)
    alive.assert_not_called()
    kill.assert_not_called()


def test_force_exit_returns_when_process_dies_during_poll() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 1234}))
    with (
        patch('PlatynUI.ui.application._process_alive', return_value=False) as alive,
        patch('PlatynUI.ui.application._kill_process') as kill,
    ):
        a._force_exit(timeout=0.1)
    alive.assert_called_once_with(1234)
    kill.assert_not_called()


def test_force_exit_kills_process_when_timeout_expires() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 1234}))
    with (
        patch('PlatynUI.ui.application._process_alive', return_value=True),
        patch('PlatynUI.ui.application._kill_process') as kill,
    ):
        a._force_exit(timeout=0.05)
    kill.assert_called_once_with(1234)


# ---------------------------------------------------------------------------
# exit (orchestration)
# ---------------------------------------------------------------------------


def test_exit_runs_request_exit_then_force_exit_then_invalidates() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 1234}))
    calls: list[str] = []

    def _record_request(self: Application) -> None:
        calls.append('request')

    def _record_force(self: Application, timeout: float) -> None:
        calls.append(f'force:{timeout}')

    def _record_invalidate(self: Application) -> None:
        calls.append('invalidate')

    with (
        patch.object(Application, '_request_exit', _record_request),
        patch.object(Application, '_force_exit', _record_force),
        patch.object(Application, 'invalidate', _record_invalidate),
    ):
        a.exit(timeout=0.2)

    assert calls == ['request', 'force:0.2', 'invalidate']


def test_exit_uses_settings_default_when_timeout_omitted() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 1234}))
    captured: list[float] = []

    with (
        patch.object(Application, '_request_exit', lambda self: None),
        patch.object(Application, '_force_exit', lambda self, timeout: captured.append(timeout)),
        patch.object(Application, 'invalidate', lambda self: None),
    ):
        a.exit()

    assert captured == [Settings.current().application_exit_timeout]


def test_exit_continues_to_force_when_request_exit_raises() -> None:
    a = Application(adapter=_app_adapter(attributes={('ProcessId', 'app'): 1234}))
    forced: list[float] = []
    invalidated: list[bool] = []

    def _boom(self: Application) -> None:
        raise RuntimeError('graceful exit failed')

    with (
        patch.object(Application, '_request_exit', _boom),
        patch.object(Application, '_force_exit', lambda self, timeout: forced.append(timeout)),
        patch.object(Application, 'invalidate', lambda self: invalidated.append(True)),
    ):
        a.exit(timeout=0.1)

    assert forced == [0.1]
    assert invalidated == [True]
