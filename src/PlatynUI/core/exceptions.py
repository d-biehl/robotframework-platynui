# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Exception hierarchy for PlatynUI.

All recoverable errors derive from `PlatynUIError` and are
retried by `ensure_that` and
`wait_for` until their timeout expires.
`PlatynUIFatalError`, `KeyboardInterrupt` and
`SystemExit` propagate immediately.
"""

__all__ = [
    'AdapterError',
    'AdapterNotFoundError',
    'AdapterNotFoundFatalError',
    'AdapterNotValidError',
    'CannotEnsureError',
    'DeviceError',
    'DuplicateRegistrationWarning',
    'ElementNotFoundError',
    'EnsureError',
    'InvalidArgumentError',
    'InvalidResultTypeError',
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
    """Non-recoverable error that bypasses retry loops."""


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
    """The given object is not a recognised `PatternBase` subclass."""


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


class InvalidResultTypeError(TypeError):
    """Operation produced a value whose type does not match the contract."""


class DuplicateRegistrationWarning(UserWarning):
    """Two distinct classes are registered with identical match criteria.

    Emitted by `ContextFactory.register_context` and
    `PatternProxyFactory.register` when the new entry's criteria dict
    equals an existing one's. Re-registering the same class is silent.
    """
