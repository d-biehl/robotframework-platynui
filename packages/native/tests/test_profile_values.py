"""Profile, settings and overrides dicts (spec: *diagnostic-logging*, configuration mistakes).

A value of the wrong type raises ``TypeError`` naming the key and the expected
type, like the fields that already raised (``origin``, ``motion``). A key that no
field reads is reported as a warning through the log bridge, delivered by the
call that received the dict, and the call proceeds.
"""

import logging
from collections.abc import Callable, Generator
from typing import Any

import platynui_native as pn
import pytest

NATIVE = 'platynui.native'


@pytest.fixture(autouse=True)
def _native_warnings(caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch) -> Generator[None, None, None]:
    """Start from the default level (warnings only) and an empty queue."""
    monkeypatch.delenv('RUST_LOG', raising=False)
    monkeypatch.delenv('PLATYNUI_LOG_LEVEL', raising=False)
    pn.set_log_level(None)
    pn.flush_logs()
    caplog.set_level(logging.WARNING, logger=NATIVE)
    caplog.clear()
    yield
    pn.flush_logs()


def unchecked(value: object) -> Any:
    """Hand a deliberately mistyped value past the type checker."""
    return value


def unknown_key_warnings(caplog: pytest.LogCaptureFixture) -> list[str]:
    return [
        r.getMessage()
        for r in caplog.records
        if r.name.startswith(NATIVE) and r.levelno == logging.WARNING and ' key; it is ignored' in r.getMessage()
    ]


# Each kind of dict, handed to the call that receives it.
Apply = Callable[[pn.Runtime, Any], object]
KINDS: dict[str, Apply] = {
    'pointer profile': lambda rt, d: rt.set_pointer_profile(d),
    'pointer settings': lambda rt, d: rt.set_pointer_settings(d),
    'pointer overrides': lambda rt, d: rt.pointer_move_to((5.0, 5.0), overrides=d),
    'keyboard profile': lambda rt, d: rt.set_keyboard_profile(d),
    'keyboard overrides': lambda rt, d: rt.keyboard_type('a', overrides=d),
}
# A key each kind reads as a number.
NUMBER_KEY = {
    'pointer profile': 'after_click_delay_ms',
    'pointer settings': 'double_click_time_ms',
    'pointer overrides': 'after_click_delay_ms',
    'keyboard profile': 'press_delay_ms',
    'keyboard overrides': 'press_delay_ms',
}


def test_a_profile_value_of_the_wrong_type_names_the_key_and_the_expected_number(
    rt_mock_platform: pn.Runtime,
) -> None:
    with pytest.raises(TypeError, match=r'after_click_delay_ms must be a number, got str .100ms.') as raised:
        rt_mock_platform.set_pointer_profile(unchecked({'after_click_delay_ms': '100ms'}))
    assert 'variant' not in str(raised.value), 'the error is not buried in a list of alternatives'


@pytest.mark.parametrize('kind', KINDS)
def test_every_kind_of_dict_rejects_a_wrong_type(kind: str, rt_mock_platform: pn.Runtime) -> None:
    key = NUMBER_KEY[kind]
    with pytest.raises(TypeError, match=rf'{key} must be a number, got str'):
        KINDS[kind](rt_mock_platform, {key: 'soon'})


@pytest.mark.parametrize(
    ('value', 'expected'),
    [
        ({'ensure_move_position': 'yes'}, r'ensure_move_position must be a bool, got str'),
        ({'overshoot_settle_steps': 2.5}, r'overshoot_settle_steps must be a non-negative integer, got float'),
        ({'scroll_step': [0, -10]}, r'scroll_step must be a tuple of two numbers, got list'),
    ],
)
def test_flags_counts_and_pairs_are_typed_too(
    value: dict[str, Any], expected: str, rt_mock_platform: pn.Runtime
) -> None:
    with pytest.raises(TypeError, match=expected):
        rt_mock_platform.set_pointer_profile(unchecked(value))


def test_a_default_button_of_the_wrong_type_is_rejected(rt_mock_platform: pn.Runtime) -> None:
    with pytest.raises(TypeError, match=r'default_button must be a PointerButton or a button number, got str'):
        rt_mock_platform.set_pointer_settings(unchecked({'default_button': 'left'}))


def test_from_like_rejects_a_wrong_type_without_a_runtime() -> None:
    with pytest.raises(TypeError, match=r'after_click_delay_ms must be a number'):
        pn.PointerProfile.from_like(unchecked({'after_click_delay_ms': '100ms'}))


def test_a_misspelled_profile_key_warns_once_and_the_call_proceeds(
    caplog: pytest.LogCaptureFixture, rt_mock_platform: pn.Runtime
) -> None:
    before = rt_mock_platform.pointer_profile().speed_factor
    rt_mock_platform.set_pointer_profile(unchecked({'speed_factr': 2, 'after_click_delay_ms': 7}))

    [warning] = unknown_key_warnings(caplog)
    assert 'speed_factr' in warning
    assert 'pointer profile' in warning
    profile = rt_mock_platform.pointer_profile()
    assert profile.speed_factor == before
    assert profile.after_click_delay_ms == 7


@pytest.mark.parametrize('kind', KINDS)
def test_every_kind_of_dict_warns_for_an_unknown_key(
    kind: str, caplog: pytest.LogCaptureFixture, rt_mock_platform: pn.Runtime
) -> None:
    KINDS[kind](rt_mock_platform, {'bogus_key': 1})

    [warning] = unknown_key_warnings(caplog)
    assert 'bogus_key' in warning
    assert kind in warning


def test_the_warning_is_delivered_by_the_call_that_received_the_dict(caplog: pytest.LogCaptureFixture) -> None:
    # `from_like` is no runtime call, and nothing flushes before the assertion.
    pn.KeyboardProfile.from_like(unchecked({'press_delay': 5}))

    [warning] = unknown_key_warnings(caplog)
    assert 'press_delay' in warning


def test_none_counts_as_absent(caplog: pytest.LogCaptureFixture, rt_mock_platform: pn.Runtime) -> None:
    before = rt_mock_platform.pointer_profile().speed_factor
    rt_mock_platform.set_pointer_profile(unchecked({'speed_factor': None}))

    assert rt_mock_platform.pointer_profile().speed_factor == before
    assert unknown_key_warnings(caplog) == []


def test_the_classes_are_still_accepted(caplog: pytest.LogCaptureFixture, rt_mock_platform: pn.Runtime) -> None:
    rt_mock_platform.set_pointer_profile(pn.PointerProfile(after_click_delay_ms=9))
    rt_mock_platform.keyboard_type('a', overrides=pn.KeyboardOverrides(press_delay_ms=1))

    assert rt_mock_platform.pointer_profile().after_click_delay_ms == 9
    assert unknown_key_warnings(caplog) == []


def test_neither_a_dict_nor_the_class_is_a_type_error(rt_mock_platform: pn.Runtime) -> None:
    with pytest.raises(TypeError):
        rt_mock_platform.set_pointer_profile(unchecked('fast'))
