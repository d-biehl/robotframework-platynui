# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Capability-marker pattern ABCs (design document section 5).

Patterns describe **what** an element can do; concrete implementations
live in adapter packages. Each subclass declares a stable Reverse-DNS
:attr:`PatternBase.pattern_name` so that adapters can advertise and
invoke patterns over wire protocols (Rust FFI, JSON-RPC, ...).
"""

from __future__ import annotations

from abc import ABC
from typing import ClassVar

from ..types import PatternName

__all__ = ['PatternBase']


class PatternBase(ABC):
    """Base class for all capability markers.

    Subclasses must set :attr:`pattern_name` to a stable Reverse-DNS
    identifier (e.g. ``"org.platynui.patterns.Activatable"``). The
    framework does **not** validate the format — Reverse-DNS is a
    convention, not a hard rule, mirroring the Rust ``PatternId`` type.
    Third-party patterns should use their own namespace
    (``com.acme.patterns.*``) to avoid collisions.
    """

    pattern_name: ClassVar[PatternName]
