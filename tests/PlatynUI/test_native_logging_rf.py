"""Native diagnostics in the Robot Framework log (spec: *native-logging*).

Runs ``robot/native_logging.robot`` in a separate process, so every run starts
with a fresh native logging state, and reads the result back from
``output.xml``. Needs the mock build of ``platynui_native`` (``just
test-python`` builds it), because the fixture uses the mock provider and the
hidden test emitter.
"""

import os
import re
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

import pytest
from robot.api import ExecutionResult
from robot.result import Keyword, Message, Result, TestCase as RobotTestCase

FIXTURE = Path(__file__).parent / 'robot' / 'native_logging.robot'
TIME = re.compile(r'\d{2}:\d{2}:\d{2}\.\d{3}')


def run_fixture(tmp_path: Path, *, native_log_level: str | None, loglevel: str) -> Result:
    args = [sys.executable, '-m', 'robot', '--outputdir', str(tmp_path), '--log', 'NONE', '--report', 'NONE']
    args += ['--loglevel', loglevel, '--consolewidth', '80']
    if native_log_level is not None:
        args += ['--variable', f'NATIVE_LOG_LEVEL:{native_log_level}']
    env = {k: v for k, v in os.environ.items() if k not in ('RUST_LOG', 'PLATYNUI_LOG_LEVEL')}
    subprocess.run([*args, str(FIXTURE)], capture_output=True, text=True, timeout=120, env=env, check=False)
    return ExecutionResult(str(tmp_path / 'output.xml'))


@pytest.fixture(scope='module')
def native_debug(tmp_path_factory: pytest.TempPathFactory) -> Result:
    return run_fixture(tmp_path_factory.mktemp('debug'), native_log_level='debug', loglevel='DEBUG')


@pytest.fixture(scope='module')
def native_debug_rf_info(tmp_path_factory: pytest.TempPathFactory) -> Result:
    return run_fixture(tmp_path_factory.mktemp('info'), native_log_level='debug', loglevel='INFO')


@pytest.fixture(scope='module')
def native_default(tmp_path_factory: pytest.TempPathFactory) -> Result:
    return run_fixture(tmp_path_factory.mktemp('default'), native_log_level=None, loglevel='DEBUG')


def case_named(result: Result, name: str) -> RobotTestCase:
    [case] = [t for t in result.suite.all_tests if t.name == name]
    assert case.passed, f'{name}: {case.message}'
    return case


def keywords(case: RobotTestCase, name: str) -> list[Keyword]:
    return [item for item in case.body if isinstance(item, Keyword) and item.name == name]


def messages(keyword: Keyword) -> Iterator[Message]:
    return (item for item in keyword.body if isinstance(item, Message))


def text(message: Message) -> str:
    return message.message or ''


def test_a_native_trace_is_logged_inside_the_query_that_evaluated_it(native_debug: Result) -> None:
    [query] = keywords(case_named(native_debug, 'A Native Trace Is Logged Inside The Query That Evaluated It'), 'Query')
    [trace] = [m for m in messages(query) if 'fn:trace' in text(m)]
    assert trace.level == 'DEBUG'
    assert 'label=node-count' in text(trace)


def test_a_warning_on_the_calling_thread_is_an_rf_warning(native_debug: Result) -> None:
    [evaluate] = keywords(case_named(native_debug, 'A Warning On The Calling Thread Is An RF Warning'), 'Evaluate')
    [warning] = [m for m in messages(evaluate) if 'warning on the calling thread' in text(m)]
    assert warning.level == 'WARN'
    assert any('warning on the calling thread' in text(m) for m in native_debug.errors.messages)


def test_a_background_threads_warning_names_its_thread(native_debug: Result) -> None:
    case = case_named(native_debug, "A Background Thread's Warning Names Its Thread")
    [evaluate] = keywords(case, 'Evaluate')
    [warning] = [m for m in messages(evaluate) if 'warning from a background thread' in text(m)]
    assert warning.level == 'WARN'
    assert '(thread platynui-log-test, ' in text(warning)
    assert TIME.search(text(warning)), text(warning)


def test_rfs_own_log_level_still_applies(native_debug_rf_info: Result) -> None:
    before, after = keywords(case_named(native_debug_rf_info, "RF's Own Log Level Still Applies"), 'Query')
    assert not any('before-set-log-level' in text(m) for m in messages(before))
    assert any('after-set-log-level' in text(m) and m.level == 'DEBUG' for m in messages(after))


def test_without_native_log_level_only_warnings_are_produced(native_default: Result) -> None:
    [query] = keywords(
        case_named(native_default, 'A Native Trace Is Logged Inside The Query That Evaluated It'), 'Query'
    )
    assert not any('fn:trace' in text(m) for m in messages(query))
    [evaluate] = keywords(case_named(native_default, 'A Warning On The Calling Thread Is An RF Warning'), 'Evaluate')
    assert any(m.level == 'WARN' and 'warning on the calling thread' in text(m) for m in messages(evaluate))


def test_an_invalid_native_log_level_fails_the_import_by_name(tmp_path: Path) -> None:
    result = run_fixture(tmp_path, native_log_level='verbose', loglevel='INFO')
    errors = [text(m) for m in result.errors.messages if m.level == 'ERROR']
    assert any('native_log_level' in e and 'error, warn, info, debug, trace' in e for e in errors), errors
