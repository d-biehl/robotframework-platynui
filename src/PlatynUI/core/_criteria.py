# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Internal helpers for comparing registry criteria dictionaries."""

from __future__ import annotations

import re
from typing import Any

__all__ = ['criteria_equal', 'normalize_criterion']


def normalize_criterion(value: Any) -> Any:
    """Return an equality-stable form of a criterion value.

    `re.Pattern` instances are reduced to ``(pattern, flags)`` tuples so
    that two patterns compiled from the same source compare equal.
    Dict values are normalised recursively so the nested ``attributes``
    map is compared correctly.
    """
    if isinstance(value, re.Pattern):
        return (value.pattern, value.flags)
    if isinstance(value, dict):
        return {k: normalize_criterion(v) for k, v in value.items()}
    return value


def criteria_equal(a: dict[str, Any], b: dict[str, Any]) -> bool:
    """Return True when both criteria dicts contain the same keys/values."""
    if a.keys() != b.keys():
        return False
    return all(normalize_criterion(a[k]) == normalize_criterion(b[k]) for k in a)
