# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# mypy: disable-error-code="type-abstract"
# pyright: reportPrivateUsage=false, reportUnusedFunction=false
#
# Tests poke the proxy registry's protected list and pass pattern ABCs
# as runtime values; both diagnostics are out of scope for this file.
# ``reportUnusedFunction`` is disabled because pytest fixtures are
# discovered at runtime and look unused to pyright.

"""Tests for :mod:`PlatynUI.core.adapter_proxy`.

Covers the composition-based :class:`AdapterProxy` (design §A.4 lines
1639-1643) and the :class:`PatternProxyFactory` registry that drives
:func:`pattern_proxy_for` decorations (§4.2-4.3).
"""

from collections.abc import Iterator, Sequence

import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_proxy import (
    AdapterProxy,
    PatternProxyFactory,
    pattern_proxy_for,
)
from PlatynUI.core.exceptions import (
    DuplicateRegistrationWarning,
    NotAPatternTypeError,
    PatternNotSupportedError,
)
from PlatynUI.core.patterns import Activatable, PatternBase

# ---------------------------------------------------------------------------
# Test fixtures
# ---------------------------------------------------------------------------


class _RecordingActivatable(Activatable):
    """Concrete :class:`Activatable` implementation for delegation tests."""

    def __init__(self) -> None:
        self.activate_calls = 0

    def activate(self) -> None:
        self.activate_calls += 1

    @property
    def is_activation_enabled(self) -> bool:
        return True

    @property
    def default_accelerator(self) -> str | None:
        return None


class _FakeAdapter(Adapter):
    """Minimal :class:`Adapter` for proxy delegation tests."""

    def __init__(
        self,
        *,
        runtime_id: str = 'fake-1',
        valid: bool = True,
        role: str = 'Unknown',
        framework_id: str = '',
        class_name: str = '',
        tag_name: str = '',
        attributes: dict[tuple[str, str], object] | None = None,
        resolvable: dict[str, PatternBase] | None = None,
    ) -> None:
        super().__init__()
        self._runtime_id = runtime_id
        self._valid = valid
        self._role = role
        self._framework_id = framework_id
        self._class_name = class_name
        self._tag_name = tag_name
        self._attributes = dict(attributes) if attributes else {}
        self._resolvable = dict(resolvable) if resolvable else {}

    @property
    def valid(self) -> bool:
        return self._valid

    @property
    def runtime_id(self) -> str:
        return self._runtime_id

    @property
    def parent(self) -> Adapter | None:
        return None

    @property
    def children(self) -> Sequence[Adapter]:
        return ()

    @property
    def name(self) -> str:
        return ''

    @property
    def class_name(self) -> str:
        return self._class_name

    @property
    def tag_name(self) -> str:
        return self._tag_name

    @property
    def role(self) -> str:
        return self._role

    @property
    def supported_roles(self) -> set[str]:
        return {self._role}

    @property
    def framework_id(self) -> str:
        return self._framework_id

    def attribute_names(self, namespace: str | None = None) -> set[str]:
        if namespace is None:
            return {name for _, name in self._attributes}
        return {name for ns, name in self._attributes if ns == namespace}

    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        try:
            return self._attributes[(namespace, name)]
        except KeyError as exc:
            raise KeyError(f'{namespace}:{name}') from exc

    def attributes(self) -> Iterator[tuple[str, str, object]]:
        return ((ns, name, value) for (ns, name), value in self._attributes.items())

    def supported_patterns(self) -> set[type[PatternBase]]:
        return set()

    def supported_pattern_names(self) -> set[str]:
        return set(self._resolvable)

    def _resolve_pattern(self, pattern_name: str) -> PatternBase | None:
        return self._resolvable.get(pattern_name)


class _ActivatableProxy(AdapterProxy, Activatable):
    """Proxy that injects an :class:`Activatable` implementation."""

    def __init__(self, adapter: Adapter) -> None:
        super().__init__(adapter)
        self.activate_calls = 0

    def activate(self) -> None:
        self.activate_calls += 1

    @property
    def is_activation_enabled(self) -> bool:
        return True

    @property
    def default_accelerator(self) -> str | None:
        return 'Enter'


class _BareProxy(AdapterProxy):
    """Proxy without any pattern mix-in; delegates everything."""


# ---------------------------------------------------------------------------
# Registry isolation
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _clear_registry() -> Iterator[None]:
    """Each test gets an empty :class:`PatternProxyFactory`."""
    PatternProxyFactory.clear()
    yield
    PatternProxyFactory.clear()


# ---------------------------------------------------------------------------
# Construction & wrapping
# ---------------------------------------------------------------------------


def test_init_rejects_non_adapter() -> None:
    with pytest.raises(TypeError):
        AdapterProxy('not-an-adapter')  # type: ignore[arg-type]


