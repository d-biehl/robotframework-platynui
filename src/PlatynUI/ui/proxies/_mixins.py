# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Shared action helpers for default proxies.

`click_adapter` / `ctrl_click_adapter` / `type_keys_on_adapter` wrap
`AdapterMouseProxy` / `AdapterKeyboardProxy` for synthetic
`Activatable`, `Toggleable`, `Selectable`, `MultiSelectable`,
`TextEditable`, `Clearable`, ... implementations on default proxies.
"""

from ...core.adapter import Adapter
from ...core.devices import AdapterKeyboardProxy, AdapterMouseProxy

__all__ = ['click_adapter', 'ctrl_click_adapter', 'type_keys_on_adapter']


def click_adapter(adapter: Adapter, *, times: int = 1) -> None:
    """Single (or multi) left-click on the adapter's activation point."""
    AdapterMouseProxy(adapter).click(times=times)


def ctrl_click_adapter(adapter: Adapter) -> None:
    """Ctrl+Click on the adapter's activation point.

    Holds ``Ctrl`` down, performs a single click, and releases ``Ctrl``
    in a ``try/finally`` so the modifier is never left pressed if the
    click raises. Used by `MultiSelectable.add_to_selection` /
    `MultiSelectable.remove_from_selection` synthesis on item proxies.
    """
    keyboard = AdapterKeyboardProxy(adapter)
    keyboard.press_keys('<Ctrl>')
    try:
        AdapterMouseProxy(adapter).click()
    finally:
        keyboard.release_keys('<Ctrl>')


def type_keys_on_adapter(adapter: Adapter, sequence: str) -> None:
    """Send a key sequence to the OS while ``adapter`` is the focused element.

    The adapter is expected to be focused already (the proxy that calls
    this helper has typically clicked or invoked focus first); this
    function only delegates to the keyboard subsystem.
    """
    AdapterKeyboardProxy(adapter).type_keys(sequence)
