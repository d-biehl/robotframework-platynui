# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Activation pattern."""

from __future__ import annotations

from abc import abstractmethod

from .base import PatternBase

__all__ = ['Activatable']


class Activatable(PatternBase):
    """An element that can be activated, e.g. a button click or menu invoke.

    Combines the activation action with its observable state, so
    callers can both check whether activation is currently allowed
    and invoke it.
    """

    pattern_name = 'org.platynui.patterns.Activatable'

    @abstractmethod
    def activate(self) -> None:
        """Trigger the element's primary action."""

    @property
    @abstractmethod
    def is_activation_enabled(self) -> bool:
        """Whether `activate` is currently available."""

    @property
    @abstractmethod
    def default_accelerator(self) -> str | None:
        """The suggested keyboard accelerator (e.g. ``"Enter"``), or ``None``."""
