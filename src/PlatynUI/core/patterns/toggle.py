# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Toggle / check-state pattern."""

from __future__ import annotations

from abc import abstractmethod
from enum import Enum

from .base import PatternBase

__all__ = ['ToggleState', 'Toggleable']


class ToggleState(Enum):
    """Tri-state toggle value: ``OFF``, ``ON``, or ``INDETERMINATE``."""

    OFF = 'off'
    ON = 'on'
    INDETERMINATE = 'indeterminate'


class Toggleable(PatternBase):
    """An element that can be toggled and exposes its current state.

    Combines the toggle action with its observable state so callers
    can both read whether the element is on or off and flip it.
    """

    pattern_name = 'org.platynui.patterns.Toggleable'

    @abstractmethod
    def toggle(self) -> None:
        """Advance to the next toggle state."""

    @property
    @abstractmethod
    def state(self) -> ToggleState:
        """The current toggle state."""

    @property
    @abstractmethod
    def supports_three_state(self) -> bool:
        """Whether `state` may legitimately return ``INDETERMINATE``."""
