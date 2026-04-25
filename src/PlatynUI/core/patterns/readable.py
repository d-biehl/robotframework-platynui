# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Readable pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Readable']


class Readable(PatternBase):
    """An element with a read-only/editable distinction."""

    pattern_name = 'org.platynui.patterns.Readable'

    @property
    @abstractmethod
    def is_readonly(self) -> bool:
        """Whether the element is currently read-only."""
