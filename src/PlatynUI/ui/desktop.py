# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`Desktop` page-object — the conventional process-wide root."""

from __future__ import annotations

from ..core.locator import Locator
from .desktopbase import DesktopBase

__all__ = ['Desktop']


class Desktop(DesktopBase):
    """Conventional desktop root with the absolute ``/.`` XPath locator."""


# Auto-registration via `__init_subclass__` sets `_locator` to a
# role-only `Locator(role="Desktop")`. Override it with the absolute
# path that pins the root in the adapter tree.
Desktop._locator = Locator(path='/.', role='Desktop')
