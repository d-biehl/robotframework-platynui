# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# mypy: disable-error-code="type-abstract"
# pyright: reportPrivateUsage=false
#
# Tests deliberately pass pattern ABC classes to ``get_pattern`` (that
# is the public API) and inspect the adapter's protected resolution
# cache and validity flag to pin down the four-step algorithm. Both
# diagnostics are out of scope for this file.

"""Tests for :mod:`PlatynUI.core.adapter`.

The Adapter ABC implements a four-step pattern-resolution algorithm
(design section A.4) once on the base class and exposes a single
``_resolve_pattern`` hook for adapter backends. These tests pin down
the algorithm via a minimal in-memory adapter fixture so that future
adapter implementations (mock, Rust, JSON-RPC) can rely on the
contract.
"""

from collections.abc import Iterator, Sequence

import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import (
    AdapterNotValidError,
    NotAPatternTypeError,
    PatternNotSupportedError,
)
from PlatynUI.core.patterns import Activatable, Focusable, PatternBase, Toggleable, ToggleState


class _FakePattern(PatternBase):
    """Pattern not registered in :mod:`PlatynUI.core.patterns`."""

    pattern_name = 'org.example.patterns.Fake'


class _PatternWithoutName(PatternBase):
    """Subclass that forgets to declare ``pattern_name``."""

    # Inherits the bare ClassVar declaration without a value.


class _FakeFocusable(Focusable):
    """Concrete :class:`Focusable` returned by the lookup hook in tests."""

    def __init__(self) -> None:
        self.focus_calls = 0

    @property
    def is_focused(self) -> bool:
        return False

    def focus(self) -> None:
        self.focus_calls += 1


class _FakeAdapter(Adapter):
    """Minimal :class:`Adapter` implementation for algorithm tests.

    Records calls to :meth:`_resolve_pattern` and serves patterns from a
    constructor-provided dict so individual tests can drive Step 3
    behaviour deterministically.
    """

    def __init__(
        self,
        *,
        runtime_id: str = 'fake-1',
        valid: bool = True,
        resolvable: dict[str, PatternBase] | None = None,
    ) -> None:
        super().__init__()
        self._runtime_id = runtime_id
        self._valid = valid
        self._resolvable = dict(resolvable) if resolvable else {}
        self.resolve_calls: list[str] = []

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
        return ''

    @property
    def role(self) -> str:
        return 'Unknown'

    @property
    def supported_roles(self) -> set[str]:
        return {'Unknown'}

    @property
    def framework_id(self) -> str:
        return ''

    def attribute_names(self, namespace: str | None = None) -> set[str]:
        return set()

    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        raise KeyError(name)

    def attributes(self) -> Iterator[tuple[str, str, object]]:
        return iter(())

    def supported_patterns(self) -> set[type[PatternBase]]:
        return set()

    def supported_pattern_names(self) -> set[str]:
        return set(self._resolvable)

    def _resolve_pattern(self, pattern_name: str) -> PatternBase | None:
        self.resolve_calls.append(pattern_name)
        return self._resolvable.get(pattern_name)


class _NativeFocusableAdapter(_FakeAdapter, Focusable):
    """Adapter that natively implements :class:`Focusable` (Step 1 case)."""

    @property
    def is_focused(self) -> bool:
        return True

    def focus(self) -> None:
        return None


# ---------------------------------------------------------------------------
# ABC contract & identity helpers
# ---------------------------------------------------------------------------


def test_adapter_is_abstract() -> None:
    with pytest.raises(TypeError):
        Adapter()  # type: ignore[abstract]


def test_adapter_pattern_name_is_reverse_dns() -> None:
    assert Adapter.pattern_name == 'org.platynui.core.Adapter'


def test_tag_name_defaults_to_empty_string() -> None:
    assert _FakeAdapter().tag_name == ''


def test_equality_uses_runtime_id() -> None:
    a = _FakeAdapter(runtime_id='same')
    b = _FakeAdapter(runtime_id='same')
    c = _FakeAdapter(runtime_id='other')
    assert a == b
    assert a != c
    assert hash(a) == hash(b)