def test_adapter_property_returns_wrapped_instance() -> None:
    a = _FakeAdapter()
    proxy = _BareProxy(a)
    assert proxy.adapter is a


# ---------------------------------------------------------------------------
# Delegation
# ---------------------------------------------------------------------------


def test_delegates_identity_and_structure() -> None:
    a = _FakeAdapter(runtime_id='rt-7', framework_id='fw', role='Button')
    proxy = _BareProxy(a)
    assert proxy.runtime_id == 'rt-7'
    assert proxy.valid is True
    assert proxy.parent is None
    assert proxy.children == ()
    assert proxy.role == 'Button'
    assert proxy.framework_id == 'fw'
    assert proxy.supported_roles == {'Button'}


def test_delegates_attribute_lookups() -> None:
    a = _FakeAdapter(attributes={('control', 'IsFocused'): True})
    proxy = _BareProxy(a)
    assert proxy.attribute_value('IsFocused') is True
    assert proxy.attribute_names() == {'IsFocused'}
    assert list(proxy.attributes()) == [('control', 'IsFocused', True)]


# ---------------------------------------------------------------------------
# Equality follows the wrapped adapter
# ---------------------------------------------------------------------------


def test_proxy_equals_proxy_with_same_adapter_id() -> None:
    a1 = _FakeAdapter(runtime_id='id')
    a2 = _FakeAdapter(runtime_id='id')
    assert _BareProxy(a1) == _BareProxy(a2)


def test_proxy_equals_bare_adapter_with_same_id() -> None:
    a1 = _FakeAdapter(runtime_id='id')
    a2 = _FakeAdapter(runtime_id='id')
    proxy = _BareProxy(a1)
    assert proxy == a2
    assert hash(proxy) == hash(a1)


def test_proxy_equality_with_unrelated_returns_notimplemented() -> None:
    proxy = _BareProxy(_FakeAdapter())
    assert proxy.__eq__('foo') is NotImplemented


# ---------------------------------------------------------------------------
# Pattern discovery — union semantics
# ---------------------------------------------------------------------------


def test_supported_patterns_unions_proxy_and_adapter() -> None:
    fake = _RecordingActivatable()
    a = _FakeAdapter(resolvable={Activatable.pattern_name: fake})
    # _ActivatableProxy mixes Activatable; adapter additionally exposes it.
    proxy = _ActivatableProxy(a)
    assert Activatable in proxy.supported_patterns()
    names = proxy.supported_pattern_names()
    assert Activatable.pattern_name in names


def test_supports_pattern_true_via_mixin_even_if_adapter_unaware() -> None:
    a = _FakeAdapter()
    proxy = _ActivatableProxy(a)
    assert proxy.supports_pattern(Activatable) is True


def test_supports_pattern_delegates_when_no_mixin() -> None:
    a = _FakeAdapter()
    proxy = _BareProxy(a)
    assert proxy.supports_pattern(Activatable) is False


# ---------------------------------------------------------------------------
# get_pattern: argument validation
# ---------------------------------------------------------------------------


def test_get_pattern_rejects_non_pattern_class() -> None:
    proxy = _BareProxy(_FakeAdapter())
    with pytest.raises(NotAPatternTypeError):
        proxy.get_pattern(int)  # type: ignore[type-var]


def test_get_pattern_rejects_non_type() -> None:
    proxy = _BareProxy(_FakeAdapter())
    with pytest.raises(NotAPatternTypeError):
        proxy.get_pattern('nope')  # type: ignore[call-overload]


# ---------------------------------------------------------------------------
# get_pattern: Step 1 (proxy mix-in) wins
# ---------------------------------------------------------------------------


def test_get_pattern_returns_proxy_when_mixin_matches() -> None:
    """The proxy-provided implementation overrides the adapter."""

    fake = _RecordingActivatable()
    a = _FakeAdapter(resolvable={Activatable.pattern_name: fake})
    proxy = _ActivatableProxy(a)

    result = proxy.get_pattern(Activatable)
    assert result is proxy
    # Adapter must NOT be consulted when the proxy itself satisfies the type.
    proxy.get_pattern(Activatable).activate()
    assert proxy.activate_calls == 1
    assert fake.activate_calls == 0


# ---------------------------------------------------------------------------
# get_pattern: Step 2 (delegate to wrapped adapter)
# ---------------------------------------------------------------------------


def test_get_pattern_delegates_to_adapter_when_proxy_lacks_mixin() -> None:
    fake = _RecordingActivatable()
    a = _FakeAdapter(resolvable={Activatable.pattern_name: fake})
    proxy = _BareProxy(a)
    assert proxy.get_pattern(Activatable) is fake


def test_get_pattern_propagates_adapter_error() -> None:
    a = _FakeAdapter()
    proxy = _BareProxy(a)
    with pytest.raises(PatternNotSupportedError):
        proxy.get_pattern(Activatable)


