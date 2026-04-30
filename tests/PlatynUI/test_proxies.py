# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests poke proxy module internals (monkeypatching ``click_adapter`` /
# ``type_keys_on_adapter`` rebound symbols) and that's the whole point.
# ``reportUnnecessaryTypeIgnoreComment`` is disabled because the
# ``# type: ignore[no-any-return]`` markers are needed by mypy but
# pyright doesn't flag the underlying ``Any`` return.

"""Tests for the default proxies in :mod:`PlatynUI.ui.proxies`.

Covers the synthetic `Activatable` / `Toggleable` / `TextEditable` /
`Clearable` / `Selectable` / `Expandable` implementations on item-flavoured
proxies. Container-side lookups (`List`, `Tree`, `TabList`, `Menu`,
`MenuBar`, `Table`, `ComboBox`) live on their context classes via
`ItemContainer[I]` and are covered by the per-context test modules.

The tests monkeypatch the action helpers (`click_adapter`,
`type_keys_on_adapter`) at the call site (each proxy module imports
them by name, so rebinding in that module is what intercepts the
call). No real input is generated.
"""

from typing import Any
from unittest.mock import MagicMock

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ElementStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_proxy import PatternProxyFactory
from PlatynUI.core.patterns import ToggleState
from PlatynUI.ui.proxies import (
    base,
    buttons,
    combobox,
    item,
    text,
)

# ``base`` etc. are imported for their import-side-effect (registering
# the proxies); silence the unused-import diagnostic.
_ = (base, buttons, combobox, item, text)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _adapter(
    *,
    role: str,
    children: 'list[Adapter] | None' = None,
    attributes: 'dict[tuple[str, str], object] | None' = None,
    is_enabled: bool = True,
) -> Adapter:
    """Build an adapter with an :class:`Element` stub already wired up."""
    return make_adapter(  # type: ignore[no-any-return]
        role=role,
        children=children or [],
        attributes=attributes,
        pattern_map={patterns.Element: ElementStub(is_enabled=is_enabled)},
    )


def _resolve(adapter: Adapter) -> Any:
    """Resolve the adapter to its proxy via the registry."""
    return PatternProxyFactory.find_proxy_for(adapter)


@pytest.fixture
def patch_actions(monkeypatch: pytest.MonkeyPatch) -> dict[str, MagicMock]:
    """Patch :func:`click_adapter` / :func:`type_keys_on_adapter` in
    every proxy module that imported them by name."""
    click = MagicMock(name='click_adapter')
    typer = MagicMock(name='type_keys_on_adapter')
    for module in (buttons, combobox, item, text):
        if hasattr(module, 'click_adapter'):
            monkeypatch.setattr(module, 'click_adapter', click)
        if hasattr(module, 'type_keys_on_adapter'):
            monkeypatch.setattr(module, 'type_keys_on_adapter', typer)
    return {'click': click, 'type_keys': typer}


# ---------------------------------------------------------------------------
# Registry — every default proxy resolves
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    ('role', 'expected_type'),
    [
        ('Element', base.ElementProxy),
        ('Control', base.ControlProxy),
        ('Button', buttons.ButtonProxy),
        ('CheckBox', buttons.CheckBoxProxy),
        ('Edit', text.EditProxy),
        ('Text', text.TextProxy),
        ('ComboBox', combobox.ComboBoxProxy),
        ('Item', item.ItemProxy),
        ('ListItem', item.ListItemProxy),
        ('TabItem', item.TabItemProxy),
        ('TreeItem', item.TreeItemProxy),
        ('Row', item.RowProxy),
        ('Cell', item.CellProxy),
        ('MenuItem', item.MenuItemProxy),
    ],
)
def test_registry_resolves_role_to_proxy(role: str, expected_type: type) -> None:
    a = _adapter(role=role)
    proxy = _resolve(a)
    assert isinstance(proxy, expected_type)


# ---------------------------------------------------------------------------
# ButtonProxy
# ---------------------------------------------------------------------------


def test_button_proxy_activate_clicks(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Button')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Activatable).activate()
    patch_actions['click'].assert_called_once_with(a)


def test_button_proxy_is_activation_enabled_reads_element() -> None:
    a = _adapter(role='Button', is_enabled=False)
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.Activatable).is_activation_enabled is False


# ---------------------------------------------------------------------------
# CheckBoxProxy
# ---------------------------------------------------------------------------


def test_checkbox_proxy_toggle_clicks(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='CheckBox')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Toggleable).toggle()
    patch_actions['click'].assert_called_once_with(a)


def test_checkbox_proxy_state_reads_is_toggled_bool() -> None:
    a = _adapter(role='CheckBox', attributes={('IsToggled', 'control'): True})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.Toggleable).state is ToggleState.ON


def test_checkbox_proxy_state_falls_back_to_is_selected() -> None:
    a = _adapter(role='CheckBox', attributes={('IsSelected', 'control'): True})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.Toggleable).state is ToggleState.ON


