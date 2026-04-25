# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.desktop`` and ``PlatynUI.ui.desktopbase``.

Covers the absolute ``/.``-locator on `Desktop`, the no-op overrides on
`DesktopBase` (`_application_is_ready`, `_before_get_screenshot`), and
the desktop-specific mouse and keyboard proxies.
"""

from __future__ import annotations

from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.devices import KeyboardAction, MouseAction
from PlatynUI.core.locator import Locator
from PlatynUI.core.types import Point
from PlatynUI.ui.desktop import Desktop
from PlatynUI.ui.desktopbase import (
    DesktopBase,
    _DesktopKeyboardProxy,
    _DesktopMouseProxy,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _desktop_adapter() -> object:
    """Build a minimal Desktop adapter with `Element` pattern only."""
    return make_adapter(role='Desktop', pattern_map={patterns.Element: ElementStub()})


# ---------------------------------------------------------------------------
# Desktop locator
# ---------------------------------------------------------------------------


def test_desktop_uses_absolute_root_locator() -> None:
    assert Desktop._locator == Locator(path='/.', role='Desktop')


def test_desktop_default_role_is_desktop() -> None:
    assert Desktop.default_role == 'Desktop'


def test_desktopbase_default_role_is_desktop() -> None:
    assert DesktopBase.default_role == 'Desktop'


# ---------------------------------------------------------------------------
# _application_is_ready / _before_get_screenshot overrides
# ---------------------------------------------------------------------------


def test_application_is_ready_true_without_has_user_input() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    assert d._application_is_ready() is True


def test_before_get_screenshot_is_noop() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    # Returns None without raising — no in-view check is performed.
    assert d._before_get_screenshot() is None


# ---------------------------------------------------------------------------
# Mouse / keyboard proxy installation
# ---------------------------------------------------------------------------


def test_mouse_proxy_is_desktop_specific() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    assert isinstance(d.mouse, _DesktopMouseProxy)


def test_keyboard_proxy_is_desktop_specific() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    assert isinstance(d.keyboard, _DesktopKeyboardProxy)


# ---------------------------------------------------------------------------
# _DesktopMouseProxy behaviour
# ---------------------------------------------------------------------------


def test_desktop_mouse_default_click_position_is_origin() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    proxy = d.mouse
    assert isinstance(proxy, _DesktopMouseProxy)
    assert proxy.default_click_position == Point(0, 0)


def test_desktop_mouse_before_action_skips_interactability_checks() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    proxy = d.mouse
    assert isinstance(proxy, _DesktopMouseProxy)
    # No exception means the in-view / top-level checks are bypassed.
    for action in MouseAction:
        proxy.before_action(action)


# ---------------------------------------------------------------------------
# _DesktopKeyboardProxy behaviour
# ---------------------------------------------------------------------------


def test_desktop_keyboard_before_action_skips_interactability_checks() -> None:
    d = DesktopBase(adapter=_desktop_adapter())
    proxy = d.keyboard
    assert isinstance(proxy, _DesktopKeyboardProxy)
    for action in KeyboardAction:
        proxy.before_action(action)


# ---------------------------------------------------------------------------
# Desktop subclass inheritance
# ---------------------------------------------------------------------------


def test_desktop_inherits_desktopbase_overrides() -> None:
    d = Desktop(adapter=_desktop_adapter())
    assert isinstance(d.mouse, _DesktopMouseProxy)
    assert isinstance(d.keyboard, _DesktopKeyboardProxy)
    assert d._application_is_ready() is True
