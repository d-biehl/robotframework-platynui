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
    """Tri-state toggle value (mirrors UIA / AT-SPI semantics)."""

    OFF = 'off'
    ON = 'on'
    INDETERMINATE = 'indeterminate'


class Toggleable(PatternBase):
    """Element can be toggled and exposes its current state.

    Bundles the action (:meth:`toggle`) with its observable attributes
    (:attr:`state`, :attr:`supports_three_state`) — mirrors the Rust
    ``toggleable`` capability group.
    """

    pattern_name = 'org.platynui.patterns.Toggleable'

    @abstractmethod
    def toggle(self) -> None: ...

    @property
    @abstractmethod
    def state(self) -> ToggleState: ...

    @property
    @abstractmethod
    def supports_three_state(self) -> bool: ...
