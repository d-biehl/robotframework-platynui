# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Text capabilities — read-only content, editing, and clearing.

Mirrors the Rust attribute groups ``text_content``, ``text_editable`` and
``clearable``. Each pattern is a separate capability so adapters can opt in
independently (e.g. a label exposes only :class:`TextContent`, an entry
field exposes :class:`TextContent` + :class:`TextEditable` + optionally
:class:`Clearable`).
"""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Clearable', 'TextContent', 'TextEditable']


class TextContent(PatternBase):
    """Element exposes its current textual content (read-only).

    Attributes mirror the Rust ``text_content`` group: ``Text``,
    ``Locale``, ``IsTruncated``.
    """

    pattern_name = 'org.platynui.patterns.TextContent'

    @property
    @abstractmethod
    def text(self) -> str: ...

    @property
    @abstractmethod
    def locale(self) -> str: ...

    @property
    @abstractmethod
    def is_truncated(self) -> bool: ...


class TextEditable(PatternBase):
    """Element accepts a new text value and exposes editing constraints.

    Attributes mirror the Rust ``text_editable`` group: ``IsReadOnly``,
    ``MaxLength``, ``SupportsPasswordMode``.
    """

    pattern_name = 'org.platynui.patterns.TextEditable'

    @abstractmethod
    def set_text(self, value: str) -> None: ...

    @property
    @abstractmethod
    def is_readonly(self) -> bool: ...

    @property
    @abstractmethod
    def max_length(self) -> int | None:
        """Maximum length in characters, or :data:`None` if unbounded."""

    @property
    @abstractmethod
    def supports_password_mode(self) -> bool: ...


class Clearable(PatternBase):
    """Element supports a dedicated clear operation.

    Pure action capability — no observable attributes (mirrors the empty
    Rust ``clearable`` attribute module).
    """

    pattern_name = 'org.platynui.patterns.Clearable'

    @abstractmethod
    def clear(self) -> None: ...
