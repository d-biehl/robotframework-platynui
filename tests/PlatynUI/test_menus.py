# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false

"""Unit tests for ``PlatynUI.ui.menus``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ActivatableStub,
    ElementStub,
    FocusableStub,
    ResponsiveStub,
    WindowStateStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.settings import Settings
from PlatynUI.ui.menus import Menu, MenuBar, MenuItem


@pytest.fixture(autouse=True)
def _fast_settings() -> Iterator[None]:
    with Settings(
        ensure_timeout=0.05,
        ensure_delay=0.0,
        exists_timeout=0.05,
        wait_for_timeout=0.05,
        wait_for_delay=0.0,
    ):
        yield


def _window_adapter() -> Adapter:
    desktop = make_adapter(role='Desktop')
    return make_adapter(  # type: ignore[no-any-return]
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.Responsive: ResponsiveStub(True),
            patterns.WindowState: WindowStateStub(is_active=True),
            patterns.Focusable: FocusableStub(is_focused=True),
        },
    )


def _menu_item_adapter(
    *,
    runtime_id: str = 'mi',
    parent: Adapter | None = None,
    activatable: ActivatableStub | None = None,
) -> Adapter:
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if activatable is not None:
        pmap[patterns.Activatable] = activatable
    return make_adapter(  # type: ignore[no-any-return]
        role='MenuItem',
        runtime_id=runtime_id,
        parent=parent if parent is not None else _window_adapter(),
        pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def test_menu_registered_with_role_menu() -> None:
    from PlatynUI.core.context import ContextFactory

    adapter = make_adapter(
        role='Menu',
        parent=_window_adapter(),
        pattern_map={patterns.Element: ElementStub()},
    )
    assert ContextFactory().find_context_class_for(adapter) is Menu


def test_menu_bar_registered_with_role_menubar() -> None:
    from PlatynUI.core.context import ContextFactory

    adapter = make_adapter(
        role='MenuBar',
        parent=_window_adapter(),
        pattern_map={patterns.Element: ElementStub()},
    )
    assert ContextFactory().find_context_class_for(adapter) is MenuBar


def test_menu_item_registered_with_role_menuitem() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_menu_item_adapter(activatable=ActivatableStub()))
    assert cls is MenuItem


# ---------------------------------------------------------------------------
# MenuItem.activate — leaf without ancestors
# ---------------------------------------------------------------------------


def test_activate_calls_activatable_when_no_ancestors() -> None:
    activatable = ActivatableStub()
    leaf = _menu_item_adapter(activatable=activatable)

    MenuItem(adapter=leaf).activate()

    assert activatable.activate_calls == 1


def test_activate_raises_when_activatable_pattern_missing() -> None:
    leaf = _menu_item_adapter()  # no Activatable in pmap
    with pytest.raises(PatternNotSupportedError):
        MenuItem(adapter=leaf).activate()