def test_equality_with_non_adapter_returns_notimplemented() -> None:
    a = _FakeAdapter()
    # ``==`` against an unrelated type must defer to the other operand,
    # which Python signals via ``NotImplemented``. We invoke ``__eq__``
    # directly to observe the sentinel.
    assert a.__eq__('not-an-adapter') is NotImplemented


# ---------------------------------------------------------------------------
# supports_pattern default
# ---------------------------------------------------------------------------


def test_supports_pattern_uses_supported_patterns_by_default() -> None:
    class _SupportingAdapter(_FakeAdapter):
        def supported_patterns(self) -> set[type[PatternBase]]:
            return {Activatable}

    adapter = _SupportingAdapter()
    assert adapter.supports_pattern(Activatable) is True
    assert adapter.supports_pattern(Focusable) is False


# ---------------------------------------------------------------------------
# get_pattern: argument validation
# ---------------------------------------------------------------------------


def test_get_pattern_rejects_non_type() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(NotAPatternTypeError):
        adapter.get_pattern('not-a-class')  # type: ignore[call-overload]


def test_get_pattern_rejects_non_pattern_class() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(NotAPatternTypeError):
        adapter.get_pattern(int)  # type: ignore[type-var]


def test_get_pattern_rejects_pattern_without_pattern_name() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(NotAPatternTypeError):
        adapter.get_pattern(_PatternWithoutName)


# ---------------------------------------------------------------------------
# get_pattern: Step 1 - self isinstance
# ---------------------------------------------------------------------------


def test_get_pattern_returns_self_when_natively_implemented() -> None:
    adapter = _NativeFocusableAdapter()
    result = adapter.get_pattern(Focusable)
    assert result is adapter
    # Step 1 must short-circuit: the resolution hook is never consulted.
    assert adapter.resolve_calls == []


def test_get_pattern_step1_works_even_when_invalid() -> None:
    """``isinstance(self, ...)`` is intrinsic and does not need a live handle."""

    adapter = _NativeFocusableAdapter(valid=False)
    assert adapter.get_pattern(Focusable) is adapter
    assert adapter.resolve_calls == []


# ---------------------------------------------------------------------------
# get_pattern: Step 2 - cache hit
# ---------------------------------------------------------------------------


def test_get_pattern_caches_resolved_instance() -> None:
    fake = _FakeFocusable()
    adapter = _FakeAdapter(resolvable={Focusable.pattern_name: fake})

    first = adapter.get_pattern(Focusable)
    second = adapter.get_pattern(Focusable)

    assert first is fake
    assert second is fake
    # The hook must be consulted exactly once; the second call hits the cache.
    assert adapter.resolve_calls == [Focusable.pattern_name]


def test_get_pattern_cache_hit_does_not_check_validity() -> None:
    """Once cached, an adapter that has gone invalid still serves the entry.

    The lifetime contract is enforced before the *first* lookup; a
    cached pattern reference may legitimately outlive the ``valid``
    flag for the duration of an in-flight call.
    """

    fake = _FakeFocusable()
    adapter = _FakeAdapter(resolvable={Focusable.pattern_name: fake})
    assert adapter.get_pattern(Focusable) is fake

    adapter._valid = False
    assert adapter.get_pattern(Focusable) is fake


# ---------------------------------------------------------------------------
# get_pattern: Step 3 - hook lookup + caching
# ---------------------------------------------------------------------------


def test_get_pattern_invokes_resolve_hook() -> None:
    fake = _FakeFocusable()
    adapter = _FakeAdapter(resolvable={Focusable.pattern_name: fake})
    assert adapter.get_pattern(Focusable) is fake


def test_get_pattern_raises_when_resolve_returns_wrong_type() -> None:
    """Defensive check: a misbehaving backend must not silently corrupt the cache."""

    fake = _FakeFocusable()  # ``Focusable``, not ``Activatable``
    adapter = _FakeAdapter(resolvable={Activatable.pattern_name: fake})
    with pytest.raises(PatternNotSupportedError, match='not a Activatable'):
        adapter.get_pattern(Activatable)
    # The bad result must not poison the cache for subsequent lookups.
    assert Activatable.pattern_name not in adapter._pattern_impls


