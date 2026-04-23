# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.ensure`` and ``PlatynUI.core.predicate``."""

from __future__ import annotations

import pytest

from PlatynUI.core import (
    CannotEnsureError,
    PlatynUIFatalError,
    Settings,
    ensure_that,
    predicate,
)


@pytest.fixture(autouse=True)
def _fast_settings() -> None:  # pyright: ignore[reportUnusedFunction]
    Settings.set_current(Settings(ensure_timeout=0.5, ensure_delay=0.01))


def test_predicate_decorator_attaches_metadata() -> None:
    @predicate('element {0} is great')
    def p() -> bool:
        return True

    assert p.__predicate__ is True  # type: ignore[attr-defined]
    assert p.message == 'element {0} is great'  # type: ignore[attr-defined]


def test_ensure_succeeds_immediately() -> None:
    assert ensure_that(object(), lambda: True, lambda: True) is True


def test_ensure_skips_already_succeeded_predicates_when_all_pass() -> None:
    """Once every predicate has been seen as True in the same iteration,
    the memo prevents a redundant re-check before returning."""
    calls = {'a': 0, 'b': 0}

    def a() -> bool:
        calls['a'] += 1
        return True

    def b() -> bool:
        calls['b'] += 1
        return True

    assert ensure_that(object(), a, b, timeout=1.0) is True
    # Both pass in the first iteration; succeeded_predicates ends with both
    # entries, but ensure_that returns immediately after the loop.
    assert calls['a'] == 1
    assert calls['b'] == 1


def test_ensure_resets_memo_when_a_later_predicate_fails() -> None:
    a_calls = 0
    b_returns = iter([False, True])

    def a() -> bool:
        nonlocal a_calls
        a_calls += 1
        return True

    def b() -> bool:
        return next(b_returns)

    assert ensure_that(object(), a, b, timeout=1.0) is True
    # Iteration 1: a=True (memo+a), b=False → memo cleared.
    # Iteration 2: a=True (re-checked), b=True → success.
    # → a was called twice.
    assert a_calls == 2


def test_ensure_timeout_raises_cannot_ensure() -> None:
    @predicate('element {0} is enabled')
    def never() -> bool:
        return False

    with pytest.raises(CannotEnsureError) as info:
        ensure_that('ctx', never, timeout=0.05)
    assert "element 'ctx' is enabled" in str(info.value)


def test_ensure_timeout_returns_false_when_raise_disabled() -> None:
    assert ensure_that(object(), lambda: False, timeout=0.05, raise_exception=False) is False


def test_failed_func_called_between_retries() -> None:
    calls = {'n': 0}

    def failed() -> None:
        calls['n'] += 1

    ensure_that(object(), lambda: False, timeout=0.05, raise_exception=False, failed_func=failed)
    assert calls['n'] >= 1


def test_fatal_error_aborts_immediately() -> None:
    def boom() -> bool:
        raise PlatynUIFatalError('boom')

    with pytest.raises(PlatynUIFatalError):
        ensure_that(object(), boom, timeout=1.0)


def test_nested_ensure_inherits_outer_timeout() -> None:
    @predicate('inner')
    def inner_fail() -> bool:
        return False

    def outer() -> bool:
        # Inner ensure inherits the outer's already-elapsed clock and
        # raise_exception=False policy, so it returns False quickly
        # rather than raising.
        return ensure_that(object(), inner_fail)

    assert ensure_that(object(), outer, timeout=0.05, raise_exception=False) is False


def test_non_callable_predicate_is_fatal() -> None:
    with pytest.raises(PlatynUIFatalError):
        ensure_that(object(), 'not-callable')  # type: ignore[arg-type]


def test_full_repr_uses_object_repr_for_ctx_in_message() -> None:
    class Ctx:
        def __repr__(self) -> str:
            return '<Widget#42>'

    @predicate('{0} is ready')
    def never() -> bool:
        return False

    with pytest.raises(CannotEnsureError) as info:
        ensure_that(Ctx(), never, timeout=0.05)
    assert '<Widget#42> is ready' in str(info.value)


def test_full_repr_uses_full_repr_method_when_available() -> None:
    class Ctx:
        def full_repr(self) -> str:
            return 'FULL!'

    @predicate('{0}')
    def never() -> bool:
        return False

    with pytest.raises(CannotEnsureError) as info:
        ensure_that(Ctx(), never, timeout=0.05)
    assert 'FULL!' in str(info.value)
