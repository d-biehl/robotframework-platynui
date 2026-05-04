# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Adapter-aware concretions of the mouse and keyboard proxies.

`AdapterMouseProxy` and `AdapterKeyboardProxy` bind the abstract
`MouseProxy` / `KeyboardProxy` from `core.devices` to a UI
`Adapter`. They read the bounding box and activation hints from the
adapter's pattern set and expose the activation logger for default
proxies.

The split into a separate module keeps `core.devices` free of any
dependency on `Adapter` and `patterns`, making the abstract layer
reusable in pure-geometric tests.
"""

import logging
from typing import TYPE_CHECKING, override

from . import patterns
from .devices import KeyboardProxy, MouseAction, MouseProxy
from .types import Point, Rect

if TYPE_CHECKING:
    from .adapter import Adapter

__all__ = ['AdapterKeyboardProxy', 'AdapterMouseProxy']

_LOGGER = logging.getLogger('platynui.devices')


class AdapterMouseProxy(MouseProxy):
    """Standard `MouseProxy` bound to a UI adapter.

    Reads the bounding box from the adapter's ``Element`` pattern and
    determines the default click position via a two-stage fallback,
    preferring an explicit activation target over the element centre:

    1. Centre of ``ActivationTarget.activation_area`` if set.
    2. ``ActivationTarget.activation_point`` if the pattern is supported.
    3. Centre of ``Element.bounds`` otherwise.

    When the adapter exposes an ``ActivationTarget.activation_hint``,
    each action logs it on DEBUG via the ``platynui.devices`` logger.
    """

    def __init__(self, adapter: 'Adapter') -> None:
        self._adapter = adapter

    @property
    @override
    def base_rect(self) -> Rect:
        return self._adapter.get_pattern(patterns.Element).bounds

    @property
    @override
    def default_click_position(self) -> Point:
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            target = self._adapter.get_pattern(patterns.ActivationTarget)
            if target.activation_area is not None:
                return target.activation_area.center()
            return target.activation_point
        return self._adapter.get_pattern(patterns.Element).bounds.center()

    @override
    def before_action(self, action: MouseAction) -> None:
        if not _LOGGER.isEnabledFor(logging.DEBUG):
            return
        if not self._adapter.supports_pattern(patterns.ActivationTarget):
            return
        hint = self._adapter.get_pattern(patterns.ActivationTarget).activation_hint
        if hint:
            _LOGGER.debug('mouse %s: %s', action.value, hint)


class AdapterKeyboardProxy(KeyboardProxy):
    """Standard `KeyboardProxy` bound to a UI adapter.

    The adapter reference is held so subclasses can implement focus
    and verification logic via `before_action` /
    `after_action`.
    """

    def __init__(self, adapter: 'Adapter') -> None:
        self._adapter = adapter
