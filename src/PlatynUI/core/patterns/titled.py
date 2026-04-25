# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Titled pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Titled']


class Titled(PatternBase):
    """An element that exposes a human-readable title separate from its name."""

    pattern_name = 'org.platynui.patterns.Titled'

    @property
    @abstractmethod
    def title(self) -> str:
        """The current title text."""
