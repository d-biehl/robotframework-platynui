# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""PlatynUI core layer.

Re-exports the primitives needed to build adapters and contexts:
the `Adapter` contract, capability patterns, the runtime
singleton, locator, predicate, and wait helpers, and shared types.
"""

from .adapter import Adapter
from .adapter_devices import AdapterKeyboardProxy, AdapterMouseProxy
from .adapter_factory import AdapterFactory, AdapterFactoryAccessor, RuntimeAdapterFactory, adapter_factory
from .adapter_proxy import AdapterProxy, PatternProxyFactory, pattern_proxy_for
from .adapters import UiNodeAdapter
from .context import ContextBase, ContextFactory, UnknownContext, context
from .descriptor import (
    ElementDescriptor,
    PatternT,
    RootElementDescriptor,
    reset_root_element_storage,
    set_root_element_storage,
)
from .devices import (
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
from .types import FrameworkId, MouseButton, PatternName, RoleName
from .wait import wait_for
from .weight_calculator import AdapterLike, WeightCalculator

__all__ = [
    'Adapter',
    'AdapterError',
    'AdapterFactory',
    'AdapterFactoryAccessor',
    'AdapterKeyboardProxy',
    'AdapterLike',
    'AdapterMouseProxy',
    'AdapterNotFoundError',
    'AdapterNotFoundFatalError',
    'AdapterNotValidError',
    'AdapterProxy',
    'Anchor',
    'CannotEnsureError',
    'ContextBase',
    'ContextFactory',
    'DeviceError',
    'ElementDescriptor',
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
    'PatternT',
    'PlatynUIError',
    'PlatynUIFatalError',
    'RoleName',
    'RootElementDescriptor',
    'Runtime',
    'RuntimeAdapterFactory',
    'Settings',
    'UiNodeAdapter',
    'UnknownContext',
    'VirtualPoint',
    'WeightCalculator',
    'adapter_factory',
    'add_ensure_hook',
    'context',
    'ensure_that',
    'full_repr',
    'locator',
    'pattern_proxy_for',
    'predicate',
    'reset_root_element_storage',
    'runtime',
    'set_root_element_storage',
    'wait_for',
]
