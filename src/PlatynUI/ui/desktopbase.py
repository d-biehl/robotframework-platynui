# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`DesktopBase` base for desktop-root contexts."""

from typing import override

from ..core.devices import KeyboardAction, MouseAction
from ..core.types import Point
from .element import (
    Element,
    _ElementKeyboardProxy,
    _ElementMouseProxy,
)

__all__ = ['DesktopBase']


class _DesktopMouseProxy(_ElementMouseProxy):
    """Mouse proxy for the desktop root with screen origin ``(0, 0)`` as default click position."""

    @property
    @override
    def default_click_position(self) -> Point:
        return Point(0, 0)

    @override
    def before_action(self, action: MouseAction) -> None:
        pass


class _DesktopKeyboardProxy(_ElementKeyboardProxy):
    """Keyboard proxy for the desktop root."""

    @override
    def before_action(self, action: KeyboardAction) -> None:
        pass


class DesktopBase(Element, register=False):
    """Base for desktop-root contexts."""

    default_role = 'Desktop'

    @override
    def _create_mouse_proxy(self) -> _ElementMouseProxy:
        return _DesktopMouseProxy(self)

    @override
    def _create_keyboard_proxy(self) -> _ElementKeyboardProxy:
        return _DesktopKeyboardProxy(self)

    @override
    def _application_is_ready(self) -> bool:
        # Desktop has no enclosing application — always ready.
        return True

    @override
    def _before_get_screenshot(self) -> None:
        # Desktop is always in view; skip the in-view check.
        pass