def test_get_pattern_validates_handle_before_hook() -> None:
    adapter = _FakeAdapter(valid=False, resolvable={Focusable.pattern_name: _FakeFocusable()})
    with pytest.raises(AdapterNotValidError):
        adapter.get_pattern(Focusable)
    assert adapter.resolve_calls == []


# ---------------------------------------------------------------------------
# get_pattern: Step 4 - not supported
# ---------------------------------------------------------------------------


def test_get_pattern_raises_when_unsupported() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(PatternNotSupportedError):
        adapter.get_pattern(Focusable)


def test_get_pattern_returns_none_when_unsupported_and_silenced() -> None:
    adapter = _FakeAdapter()
    assert adapter.get_pattern(Focusable, raise_exception=False) is None


# ---------------------------------------------------------------------------
# get_pattern_by_name
# ---------------------------------------------------------------------------


def test_get_pattern_by_name_resolves_and_caches() -> None:
    fake = _FakeFocusable()
    adapter = _FakeAdapter(resolvable={Focusable.pattern_name: fake})

    first = adapter.get_pattern_by_name(Focusable.pattern_name)
    second = adapter.get_pattern_by_name(Focusable.pattern_name)

    assert first is fake
    assert second is fake
    assert adapter.resolve_calls == [Focusable.pattern_name]


def test_get_pattern_by_name_shares_cache_with_get_pattern() -> None:
    """Both entry points use the same ``_pattern_impls`` table."""

    fake = _FakeFocusable()
    adapter = _FakeAdapter(resolvable={Focusable.pattern_name: fake})

    typed = adapter.get_pattern(Focusable)
    by_name = adapter.get_pattern_by_name(Focusable.pattern_name)

    assert typed is by_name
    assert adapter.resolve_calls == [Focusable.pattern_name]


def test_get_pattern_by_name_raises_for_unknown_name() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(PatternNotSupportedError):
        adapter.get_pattern_by_name('org.example.patterns.Unknown')


def test_get_pattern_by_name_returns_none_when_silenced() -> None:
    adapter = _FakeAdapter()
    assert adapter.get_pattern_by_name('org.example.patterns.Unknown', raise_exception=False) is None


def test_get_pattern_by_name_rejects_empty_name() -> None:
    adapter = _FakeAdapter()
    with pytest.raises(NotAPatternTypeError):
        adapter.get_pattern_by_name('')


def test_get_pattern_by_name_validates_handle() -> None:
    adapter = _FakeAdapter(valid=False, resolvable={Focusable.pattern_name: _FakeFocusable()})
    with pytest.raises(AdapterNotValidError):
        adapter.get_pattern_by_name(Focusable.pattern_name)


# ---------------------------------------------------------------------------
# Cross-pattern interaction
# ---------------------------------------------------------------------------


def test_get_pattern_supports_unknown_third_party_pattern() -> None:
    """Adapters may resolve patterns whose Python class lives outside ``PlatynUI.core``."""

    class _ConcreteFake(_FakePattern):
        pass

    instance = _ConcreteFake()
    adapter = _FakeAdapter(resolvable={_FakePattern.pattern_name: instance})
    assert adapter.get_pattern(_FakePattern) is instance


def test_distinct_patterns_cache_independently() -> None:
    focus = _FakeFocusable()

    class _FakeToggleable(Toggleable):
        @property
        def state(self) -> ToggleState:
            return ToggleState.OFF

        @property
        def supports_three_state(self) -> bool:
            return False

        def toggle(self) -> None:
            return None

    toggle = _FakeToggleable()
    adapter = _FakeAdapter(
        resolvable={
            Focusable.pattern_name: focus,
            Toggleable.pattern_name: toggle,
        }
    )

    assert adapter.get_pattern(Focusable) is focus
    assert adapter.get_pattern(Toggleable) is toggle
    assert sorted(adapter.resolve_calls) == sorted([Focusable.pattern_name, Toggleable.pattern_name])
