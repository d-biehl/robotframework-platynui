# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Technology marker base class.

A *technology* identifies an adapter family (e.g. the Rust-backed UI tree,
a JSON-RPC remote, the in-process mock). Each technology subclass is a
singleton-ish marker; the actual ``adapter_factory`` hook is added in
Phase 2 once the ``Adapter`` ABC lands.
"""

from __future__ import annotations

from abc import ABC

from .types import TechnologyName

__all__ = ['Technology']


class Technology(ABC):
    """Base class for adapter technology markers."""

    @property
    def name(self) -> TechnologyName:
        """Human-readable name; defaults to the subclass qualified name."""
        return type(self).__qualname__
