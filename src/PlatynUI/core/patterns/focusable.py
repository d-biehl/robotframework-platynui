# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Keyboard focus pattern."""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Focusable']


class Focusable(PatternBase):
    """Element can receive keyboard focus and exposes its focus state.

    The :attr:`is_focused` attribute mirrors the Rust ``focusable``
    attribute group (``IsFocused``). The :meth:`focus` action is a
    Python-side capability — the Rust runtime does not require a
    matching attribute since focus changes are purely behavioural.
    """

    pattern_name = 'org.platynui.patterns.Focusable'

    @property
    @abstractmethod
    def is_focused(self) -> bool: ...

    @abstractmethod
    def focus(self) -> None:
        """Move keyboard focus to this element."""
