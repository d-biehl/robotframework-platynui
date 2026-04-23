# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Capability-marker pattern ABCs.

Each pattern is an `ABC` carrying a Reverse-DNS
``pattern_name`` identifier; concrete implementations live in adapter
packages.

Capability-group patterns (`Element`, `Toggleable`,
`Activatable`) bundle related state and actions into a single
pattern instead of splitting them into per-attribute markers.

There is no generic ``Properties`` pattern: arbitrary key/value reads
go through the adapter's namespaced attribute API
(``adapter.attribute_value(name, namespace=...)``).
"""

from __future__ import annotations

from ..types import Point, Rect
from .activation import Activatable
from .activation_target import ActivationTarget
from .base import PatternBase
from .element import Element
from .focusable import Focusable
from .text import Clearable, TextContent, TextEditable
from .toggle import Toggleable, ToggleState

__all__ = [
    'Activatable',
    'ActivationTarget',
    'Clearable',
    'Element',
    'Focusable',
    'PatternBase',
    'Point',
    'Rect',
    'TextContent',
    'TextEditable',
    'ToggleState',
    'Toggleable',
]
