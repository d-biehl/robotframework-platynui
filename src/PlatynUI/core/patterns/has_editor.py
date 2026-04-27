# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Inline-editor lifecycle pattern."""

from abc import abstractmethod

from .base import PatternBase

__all__ = ['HasEditor']


class HasEditor(PatternBase):
    """An element that opens an inline editor for its value."""

    pattern_name = 'org.platynui.patterns.HasEditor'

    @abstractmethod
    def open_editor(self) -> None:
        """Open the inline editor."""

    @abstractmethod
    def accept(self) -> None:
        """Confirm the current edit and close the editor."""

    @abstractmethod
    def cancel(self) -> None:
        """Discard the current edit and close the editor."""
