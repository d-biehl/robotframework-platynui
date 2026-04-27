# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Concrete `Adapter` implementations.

Ships the single production adapter, `UiNodeAdapter`, which
wraps the platform UI tree. Test variations are layered on top
through `AdapterProxy` overlays
rather than through alternative adapter classes.
"""

from .ui_node import UiNodeAdapter

__all__ = ['UiNodeAdapter']
