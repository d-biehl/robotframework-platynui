"""Native log-level requests and keyword-boundary delivery (spec: *native-logging*)."""

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
        ValueError, match=r"native_log_level must be one of error, warn, info, debug, trace, got 'verbose'"
    ):
        levels.request('verbose')


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
