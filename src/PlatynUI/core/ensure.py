# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Verification driver for outcome-contract style predicates.

`ensure_that` polls a list of zero-argument predicates and
returns once all of them hold, or raises
`CannotEnsureError` after the timeout.
Compared to `wait_for` it adds stage
memoisation, re-entrant timeout sharing, observation hooks
(`add_ensure_hook`) and a per-iteration failure callback.
"""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from typing import Any, Protocol, runtime_checkable

from .exceptions import CannotEnsureError, PlatynUIFatalError
from .settings import Settings

__all__ = ['add_ensure_hook', 'ensure_that', 'full_repr']


@runtime_checkable
class _HasFullRepr(Protocol):
    def full_repr(self) -> str: ...


def full_repr(obj: Any) -> str:
    """Return ``obj.full_repr()`` if available, else ``repr(obj)``."""
    try:
        if isinstance(obj, _HasFullRepr):
            return obj.full_repr()
    except Exception:
        pass
    return repr(obj)


class _EnsureLocal(threading.local):
    def __init__(self) -> None:
        self.start_time: float | None = None
        self.raise_exception: bool | None = None
        self.depth: int = 0
        self.succeeded: list[Callable[[], bool]] = []
        self.timeout: float | None = None


_thread_local = _EnsureLocal()
_hooks: list[Callable[[Any], None]] = []


def add_ensure_hook(hook: Callable[[Any], None]) -> None:
    """Register a global hook called once per ``ensure_that`` iteration.

    The hook receives the current ``context`` and may raise
    `PlatynUIFatalError` to abort retries.
    """
    _hooks.append(hook)


def _exec_hooks(context: Any) -> None:
    for hook in _hooks:
        hook(context)


def ensure_that(
    context: object,
    *predicates: Callable[[], bool] | None,
    timeout: float | None = None,
    raise_exception: bool | None = None,
    failed_func: Callable[[], None] | None = None,
) -> bool:
    """Verify that all ``predicates`` hold within the timeout.

    Return ``True`` once every predicate has returned truthy in the
    same iteration. On timeout, raise
    `CannotEnsureError` (default) or
    return ``False`` if ``raise_exception`` is ``False``. ``None``
    entries in ``predicates`` are skipped.

    ``timeout`` defaults to
    ``Settings.current().ensure_timeout``. On a re-entrant call the
    outer scope's timeout and ``raise_exception`` policy win.
    ``failed_func`` runs between iterations and is typically used to
    invalidate cached adapter handles.

    Predicates marked with
    `predicate` carry a ``message``
    attribute that is formatted into the failure message;
    `PlatynUIFatalError`, `KeyboardInterrupt` and
    `SystemExit` raised from a predicate propagate immediately.
    """
    if timeout is None:
        timeout = Settings.current().ensure_timeout

    _thread_local.depth += 1
    try:
        if _thread_local.depth == 1:
            _thread_local.succeeded = []
            _thread_local.raise_exception = raise_exception
            _thread_local.timeout = timeout
        else:
            # Inherit the outer scope's raise/timeout policy.
            raise_exception = _thread_local.raise_exception

        if raise_exception is None:
            raise_exception = True

        owns_start_time = False
        if _thread_local.start_time is None:
            _thread_local.start_time = time.monotonic()
            owns_start_time = True

        last_exception: BaseException | None = None
        last_predicate: Callable[[], bool] | None = None
        result = False

        try:
            while not result:
                result = True
                for predicate in predicates:
                    if predicate is None:
                        continue
                    if not callable(predicate):
                        raise PlatynUIFatalError(f'{predicate!r} is not callable')
                    if predicate in _thread_local.succeeded:
                        continue

                    last_predicate = predicate
                    try:
                        _exec_hooks(context)
                        ok = predicate()
                    except (PlatynUIFatalError, KeyboardInterrupt, SystemExit):
                        raise
                    except BaseException as exc:
                        last_exception = exc
                        result = False
                        break

                    if not ok:
                        # Reset memo: all pre-conditions must hold simultaneously.
                        _thread_local.succeeded.clear()
                        result = False
                        break

                    _thread_local.succeeded.append(predicate)

                if result:
                    break

                start = _thread_local.start_time
                effective_timeout = _thread_local.timeout
                # Both fields are always set above (depth>=1 init block +
                # owns_start_time block) before reaching this point.
                assert start is not None
                assert effective_timeout is not None

                if time.monotonic() - start > effective_timeout:
                    break

                time.sleep(Settings.current().ensure_delay)
                if failed_func is not None:
                    failed_func()
        finally:
            if owns_start_time:
                _thread_local.start_time = None

        if not result and raise_exception:
            message_template = getattr(last_predicate, 'message', None)
            if last_predicate is not None and message_template:
                head = str(message_template).format(full_repr(context))
            elif last_predicate is not None:
                head = f'{last_predicate} for {full_repr(context)}'
            else:
                head = f'predicate for {full_repr(context)}'

            if last_exception is not None:
                detail = str(last_exception).strip() or repr(last_exception)
                message = f'Cannot ensure that {head},\n   because {detail}'
            else:
                message = f'Cannot ensure that {head}'

            raise CannotEnsureError(message) from last_exception

        return result
    finally:
        _thread_local.depth -= 1
        assert _thread_local.depth >= 0
