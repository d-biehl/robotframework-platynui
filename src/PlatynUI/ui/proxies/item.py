# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Default proxies for ``Item`` and its specialisations.

`ItemProxy` carries `IsSelectable` / `Selectable` / `MultiSelectable`
/ `Activatable` / `HasEditor` / `TextEditable` / `Clearable`
synthesised over click + Ctrl+Click + Ctrl+A + type. Subclasses
(`ListItemProxy`, `TreeItemProxy`, `TabItemProxy`, `RowProxy`,
`CellProxy`, `MenuItemProxy`) inherit and add container or expandable
capabilities where the role demands.

Read/Action split (Rev. 46): native adapters expose Read-patterns
(`IsSelectable`, `IsExpandable`) directly via the provider attribute
set; default proxies synthesise the read fallback from the same
attributes (``IsSelected``, ``IsExpanded``, ``CanExpand``) so the
proxy works even on adapters that lack the Rust-side pattern wiring.
Action-patterns (`Selectable`, `MultiSelectable`, `Expandable`) are
exclusively the proxy's domain and never touch native control APIs.
"""

from typing import override

from ...core import patterns
from ...core.adapter_devices import AdapterKeyboardProxy, AdapterMouseProxy
from ...core.adapter_proxy import pattern_proxy_for
from .base import ElementProxy

__all__ = [
    'CellProxy',
    'ItemProxy',
    'ListItemProxy',
    'MenuItemProxy',
    'RowProxy',
    'TabItemProxy',
    'TreeItemProxy',
]


@pattern_proxy_for(role='Item')
class ItemProxy(
    ElementProxy,
    patterns.IsSelectable,
    patterns.Selectable,
    patterns.MultiSelectable,
    patterns.HasEditor,
    patterns.TextEditable,
    patterns.Clearable,
    patterns.Activatable,
):
    """Default proxy for items inside a container."""

    # ----- IsSelectable (Read) ----------------------------------------

    @property
    @override
    def is_selected(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsSelected'))
        except KeyError:
            return False

    # ----- Selectable (Action: single-select) -------------------------

    @override
    def select(self) -> None:
        AdapterMouseProxy(self.adapter).click()

    # ----- MultiSelectable (Action: additive selection) ---------------

    @override
    def add_to_selection(self) -> None:
        AdapterMouseProxy(self.adapter).ctrl_click()

    @override
    def remove_from_selection(self) -> None:
        AdapterMouseProxy(self.adapter).ctrl_click()

    # ----- Activatable ------------------------------------------------

    @override
    def activate(self) -> None:
        AdapterMouseProxy(self.adapter).click(times=2)

    @property
    @override
    def is_activation_enabled(self) -> bool:
        return self.adapter.get_pattern(patterns.Element).is_enabled

    @property
    @override
    def default_accelerator(self) -> str | None:
        return None

    # ----- HasEditor --------------------------------------------------

    @override
    def open_editor(self) -> None:
        AdapterMouseProxy(self.adapter).click(times=2)

    @override
    def accept(self) -> None:
        AdapterKeyboardProxy(self.adapter).type_keys('<Enter>')

    @override
    def cancel(self) -> None:
        AdapterKeyboardProxy(self.adapter).type_keys('<Escape>')

    # ----- TextEditable -----------------------------------------------

    @override
    def set_text(self, value: str) -> None:
        keyboard = AdapterKeyboardProxy(self.adapter)
        keyboard.type_keys('<Ctrl+A>')
        keyboard.type_keys(value)

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
        return None

    @property
    @override
    def supports_password_mode(self) -> bool:
        return False

    @property
    @override
    def is_multi_line(self) -> bool:
        return False

    # ----- Clearable --------------------------------------------------

    @override
    def clear(self) -> None:
        keyboard = AdapterKeyboardProxy(self.adapter)
        keyboard.type_keys('<Ctrl+A>')
        keyboard.type_keys('<Delete>')


@pattern_proxy_for(role='ListItem')
class ListItemProxy(ItemProxy):
    """Default proxy for ``ListItem``."""


@pattern_proxy_for(role='TabItem')
class TabItemProxy(ItemProxy):
    """Default proxy for ``TabItem``."""


@pattern_proxy_for(role='TreeItem')
class TreeItemProxy(ItemProxy, patterns.IsExpandable, patterns.Expandable):
    """Default proxy for ``TreeItem`` — adds expand/collapse."""

    # ----- IsExpandable (Read) ----------------------------------------

    @property
    @override
    def can_expand(self) -> bool:
        try:
            value = self.adapter.attribute_value('CanExpand')
        except KeyError:
            value = None
        if value is not None:
            return bool(value)
        return any(child.role == 'TreeItem' for child in self.adapter.children)

    @property
    @override
    def is_expanded(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsExpanded'))
        except KeyError:
            return False

    # ----- Expandable (Action) ----------------------------------------

    @override
    def expand(self) -> None:
        if not self.is_expanded:
            AdapterMouseProxy(self.adapter).click()

    @override
    def collapse(self) -> None:
        if self.is_expanded:
            AdapterMouseProxy(self.adapter).click()


@pattern_proxy_for(role='Row')
class RowProxy(ItemProxy):
    """Default proxy for ``Row``."""


@pattern_proxy_for(role='Cell')
class CellProxy(ItemProxy):
    """Default proxy for ``Cell``."""


@pattern_proxy_for(role='MenuItem')
class MenuItemProxy(ItemProxy, patterns.IsExpandable, patterns.Expandable):
    """Default proxy for ``MenuItem`` — adds submenu open/close."""

    # ----- IsExpandable (Read) ----------------------------------------

    @property
    @override
    def can_expand(self) -> bool:
        try:
            value = self.adapter.attribute_value('CanExpand')
        except KeyError:
            value = None
        if value is not None:
            return bool(value)
        return any(child.role in ('MenuItem', 'Menu') for child in self.adapter.children)

    @property
    @override
    def is_expanded(self) -> bool:
        try:
            return bool(self.adapter.attribute_value('IsExpanded'))
        except KeyError:
            return False

    # ----- Expandable (Action) ----------------------------------------

    @override
    def expand(self) -> None:
        if not self.is_expanded:
            AdapterMouseProxy(self.adapter).click()

    @override
    def collapse(self) -> None:
        if self.is_expanded:
            AdapterMouseProxy(self.adapter).click()
