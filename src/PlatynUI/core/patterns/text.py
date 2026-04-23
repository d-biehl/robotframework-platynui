# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Text capabilities: read-only content, editing, and clearing.

Each capability is a separate pattern so adapters can opt in
independently. A label exposes only `TextContent`; an entry
field exposes `TextContent`, `TextEditable`, and
optionally `Clearable`.
"""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Clearable', 'TextContent', 'TextEditable']


class TextContent(PatternBase):
    """An element that exposes its current textual content (read-only)."""

    pattern_name = 'org.platynui.patterns.TextContent'

    @property
    @abstractmethod
    def text(self) -> str:
        """The current text content."""

    @property
    @abstractmethod
    def locale(self) -> str:
        """The BCP-47 locale tag for `text`, or empty if unknown."""

    @property
    @abstractmethod
    def is_truncated(self) -> bool:
        """Whether the displayed text is shortened (e.g. with an ellipsis)."""


class TextEditable(PatternBase):
    """An element that accepts a new text value and exposes editing constraints."""

    pattern_name = 'org.platynui.patterns.TextEditable'

    @abstractmethod
    def set_text(self, value: str) -> None:
        """Replace the current content with ``value``."""

    @property
    @abstractmethod
    def is_readonly(self) -> bool:
        """Whether the field rejects edits."""

    @property
    @abstractmethod
    def max_length(self) -> int | None:
        """The maximum length in characters, or `None` if unbounded."""

    @property
    @abstractmethod
    def supports_password_mode(self) -> bool:
        """Whether the field can mask its content, e.g. for password input."""


class Clearable(PatternBase):
    """An element that supports a dedicated clear operation.

    Pure action capability; carries no observable state.
    """

    pattern_name = 'org.platynui.patterns.Clearable'

    @abstractmethod
    def clear(self) -> None:
        """Remove the current content."""
