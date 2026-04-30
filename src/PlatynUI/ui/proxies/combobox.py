# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Default proxy for ``ComboBox`` — selectable + expandable + editable."""

from typing import override

from ...core import patterns
from ...core.adapter_proxy import pattern_proxy_for
from ._mixins import click_adapter, type_keys_on_adapter
from .base import ControlProxy

__all__ = ['ComboBoxProxy']


@pattern_proxy_for(role='ComboBox')
class ComboBoxProxy(
    ControlProxy,
    patterns.IsExpandable,
    patterns.Expandable,
    patterns.IsSelectable,
    patterns.Selectable,
    patterns.TextContent,
    patterns.TextEditable,
):
    """Default proxy for combo boxes.

    Items are ``ListItem`` children of the dropdown popup; the lookup
    lives on the `ComboBox` context (which opens the popup before the
    walk). Expansion toggles via a click on the combobox itself; reading
    state uses the ``IsExpanded`` attribute. The text-edit synthesis
    mirrors `EditProxy.set_text` (click + Ctrl+A + type). Read/Action
    split (Rev. 46): `IsExpandable`/`IsSelectable` carry state,
    `Expandable`/`Selectable` carry the click actions.
    """

    # ----- IsExpandable (Read) ----------------------------------------

    @property
    @override
    def can_expand(self) -> bool:
        return self.adapter.get_pattern(patterns.Element).is_enabled

    @property
    @override
    def is_expanded(self) -> bool:
        try:
            value = self.adapter.attribute_value('IsExpanded')
        except KeyError:
            return False
        return bool(value)

    # ----- Expandable (Action) ----------------------------------------

    @override
    def expand(self) -> None:
        if not self.is_expanded:
            click_adapter(self.adapter)

    @override
    def collapse(self) -> None:
        if self.is_expanded:
            click_adapter(self.adapter)

    # ----- IsSelectable (Read) ----------------------------------------

    @property
    @override
    def is_selected(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsSelected'))
        except KeyError:
            return False

    # ----- Selectable (Action) ----------------------------------------

    @override
    def select(self) -> None:
        click_adapter(self.adapter)

    # ----- TextContent ------------------------------------------------

    @property
    @override
    def text(self) -> str:
        try:
            value = self.adapter.attribute_value('Value')
        except KeyError:
            try:
                value = self.adapter.attribute_value('Text')
            except KeyError:
                value = ''
        return value if isinstance(value, str) else ''

    @property
    @override
    def locale(self) -> str:
        try:
            value = self.adapter.attribute_value('Locale')
        except KeyError:
            return ''
        return value if isinstance(value, str) else ''

    @property
    @override
    def is_truncated(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsTruncated'))
        except KeyError:
            return False

    # ----- TextEditable -----------------------------------------------

    @override
    def set_text(self, value: str) -> None:
        click_adapter(self.adapter)
        type_keys_on_adapter(self.adapter, '<Ctrl+A>')
        type_keys_on_adapter(self.adapter, value)

    @property
    @override
    def is_readonly(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsReadOnly'))
        except KeyError:
            return False

    @property
    @override
    def max_length(self) -> int | None:
        try:
            value = self.adapter.attribute_value('MaxLength')
        except KeyError:
            return None
        if isinstance(value, int) and value >= 0:
            return value
        return None

    @property
    @override
    def supports_password_mode(self) -> bool:
        return False

    @property
    @override
    def is_multi_line(self) -> bool:
        return False