def test_get_pattern_returns_none_with_raise_exception_false() -> None:
    a = _FakeAdapter()
    proxy = _BareProxy(a)
    assert proxy.get_pattern(Activatable, raise_exception=False) is None


# ---------------------------------------------------------------------------
# get_pattern_by_name
# ---------------------------------------------------------------------------


def test_get_pattern_by_name_finds_proxy_mixin() -> None:
    proxy = _ActivatableProxy(_FakeAdapter())
    assert proxy.get_pattern_by_name(Activatable.pattern_name) is proxy


def test_get_pattern_by_name_delegates_when_not_in_proxy() -> None:
    fake = _RecordingActivatable()
    a = _FakeAdapter(resolvable={Activatable.pattern_name: fake})
    proxy = _BareProxy(a)
    assert proxy.get_pattern_by_name(Activatable.pattern_name) is fake


def test_get_pattern_by_name_rejects_empty_string() -> None:
    proxy = _BareProxy(_FakeAdapter())
    with pytest.raises(NotAPatternTypeError):
        proxy.get_pattern_by_name('')


# ---------------------------------------------------------------------------
# Repr
# ---------------------------------------------------------------------------


def test_repr_includes_class_and_adapter() -> None:
    proxy = _BareProxy(_FakeAdapter(runtime_id='abc'))
    text = repr(proxy)
    assert '_BareProxy' in text
    assert 'abc' in text


# ---------------------------------------------------------------------------
# Subclass relationship
# ---------------------------------------------------------------------------


def test_adapter_proxy_is_adapter_subclass() -> None:
    """Every `AdapterProxy` instance is also an `Adapter`.

    Required so that `AdapterFactory.find_one/find_all` can keep their
    `Adapter | None` return type while still returning proxied
    adapters (Designdoc §A.4 / §4.4).
    """
    assert issubclass(AdapterProxy, Adapter)
    proxy = _BareProxy(_FakeAdapter())
    assert isinstance(proxy, Adapter)


# ===========================================================================
# PatternProxyFactory
# ===========================================================================


# ---------------------------------------------------------------------------
# Registration management
# ---------------------------------------------------------------------------


def test_register_rejects_non_proxy_subclass() -> None:
    with pytest.raises(TypeError):
        PatternProxyFactory.register(int, {})  # type: ignore[arg-type]


def test_register_then_unregister_round_trip() -> None:
    PatternProxyFactory.register(_BareProxy, {'role': 'Button'})
    assert any(e.proxy_cls is _BareProxy for e in PatternProxyFactory.registrations())
    PatternProxyFactory.unregister(_BareProxy)
    assert all(e.proxy_cls is not _BareProxy for e in PatternProxyFactory.registrations())


def test_unregister_unknown_is_noop() -> None:
    PatternProxyFactory.unregister(_BareProxy)  # must not raise


def test_register_is_idempotent_replacing_previous_criteria() -> None:
    PatternProxyFactory.register(_BareProxy, {'role': 'Button'})
    PatternProxyFactory.register(_BareProxy, {'role': 'Label'})
    entries = [e for e in PatternProxyFactory.registrations() if e.proxy_cls is _BareProxy]
    assert len(entries) == 1
    assert entries[0].criteria == {'role': 'Label'}


def test_clear_drops_every_registration() -> None:
    PatternProxyFactory.register(_BareProxy, {})
    PatternProxyFactory.clear()
    assert PatternProxyFactory.registrations() == ()


# ---------------------------------------------------------------------------
# find_proxy_for: selection semantics
# ---------------------------------------------------------------------------


def test_find_proxy_for_returns_adapter_when_registry_empty() -> None:
    a = _FakeAdapter()
    assert PatternProxyFactory.find_proxy_for(a) is a


def test_find_proxy_for_returns_adapter_when_no_match() -> None:
    PatternProxyFactory.register(_ActivatableProxy, {'role': 'Button'})
    a = _FakeAdapter(role='Label')
    assert PatternProxyFactory.find_proxy_for(a) is a


def test_find_proxy_for_wraps_matching_proxy() -> None:
    PatternProxyFactory.register(_ActivatableProxy, {'role': 'Button'})
    a = _FakeAdapter(role='Button')
    result = PatternProxyFactory.find_proxy_for(a)
    assert isinstance(result, _ActivatableProxy)
    assert result.adapter is a


def test_find_proxy_for_picks_highest_score() -> None:
    """A more specific (higher-weighted) registration wins."""

    class _SpecificProxy(AdapterProxy):
        pass

    # Generic match (role only): 10 000
    PatternProxyFactory.register(_BareProxy, {'role': 'Button'})
    # Specific match (role + framework_id): much higher
    PatternProxyFactory.register(
        _SpecificProxy,
        {
            'role': 'Button',
            'framework_id': 'wpf',
        },
    )

    a = _FakeAdapter(role='Button', framework_id='wpf')
    result = PatternProxyFactory.find_proxy_for(a)
    assert isinstance(result, _SpecificProxy)


