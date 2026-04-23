# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Decorator marking a callable as an ``ensure_that`` predicate.

A predicate is a zero-argument callable returning a truthy value when
satisfied. The decorator attaches a human-readable failure message that
`ensure_that` interpolates into the
resulting `CannotEnsureError`. Use
``{0}`` in the message as a placeholder for the context object.

Example::

    @predicate('{0} is visible')
    def _is_visible() -> bool:
        return element.is_visible

    ensure_that(element, _is_visible)
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, TypeVar

__all__ = ['predicate']

F = TypeVar('F', bound=Callable[..., Any])


def predicate(message: str | None = None, flags: Any = None) -> Callable[[F], F]:
    """Mark a callable as a predicate for ``ensure_that`` / ``wait_for``.

    ``message`` is a failure-message template; ``{0}`` is substituted
    with the context object passed to ``ensure_that``. ``flags`` is
    free-form metadata reserved for future use.
    """

    def decorator(func: F) -> F:
        func.__predicate__ = True  # type: ignore[attr-defined]
        func.message = message  # type: ignore[attr-defined]
        func.flags = flags  # type: ignore[attr-defined]
        return func

    return decorator
