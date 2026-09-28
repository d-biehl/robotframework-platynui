# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""When a `PlatynUI.core` context reads the UI again (spec *ui-context-lookup*).

A context's lookup asks the runtime's snapshot. After a miss, and before each
retry, the snapshot is discarded, so the next attempt reads the current UI; a
lookup that succeeds keeps it. The contexts are driven by a stub factory that
records every lookup and every discarded snapshot in one event list, and
`RuntimeAdapterFactory` by a runtime wrapper that counts ``clear_cache``.
"""

from collections.abc import Generator
from typing import Any
from unittest.mock import MagicMock

import platynui_native as _pn
import pytest

from PlatynUI.core import Settings
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, RuntimeAdapterFactory, adapter_factory
from PlatynUI.core.adapters import UiNodeAdapter
from PlatynUI.core.context import ContextBase
from PlatynUI.core.exceptions import AdapterNotFoundError, CannotEnsureError
from PlatynUI.core.locator import Locator
from PlatynUI.core.runtime import runtime


@pytest.fixture(autouse=True)
def _fast_settings() -> Generator[None]:  # pyright: ignore[reportUnusedFunction]
    previous = Settings.current()
    Settings.set_current(Settings(ensure_timeout=0.5, ensure_delay=0.01, exists_timeout=0.2))
    try:
        yield
    finally:
        Settings.set_current(previous)


def _adapter(name: str = 'OK') -> Adapter:
    a = MagicMock(spec=Adapter)
    a.role = 'Button'
    a.name = name
    a.valid = True
    return a


class _RecordingFactory(AdapterFactory):
    """Answers lookups from a script of results and records what happens."""

    def __init__(self, *answers: Adapter | None) -> None:
        self.answers = list(answers)
        self.events: list[str] = []

    def _next(self) -> Adapter | None:
        if len(self.answers) > 1:
            return self.answers.pop(0)
        return self.answers[0] if self.answers else None

    def find_one(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> Adapter | None:
        del parent, locator, parent_is_root_like, default_role, default_prefix
        found = self._next()
        self.events.append('found' if found is not None else 'missed')
        return found

    def find_all(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> list[Adapter]:
        del parent, locator, parent_is_root_like, default_role, default_prefix
        found = self._next()
        self.events.append('found' if found is not None else 'missed')
        return [found] if found is not None else []

    def discard_snapshot(self) -> None:
        self.events.append('discard')


def _child_of_root() -> ContextBase:
    """A context whose parent resolves at once, so only its own lookup varies."""
    root = ContextBase(adapter=_adapter('root'))
    return ContextBase(Locator(role='Button'), context_parent=root)


def _lookups(events: list[str]) -> list[str]:
    return [event for event in events if event != 'discard']


# ----------------------------------------------------------------------
# A failed attempt is retried against the current UI
# ----------------------------------------------------------------------


def test_an_element_that_appears_on_the_third_attempt_is_found() -> None:
    factory = _RecordingFactory(None, None, _adapter())
    with adapter_factory.override(lambda: factory):
        assert _child_of_root().exists(timeout=1.0) is True
    assert factory.events == ['missed', 'discard', 'missed', 'discard', 'found']


def test_every_retry_reads_the_current_ui() -> None:
    factory = _RecordingFactory(None, None, None, _adapter())
    with adapter_factory.override(lambda: factory):
        assert _child_of_root().exists(timeout=1.0) is True
    for index, event in enumerate(factory.events):
        if event != 'discard' and index > 0:
            assert factory.events[index - 1] == 'discard', f'a retry without a discarded snapshot: {factory.events}'


def test_a_failed_condition_is_retried_against_the_current_ui() -> None:
    factory = _RecordingFactory(_adapter())
    outcomes = iter([False, False, True])
    with adapter_factory.override(lambda: factory):
        ctx = _child_of_root()
        assert ctx.ensure_that(lambda: next(outcomes), timeout=1.0) is True
    assert factory.events.count('discard') == 2, factory.events


# ----------------------------------------------------------------------
# A lookup that gives up leaves no snapshot behind
# ----------------------------------------------------------------------


def test_an_existence_check_that_gives_up_leaves_no_snapshot_behind() -> None:
    factory = _RecordingFactory(None)
    with adapter_factory.override(lambda: factory):
        ctx = _child_of_root()
        assert ctx.exists(timeout=0.05) is False
        assert factory.events[-1] == 'discard', factory.events

        factory.answers = [_adapter()]
        assert ctx.exists(timeout=0.05) is True, 'the element that appeared meanwhile is found'


def test_a_check_that_raises_leaves_no_snapshot_behind() -> None:
    factory = _RecordingFactory(None)
    with adapter_factory.override(lambda: factory):
        ctx = _child_of_root()
        with pytest.raises(CannotEnsureError):
            ctx.ensure_that(ctx._adapter_exists, timeout=0.05)
    # The last attempt's lookup, then the discard of the give-up path; a retry
    # never follows the last attempt.
    assert factory.events[-2:] == ['missed', 'discard'], factory.events


# ----------------------------------------------------------------------
# A lookup that succeeds keeps the snapshot
# ----------------------------------------------------------------------


def test_successive_successful_lookups_share_a_snapshot() -> None:
    factory = _RecordingFactory(_adapter())
    with adapter_factory.override(lambda: factory):
        root = ContextBase(adapter=_adapter('root'))
        first = ContextBase(Locator(role='Button', name='A'), context_parent=root)
        second = ContextBase(Locator(role='Button', name='B'), context_parent=root)
        assert first.exists(timeout=0.5) is True
        assert second.exists(timeout=0.5) is True
    assert _lookups(factory.events) == ['found', 'found']
    assert 'discard' not in factory.events


# ----------------------------------------------------------------------
# RuntimeAdapterFactory discards on a miss, against the mock runtime
# ----------------------------------------------------------------------


class _CountingRuntime:
    """Delegates to a runtime and counts ``clear_cache``; its shutdown is the owner's."""

    def __init__(self, inner: _pn.Runtime) -> None:
        self._inner = inner
        self.cleared = 0

    def clear_cache(self) -> None:
        self.cleared += 1
        self._inner.clear_cache()

    def shutdown(self) -> None:
        """The wrapped runtime belongs to the enclosing override."""

    def __getattr__(self, name: str) -> Any:
        return getattr(self._inner, name)


