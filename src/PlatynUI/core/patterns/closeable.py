# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Closeable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Closeable']


class Closeable(PatternBase):
    """An element that can be closed."""

    pattern_name = 'org.platynui.patterns.Closeable'

    @property
    @abstractmethod
    def can_close(self) -> bool:
        """Whether `close` is currently available."""

    @abstractmethod
    def close(self) -> None:
        """Close the element."""
