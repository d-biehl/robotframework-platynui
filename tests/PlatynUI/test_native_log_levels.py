"""Native log-level requests and keyword-boundary delivery (spec: *native-logging*)."""

import json
import os
import subprocess
import sys
import textwrap
from types import SimpleNamespace
from typing import Any, cast

import pytest

from PlatynUI import _our_libcore
from PlatynUI._our_libcore import OurDynamicCore, keyword
from PlatynUI.core.native_logging import NativeLogLevels


class FakeNative:
    """Records every level the registry applies."""

    def __init__(self) -> None:
        self.applied: list[str | None] = []

    def set_log_level(self, level: str | None = None) -> None:
        self.applied.append(level)

    def flush_logs(self) -> None:
        pass


def registry() -> tuple[NativeLogLevels, FakeNative]:
    native = FakeNative()
    return NativeLogLevels(cast(Any, native)), native


def test_a_request_applies_its_level_and_withdrawing_it_restores_the_default() -> None:
    levels, native = registry()
    request = levels.request('DEBUG')
    requested = levels.effective
    levels.withdraw(request)
    assert (requested, levels.effective) == ('debug', None)
    assert native.applied == ['debug', None]


def test_the_most_verbose_live_request_applies() -> None:
    levels, native = registry()
    info = levels.request('info')
    trace = levels.request('trace')
    levels.request('warn')
    assert levels.effective == 'trace'
    levels.withdraw(trace)
    assert levels.effective == 'info'
    levels.withdraw(info)
    assert levels.effective == 'warn'
    assert native.applied == ['info', 'trace', 'trace', 'info', 'warn']


def test_withdrawing_twice_changes_nothing() -> None:
    levels, native = registry()
    request = levels.request('debug')
    levels.withdraw(request)
    levels.withdraw(request)
    assert native.applied == ['debug', None]


def test_an_unknown_level_is_rejected_by_name() -> None:
    levels, _ = registry()
    with pytest.raises(
        ValueError,
        match=r'native_log_level must be one of off, error, warn \(warning\), info, debug, trace, critical, fatal, '
        r"got 'verbose'",
    ):
        levels.request('verbose')


def test_the_python_spellings_are_normalized() -> None:
    levels, native = registry()
    for level in ('WARNING', 'Critical', 'FATAL', 'Off'):
        levels.withdraw(levels.request(level))
    assert native.applied == ['warn', None, 'error', None, 'error', None, 'off', None]


def test_off_ranks_below_error() -> None:
    levels, native = registry()
    off = levels.request('off')
    error = levels.request('error')
    assert levels.effective == 'error'
    levels.withdraw(error)
    assert levels.effective == 'off'
    levels.withdraw(off)
    assert native.applied == ['off', 'error', 'off', None]


def test_a_level_that_is_no_level_in_the_environment_is_reported_once() -> None:
    # In a fresh process, because the extension reports each rejected value once
    # per process: at import, and not again when two request/withdraw cycles
    # rebuild the filter.
    code = textwrap.dedent(
        """
        import json, logging
        records = []
        class Collect(logging.Handler):
            def emit(self, record):
                records.append((record.levelname, record.getMessage()))
        logging.getLogger('platynui.native').addHandler(Collect(level=1))
        logging.getLogger('platynui.native').setLevel(1)
        from platynui_native import _native, flush_logs
        from PlatynUI.core.native_logging import native_log_levels
        for _ in range(2):
            native_log_levels.withdraw(native_log_levels.request('debug'))
        _native._emit_log_for_tests('info', 'default hides info')
        _native._emit_log_for_tests('warn', 'default shows warnings')
        flush_logs()
        print(json.dumps(records))
        """
    )
    env = {k: v for k, v in os.environ.items() if k != 'RUST_LOG'} | {'PLATYNUI_LOG_LEVEL': 'verbose'}
    result = subprocess.run(
        [sys.executable, '-c', code], capture_output=True, text=True, timeout=60, env=env, check=False
    )
    assert result.returncode == 0, result.stderr
    records = [(str(level), str(message)) for level, message in json.loads(result.stdout)]
    [(level, message)] = [(level, m) for level, m in records if 'PLATYNUI_LOG_LEVEL' in m]
    assert level == 'WARNING'
    assert 'verbose' in message
    shown = ' | '.join(m for _, m in records)
    assert 'default shows warnings' in shown
    assert 'default hides info' not in shown


def test_an_extension_without_native_logging_asks_for_a_rebuild() -> None:
    levels = NativeLogLevels(cast(Any, SimpleNamespace()))
    with pytest.raises(RuntimeError, match='rebuild the native module'):
        levels.request('debug')


class Library(OurDynamicCore):
    def __init__(self) -> None:
        super().__init__([])

    @keyword
    def passes(self) -> str:
        return 'result'

    @keyword
    def fails(self) -> None:
        raise AssertionError('failed on purpose')


@pytest.fixture
def flushes(monkeypatch: pytest.MonkeyPatch) -> list[str]:
    calls: list[str] = []
    monkeypatch.setattr(_our_libcore, 'flush_native_logs', lambda: calls.append('flush'))
    return calls


def test_a_keyword_is_run_between_two_deliveries(flushes: list[str]) -> None:
    assert Library().run_keyword('passes', []) == 'result'
    assert flushes == ['flush', 'flush']


def test_a_failing_keyword_still_delivers_and_keeps_its_error(flushes: list[str]) -> None:
    with pytest.raises(AssertionError, match='failed on purpose'):
        Library().run_keyword('fails', [])
    assert flushes == ['flush', 'flush']
