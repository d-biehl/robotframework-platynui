# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Pattern for controls with an explicit activation hit-point."""

from __future__ import annotations

from abc import abstractmethod

from ..types import Point, Rect
from .base import PatternBase

__all__ = ['ActivationTarget']


class ActivationTarget(PatternBase):
    """A control that exposes a recommended activation point.

    Some controls have a clearly defined hit-zone that differs from the
    geometric centre of their bounding box (e.g. a checkbox whose label
    is wide but whose actual click target is the small box on the left).
    Implement this pattern to let the mouse layer click the right spot
    without the caller having to know about it.

    Coordinates are absolute (desktop coordinate system) and must fall
    inside the corresponding ``Element.bounds``.
    """

    pattern_name = 'org.platynui.patterns.ActivationTarget'

    @property
    @abstractmethod
    def activation_point(self) -> Point:
        """The single recommended hit-point. Always required."""

    @property
    def activation_area(self) -> Rect | None:
        """An extended hit-zone, or ``None`` if a single point is enough.

        When set, the mouse layer clicks the centre of the area instead
        of `activation_point`.
        """
        return None

    @property
    def activation_hint(self) -> str | None:
        """A short human-readable description of the target, or ``None``.

        Useful for diagnostics; logged on DEBUG before each mouse action.
        """
        return None
