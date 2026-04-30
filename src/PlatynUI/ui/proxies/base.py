# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Base default proxies — `ElementProxy` and `ControlProxy`.

Both are markers without synthetic pattern mix-ins: `Element`,
`ActivationTarget`, `Readable`, and `Focusable` are delivered by the
`UiNodeAdapter` directly. They exist so role-only adapters classified
as ``Element`` or ``Control`` still pass through `find_proxy_for` and
inherit the proxy class hierarchy used by more specific subclasses.
"""

from ...core.adapter_proxy import AdapterProxy, pattern_proxy_for

__all__ = ['ControlProxy', 'ElementProxy']


@pattern_proxy_for(role='Element')
class ElementProxy(AdapterProxy):
    """Default proxy for adapters classified as ``Element``."""


@pattern_proxy_for(role='Control')
class ControlProxy(ElementProxy):
    """Default proxy for adapters classified as ``Control``."""
