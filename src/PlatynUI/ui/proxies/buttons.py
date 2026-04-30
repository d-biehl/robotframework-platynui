# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Default proxies for button-like roles.

`ButtonProxy` provides synthetic `Activatable` (left-click);
`CheckBoxProxy` provides synthetic `Toggleable` that flips state via a
single click and reads the current state from the
``Toggleable.state`` attribute on the adapter (or falls back to a
two-state interpretation of `Selectable.is_selected`).
"""

from typing import override

from ...core import patterns
from ...core.adapter_proxy import pattern_proxy_for
from ...core.patterns import ToggleState
from ._mixins import click_adapter
from .base import ControlProxy

__all__ = ['ButtonProxy', 'CheckBoxProxy']


@pattern_proxy_for(role='Button')
class ButtonProxy(ControlProxy, patterns.Activatable):
    """Default proxy for ``Button`` (and link-like) roles."""

    @override
    def activate(self) -> None:
        click_adapter(self.adapter)

    @property
    @override
    def is_activation_enabled(self) -> bool:
        return self.adapter.get_pattern(patterns.Element).is_enabled

    @property
    @override
    def default_accelerator(self) -> str | None:
        return None


@pattern_proxy_for(role='CheckBox')
class CheckBoxProxy(ControlProxy, patterns.Toggleable):
    """Default proxy for ``CheckBox``, ``RadioButton``, ``ToggleButton``."""

    @override
    def toggle(self) -> None:
        click_adapter(self.adapter)

    @property
    @override
    def state(self) -> ToggleState:
        # Prefer an attribute-provided state if the adapter exposes
        # ``Toggleable.state`` natively; otherwise use ``IsSelected``.
        try:
            value = self.adapter.attribute_value('IsToggled')
        except KeyError:
            value = None
        if isinstance(value, str):
            try:
                return ToggleState(value.lower())
            except ValueError:
                pass
        if isinstance(value, bool):
            return ToggleState.ON if value else ToggleState.OFF
        try:
            selected = self.adapter.attribute_value('IsSelected')
        except KeyError:
            selected = False
        return ToggleState.ON if bool(selected) else ToggleState.OFF

    @property
    @override
    def supports_three_state(self) -> bool:
        return False
