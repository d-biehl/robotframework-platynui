"""Native diagnostics reach Python ``logging`` (spec: *native-logging*).

The native core queues every enabled ``tracing`` event; queued records are
delivered to ``logging`` on the calling thread when a runtime call returns and
on :func:`platynui_native.flush_logs`. The hidden emitter
``_native._emit_log_for_tests`` exists only in the mock build these tests run
against. It emits like a native call does — on the calling thread, or on a
Rust thread the call joins while it holds the interpreter — and delivers when
it returns, as a runtime call does.
"""

import json
import logging
import os
import subprocess
import sys
import textwrap
from collections.abc import Generator

import platynui_native as pn
import pytest
from platynui_native import _native  # pyright: ignore[reportPrivateUsage]

NATIVE = 'platynui.native'
# Private and absent from the stubs on purpose; compiled into mock builds only.
emit = getattr(_native, '_emit_log_for_tests')  # noqa: B009
# The emitter lives in the extension's own logging module.
EMITTER_LOGGER = 'platynui.native.native.log_bridge'


@pytest.fixture(autouse=True)
def _native_logging(caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch) -> Generator[None, None, None]:
    """Start every test from the default level, an empty queue and a clean environment."""
    monkeypatch.delenv('RUST_LOG', raising=False)
    monkeypatch.delenv('PLATYNUI_LOG_LEVEL', raising=False)
    pn.set_log_level(None)
    pn.flush_logs()
    caplog.set_level(1, logger=NATIVE)
    caplog.clear()
    yield
    monkeypatch.delenv('RUST_LOG', raising=False)
    monkeypatch.delenv('PLATYNUI_LOG_LEVEL', raising=False)
    pn.set_log_level(None)
    pn.flush_logs()


def native_records(caplog: pytest.LogCaptureFixture) -> list[logging.LogRecord]:
    return [r for r in caplog.records if r.name == NATIVE or r.name.startswith(NATIVE + '.')]


def messages(caplog: pytest.LogCaptureFixture) -> list[str]:
    return [r.getMessage() for r in native_records(caplog)]


def test_a_native_warning_arrives_once_as_a_warning_with_its_fields(caplog: pytest.LogCaptureFixture) -> None:
    emit('warn', 'something happened')

    [record] = native_records(caplog)
    assert record.levelno == logging.WARNING
    assert record.name == EMITTER_LOGGER
    message = record.getMessage()
    assert message.startswith('[native.log_bridge] something happened'), message
    assert 'alpha=1' in message
    assert 'beta="two"' in message
    assert getattr(record, 'native_fields') == {'alpha': '1', 'beta': '"two"'}  # noqa: B009


def test_levels_map_one_to_one(caplog: pytest.LogCaptureFixture) -> None:
    pn.set_log_level('TRACE')
    for level in ('error', 'warn', 'info', 'debug', 'trace'):
        emit(level, f'at {level}')

    assert [r.levelno for r in native_records(caplog)] == [
        logging.ERROR,
        logging.WARNING,
        logging.INFO,
        logging.DEBUG,
        5,
    ]
    assert logging.getLevelName(5) == 'TRACE'


def test_only_warnings_and_errors_are_produced_by_default(caplog: pytest.LogCaptureFixture) -> None:
    for level in ('trace', 'debug', 'info', 'warn', 'error'):
        emit(level, f'at {level}')

    assert [r.levelno for r in native_records(caplog)] == [logging.WARNING, logging.ERROR]


def test_an_unknown_level_is_rejected_by_name() -> None:
    with pytest.raises(ValueError, match=r'verbose.*error, warn, info, debug, trace'):
        pn.set_log_level('verbose')


def test_the_xpath_trace_function_is_visible(caplog: pytest.LogCaptureFixture, rt_mock_platform: pn.Runtime) -> None:
    pn.set_log_level('debug')
    rt_mock_platform.evaluate("trace(1 + 2, 'sum')")

    [record] = [r for r in native_records(caplog) if 'fn:trace' in r.getMessage()]
    assert record.levelno == logging.DEBUG
    assert record.name == 'platynui.native.xpath.engine.functions.diagnostics'
    assert 'label=sum' in record.getMessage()
    assert 'value=3' in record.getMessage()


