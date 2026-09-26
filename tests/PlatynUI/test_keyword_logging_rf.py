"""Keyword log lines, typed text and error texts in the Robot Framework log (spec: *diagnostic-logging*).

Runs ``robot/keyword_logging.robot`` in a separate process per configuration, so every run starts with
a fresh native logging state, and reads the result back from ``output.xml``. Needs the mock build of
``platynui_native`` (``just test-python`` builds it).
"""

import os
import re
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest
from robot.api import ExecutionResult
from robot.result import Keyword, Message, Result, TestCase as RobotTestCase

FIXTURE = Path(__file__).parent / 'robot' / 'keyword_logging.robot'
TEXT = 'zq8wx'
SECRET = 'sx4vy'
BAD = 'Qz7<Kq9>w'
NATIVE = re.compile(r'^\[[\w.:]+\] ')


def run_fixture(tmp_path: Path, *, native_log_level: str | None, loglevel: str, tests: list[str]) -> Result:
    args = [sys.executable, '-m', 'robot', '--outputdir', str(tmp_path), '--log', 'NONE', '--report', 'NONE']
    args += ['--loglevel', loglevel, '--consolewidth', '80']
    if native_log_level is not None:
        args += ['--variable', f'NATIVE_LOG_LEVEL:{native_log_level}']
    for test in tests:
        args += ['--test', test]
    env = {k: v for k, v in os.environ.items() if k not in ('RUST_LOG', 'PLATYNUI_LOG_LEVEL')}
    env |= {'PLATYNUI_TEST_SECRET': SECRET, 'PLATYNUI_TEST_BAD_SECRET': BAD}
    subprocess.run([*args, str(FIXTURE)], capture_output=True, text=True, timeout=120, env=env, check=False)
    return ExecutionResult(str(tmp_path / 'output.xml'))


def output_xml(result: Result) -> str:
    return Path(str(result.source)).read_text(encoding='utf-8')


@pytest.fixture(scope='module')
def rf_debug(tmp_path_factory: pytest.TempPathFactory) -> Result:
    tests = [
        'A Click Is Traced',
        'An Unknown Key Name Says Why',
        'An Unknown Key Name In A Secret Says Only Where',
        'A Single Backslash Before A Bracket Is Consumed By Robot Framework',
        'A Doubled Backslash Types The Bracket',
        'An Assertion Without A Message',
        'A Failed Activation Is Traced',
        'A Vanished Root Is Named As The Cause',
    ]
    return run_fixture(tmp_path_factory.mktemp('debug'), native_log_level=None, loglevel='DEBUG', tests=tests)


@pytest.fixture(scope='module')
def rf_info(tmp_path_factory: pytest.TempPathFactory) -> Result:
    tests = ['A Click Is Traced', 'Ten Clicks Add No Info Record']
    return run_fixture(tmp_path_factory.mktemp('info'), native_log_level=None, loglevel='INFO', tests=tests)


@pytest.fixture(scope='module')
def native_info(tmp_path_factory: pytest.TempPathFactory) -> Result:
    tests = ['Ten Clicks Add No Info Record']
    return run_fixture(tmp_path_factory.mktemp('native-info'), native_log_level='info', loglevel='INFO', tests=tests)


@pytest.fixture(scope='module')
def typing_debug(tmp_path_factory: pytest.TempPathFactory) -> Result:
    tests = ['Typing Is Traced Without Its Text', 'A Secret Is Typed Without Being Shown']
    return run_fixture(tmp_path_factory.mktemp('typing'), native_log_level='debug', loglevel='TRACE', tests=tests)


@pytest.fixture(scope='module')
def typing_trace(tmp_path_factory: pytest.TempPathFactory) -> Result:
    tests = ['Typing Is Traced Without Its Text', 'A Secret Is Typed Without Being Shown']
    return run_fixture(tmp_path_factory.mktemp('trace'), native_log_level='trace', loglevel='TRACE', tests=tests)


def case_named(result: Result, name: str) -> RobotTestCase:
    [case] = [t for t in result.suite.all_tests if t.name == name]
    return case


def passed(result: Result, name: str) -> RobotTestCase:
    case = case_named(result, name)
    assert case.passed, f'{name}: {case.message}'
    return case


def failed(result: Result, name: str) -> str:
    case = case_named(result, name)
    assert case.failed, f'{name} passed'
    return str(case.message)


def all_keywords(item: Any) -> Iterator[Keyword]:
    for child in item.body:
        if isinstance(child, Keyword):
            yield child
            yield from all_keywords(child)
        elif hasattr(child, 'body'):
            yield from all_keywords(child)  # FOR, IF, ... and their iterations


def keywords(case: RobotTestCase, name: str) -> list[Keyword]:
    return [kw for kw in all_keywords(case) if kw.name == name]


def messages(keyword: Keyword) -> list[Message]:
    return [item for item in keyword.body if isinstance(item, Message)]


def text(message: Message) -> str:
    return message.message or ''


def action_lines(keyword: Keyword) -> list[Message]:
    """The keyword's own action line: a DEBUG message that is neither native nor RF's own."""
    return [
        m
        for m in messages(keyword)
        if m.level == 'DEBUG' and not NATIVE.match(text(m)) and not text(m).startswith(('Arguments:', 'Return:'))
    ]


# ---- Keyword action lines -------------------------------------------------------------------


