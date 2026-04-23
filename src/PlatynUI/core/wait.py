# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Lean polling helper (design document section A.3).

``wait_for`` polls a sequence of zero-arg predicates until either all of
them return ``True`` within the timeout, or the timeout expires. It does
not raise on timeout, does not retain a stage memo, and does not invoke
hooks; for those features see :func:`PlatynUI.core.ensure.ensure_that`.
"""

from __future__ import annotations

import time
from collections.abc import Callable

from .exceptions import PlatynUIFatalError
from .settings import Settings

__all__ = ['wait_for']


def wait_for(
    *predicates: Callable[[], bool],
    timeout: float | None = None,
    delay: float | None = None,
    invalidate: Callable[[], None] | None = None,
) -> bool:
    """Poll ``predicates`` until they all hold or the timeout expires.

    Args:
        predicates: Zero-arg callables that return truthy when satisfied.
        timeout: Polling deadline in seconds (default
            ``Settings.current().wait_for_timeout``).
        delay: Sleep between iterations in seconds (default
            ``Settings.current().wait_for_delay``).
        invalidate: Optional hook executed between iterations, typically to
            invalidate cached adapter handles.

    Returns:
        ``True`` if every predicate returned truthy within the timeout,
        otherwise ``False``. Never raises on timeout.

    Raises:
        PlatynUIFatalError: re-raised from a predicate without retry.
        KeyboardInterrupt: re-raised without retry.
        SystemExit: re-raised without retry.
    """
    settings = Settings.current()
    effective_timeout = settings.wait_for_timeout if timeout is None else timeout
    effective_delay = settings.wait_for_delay if delay is None else delay

    start = time.monotonic()
    while True:
        all_ok = True
        for predicate in predicates:
            if time.monotonic() - start > effective_timeout:
                return False
            try:
                ok = predicate()
            except (PlatynUIFatalError, KeyboardInterrupt, SystemExit):
                raise
            if not ok:
                all_ok = False
                break

        if all_ok:
            return True

        if time.monotonic() - start > effective_timeout:
            return False

        time.sleep(effective_delay)

        if invalidate is not None:
            invalidate()