def test_a_thread_that_logs_is_delivered_on_the_calling_thread(caplog: pytest.LogCaptureFixture) -> None:
    emit('warn', 'from a background thread', on_background_thread=True)

    [record] = native_records(caplog)
    assert record.threadName == 'platynui-log-test'
    assert record.getMessage().startswith('[native.log_bridge] from a background thread')
    assert '(thread platynui-log-test, ' in record.getMessage()


def test_a_thread_that_logs_while_the_caller_waits_for_it_does_not_deadlock() -> None:
    # In a subprocess, so a regression fails on the timeout instead of hanging
    # the test run: the emitter joins its thread while holding the interpreter.
    code = textwrap.dedent(
        """
        from platynui_native import _native
        _native._emit_log_for_tests('warn', 'joined', on_background_thread=True)
        print('returned')
        """
    )
    result = subprocess.run([sys.executable, '-c', code], capture_output=True, text=True, timeout=60, check=False)
    assert result.returncode == 0, result.stderr
    assert 'returned' in result.stdout


def test_a_handler_that_calls_back_into_the_runtime(rt_mock_platform: pn.Runtime) -> None:
    pn.set_log_level('debug')
    seen: list[str] = []

    class CallingBack(logging.Handler):
        def emit(self, record: logging.LogRecord) -> None:
            seen.append(record.getMessage())
            if len(seen) == 1:
                rt_mock_platform.evaluate("trace(2, 'nested')")

    handler = CallingBack(level=1)
    logger = logging.getLogger(NATIVE)
    logger.addHandler(handler)
    try:
        rt_mock_platform.evaluate("trace(1, 'outer')")
    finally:
        logger.removeHandler(handler)

    traces = [m for m in seen if 'fn:trace' in m]
    assert len([m for m in traces if 'label=outer' in m]) == 1, seen
    assert len([m for m in traces if 'label=nested' in m]) == 1, seen


def test_a_flood_is_cut_off_and_reported(caplog: pytest.LogCaptureFixture) -> None:
    emit('warn', 'flood', count=10_005)

    records = messages(caplog)
    floods = [m for m in records if '] flood #' in m]
    assert len(floods) == 10_000
    assert [int(m.split('#')[1].split()[0]) for m in floods] == list(range(10_000)), 'delivered in emission order'
    [dropped] = [m for m in records if 'dropped' in m]
    assert '5 native log records were dropped' in dropped


def test_rust_log_takes_precedence_over_the_requested_level(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv('RUST_LOG', 'platynui_native=debug')
    pn.set_log_level('warn')
    emit('debug', 'wanted by RUST_LOG')

    assert any('wanted by RUST_LOG' in m for m in messages(caplog))


def test_the_requested_level_takes_precedence_over_platynui_log_level(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv('PLATYNUI_LOG_LEVEL', 'debug')
    pn.set_log_level(None)
    emit('debug', 'wanted by PLATYNUI_LOG_LEVEL')
    pn.set_log_level('warn')
    emit('debug', 'overruled by the request')

    assert any('wanted by PLATYNUI_LOG_LEVEL' in m for m in messages(caplog))
    assert not any('overruled by the request' in m for m in messages(caplog))


def test_an_invalid_environment_value_keeps_the_default_and_is_named(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv('PLATYNUI_LOG_LEVEL', 'platynui=loud')
    pn.set_log_level(None)
    emit('info', 'not produced')

    records = native_records(caplog)
    [rejected] = [r for r in records if 'PLATYNUI_LOG_LEVEL' in r.getMessage()]
    assert rejected.levelno == logging.WARNING
    assert 'platynui=loud' in rejected.getMessage()
    assert not any('not produced' in r.getMessage() for r in records)


def test_the_environment_is_honoured_from_import_on() -> None:
    code = textwrap.dedent(
        """
        import json, logging
        records = []
        class Collect(logging.Handler):
            def emit(self, record):
                records.append(record.getMessage())
        logging.getLogger('platynui.native').addHandler(Collect(level=1))
        logging.getLogger('platynui.native').setLevel(1)
        from platynui_native import _native
        _native._emit_log_for_tests('debug', 'debug from import on')
        print(json.dumps(records))
        """
    )
    env = {k: v for k, v in os.environ.items() if k != 'RUST_LOG'} | {'PLATYNUI_LOG_LEVEL': 'debug'}
    result = subprocess.run(
        [sys.executable, '-c', code], capture_output=True, text=True, timeout=60, env=env, check=False
    )
    assert result.returncode == 0, result.stderr
    assert any('debug from import on' in m for m in json.loads(result.stdout))
