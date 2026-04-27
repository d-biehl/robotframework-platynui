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
    ExpandableStub,
    FocusableStub,
    HasUserInputStub,
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
            patterns.HasUserInput: HasUserInputStub(True),
            patterns.Focusable: FocusableStub(is_focused=True),
        },
    )


def _menu_item_adapter(
    *,
    runtime_id: str = 'mi',
    parent: Adapter | None = None,
    activatable: ActivatableStub | None = None,
    expandable: ExpandableStub | None = None,
) -> Adapter:
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if activatable is not None:
        pmap[patterns.Activatable] = activatable
    if expandable is not None:
        pmap[patterns.Expandable] = expandable
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
        role='Menu', parent=_window_adapter(),
        pattern_map={patterns.Element: ElementStub()},
    )
    assert ContextFactory().find_context_class_for(adapter) is Menu


def test_menu_bar_registered_with_role_menubar() -> None:
    from PlatynUI.core.context import ContextFactory

    adapter = make_adapter(
        role='MenuBar', parent=_window_adapter(),
        pattern_map={patterns.Element: ElementStub()},
    )
    assert ContextFactory().find_context_class_for(adapter) is MenuBar


def test_menu_item_registered_with_role_menuitem() -> None:
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(
        _menu_item_adapter(activatable=ActivatableStub())
    )
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


# ---------------------------------------------------------------------------
# MenuItem.activate — opens ancestor chain outermost → innermost
# ---------------------------------------------------------------------------


def test_activate_expands_ancestors_outer_to_inner() -> None:
    """File → Recent → MyDoc: opens File, then Recent, then activates MyDoc."""
    window = _window_adapter()

    # Outermost ancestor "File"
    file_expand = ExpandableStub(is_expanded=False)
    file_adapter = _menu_item_adapter(
        runtime_id='File',
        parent=window,
        activatable=ActivatableStub(),
        expandable=file_expand,
    )

    # Middle ancestor "Recent"
    recent_expand = ExpandableStub(is_expanded=False)
    recent_adapter = _menu_item_adapter(
        runtime_id='Recent',
        parent=file_adapter,
        activatable=ActivatableStub(),
        expandable=recent_expand,
    )

    # Leaf "MyDoc"
    leaf_activatable = ActivatableStub()
    leaf_adapter = _menu_item_adapter(
        runtime_id='MyDoc',
        parent=recent_adapter,
        activatable=leaf_activatable,
    )

    MenuItem(adapter=leaf_adapter).activate()

    # Both ancestors expanded once, in order, then leaf activated.
    assert file_expand.expand_calls == 1
    assert recent_expand.expand_calls == 1
    assert leaf_activatable.activate_calls == 1


def test_activate_skips_already_expanded_ancestor() -> None:
    window = _window_adapter()

    file_expand = ExpandableStub(is_expanded=True)  # already open
    file_adapter = _menu_item_adapter(
        runtime_id='File',
        parent=window,
        activatable=ActivatableStub(),
        expandable=file_expand,
    )
    leaf_activatable = ActivatableStub()
    leaf_adapter = _menu_item_adapter(
        runtime_id='Open',
        parent=file_adapter,
        activatable=leaf_activatable,
    )

    MenuItem(adapter=leaf_adapter).activate()

    assert file_expand.expand_calls == 0  # untouched
    assert leaf_activatable.activate_calls == 1


def test_activate_silently_skips_ancestor_without_expandable_pattern() -> None:
    """A MenuBar-style ancestor that opens on hover may lack Expandable."""
    window = _window_adapter()

    # Ancestor MenuItem WITHOUT Expandable pattern.
    bar_adapter = _menu_item_adapter(
        runtime_id='Bar',
        parent=window,
        activatable=ActivatableStub(),
        # expandable=None deliberately
    )
    leaf_activatable = ActivatableStub()
    leaf_adapter = _menu_item_adapter(
        runtime_id='Item',
        parent=bar_adapter,
        activatable=leaf_activatable,
    )

    # Must not raise; must still activate the leaf.
    MenuItem(adapter=leaf_adapter).activate()

    assert leaf_activatable.activate_calls == 1


def test_activate_stops_walk_at_window_boundary() -> None:
    """Non-MenuItem ancestors (Menu, MenuBar, Window) are ignored as openers."""
    window = _window_adapter()
    # Direct child of Window — no MenuItem ancestors.
    leaf_activatable = ActivatableStub()
    leaf_adapter = _menu_item_adapter(
        runtime_id='Top',
        parent=window,
        activatable=leaf_activatable,
    )

    MenuItem(adapter=leaf_adapter).activate()

    assert leaf_activatable.activate_calls == 1


def test_activate_walks_through_intermediate_menu_container() -> None:
    """A `Menu` (popup container) sits between two MenuItems and is skipped."""
    window = _window_adapter()

    file_expand = ExpandableStub(is_expanded=False)
    file_adapter = _menu_item_adapter(
        runtime_id='File',
        parent=window,
        activatable=ActivatableStub(),
        expandable=file_expand,
    )

    # The popup `Menu` container that holds the leaf entries.
    menu_adapter = make_adapter(
        role='Menu',
        runtime_id='FilePopup',
        parent=file_adapter,
        pattern_map={patterns.Element: ElementStub()},
    )

    leaf_activatable = ActivatableStub()
    leaf_adapter = _menu_item_adapter(
        runtime_id='Open',
        parent=menu_adapter,
        activatable=leaf_activatable,
    )

    MenuItem(adapter=leaf_adapter).activate()

    # `Menu` is not a MenuItem, so it is skipped during ancestor collection,
    # but the walk continues past it and finds File as an ancestor to expand.
    assert file_expand.expand_calls == 1
    assert leaf_activatable.activate_calls == 1
