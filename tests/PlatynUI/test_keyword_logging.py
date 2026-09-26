"""Keyword action lines without the Robot Framework runner (spec: *diagnostic-logging*)."""

import logging
import subprocess
import sys
import textwrap
from typing import Any

import pytest
from platynui_native import AttributeNotFoundError, Point

from PlatynUI.BareMetal import BareMetal, UiNodeDescriptor


class _Node:
    def __init__(self, calls: list[str]) -> None:
        self.calls = calls

    def describe(self) -> str:
        self.calls.append('describe')
        return 'Button "OK"'

    def attribute(self, name: str, namespace: str | None = None) -> Any:
        if name == 'ActivationPoint':
            return Point(1.0, 2.0)
        raise AttributeNotFoundError(name)


class _Runtime:
    def __init__(self, calls: list[str]) -> None:
        self.calls = calls

    def bring_to_front(self, node: Any) -> None:
        self.calls.append('bring_to_front')

    def pointer_click(self, point: Any, button: Any, overrides: Any) -> None:
        self.calls.append('pointer_click')


def _click(monkeypatch: pytest.MonkeyPatch) -> list[str]:
    calls: list[str] = []
    node = _Node(calls)
    monkeypatch.setattr(UiNodeDescriptor, 'resolve', lambda *_args, **_kwargs: node)
    library = BareMetal()
    library.__dict__['runtime'] = _Runtime(calls)
    library.pointer_click(UiNodeDescriptor(None, '//Button'))
    return calls


def test_the_element_is_described_before_the_action_at_debug(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    with caplog.at_level(logging.DEBUG, logger='platynui.baremetal'):
        calls = _click(monkeypatch)
    assert calls.index('describe') < calls.index('pointer_click'), calls
    assert [r.getMessage() for r in caplog.records if r.name == 'platynui.baremetal'] == [
        'clicked Button "OK" at (1, 2), its activation point; button LEFT, 1 click'
    ]


def test_the_element_is_not_described_at_info(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    with caplog.at_level(logging.INFO, logger='platynui.baremetal'):
        calls = _click(monkeypatch)
    assert 'describe' not in calls, calls
    assert 'pointer_click' in calls


_OLDER_ROBOT = textwrap.dedent(
    """
    import sys
    import types
    from typing import get_type_hints

    # Robot Framework before 7.4: robot.api.types without Secret. BuiltIn needs the rest.
    stub = types.ModuleType('robot.api.types')
    stub.KeywordArgument = type('KeywordArgument', (), {})
    stub.KeywordName = type('KeywordName', (str,), {})
    sys.modules['robot.api.types'] = stub

    from PlatynUI.BareMetal import BareMetal

    assert get_type_hints(BareMetal.keyboard_type)['text'] is str, get_type_hints(BareMetal.keyboard_type)
    library = BareMetal(use_mock=True)
    library.keyboard_type(None, 'abc')
    try:
        library.keyboard_type(None, 'Qz7<Kq9>w')
    except Exception as exc:
        assert 'Kq9' in str(exc), exc
    else:
        raise AssertionError('an unknown key name must fail')
    print('ok')
    """
)


def test_keyboard_keywords_work_without_secret() -> None:
    result = subprocess.run(
        [sys.executable, '-c', _OLDER_ROBOT], capture_output=True, text=True, timeout=120, check=False
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == 'ok'
