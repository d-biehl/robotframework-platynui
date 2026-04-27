# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Context classes for button-like widgets: ``Button`` and ``CheckBox``."""

from abc import abstractmethod
from typing import override

from ..core import patterns
from ..core.patterns import ToggleState
from .control import Control

__all__ = ['AbstractButton', 'Button', 'CheckBox']


class AbstractButton(Control, register=False):
    """Base for widgets with a label and a single primary action."""

    @property
    def text(self) -> str:
        """The label currently shown on the widget."""
        self.ensure_that(self._application_is_ready)
        content = self.adapter.get_pattern(patterns.TextContent, raise_exception=False)
        return content.text if content is not None else ''

    @abstractmethod
    def activate(self) -> None:
        """Trigger the widget's primary user action."""


class Button(AbstractButton):
    """A push-button whose primary action is a single invoke."""

    @override
    def activate(self) -> None:
        """Press the button."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class CheckBox(AbstractButton):
    """A two- or three-state checkbox."""

    @property
    def state(self) -> ToggleState:
        """The current toggle state."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Toggleable).state

    @property
    def is_checked(self) -> bool:
        """Whether ``state`` equals ``ToggleState.ON``."""
        return self.state is ToggleState.ON

    @property
    def is_unchecked(self) -> bool:
        """Whether ``state`` equals ``ToggleState.OFF``."""
        return self.state is ToggleState.OFF

    @override
    def activate(self) -> None:
        """Check the checkbox."""
        self.check()

    def check(self) -> None:
        """Ensure the checkbox is in the ``ON`` state."""
        self.set_state(ToggleState.ON)

    def uncheck(self) -> None:
        """Ensure the checkbox is in the ``OFF`` state."""
        self.set_state(ToggleState.OFF)

    def toggle(self) -> None:
        """Advance to the next state."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
        )
        self.adapter.get_pattern(patterns.Toggleable).toggle()
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def set_state(self, state: ToggleState) -> None:
        """Toggle until the checkbox reports ``state``."""
        for _ in ToggleState:
            if self.state is state:
                return
            self.toggle()
