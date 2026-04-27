# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`Desktop` context — the conventional process-wide root."""

from ..core.locator import Locator
from .desktopbase import DesktopBase

__all__ = ['Desktop']


class Desktop(DesktopBase):
    """Conventional desktop root with the absolute ``/.`` XPath locator."""


# Pin the desktop root with the absolute ``/.`` XPath, replacing the
# role-only locator set by ``__init_subclass__``.
Desktop._locator = Locator(path='/.', role='Desktop')