def test_a_click_is_traced_at_debug(rf_debug: Result) -> None:
    [click] = keywords(passed(rf_debug, 'A Click Is Traced'), 'Pointer Click')
    [line] = action_lines(click)
    assert text(line) == 'clicked Button "OK" at (200, 636), its activation point; button LEFT, 1 click'


def test_no_action_line_at_the_default_level(rf_info: Result) -> None:
    [click] = keywords(passed(rf_info, 'A Click Is Traced'), 'Pointer Click')
    assert not any(text(m).startswith('clicked') for m in messages(click))


# ---- Info records mark lifecycle, not operations ----------------------------------------------


def test_no_native_info_record_by_default(rf_info: Result) -> None:
    case = passed(rf_info, 'Ten Clicks Add No Info Record')
    assert not any(m.level == 'INFO' and NATIVE.match(text(m)) for kw in all_keywords(case) for m in messages(kw))


def test_native_info_names_the_runtime_and_no_click_adds_one(native_info: Result) -> None:
    case = passed(native_info, 'Ten Clicks Add No Info Record')
    [query] = keywords(case, 'Query')
    [initialized] = [m for m in messages(query) if 'runtime initialized' in text(m)]
    assert initialized.level == 'INFO'
    assert 'backend="mock"' in text(initialized), text(initialized)
    assert 'mock' in text(initialized).split('providers=', 1)[1], text(initialized)
    clicks = keywords(case, 'Pointer Click')
    assert len(clicks) == 10
    assert not any(m.level == 'INFO' and NATIVE.match(text(m)) for kw in clicks for m in messages(kw))


# ---- Typed text ---------------------------------------------------------------------------------


def test_typing_logs_the_length_and_never_the_text(typing_debug: Result) -> None:
    [typed] = keywords(passed(typing_debug, 'Typing Is Traced Without Its Text'), 'Keyboard Type')
    [line] = action_lines(typed)
    assert text(line) == f'typed into the focused element; {len(TEXT)} characters'
    own = [text(m) for m in messages(typed) if NATIVE.match(text(m)) or m in action_lines(typed)]
    assert own, 'the native debug records of the keyboard are there'
    assert not any(TEXT in t for t in own), own
    assert not any('mock-keyboard: press' in t or 'mock-keyboard: release' in t for t in own), own


def test_a_secret_is_typed_and_never_shown(typing_debug: Result) -> None:
    [typed] = keywords(passed(typing_debug, 'A Secret Is Typed Without Being Shown'), 'Keyboard Type')
    [line] = action_lines(typed)
    assert text(line) == 'typed into the focused element; secret'
    assert SECRET not in output_xml(typing_debug)


def pressed_keys(keyword: Keyword) -> str:
    keys = [re.search(r' key=(\S+)', text(m)) for m in messages(keyword) if 'mock-keyboard: press' in text(m)]
    return ''.join(k.group(1) for k in keys if k is not None)


def test_key_records_moved_to_trace(typing_trace: Result) -> None:
    [typed] = keywords(passed(typing_trace, 'Typing Is Traced Without Its Text'), 'Keyboard Type')
    assert pressed_keys(typed).lower() == TEXT
    [secret] = keywords(passed(typing_trace, 'A Secret Is Typed Without Being Shown'), 'Keyboard Type')
    assert pressed_keys(secret).lower() == SECRET, 'the secret was typed'


def test_an_unknown_key_name_says_why_without_the_text(rf_debug: Result) -> None:
    message = failed(rf_debug, 'An Unknown Key Name Says Why')
    assert 'Kq9' in message
    assert 'position 5' in message, message
    assert r'\<' in message, message
    assert r'\\<' in message, message
    assert 'Qz7' not in message


def test_an_unknown_key_name_in_a_secret_says_only_where(rf_debug: Result) -> None:
    message = failed(rf_debug, 'An Unknown Key Name In A Secret Says Only Where')
    assert 'position 5' in message, message
    assert 'Kq9' not in message, message
    assert 'Qz7' not in message, message
    assert BAD not in output_xml(rf_debug)


def test_robot_frameworks_own_escape_is_explained(rf_debug: Result) -> None:
    message = failed(rf_debug, 'A Single Backslash Before A Bracket Is Consumed By Robot Framework')
    assert r'\\<' in message, message
    passed(rf_debug, 'A Doubled Backslash Types The Bracket')


# ---- Errors and fallbacks -----------------------------------------------------------------------


def test_an_assertion_without_a_message_does_not_start_with_none(rf_debug: Result) -> None:
    message = failed(rf_debug, 'An Assertion Without A Message')
    assert not message.startswith('None'), message


def test_a_failed_activation_is_traced_and_the_click_proceeds(rf_debug: Result) -> None:
    [click] = keywords(passed(rf_debug, 'A Failed Activation Is Traced'), 'Pointer Click')
    [activation] = [m for m in messages(click) if 'to the front' in text(m)]
    assert activation.level == 'DEBUG'
    assert text(activation).startswith('could not bring Desktop "'), text(activation)
    assert any(text(m).startswith('clicked Desktop "') for m in action_lines(click))


def test_a_vanished_root_is_named_as_the_cause(rf_debug: Result) -> None:
    message = failed(rf_debug, 'A Vanished Root Is Named As The Cause')
    expected = 'RootNotFoundError: The root set by Set Root, \'//Window[@Name="Does Not Exist"]\', was not found'
    assert message.startswith(expected), message
    assert 'within timeout of 0.5 seconds' in message, message
    assert message.endswith('\'.//Button[@Name="OK"]\' was not evaluated.'), message
    assert 'UiNodeDescriptor' not in message
