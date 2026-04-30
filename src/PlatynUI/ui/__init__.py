# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""PlatynUI context hierarchy.

Public class tree::

    ContextBase                 (core/context.py)
    ├── UnknownContext          (core/context.py — fallback)
    ├── Application             (ui/application.py)
    └── Element                 (ui/element.py)
        ├── DesktopBase         (ui/desktopbase.py)
        │   └── Desktop         (ui/desktop.py — locator "/.")
        ├── Item                (ui/item.py — marker, register=False)
        │   ├── ListItem        (ui/lists.py)
        │   ├── TreeItem        (ui/tree.py)
        │   ├── Row             (ui/table.py)
        │   ├── Cell            (ui/table.py)
        │   └── TabItem         (ui/tabs.py)
        └── Control             (ui/control.py — adds focus)
            ├── AbstractButton  (ui/buttons.py — register=False)
            │   ├── Button      (ui/buttons.py)
            │   └── CheckBox    (ui/buttons.py)
            ├── Text            (ui/text.py)
            ├── Edit            (ui/text.py)
            ├── ComboBox        (ui/combobox.py)
            ├── List            (ui/lists.py)
            ├── Tree            (ui/tree.py)
            ├── Table           (ui/table.py)
            ├── TabList         (ui/tabs.py)
            ├── Menu            (ui/menus.py)
            ├── MenuBar         (ui/menus.py)
            ├── MenuItem        (ui/menus.py)
            └── Window          (ui/window.py — window capabilities)
                └── Frame       (ui/window.py — marker subclass)
"""

from .application import Application
from .buttons import AbstractButton, Button, CheckBox
from .combobox import ComboBox
from .control import Control
from .desktop import Desktop
from .desktopbase import DesktopBase
from .element import Element
from .item import Item
from .lists import List, ListItem
from .menus import Menu, MenuBar, MenuItem
from .table import Cell, Row, Table
from .tabs import TabItem, TabList
from .text import Edit, Text
from .tree import Tree, TreeItem
from .window import Frame, Window

__all__ = [
    'AbstractButton',
    'Application',
    'Button',
    'Cell',
    'CheckBox',
    'ComboBox',
    'Control',
    'Desktop',
    'DesktopBase',
    'Edit',
    'Element',
    'Frame',
    'Item',
    'List',
    'ListItem',
    'Menu',
    'MenuBar',
    'MenuItem',
    'Row',
    'TabItem',
    'TabList',
    'Table',
    'Text',
    'Tree',
    'TreeItem',
    'Window',
]
