# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Predicate decorator (design document section A.3).

The decorator attaches a human-readable failure message to a zero-arg
predicate so that :func:`PlatynUI.core.ensure.ensure_that` can render
meaningful errors. Use ``{0}`` in the message to interpolate the context.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, TypeVar

__all__ = ['predicate']

F = TypeVar('F', bound=Callable[..., Any])


def predicate(message: str | None = None, flags: Any = None) -> Callable[[F], F]:
    """Mark a callable as an ``ensure_that`` / ``wait_for`` predicate.

    Args:
        message: Failure message template (``{0}`` is replaced by the
            ``ensure_that`` context's ``full_repr`` / ``repr``).
        flags: Free-form metadata, currently unused by the framework.
    """

    def decorator(func: F) -> F:
        func.__predicate__ = True  # type: ignore[attr-defined]
        func.message = message  # type: ignore[attr-defined]
        func.flags = flags  # type: ignore[attr-defined]
        return func

    return decorator
