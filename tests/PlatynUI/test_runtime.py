# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for ``PlatynUI.core.runtime``."""

from __future__ import annotations

from typing import Any
from unittest.mock import MagicMock

import pytest

from PlatynUI.core.runtime import Runtime, runtime


@pytest.fixture(autouse=True)
def _reset_runtime() -> Any:  # pyright: ignore[reportUnusedFunction]
    """Ensure each test starts and ends with a clean runtime singleton."""
    runtime.reset()
    yield
    runtime.reset()


def test_runtime_singleton_is_module_level() -> None:
    """``runtime`` is a singleton accessor of type ``Runtime``."""
    assert isinstance(runtime, Runtime)


def test_is_initialised_false_before_first_access() -> None:
    """No native runtime is created until ``current`` is read or ``set``."""
    assert runtime.is_initialised() is False


def test_set_replaces_active_runtime() -> None:
    """``set`` installs the given object; ``current`` returns it."""
    fake = MagicMock(name='NativeRuntime')
    runtime.set(fake)
    assert runtime.is_initialised() is True
    assert runtime.current is fake


def test_set_does_not_shutdown_previous() -> None:
    """``set`` does *not* call ``shutdown`` on the displaced runtime."""
    first = MagicMock(name='first')
    second = MagicMock(name='second')
    runtime.set(first)
    runtime.set(second)
    first.shutdown.assert_not_called()
    assert runtime.current is second


def test_reset_calls_shutdown_and_clears() -> None:
    """``reset`` invokes ``shutdown`` and drops the reference."""
    fake = MagicMock(name='NativeRuntime')
    runtime.set(fake)
    runtime.reset()
    fake.shutdown.assert_called_once_with()
    assert runtime.is_initialised() is False


def test_reset_swallows_shutdown_exceptions() -> None:
    """A failing ``shutdown`` does not propagate from ``reset``."""
    fake = MagicMock(name='NativeRuntime')
    fake.shutdown.side_effect = RuntimeError('boom')
    runtime.set(fake)
    runtime.reset()  # must not raise
    assert runtime.is_initialised() is False


def test_reset_on_uninitialised_is_noop() -> None:
    """Calling ``reset`` without a prior ``set`` does nothing."""
    runtime.reset()
    assert runtime.is_initialised() is False


def test_current_lazy_creates_default(monkeypatch: pytest.MonkeyPatch) -> None:
    """``current`` constructs a default ``platynui_native.Runtime`` lazily."""
    sentinel = MagicMock(name='DefaultNativeRuntime')
    factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    assert runtime.is_initialised() is False
    obtained = runtime.current
    factory.assert_called_once_with()
    assert obtained is sentinel
    assert runtime.is_initialised() is True


def test_current_caches_lazy_instance(monkeypatch: pytest.MonkeyPatch) -> None:
    """The lazy default is created exactly once and reused."""
    sentinel = MagicMock(name='DefaultNativeRuntime')
    factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    first = runtime.current
    second = runtime.current
    assert first is second
    factory.assert_called_once_with()


def test_set_overrides_lazy_default(monkeypatch: pytest.MonkeyPatch) -> None:
    """An explicit ``set`` after lazy creation replaces the cached value."""
    sentinel = MagicMock(name='DefaultNativeRuntime')
    factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    _ = runtime.current  # trigger lazy creation
    replacement = MagicMock(name='Replacement')
    runtime.set(replacement)
    assert runtime.current is replacement


def test_reset_then_current_creates_fresh_default(monkeypatch: pytest.MonkeyPatch) -> None:
    """After ``reset``, ``current`` creates a brand-new default runtime."""
    factory = MagicMock(side_effect=[MagicMock(name='first'), MagicMock(name='second')])
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    first = runtime.current
    runtime.reset()
    second = runtime.current
    assert first is not second
    assert factory.call_count == 2


def test_runtime_class_alias_matches_singleton_type() -> None:
    """The exported ``Runtime`` symbol is the singleton's class."""
    assert isinstance(runtime, Runtime)
    assert type(runtime) is Runtime
