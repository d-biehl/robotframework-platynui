# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Concrete :class:`~PlatynUI.core.adapter.Adapter` implementations.

Currently only the native :class:`UiNodeAdapter` (backed by the Rust
``platynui_native.UiNode``) is shipped — see design doc §A.4a. Additional
adapter backends are intentionally out of scope; the runtime treats the
native UiNode as *the* technology layer and Python-side variation
(stubs, spies, scripted behaviour) is composed via
:class:`~PlatynUI.core.adapter_proxy.AdapterProxy` overlays instead of
parallel adapter implementations.
"""

from __future__ import annotations

from .ui_node import UiNodeAdapter, UiNodeTechnology

__all__ = ['UiNodeAdapter', 'UiNodeTechnology']
