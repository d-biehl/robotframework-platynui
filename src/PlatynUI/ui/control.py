# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`Control` page-object base for focusable UI elements."""

from __future__ import annotations

from typing import override

from ..core import patterns
from ..core.devices import KeyboardAction
from ..core.predicate import predicate
from .element import Element, _ElementKeyboardProxy

__all__ = ['Control']


class _ControlKeyboardProxy(_ElementKeyboardProxy):
    """Keyboard proxy that ensures the owning control has focus before each action."""

    @override
    def before_action(self, action: KeyboardAction) -> None:
        super().before_action(action)
        self._element.ensure_that(self._element._control_has_focus)  # type: ignore[attr-defined]


class Control(Element, register=False):
    """Page-object base for focusable UI elements.

    Adds a focus contract on top of `Element`: `has_focus`,
    `focus`, the `_control_has_focus` predicate, and a
    keyboard proxy that ensures the control is focused before sending
    keys.
    """

    @property
    def has_focus(self) -> bool:
        """Whether the control currently has keyboard focus.

        Convenience shortcut over the `Focusable` pattern; defaults
        to ``False`` when the adapter does not expose `Focusable`.
        """
        focusable = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        return focusable.is_focused if focusable is not None else False

    def focus(self) -> None:
        """Move keyboard focus to this control."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        focusable = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        if focusable is not None:
            focusable.focus()

    @predicate('control {0} has focus')
    def _control_has_focus(self) -> bool:
        if self.has_focus:
            return True
        self.focus()
        return self.has_focus

    @override
    def _create_keyboard_proxy(self) -> _ElementKeyboardProxy:
        return _ControlKeyboardProxy(self)
