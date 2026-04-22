# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.wait``."""

from __future__ import annotations

import time

import pytest

from PlatynUI.core import PlatynUIFatalError, Settings, wait_for


@pytest.fixture(autouse=True)
def _fast_settings() -> None:  # pyright: ignore[reportUnusedFunction]
    Settings.set_current(Settings(wait_for_timeout=0.5, wait_for_delay=0.01))


def test_returns_true_immediately_when_all_predicates_hold() -> None:
    assert wait_for(lambda: True, lambda: True) is True


def test_returns_false_on_timeout() -> None:
    start = time.monotonic()
    result = wait_for(lambda: False, timeout=0.1, delay=0.01)
    assert result is False
    assert time.monotonic() - start >= 0.1


def test_polls_until_predicate_flips() -> None:
    counter = {'n': 0}

    def predicate() -> bool:
        counter['n'] += 1
        return counter['n'] >= 3

    assert wait_for(predicate, timeout=1.0, delay=0.01) is True
    assert counter['n'] >= 3


def test_invalidate_called_between_iterations() -> None:
    invalidations = {'n': 0}

    def invalidate() -> None:
        invalidations['n'] += 1

    wait_for(lambda: False, timeout=0.05, delay=0.01, invalidate=invalidate)
    assert invalidations['n'] >= 1


def test_fatal_error_propagates_without_retry() -> None:
    def boom() -> bool:
        raise PlatynUIFatalError('nope')

    with pytest.raises(PlatynUIFatalError):
        wait_for(boom, timeout=1.0, delay=0.01)


def test_uses_settings_defaults() -> None:
    Settings.set_current(Settings(wait_for_timeout=0.05, wait_for_delay=0.01))
    start = time.monotonic()
    assert wait_for(lambda: False) is False
    elapsed = time.monotonic() - start
    assert 0.04 <= elapsed < 0.5
