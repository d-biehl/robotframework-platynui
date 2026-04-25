# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Movable pattern."""

from abc import abstractmethod

from ..types import Point
from .base import PatternBase

__all__ = ['Movable']


class Movable(PatternBase):
    """An element whose position on screen can be changed."""

    pattern_name = 'org.platynui.patterns.Movable'

    @property
    @abstractmethod
    def can_move(self) -> bool:
        """Whether `move_to` is currently available."""

    @abstractmethod
    def move_to(self, point: Point) -> None:
        """Move the element so its top-left corner is at `point` (screen coordinates)."""
