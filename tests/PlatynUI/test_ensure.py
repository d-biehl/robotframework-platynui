# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.ensure`` and ``PlatynUI.core.predicate``."""

import time

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


# ---------------------------------------------------------------------------
# Nested ensure_that semantics (re-entrant scope sharing)
# ---------------------------------------------------------------------------


def test_nested_inner_raise_false_is_honoured_under_outer_raise_true() -> None:
    """Explicit inner ``raise_exception=False`` survives an outer raising scope.

    This is the contract relied on by ``Window.close()`` →
    ``_window_is_gone`` → ``self.exists()``: the caller passed
    ``exists(raise_exception=False)`` and that local policy must win
    regardless of the surrounding ``ensure_that`` configuration.
    """

    @predicate('inner-never')
    def inner_fail() -> bool:
        return False

    def outer() -> bool:
        # Inner explicitly opts out of raising — its own policy wins,
        # so the failure surfaces as a ``False`` return rather than a
        # ``CannotEnsureError``.
        return ensure_that(object(), inner_fail, raise_exception=False) or True

    # Outer succeeds (``outer`` returns ``True``) — no exception bubbles up.
    assert ensure_that(object(), outer, timeout=0.05) is True


def test_nested_inner_without_raise_inherits_outer_policy() -> None:
    """When inner leaves ``raise_exception`` unset, the outer scope's policy applies.

    Counterpart to
    :func:`test_nested_inner_raise_false_is_honoured_under_outer_raise_true`:
    only an *explicit* nested value overrides; the default (``None``)
    still inherits from the outermost frame.
    """

    @predicate('inner-never')
    def inner_fail() -> bool:
        return False

    def outer() -> bool:
        # No explicit raise_exception → inherits outer's True → raises.
        ensure_that(object(), inner_fail)
        return True  # pragma: no cover - unreachable

    with pytest.raises(CannotEnsureError):
        ensure_that(object(), outer, timeout=0.05)


def test_nested_inner_timeout_is_overridden_by_outer_timeout() -> None:
    """Inner ``timeout`` argument is ignored; the outer clock governs."""

    def outer() -> bool:
        # Inner asks for a 10s timeout but the outer scope already set
        # 0.05s — the inner call must respect the outer budget.
        return ensure_that(object(), lambda: False, timeout=10.0)

    start = time.monotonic()
    result = ensure_that(object(), outer, timeout=0.05, raise_exception=False)
    elapsed = time.monotonic() - start
    assert result is False
    # Bounded comfortably below the inner's nominal 10s — anything under
    # one second proves the inner timeout did not take effect.
    assert elapsed < 1.0


def test_nested_three_level_explicit_raise_false_isolates_failure() -> None:
    """Three-level nesting: explicit ``raise_exception=False`` swallows the failure.

    Models the real ``Window.close → _window_is_gone → exists →
    _adapter_exists → ensure_that(_parent_exists)`` chain, where
    ``exists()`` requests ``raise_exception=False`` so that an absent
    window surfaces as ``False`` rather than aborting the close
    sequence.
    """

    @predicate('innermost')
    def innermost() -> bool:
        return False

    def middle() -> bool:
        # Explicit raise_exception=False is honoured: innermost
        # times out and the call returns False, the middle predicate
        # itself reports success.
        ensure_that(object(), innermost, raise_exception=False)
        return True

    def outer() -> bool:
        ensure_that(object(), middle, raise_exception=False)
        return True

    assert ensure_that(object(), outer, timeout=0.05) is True


def test_nested_succeeded_memo_is_shared_with_outer() -> None:
    """A predicate marked succeeded inside a nested call is not re-evaluated outside.

    The thread-local ``succeeded`` list is initialised only on the
    outermost ``ensure_that``; nested calls append to and read from the
    same list.
    """

    counter = {'p': 0}

    @predicate('shared')
    def p() -> bool:
        counter['p'] += 1
        return True

    def outer() -> bool:
        # Inner call evaluates ``p`` once and adds it to the shared memo.
        ensure_that(object(), p)
        # Direct evaluation in the outer scope: ``p`` should be skipped
        # because the memo still contains it from the inner call.
        ensure_that(object(), p)
        return True

    assert ensure_that(object(), outer, timeout=1.0) is True
    # ``p`` is invoked exactly once: inner adds it to ``succeeded``;
    # outer's second call hits the memo and short-circuits.
    assert counter['p'] == 1


def test_nested_inner_does_not_reset_outer_start_time() -> None:
    """Only the outermost frame owns ``start_time``; inner must not reset it."""

    @predicate('never-passes')
    def fails() -> bool:
        return False

    inner_observations: list[bool] = []

    def outer() -> bool:
        # Nested call must respect the outer 0.05s budget rather than
        # restarting its own clock — observe a False return promptly.
        inner_result = ensure_that(object(), fails)
        inner_observations.append(inner_result)
        return True

    start = time.monotonic()
    # Outer succeeds (its own predicate returns True after the inner
    # call completes); inner returns False because it inherits the
    # outer's already-running clock.
    assert ensure_that(object(), outer, timeout=0.05, raise_exception=False) is True
    elapsed = time.monotonic() - start
    # If the inner had restarted the start_time, we would loop on the
    # inner predicate for ~0.05s on every outer iteration — but with a
    # shared clock the whole call still finishes well under one second.
    assert elapsed < 1.0
    assert inner_observations == [False]


def test_depth_resets_to_zero_on_predicate_exception() -> None:
    """``depth`` and ``start_time`` are restored even if a predicate raises."""
    # Sanity: depth starts at zero.
    assert _ensure_depth() == 0

    @predicate('boomy')
    def boom() -> bool:
        raise PlatynUIFatalError('boom')

    with pytest.raises(PlatynUIFatalError):
        ensure_that(object(), boom, timeout=1.0)

    # The finally blocks must restore the thread-local to a pristine state
    # so the next ensure_that call does not inherit stale data.
    assert _ensure_depth() == 0
    assert _ensure_start_time() is None


def _ensure_depth() -> int:
    from PlatynUI.core.ensure import _thread_local

    return _thread_local.depth


def _ensure_start_time() -> float | None:
    from PlatynUI.core.ensure import _thread_local

    return _thread_local.start_time


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
