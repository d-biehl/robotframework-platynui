# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Consistency suite for Read/Action pattern pairing across default proxies (Rev. 46).

Whenever a default proxy class exposes a Read-pattern (`IsSelectable`,
`IsExpandable`), the same proxy class must also expose the matching
Action-pattern (`Selectable`, `Expandable`). This guards against
asymmetric registrations where a UI class can read state but never
trigger the action — or vice versa.
"""

import importlib

import pytest

from PlatynUI.core import patterns
from PlatynUI.core.adapter_proxy import PatternProxyFactory

# Force registration of the default proxies under test.
importlib.import_module('PlatynUI.ui.proxies')


READ_TO_ACTION: dict[type, type] = {
    patterns.IsSelectable: patterns.Selectable,
    patterns.IsExpandable: patterns.Expandable,
}


@pytest.mark.parametrize(('read_pattern', 'action_pattern'), list(READ_TO_ACTION.items()))
def test_read_pattern_implies_action_pattern_per_proxy(
    read_pattern: type,
    action_pattern: type,
) -> None:
    """Every proxy carrying a Read-pattern also carries the Action-pattern."""
    offenders: list[str] = []
    for entry in PatternProxyFactory.registrations():
        proxy_cls = entry.proxy_cls
        if issubclass(proxy_cls, read_pattern) and not issubclass(proxy_cls, action_pattern):
            offenders.append(f'{proxy_cls.__module__}.{proxy_cls.__qualname__}')
    assert offenders == [], f'proxies expose {read_pattern.__name__} without {action_pattern.__name__}: {offenders}'


@pytest.mark.parametrize(('read_pattern', 'action_pattern'), list(READ_TO_ACTION.items()))
def test_action_pattern_implies_read_pattern_per_proxy(
    read_pattern: type,
    action_pattern: type,
) -> None:
    """Every proxy carrying an Action-pattern also carries the Read-pattern."""
    offenders: list[str] = []
    for entry in PatternProxyFactory.registrations():
        proxy_cls = entry.proxy_cls
        if issubclass(proxy_cls, action_pattern) and not issubclass(proxy_cls, read_pattern):
            offenders.append(f'{proxy_cls.__module__}.{proxy_cls.__qualname__}')
    assert offenders == [], f'proxies expose {action_pattern.__name__} without {read_pattern.__name__}: {offenders}'


def test_multi_selectable_proxies_also_expose_is_selectable() -> None:
    """`MultiSelectable` operates on items — they must also report `IsSelectable`."""
    offenders: list[str] = []
    for entry in PatternProxyFactory.registrations():
        proxy_cls = entry.proxy_cls
        if issubclass(proxy_cls, patterns.MultiSelectable) and not issubclass(proxy_cls, patterns.IsSelectable):
            offenders.append(f'{proxy_cls.__module__}.{proxy_cls.__qualname__}')
    assert offenders == [], f'proxies expose MultiSelectable without IsSelectable: {offenders}'