def test_find_proxy_for_uses_attribute_criteria() -> None:
    PatternProxyFactory.register(
        _ActivatableProxy,
        {
            'role': 'Label',
            'attributes': {'ClassName': 'MyApp.FakeButton'},
        },
    )
    matching = _FakeAdapter(
        role='Label',
        attributes={('control', 'ClassName'): 'MyApp.FakeButton'},
    )
    other = _FakeAdapter(
        role='Label',
        attributes={('control', 'ClassName'): 'OtherClass'},
    )
    assert isinstance(PatternProxyFactory.find_proxy_for(matching), _ActivatableProxy)
    assert PatternProxyFactory.find_proxy_for(other) is other


# ---------------------------------------------------------------------------
# pattern_proxy_for decorator
# ---------------------------------------------------------------------------


def test_pattern_proxy_for_registers_class() -> None:
    @pattern_proxy_for(role='__test_button__')
    class _DecoratedProxy(AdapterProxy):
        pass

    entries = [e for e in PatternProxyFactory.registrations() if e.proxy_cls is _DecoratedProxy]
    assert len(entries) == 1
    assert entries[0].criteria['role'] == '__test_button__'


def test_pattern_proxy_for_returns_class_unchanged() -> None:
    @pattern_proxy_for(role='__test_button__')
    class _DecoratedProxy(AdapterProxy):
        pass

    # Decorator must not wrap the class itself.
    assert _DecoratedProxy.__name__ == '_DecoratedProxy'
    assert issubclass(_DecoratedProxy, AdapterProxy)


def test_pattern_proxy_for_supports_full_criteria_set() -> None:
    @pattern_proxy_for(
        role='__test_button__',
        framework_id='wpf',
        class_name='Btn',
        tag_name='button',
        attributes={'IsFocused': True},
    )
    class _FullProxy(AdapterProxy):
        pass

    entry = next(e for e in PatternProxyFactory.registrations() if e.proxy_cls is _FullProxy)
    assert entry.criteria == {
        'role': '__test_button__',
        'framework_id': 'wpf',
        'class_name': 'Btn',
        'tag_name': 'button',
        'attributes': {'IsFocused': True},
    }


def test_pattern_proxy_for_end_to_end_resolves_via_factory() -> None:
    @pattern_proxy_for(role='__test_button__')
    class _EndToEndProxy(AdapterProxy, Activatable):
        def __init__(self, adapter: Adapter) -> None:
            super().__init__(adapter)
            self.activate_calls = 0

        def activate(self) -> None:
            self.activate_calls += 1

        @property
        def is_activation_enabled(self) -> bool:
            return True

        @property
        def default_accelerator(self) -> str | None:
            return None

    a = _FakeAdapter(role='__test_button__')
    facade = PatternProxyFactory.find_proxy_for(a)
    assert isinstance(facade, _EndToEndProxy)
    activatable = facade.get_pattern(Activatable)
    activatable.activate()
    assert facade.activate_calls == 1


def test_duplicate_proxy_registration_emits_warning() -> None:
    @pattern_proxy_for(role='__test_dup_proxy__')
    class _FirstDupProxy(AdapterProxy, Activatable):
        def activate(self) -> None: ...

        @property
        def is_activation_enabled(self) -> bool:
            return True

        @property
        def default_accelerator(self) -> str | None:
            return None

    with pytest.warns(DuplicateRegistrationWarning, match='__test_dup_proxy__'):

        @pattern_proxy_for(role='__test_dup_proxy__')
        class _SecondDupProxy(AdapterProxy, Activatable):
            def activate(self) -> None: ...

            @property
            def is_activation_enabled(self) -> bool:
                return True

            @property
            def default_accelerator(self) -> str | None:
                return None

    classes = [e.proxy_cls for e in PatternProxyFactory.registrations()]
    assert _FirstDupProxy in classes
    assert _SecondDupProxy in classes


def test_re_registering_same_proxy_class_is_silent() -> None:
    import warnings as _warnings

    @pattern_proxy_for(role='__test_reuse_proxy__')
    class _ReuseProxy(AdapterProxy, Activatable):
        def activate(self) -> None: ...

        @property
        def is_activation_enabled(self) -> bool:
            return True

        @property
        def default_accelerator(self) -> str | None:
            return None

    with _warnings.catch_warnings():
        _warnings.simplefilter('error', DuplicateRegistrationWarning)
        # Re-registering the same class with the same criteria stays silent
        # because the previous entry is removed before the duplicate check.
        PatternProxyFactory.register(_ReuseProxy, {'role': '__test_reuse_proxy__'})
