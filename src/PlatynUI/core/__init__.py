# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""PlatynUI core layer (Phase 1: foundation modules).

Public re-exports for the most commonly used primitives. Concrete adapter
implementations and page-object base classes land in subsequent phases.
"""

from __future__ import annotations

from .adapter import Adapter
from .adapter_proxy import AdapterFacade, AdapterProxy, PatternProxyFactory, pattern_proxy_for
from .adapters import UiNodeAdapter, UiNodeTechnology
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
from .types import FrameworkId, PatternName, RoleName, TechnologyName
from .wait import wait_for
from .weight_calculator import AdapterLike, WeightCalculator

__all__ = [
    'Adapter',
    'AdapterError',
    'AdapterFacade',
    'AdapterLike',
    'AdapterNotFoundError',
    'AdapterNotFoundFatalError',
    'AdapterNotValidError',
    'AdapterProxy',
    'CannotEnsureError',
    'DeviceError',
    'ElementNotFoundError',
    'EnsureError',
    'FrameworkId',
    'InvalidArgumentError',
    'Locator',
    'LocatorError',
    'LocatorScope',
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
