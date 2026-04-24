# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.settings``."""

from dataclasses import FrozenInstanceError

import pytest

from PlatynUI.core import Settings


def test_defaults_are_immutable() -> None:
    settings = Settings()
    assert settings.ensure_timeout == pytest.approx(15.0)
    with pytest.raises(FrozenInstanceError):
        settings.ensure_timeout = 1.0  # type: ignore[misc]


def test_current_returns_default_when_unset() -> None:
    Settings.set_current(Settings())
    assert Settings.current().ensure_timeout == pytest.approx(15.0)


def test_set_current_replaces_singleton() -> None:
    try:
        Settings.set_current(Settings(ensure_timeout=42.0))
        assert Settings.current().ensure_timeout == pytest.approx(42.0)
    finally:
        Settings.set_current(Settings())


def test_with_block_pushes_and_restores() -> None:
    Settings.set_current(Settings(ensure_timeout=10.0))
    try:
        with Settings(ensure_timeout=99.0):
            assert Settings.current().ensure_timeout == pytest.approx(99.0)
            with Settings(ensure_timeout=1.0):
                assert Settings.current().ensure_timeout == pytest.approx(1.0)
            assert Settings.current().ensure_timeout == pytest.approx(99.0)
        assert Settings.current().ensure_timeout == pytest.approx(10.0)
    finally:
        Settings.set_current(Settings())


def test_kw_only_construction() -> None:
    with pytest.raises(TypeError):
        Settings(1.0)  # type: ignore[misc]
