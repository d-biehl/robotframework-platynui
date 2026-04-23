# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Capability-marker pattern ABCs.

See ``docs/python-library-design.md`` section 5 for the design rationale.
Each pattern is an ``abc.ABC`` carrying a Reverse-DNS ``pattern_name``
identifier; concrete implementations live in adapter packages.

The pattern set is aligned 1:1 with the Rust ``pattern_ids`` and
attribute-group modules (``crates/core/src/ui/identifiers.rs``,
``crates/core/src/ui/attributes.rs``). Capability-group patterns
(``Element``, ``Toggleable``, ``Activatable``) bundle related attributes
and actions instead of splitting them into per-attribute markers.

Note: there is intentionally no ``Properties``/``NativeProperties``
pattern — generic key/value reads go through the adapter's namespaced
attribute API (``adapter.attribute_value(name, namespace=...)``) rather
than through a pattern. See design doc §A.4 / §5.
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
