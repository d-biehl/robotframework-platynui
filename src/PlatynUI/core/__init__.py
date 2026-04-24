# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""PlatynUI core layer.

Re-exports the primitives needed to build adapters and page objects:
the `Adapter` contract, capability patterns, the runtime
singleton, locator, predicate, and wait helpers, and shared types.
"""

from .adapter import Adapter
from .adapter_proxy import AdapterFacade, AdapterProxy, PatternProxyFactory, pattern_proxy_for
from .adapters import UiNodeAdapter, UiNodeTechnology
from .devices import (
    AdapterKeyboardProxy,
    AdapterMouseProxy,
    Anchor,
    KeyboardAction,
    KeyboardProxy,
    MouseAction,
    MouseProxy,
    VirtualPoint,
)
from .ensure import add_ensure_hook, ensure_that, full_repr
from .exceptions import (
    AdapterError,
    AdapterNotFoundError,
    AdapterNotFoundFatalError,
    AdapterNotValidError,
    CannotEnsureError,
    DeviceError,
    ElementNotFoundError,
    EnsureError,
    InvalidArgumentError,
    LocatorError,
    MultipleElementsFoundError,
    NoDisplayDeviceError,
    NoKeyboardDeviceError,
    NoLocatorDefinedError,
    NoMouseDeviceError,
    NotAPatternTypeError,
    NotSupportedError,
    PatternNotSupportedError,
    PlatynUIError,
    PlatynUIFatalError,
)
from .locator import Locator, LocatorScope, locator
from .predicate import predicate
from .runtime import Runtime, runtime
from .settings import Settings
from .technology import Technology
from .types import FrameworkId, MouseButton, PatternName, RoleName, TechnologyName
from .wait import wait_for
from .weight_calculator import AdapterLike, WeightCalculator

__all__ = [
    'Adapter',
    'AdapterError',
    'AdapterFacade',
    'AdapterKeyboardProxy',
    'AdapterLike',
    'AdapterMouseProxy',
    'AdapterNotFoundError',
    'AdapterNotFoundFatalError',
    'AdapterNotValidError',
    'AdapterProxy',
    'Anchor',
    'CannotEnsureError',
    'DeviceError',
    'ElementNotFoundError',
    'EnsureError',
    'FrameworkId',
    'InvalidArgumentError',
    'KeyboardAction',
    'KeyboardProxy',
    'Locator',
    'LocatorError',
    'LocatorScope',
    'MouseAction',
    'MouseButton',
    'MouseProxy',
    'MultipleElementsFoundError',
    'NoDisplayDeviceError',
    'NoKeyboardDeviceError',
    'NoLocatorDefinedError',
    'NoMouseDeviceError',
    'NotAPatternTypeError',
    'NotSupportedError',
    'PatternName',
    'PatternNotSupportedError',
    'PatternProxyFactory',
    'PlatynUIError',
    'PlatynUIFatalError',
    'RoleName',
    'Runtime',
    'Settings',
    'Technology',
    'TechnologyName',
    'UiNodeAdapter',
    'UiNodeTechnology',
    'VirtualPoint',
    'WeightCalculator',
    'add_ensure_hook',
    'ensure_that',
    'full_repr',
    'locator',
    'pattern_proxy_for',
    'predicate',
    'runtime',
    'wait_for',
]
