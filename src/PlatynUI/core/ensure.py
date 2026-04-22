# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Verification driver (design document section A.3).

``ensure_that`` is the outcome-contract primitive used by UI page-object
methods to gate operations on pre-conditions and to validate post-
conditions. It supports stage memoisation, invalidation hooks, and
re-entrancy via a thread-local stack so that nested ``ensure_that`` calls
share the outer timeout instead of starting their own.
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
    """Return ``obj.full_repr()`` if available, otherwise ``repr(obj)``."""
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

    Hooks receive the ``context`` and may raise :class:`PlatynUIFatalError`
    to abort retries.
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

    Predicates marked via :func:`PlatynUI.core.predicate.predicate` carry a
    ``message`` attribute that is interpolated into the failure exception.

    Args:
        context: The page-object or element the predicates run against; used
            for failure messages and adapter invalidation.
        predicates: Zero-arg callables; ``None`` entries are skipped.
        timeout: Polling deadline in seconds (default
            ``Settings.current().ensure_timeout``). On a re-entrant call the
            outer timeout wins.
        raise_exception: ``True`` (default) raises :class:`CannotEnsureError`
            on timeout; ``False`` returns ``False`` instead.
        failed_func: Hook executed between retries (typically
            ``context.invalidate``).

    Returns:
        ``True`` if all predicates eventually returned truthy, ``False`` on
        timeout when ``raise_exception`` is ``False``.

    Raises:
        CannotEnsureError: timeout exceeded with ``raise_exception=True``.
        PlatynUIFatalError, KeyboardInterrupt, SystemExit: re-raised without
            retry.
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
