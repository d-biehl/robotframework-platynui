# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""ApplicationReady pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['ApplicationReady']


class ApplicationReady(PatternBase):
    """An application that can poll for "ready to accept input" state.

    Polling hook used by `_application_is_ready` predicates to detect
    apps that are still loading or in a "not responding" state.
    """

    pattern_name = 'org.platynui.patterns.ApplicationReady'

    @abstractmethod
    def try_ensure_ready(self) -> bool:
        """Whether the application is currently ready to accept input."""
