# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`DesktopBase` page-object base for desktop-root contexts."""

from __future__ import annotations

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
    """Mouse proxy for the desktop root.

    Uses the screen origin ``(0, 0)`` as the default click position
    and skips the interactability checks that `Element` performs.
    """

    @property
    @override
    def default_click_position(self) -> Point:
        return Point(0, 0)

    @override
    def before_action(self, action: MouseAction) -> None:
        # Desktop is always reachable; no top-level / in-view checks.
        pass


class _DesktopKeyboardProxy(_ElementKeyboardProxy):
    """Keyboard proxy for the desktop root with no in-view checks."""

    @override
    def before_action(self, action: KeyboardAction) -> None:
        # Desktop never needs to be brought into view or focused.
        pass


class DesktopBase(Element, register=False):
    """Page-object base for desktop-root contexts.

    Override target for users that want a custom desktop without
    inheriting the ``/.``-locator from `Desktop`.
    """

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
