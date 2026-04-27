# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for ``PlatynUI.core.adapter_factory``.

Two test groups:

* Singleton-accessor tests use a fresh ``AdapterFactoryAccessor`` to
  avoid touching the module-level ``adapter_factory`` (mirrors the
  ``test_runtime`` style).
* ``RuntimeAdapterFactory`` tests run against the bundled mock
  provider via ``runtime.override_with_mock()``.
"""

from collections.abc import Generator
from unittest.mock import MagicMock

import platynui_native as _pn
import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import (
    AdapterFactory,
    AdapterFactoryAccessor,
    RuntimeAdapterFactory,
    adapter_factory,
)
from PlatynUI.core.adapters import UiNodeAdapter
from PlatynUI.core.exceptions import InvalidResultTypeError
from PlatynUI.core.locator import Locator
from PlatynUI.core.runtime import runtime

# ----------------------------------------------------------------------
# Module-level singleton
# ----------------------------------------------------------------------


def test_module_singleton_is_accessor_instance() -> None:
    assert isinstance(adapter_factory, AdapterFactoryAccessor)


# ----------------------------------------------------------------------
# Initial state
# ----------------------------------------------------------------------


def test_fresh_accessor_is_not_initialised() -> None:
    af = AdapterFactoryAccessor()
    assert af.is_initialised() is False
    assert af.is_sealed() is False


# ----------------------------------------------------------------------
# Lazy default build & sealing
# ----------------------------------------------------------------------


def test_current_lazy_creates_default() -> None:
    af = AdapterFactoryAccessor()
    instance = af.current
    assert isinstance(instance, RuntimeAdapterFactory)
    assert af.is_initialised() is True
    assert af.is_sealed() is True


def test_current_returns_same_instance() -> None:
    af = AdapterFactoryAccessor()
    assert af.current is af.current


def test_use_default_after_sealing_raises() -> None:
    af = AdapterFactoryAccessor()
    _ = af.current  # seal
    with pytest.raises(RuntimeError, match='already initialised'):
        af.use_default()


def test_use_factory_after_sealing_raises() -> None:
    af = AdapterFactoryAccessor()
    _ = af.current
    with pytest.raises(RuntimeError, match='already initialised'):
        af.use_factory(RuntimeAdapterFactory)


# ----------------------------------------------------------------------
# Variant selection before sealing
# ----------------------------------------------------------------------


def test_use_factory_changes_builder() -> None:
    af = AdapterFactoryAccessor()
    custom = MagicMock(spec=AdapterFactory)
    af.use_factory(lambda: custom)
    assert af.current is custom


def test_use_default_overrides_previous_choice() -> None:
    af = AdapterFactoryAccessor()
    af.use_factory(lambda: MagicMock(spec=AdapterFactory))
    af.use_default()  # last choice wins, builder still mutable until sealing
    assert isinstance(af.current, RuntimeAdapterFactory)


# ----------------------------------------------------------------------
# Override (scope-bound)
# ----------------------------------------------------------------------


def test_override_activates_then_restores() -> None:
    af = AdapterFactoryAccessor()
    base = af.current  # seal with default
    custom = MagicMock(spec=AdapterFactory)

    with af.override(lambda: custom) as active:
        assert active is custom
        assert af.current is custom

    assert af.current is base


def test_override_supports_nesting() -> None:
    af = AdapterFactoryAccessor()
    base = af.current
    a = MagicMock(spec=AdapterFactory, name='a')
    b = MagicMock(spec=AdapterFactory, name='b')

    with af.override(lambda: a):
        assert af.current is a
        with af.override(lambda: b):
            assert af.current is b
        assert af.current is a
    assert af.current is base


def test_override_factory_called_once_on_enter() -> None:
    af = AdapterFactoryAccessor()
    _ = af.current
    builder = MagicMock(return_value=MagicMock(spec=AdapterFactory))
    with af.override(builder):
        pass
    assert builder.call_count == 1


# ----------------------------------------------------------------------
# RuntimeAdapterFactory — against the mock runtime
# ----------------------------------------------------------------------


@pytest.fixture
def native_runtime() -> Generator[_pn.Runtime]:
    with runtime.override_with_mock() as rt:
        yield rt


@pytest.fixture
def desktop_adapter(native_runtime: _pn.Runtime) -> UiNodeAdapter:
    del native_runtime
    return UiNodeAdapter.create_root()


@pytest.fixture
def factory() -> RuntimeAdapterFactory:
    return RuntimeAdapterFactory()


def test_find_one_returns_adapter(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    loc = Locator(path="//control:Window[@Name='Operations Console']")
    found = factory.find_one(desktop_adapter, loc)
    assert isinstance(found, UiNodeAdapter)
    assert found.name == 'Operations Console'


def test_find_one_returns_none_when_no_match(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    loc = Locator(path="//control:Window[@Name='Does Not Exist']")
    assert factory.find_one(desktop_adapter, loc) is None


def test_find_all_returns_list(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    loc = Locator(path='//control:Button')
    results = factory.find_all(desktop_adapter, loc)
    assert len(results) >= 1
    assert all(isinstance(a, UiNodeAdapter) for a in results)


def test_find_all_empty_when_no_match(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    loc = Locator(path='//control:Button[@Name="ZZZ"]')
    assert factory.find_all(desktop_adapter, loc) == []


def test_find_one_raises_on_scalar_xpath_result(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    # Attribute-axis XPath returns EvaluatedAttribute, not UiNode.
    loc = Locator(path='//control:Window/@Name')
    with pytest.raises(InvalidResultTypeError, match='non-node result'):
        factory.find_one(desktop_adapter, loc)


def test_find_all_raises_on_scalar_xpath_result(
    factory: RuntimeAdapterFactory,
    desktop_adapter: UiNodeAdapter,
) -> None:
    loc = Locator(path='//control:Window/@Name')
    with pytest.raises(InvalidResultTypeError, match='non-node result'):
        factory.find_all(desktop_adapter, loc)


def test_find_one_rejects_adapter_without_native_node(
    factory: RuntimeAdapterFactory,
) -> None:
    bogus = MagicMock(spec=Adapter)
    # MagicMock auto-creates attributes; explicitly remove ``native_node``
    # so the getattr-default path triggers.
    del bogus.native_node
    loc = Locator(path='//control:Window')
    with pytest.raises(TypeError, match='does not expose a native UiNode'):
        factory.find_one(bogus, loc)