@pytest.fixture
def counting_runtime() -> Generator[_CountingRuntime]:
    with runtime.override_with_mock() as native:
        counting = _CountingRuntime(native)
        with runtime.override(lambda: counting):  # type: ignore[arg-type,return-value]
            yield counting


@pytest.fixture
def desktop(counting_runtime: _CountingRuntime) -> UiNodeAdapter:
    del counting_runtime
    return UiNodeAdapter.create_root()


def test_find_one_discards_the_snapshot_only_on_a_miss(
    counting_runtime: _CountingRuntime, desktop: UiNodeAdapter
) -> None:
    factory = RuntimeAdapterFactory()
    assert factory.find_one(desktop, Locator(path="//control:Window[@Name='Operations Console']")) is not None
    assert counting_runtime.cleared == 0, 'a lookup that found its element keeps the snapshot'

    assert factory.find_one(desktop, Locator(path="//control:Window[@Name='Does Not Exist']")) is None
    assert counting_runtime.cleared == 1, 'a lookup that found nothing leaves no snapshot behind'


def test_find_all_discards_the_snapshot_only_on_a_miss(
    counting_runtime: _CountingRuntime, desktop: UiNodeAdapter
) -> None:
    factory = RuntimeAdapterFactory()
    assert factory.find_all(desktop, Locator(path='//control:Button'))
    assert counting_runtime.cleared == 0

    assert factory.find_all(desktop, Locator(path='//control:Button[@Name="ZZZ"]')) == []
    assert counting_runtime.cleared == 1


def test_single_lookups_that_find_nothing_discard_the_snapshot_and_report_as_before(
    counting_runtime: _CountingRuntime, desktop: UiNodeAdapter
) -> None:
    root = ContextBase(adapter=desktop)
    missing = Locator(path='//control:Button[@Name="ZZZ"]')

    assert root.get_all(ContextBase, locator=missing) == []
    assert list(root.iter_all(ContextBase, locator=missing)) == []
    with pytest.raises(AdapterNotFoundError):
        root.get_one(ContextBase, locator=missing)
    assert counting_runtime.cleared == 3, 'each of the three lookups discarded the snapshot'
