# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Lean polling helper.

`wait_for` polls a sequence of zero-argument predicates until
they all return truthy or the timeout expires. Unlike
`ensure_that` it never raises on timeout,
keeps no per-stage memo and runs no hooks; use it whenever a plain
boolean result is enough.
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

    Return ``True`` once every predicate returns truthy; return
    ``False`` if the timeout elapses first. ``timeout`` and ``delay``
    default to the values from `Settings`.
    ``invalidate`` is invoked between iterations and is typically used
    to drop cached adapter handles.

    `PlatynUIFatalError`,
    `KeyboardInterrupt` and `SystemExit` raised from a
    predicate propagate immediately without retry.
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
