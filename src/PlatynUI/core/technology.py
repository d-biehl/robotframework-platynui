# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Technology marker base class.

A *technology* identifies an adapter family (the native UI tree, an
in-process mock, a future remote backend). Each technology subclass acts
as a singleton-ish marker so adapters can be grouped and matched against
`WeightCalculator` criteria.
"""

from abc import ABC

from .types import TechnologyName

__all__ = ['Technology']


class Technology(ABC):
    """Base class for adapter technology markers."""

    @property
    def name(self) -> TechnologyName:
        """Human-readable name; defaults to the subclass qualified name."""
        return type(self).__qualname__
