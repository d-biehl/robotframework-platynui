# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Default proxies for ``Edit`` / ``Text`` roles.

Both build a `TextContent` / `TextEditable` / `Clearable` triple from
adapter attributes (``Value``/``Text``, ``IsReadOnly``, ``MaxLength``,
``IsPasswordField``, ``IsMultiLine``) and synthesize `set_text` /
`clear` via focus + select-all + type / delete.
"""

from typing import cast, override

from ...core import patterns
from ...core.adapter_proxy import pattern_proxy_for
from ._mixins import click_adapter, type_keys_on_adapter
from .base import ControlProxy

__all__ = ['EditProxy', 'TextProxy']


def _read_str(adapter: object, name: str, default: str = '') -> str:
    try:
        value = cast(object, adapter.attribute_value(name))  # type: ignore[attr-defined]
    except KeyError:
        return default
    return value if isinstance(value, str) else default


def _read_bool(adapter: object, name: str, default: bool = False) -> bool:
    try:
        value = cast(object, adapter.attribute_value(name))  # type: ignore[attr-defined]
    except KeyError:
        return default
    return bool(value)


@pattern_proxy_for(role='Edit')
class EditProxy(ControlProxy, patterns.TextContent, patterns.TextEditable, patterns.Clearable):
    """Default proxy for editable text fields."""

    # ----- TextContent -------------------------------------------------

    @property
    @override
    def text(self) -> str:
        return _read_str(self.adapter, 'Value') or _read_str(self.adapter, 'Text')

    @property
    @override
    def locale(self) -> str:
        return _read_str(self.adapter, 'Locale')

    @property
    @override
    def is_truncated(self) -> bool:
        return _read_bool(self.adapter, 'IsTruncated')

    # ----- TextEditable -----------------------------------------------

    @override
    def set_text(self, value: str) -> None:
        # Focus + select-all + replace: click into the field, then
        # Ctrl+A and type the new value (which replaces the selection).
        click_adapter(self.adapter)
        type_keys_on_adapter(self.adapter, '<Ctrl+A>')
        type_keys_on_adapter(self.adapter, value)

    @property
    @override
    def is_readonly(self) -> bool:
        return _read_bool(self.adapter, 'IsReadOnly')

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
        return _read_bool(self.adapter, 'IsPasswordField')

    @property
    @override
    def is_multi_line(self) -> bool:
        return _read_bool(self.adapter, 'IsMultiLine')

    # ----- Clearable --------------------------------------------------

    @override
    def clear(self) -> None:
        click_adapter(self.adapter)
        type_keys_on_adapter(self.adapter, '<Ctrl+A>')
        type_keys_on_adapter(self.adapter, '<Delete>')


@pattern_proxy_for(role='Text')
class TextProxy(EditProxy):
    """Default proxy for read-only text labels (``Text`` role).

    Inherits the same triple from `EditProxy`; the read-only nature is
    expressed via the ``IsReadOnly`` attribute reported by the adapter.
    """
