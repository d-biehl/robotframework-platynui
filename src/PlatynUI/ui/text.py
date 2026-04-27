# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Page-objects for text widgets: ``Text`` (read-only) and ``Edit`` (editable)."""

from ..core import patterns
from .control import Control

__all__ = ['Edit', 'Text']


class Text(Control):
    """A read-only text widget such as a label or status text.

    Wrappt allein das ``TextContent``-Pattern. Beschreibbare
    Eingabefelder sind ``Edit``, nicht ``Text``.
    """

    @property
    def text(self) -> str:
        """The current text content."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).text

    @property
    def is_truncated(self) -> bool:
        """Whether the displayed text is shortened (e.g. with an ellipsis)."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).is_truncated

    @property
    def locale(self) -> str:
        """The BCP-47 locale tag for ``text``, or empty if unknown."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).locale


class Edit(Control):
    """An editable text input widget.

    Lives in the same module as ``Text`` because both share the
    ``TextContent`` family, but does not inherit from ``Text`` —
    ``Edit`` has different pre-conditions (focus, not read-only)
    and a strictly larger pattern dependency.
    """

    @property
    def text(self) -> str:
        """The current text content."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextContent).text

    @text.setter
    def text(self, value: str) -> None:
        self.set_text(value)

    @property
    def max_length(self) -> int | None:
        """The maximum length in characters, or ``None`` if unbounded."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextEditable).max_length

    @property
    def supports_password_mode(self) -> bool:
        """Whether the field can mask its content (e.g. password input)."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextEditable).supports_password_mode

    @property
    def is_multi_line(self) -> bool:
        """Whether the field accepts line breaks (multi-line input)."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.TextEditable).is_multi_line

    def set_text(self, value: str) -> None:
        """Replace the current content with ``value``."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(patterns.TextEditable).set_text(value)
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def clear(self) -> None:
        """Remove the current content via the ``Clearable`` pattern."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(patterns.Clearable).clear()
        self.ensure_that(self._application_is_ready, raise_exception=False)
