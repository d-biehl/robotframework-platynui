# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Exception hierarchy for PlatynUI.

See design document section A.2. ``ensure_that`` and ``wait_for`` re-raise
:class:`PlatynUIFatalError`, :class:`KeyboardInterrupt` and
:class:`SystemExit` without retry; everything else inheriting from
:class:`PlatynUIError` is considered recoverable.
"""

from __future__ import annotations

__all__ = [
    'AdapterError',
    'AdapterNotFoundError',
    'AdapterNotFoundFatalError',
    'AdapterNotValidError',
    'CannotEnsureError',
    'DeviceError',
    'ElementNotFoundError',
    'EnsureError',
    'InvalidArgumentError',
    'LocatorError',
    'MultipleElementsFoundError',
    'NoDisplayDeviceError',
    'NoKeyboardDeviceError',
    'NoLocatorDefinedError',
    'NoMouseDeviceError',
    'NotAPatternTypeError',
    'NotSupportedError',
    'PatternNotSupportedError',
    'PlatynUIError',
    'PlatynUIFatalError',
]


class PlatynUIError(Exception):
    """Base class for all PlatynUI errors that may be retried."""


class PlatynUIFatalError(PlatynUIError):
    """Non-recoverable error.

    ``ensure_that`` and ``wait_for`` re-raise this immediately instead of
    retrying.
    """


class AdapterNotFoundFatalError(PlatynUIFatalError):
    """Adapter could not be resolved and retrying will not help."""


class AdapterError(PlatynUIError):
    """Base class for adapter-layer problems."""


class AdapterNotValidError(AdapterError):
    """Adapter handle has expired (lifetime ended)."""


class AdapterNotFoundError(AdapterError):
    """Locator did not resolve to an adapter (recoverable)."""


class PatternNotSupportedError(AdapterError):
    """Adapter does not implement the requested pattern."""


class NotAPatternTypeError(AdapterError):
    """The given object is not a recognised :class:`PatternBase` subclass."""


class EnsureError(PlatynUIError):
    """Base class for ``ensure_that`` related errors."""


class CannotEnsureError(EnsureError):
    """``ensure_that`` exhausted its timeout without all predicates holding."""


class LocatorError(PlatynUIError):
    """Base class for locator-related errors."""


class NoLocatorDefinedError(LocatorError):
    """A context-bound element has no locator information."""


class MultipleElementsFoundError(LocatorError):
    """``get_one`` matched more than one element."""


class ElementNotFoundError(AdapterNotFoundError, LocatorError):
    """Locator did not match any element (element-context alias)."""


class DeviceError(PlatynUIError):
    """Base class for input/display device errors."""


class NoMouseDeviceError(DeviceError):
    """No mouse device is available."""


class NoKeyboardDeviceError(DeviceError):
    """No keyboard device is available."""


class NoDisplayDeviceError(DeviceError):
    """No display device is available."""


# Outside the PlatynUIError hierarchy: standard Python expectations.

class NotSupportedError(NotImplementedError):
    """Operation is not supported in the current configuration."""


class InvalidArgumentError(ValueError):
    """Argument failed validation."""