def test_checkbox_proxy_state_off_when_no_attribute() -> None:
    a = _adapter(role='CheckBox')
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.Toggleable).state is ToggleState.OFF


# ---------------------------------------------------------------------------
# EditProxy / TextProxy
# ---------------------------------------------------------------------------


def test_edit_proxy_text_reads_value_first() -> None:
    a = _adapter(
        role='Edit',
        attributes={('Value', 'control'): 'hello', ('Text', 'control'): 'ignored'},
    )
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.TextContent).text == 'hello'


def test_edit_proxy_text_falls_back_to_text_attribute() -> None:
    a = _adapter(role='Edit', attributes={('Text', 'control'): 'fallback'})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.TextContent).text == 'fallback'


def test_edit_proxy_set_text_sequence(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Edit')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.TextEditable).set_text('xyz')
    patch_actions['click'].assert_called_once_with(a)
    assert patch_actions['type_keys'].call_args_list == [
        ((a, '<Ctrl+A>'), {}),
        ((a, 'xyz'), {}),
    ]


def test_edit_proxy_clear_sequence(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Edit')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Clearable).clear()
    patch_actions['click'].assert_called_once_with(a)
    assert patch_actions['type_keys'].call_args_list == [
        ((a, '<Ctrl+A>'), {}),
        ((a, '<Delete>'), {}),
    ]


def test_edit_proxy_max_length_returns_none_for_negative() -> None:
    a = _adapter(role='Edit', attributes={('MaxLength', 'control'): -1})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.TextEditable).max_length is None


def test_edit_proxy_max_length_returns_int_for_positive() -> None:
    a = _adapter(role='Edit', attributes={('MaxLength', 'control'): 32})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.TextEditable).max_length == 32


def test_text_proxy_inherits_edit_proxy_behaviour() -> None:
    a = _adapter(
        role='Text',
        attributes={('Value', 'control'): 'label', ('IsReadOnly', 'control'): True},
    )
    proxy = _resolve(a)
    tc = proxy.get_pattern(patterns.TextContent)
    te = proxy.get_pattern(patterns.TextEditable)
    assert tc.text == 'label'
    assert te.is_readonly is True


# ---------------------------------------------------------------------------
# ItemProxy + subclasses
# ---------------------------------------------------------------------------


def test_item_proxy_select_clicks_once(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Item')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Selectable).select()
    patch_actions['click'].assert_called_once_with(a)


def test_item_proxy_activate_double_clicks(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Item')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Activatable).activate()
    patch_actions['click'].assert_called_once_with(a, times=2)


def test_item_proxy_open_editor_double_clicks(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='Item')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.HasEditor).open_editor()
    patch_actions['click'].assert_called_once_with(a, times=2)


def test_tree_item_proxy_expand_clicks_when_collapsed(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='TreeItem', attributes={('IsExpanded', 'control'): False})
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Expandable).expand()
    patch_actions['click'].assert_called_once_with(a)


def test_tree_item_proxy_expand_noop_when_expanded(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='TreeItem', attributes={('IsExpanded', 'control'): True})
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Expandable).expand()
    patch_actions['click'].assert_not_called()


def test_tree_item_proxy_can_expand_reads_attribute() -> None:
    a = _adapter(role='TreeItem', attributes={('CanExpand', 'control'): True})
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.IsExpandable).can_expand is True


def test_tree_item_proxy_can_expand_falls_back_to_child_role() -> None:
    child = _adapter(role='TreeItem')
    a = _adapter(role='TreeItem', children=[child])
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.IsExpandable).can_expand is True


def test_tree_item_proxy_can_expand_false_when_no_children() -> None:
    a = _adapter(role='TreeItem')
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.IsExpandable).can_expand is False


def test_menu_item_proxy_can_expand_falls_back_to_child_role() -> None:
    child = _adapter(role='MenuItem')
    a = _adapter(role='MenuItem', children=[child])
    proxy = _resolve(a)
    assert proxy.get_pattern(patterns.IsExpandable).can_expand is True


# ---------------------------------------------------------------------------
# ComboBoxProxy
# ---------------------------------------------------------------------------


def test_combobox_expand_clicks_when_collapsed(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='ComboBox', attributes={('IsExpanded', 'control'): False})
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Expandable).expand()
    patch_actions['click'].assert_called_once_with(a)


def test_combobox_collapse_clicks_when_expanded(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='ComboBox', attributes={('IsExpanded', 'control'): True})
    proxy = _resolve(a)
    proxy.get_pattern(patterns.Expandable).collapse()
    patch_actions['click'].assert_called_once_with(a)


def test_combobox_set_text_sequence(patch_actions: dict[str, MagicMock]) -> None:
    a = _adapter(role='ComboBox')
    proxy = _resolve(a)
    proxy.get_pattern(patterns.TextEditable).set_text('foo')
    patch_actions['click'].assert_called_once_with(a)
    assert patch_actions['type_keys'].call_args_list == [
        ((a, '<Ctrl+A>'), {}),
        ((a, 'foo'), {}),
    ]
