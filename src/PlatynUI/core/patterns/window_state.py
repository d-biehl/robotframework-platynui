# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Window state pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['WindowState']


class WindowState(PatternBase):
    """Read-only status bits of a top-level window.

    Separates window status (foreground, always-on-top) from the
    `Activatable` action so that `Activatable` stays a pure action
    pattern usable for buttons and menu items as well.
    """

    pattern_name = 'org.platynui.patterns.WindowState'

    @property
    @abstractmethod
    def is_active(self) -> bool:
        """Whether the window is currently the foreground window."""

    @property
    @abstractmethod
    def is_topmost(self) -> bool:
        """Whether the window is in always-on-top mode."""

    @property
    @abstractmethod
    def is_modal(self) -> bool:
        """Whether the window is modal (blocks input to other windows)."""
