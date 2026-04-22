# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Activation pattern."""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Activatable']


class Activatable(PatternBase):
    """Element can be activated (button click, menu invoke, ...).

    Bundles the action (:meth:`activate`) with its observable attributes
    (:attr:`is_activation_enabled`, :attr:`default_accelerator`) —
    mirrors the Rust ``activatable`` capability group.
    """

    pattern_name = 'org.platynui.patterns.Activatable'

    @abstractmethod
    def activate(self) -> None: ...

    @property
    @abstractmethod
    def is_activation_enabled(self) -> bool: ...

    @property
    @abstractmethod
    def default_accelerator(self) -> str | None:
        """Suggested keyboard accelerator (e.g. ``"Enter"``), or :data:`None`."""
