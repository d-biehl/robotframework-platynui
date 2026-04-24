# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Base class for capability-marker patterns.

Patterns describe what an element can do; concrete implementations
live in adapter packages. Each subclass declares a stable Reverse-DNS
`pattern_name` so adapters can advertise and invoke
patterns across process and language boundaries.
"""

from abc import ABC
from typing import ClassVar

from ..types import PatternName

__all__ = ['PatternBase']


class PatternBase(ABC):
    """Base class for all capability markers.

    Subclasses must set `pattern_name` to a stable Reverse-DNS
    identifier such as ``"org.platynui.patterns.Activatable"``. The
    framework does not validate the format; Reverse-DNS is a
    convention that keeps third-party patterns
    (``com.acme.patterns.*``) free of collisions with built-ins.
    """

    pattern_name: ClassVar[PatternName]
