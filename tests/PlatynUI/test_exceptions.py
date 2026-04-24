# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.exceptions``."""

from PlatynUI.core import exceptions as exc


def test_fatal_inherits_from_base() -> None:
    assert issubclass(exc.PlatynUIFatalError, exc.PlatynUIError)
    assert issubclass(exc.AdapterNotFoundFatalError, exc.PlatynUIFatalError)


def test_adapter_hierarchy() -> None:
    assert issubclass(exc.AdapterNotFoundError, exc.AdapterError)
    assert issubclass(exc.AdapterNotValidError, exc.AdapterError)
    assert issubclass(exc.PatternNotSupportedError, exc.AdapterError)
    assert issubclass(exc.NotAPatternTypeError, exc.AdapterError)
    assert issubclass(exc.AdapterError, exc.PlatynUIError)


def test_ensure_hierarchy() -> None:
    assert issubclass(exc.CannotEnsureError, exc.EnsureError)
    assert issubclass(exc.EnsureError, exc.PlatynUIError)


def test_locator_hierarchy() -> None:
    assert issubclass(exc.NoLocatorDefinedError, exc.LocatorError)
    assert issubclass(exc.MultipleElementsFoundError, exc.LocatorError)
    # ElementNotFoundError participates in both branches.
    assert issubclass(exc.ElementNotFoundError, exc.AdapterNotFoundError)
    assert issubclass(exc.ElementNotFoundError, exc.LocatorError)


def test_device_hierarchy() -> None:
    assert issubclass(exc.NoMouseDeviceError, exc.DeviceError)
    assert issubclass(exc.NoKeyboardDeviceError, exc.DeviceError)
    assert issubclass(exc.NoDisplayDeviceError, exc.DeviceError)


def test_standard_python_aliases() -> None:
    assert issubclass(exc.NotSupportedError, NotImplementedError)
    assert issubclass(exc.InvalidArgumentError, ValueError)
    # And they are NOT in the PlatynUIError tree.
    assert not issubclass(exc.NotSupportedError, exc.PlatynUIError)
    assert not issubclass(exc.InvalidArgumentError, exc.PlatynUIError)
