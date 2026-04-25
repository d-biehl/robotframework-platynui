# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""HasUserInput pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['HasUserInput']


class HasUserInput(PatternBase):
    """An element that reports whether it currently accepts user input.

    Used by Window-like contexts to detect "not responding" states. The
    method may return `None` when the platform cannot answer reliably.
    """

    pattern_name = 'org.platynui.patterns.HasUserInput'

    @abstractmethod
    def accepts_user_input(self) -> bool | None:
        """Whether the element currently accepts user input.

        Returns `None` if the platform cannot determine the state.
        """
