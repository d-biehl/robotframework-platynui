# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""PlatynUI page-object hierarchy.

Public class tree::

    ContextBase                 (core/context.py)
    ├── UnknownContext          (core/context.py — fallback)
    ├── Application             (ui/application.py)
    └── Element                 (ui/element.py)
        ├── DesktopBase         (ui/desktopbase.py)
        │   └── Desktop         (ui/desktop.py — locator "/.")
        └── Control             (ui/control.py — adds focus)
            └── Window          (ui/window.py — window capabilities)
                └── Frame       (ui/window.py — marker subclass)
"""

from .application import Application
from .control import Control
from .desktop import Desktop
from .desktopbase import DesktopBase
from .element import Element
from .window import Frame, Window

__all__ = [
    'Application',
    'Control',
    'Desktop',
    'DesktopBase',
    'Element',
    'Frame',
    'Window',
]
