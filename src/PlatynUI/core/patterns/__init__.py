# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Capability-marker pattern ABCs.

Each pattern is an `ABC` carrying a Reverse-DNS
``pattern_name`` identifier; concrete implementations live in adapter
packages.
"""

from ..types import Point, Rect, Size
from .activation import Activatable
from .activation_target import ActivationTarget
from .application_ready import ApplicationReady
from .base import PatternBase
from .closeable import Closeable
from .element import Element
from .expandable import Expandable
from .focusable import Focusable
from .has_editor import HasEditor
from .item_container import ItemContainer
from .maximizable import Maximizable
from .minimizable import Minimizable
from .movable import Movable
from .readable import Readable
from .resizable import Resizable
from .responsive import Responsive
from .restorable import Restorable
from .selectable import Selectable
from .text import Clearable, TextContent, TextEditable
from .toggle import Toggleable, ToggleState
from .window_state import WindowState

__all__ = [
    'Activatable',
    'ActivationTarget',
    'ApplicationReady',
    'Clearable',
    'Closeable',
    'Element',
    'Expandable',
    'Focusable',
    'HasEditor',
    'ItemContainer',
    'Maximizable',
    'Minimizable',
    'Movable',
    'PatternBase',
    'Point',
    'Readable',
    'Rect',
    'Resizable',
    'Responsive',
    'Restorable',
    'Selectable',
    'Size',
    'TextContent',
    'TextEditable',
    'ToggleState',
    'Toggleable',
    'WindowState',
]
