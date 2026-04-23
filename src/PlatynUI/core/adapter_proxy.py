# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false
#
# Same mypy/pyright TypeVar-narrowing tradeoff as in core/adapter.py:
# isinstance(self, pattern_type) is statically unreachable for mypy
# (AdapterProxy does not extend PatternBase) but is the deliberate
# Step 1 of pattern resolution for proxies that mix in pattern ABCs.

"""Adapter proxy and pattern-proxy registry (design sections 4.2-4.3, A.4).

An :class:`AdapterProxy` wraps an :class:`Adapter` to add or override
pattern implementations. Per the design document, the proxy is a
**composition** of an adapter (held in :attr:`AdapterProxy.adapter`),
not an :class:`Adapter` subclass — the old project's inheritance-based
proxy is deliberately not reproduced here.

The proxy exposes the same public surface as :class:`Adapter` so that
caller code can treat ``Adapter | AdapterProxy`` interchangeably; this
union is published as the :data:`AdapterFacade` type alias.

Concrete proxies are registered through the :func:`pattern_proxy_for`
class decorator. The :class:`PatternProxyFactory` module-level
singleton scores registered proxies against an adapter via
:class:`WeightCalculator` and wraps the highest-scoring match.
"""

from __future__ import annotations

import re
from collections.abc import Callable, Iterator, Sequence
from typing import TYPE_CHECKING, Any, Literal, TypeAlias, TypeVar, cast, overload

from .adapter import Adapter
from .exceptions import NotAPatternTypeError
from .patterns.base import PatternBase
from .weight_calculator import WeightCalculator

if TYPE_CHECKING:
    from .technology import Technology
    from .types import FrameworkId, PatternName, RoleName

__all__ = [
    'AdapterFacade',
    'AdapterProxy',
    'PatternProxyFactory',
    'pattern_proxy_for',
]


P = TypeVar('P', bound=PatternBase)
ProxyT = TypeVar('ProxyT', bound='AdapterProxy')


#: Either a raw :class:`Adapter` or a wrapping :class:`AdapterProxy` —
#: both expose the same public surface to UI-side callers.
AdapterFacade: TypeAlias = 'Adapter | AdapterProxy'


# ---------------------------------------------------------------------------
# AdapterProxy
# ---------------------------------------------------------------------------


class AdapterProxy:
    """Composition-based proxy that adds/overrides patterns on an adapter.

    The proxy is **not** an :class:`Adapter` subclass; instead it
    forwards every adapter-shaped method to the wrapped instance. Only
    pattern resolution is enriched:

    1. If the proxy itself is an ``isinstance`` of the requested pattern
       type (i.e. it mixes in the pattern ABC and provides the methods),
       it acts as the implementation.
    2. Otherwise resolution delegates to the wrapped adapter.
    3. Otherwise raises :class:`PatternNotSupportedError` (or returns
       ``None`` when ``raise_exception=False``).

    :attr:`supported_patterns` and :attr:`supported_pattern_names`
    return the **union** of proxy-provided and adapter-provided
    patterns.
    """

    def __init__(self, adapter: Adapter) -> None:
        if not isinstance(adapter, Adapter):
            raise TypeError(
                f'AdapterProxy requires an Adapter instance; got {type(adapter).__name__}'
            )
        self._adapter = adapter

    # ------------------------------------------------------------------
    # Wrapped adapter access
    # ------------------------------------------------------------------

    @property
    def adapter(self) -> Adapter:
        """The wrapped underlying adapter."""
        return self._adapter

    # ------------------------------------------------------------------
    # Identity & lifetime — delegated
    # ------------------------------------------------------------------

    @property
    def valid(self) -> bool:
        return self._adapter.valid

    @property
    def runtime_id(self) -> str:
        return self._adapter.runtime_id

    @property
    def technology(self) -> 'Technology':
        return self._adapter.technology

    # ------------------------------------------------------------------
    # Structure — delegated
    # ------------------------------------------------------------------

    @property
    def parent(self) -> 'AdapterFacade | None':
        return self._adapter.parent

    @property
    def children(self) -> Sequence['AdapterFacade']:
        return self._adapter.children

    # ------------------------------------------------------------------
    # Search criteria — delegated
    # ------------------------------------------------------------------

    @property
    def name(self) -> str:
        return self._adapter.name

    @property
    def class_name(self) -> str:
        return self._adapter.class_name

    @property
    def tag_name(self) -> str:
        return self._adapter.tag_name

    @property
    def role(self) -> str:
        return self._adapter.role

    @property
    def supported_roles(self) -> 'set[RoleName]':
        return self._adapter.supported_roles

    @property
    def framework_id(self) -> 'FrameworkId':
        return self._adapter.framework_id

    # ------------------------------------------------------------------
    # Attributes — delegated
    # ------------------------------------------------------------------

    def attribute_names(self, namespace: str | None = None) -> set[str]:
        return self._adapter.attribute_names(namespace)

    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        return self._adapter.attribute_value(name, namespace)

    def attributes(self) -> Iterator[tuple[str, str, object]]:
        return self._adapter.attributes()

    # ------------------------------------------------------------------
    # Pattern discovery — union of proxy- and adapter-provided patterns
    # ------------------------------------------------------------------

    def _proxy_pattern_classes(self) -> set[type[PatternBase]]:
        """Pattern ABCs mixed into the concrete proxy class via MRO."""
        return {
            base
            for base in type(self).__mro__
            if isinstance(base, type)
            and base is not PatternBase
            and issubclass(base, PatternBase)
        }

    def supported_patterns(self) -> set[type[PatternBase]]:
        return self._proxy_pattern_classes() | self._adapter.supported_patterns()

    def supported_pattern_names(self) -> 'set[PatternName]':
        proxy_names = {
            cls.pattern_name
            for cls in self._proxy_pattern_classes()
            if isinstance(getattr(cls, 'pattern_name', None), str) and cls.pattern_name
        }
        return proxy_names | self._adapter.supported_pattern_names()

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        if isinstance(self, pattern_type):
            return True
        return self._adapter.supports_pattern(pattern_type)

    # ------------------------------------------------------------------
    # Pattern resolution — proxy first, then delegate
    # ------------------------------------------------------------------

    @overload
    def get_pattern(self, pattern_type: type[P]) -> P: ...

    @overload
    def get_pattern(self, pattern_type: type[P], *, raise_exception: Literal[True]) -> P: ...

    @overload
    def get_pattern(self, pattern_type: type[P], *, raise_exception: Literal[False]) -> P | None: ...

    @overload
    def get_pattern(self, pattern_type: type[P], *, raise_exception: bool) -> P | None: ...

    def get_pattern(
        self,
        pattern_type: type[P],
        *,
        raise_exception: bool = True,
    ) -> P | None:
        """Resolve ``pattern_type`` via the proxy first, then the adapter.

        See class docstring for the resolution algorithm.
        """
        if not isinstance(pattern_type, type) or not issubclass(pattern_type, PatternBase):
            raise NotAPatternTypeError(f'{pattern_type!r} is not a PatternBase subclass')

        # Step 1: proxy itself implements the pattern via mix-in.
        if isinstance(self, pattern_type):
            return cast('P', self)  # type: ignore[unreachable]

        # Step 2: delegate to the wrapped adapter.
        return self._adapter.get_pattern(pattern_type, raise_exception=raise_exception)

    def get_pattern_by_name(
        self,
        pattern_name: 'PatternName',
        *,
        raise_exception: bool = True,
    ) -> PatternBase | None:
        """Resolve by Reverse-DNS identifier (proxy first, then adapter)."""
        if not isinstance(pattern_name, str) or not pattern_name:
            raise NotAPatternTypeError(f'{pattern_name!r} is not a valid pattern name')

        # Step 1: proxy class hierarchy carries the requested identifier.
        for cls in self._proxy_pattern_classes():
            if getattr(cls, 'pattern_name', None) == pattern_name:
                return cast(PatternBase, self)

        # Step 2: delegate.
        return self._adapter.get_pattern_by_name(
            pattern_name, raise_exception=raise_exception
        )

    # ------------------------------------------------------------------
    # Identity helpers — equality follows the wrapped adapter so that
    # proxied and bare references to the same UI element compare equal.
    # ------------------------------------------------------------------

    def __eq__(self, other: object) -> bool:
        if isinstance(other, AdapterProxy):
            return self._adapter == other._adapter
        if isinstance(other, Adapter):
            return self._adapter == other
        return NotImplemented

    def __hash__(self) -> int:
        return hash(self._adapter)

    def __repr__(self) -> str:
        return f'<{type(self).__name__} adapter={self._adapter!r}>'


# ---------------------------------------------------------------------------
# Pattern proxy registry & decorator
# ---------------------------------------------------------------------------


#: Criterion keys accepted by :func:`pattern_proxy_for`. The set mirrors
#: :meth:`WeightCalculator.calculate`'s ``criteria`` dict so that proxy
#: matching uses the same scoring rules as locator matching.
_CRITERION_KEYS: frozenset[str] = frozenset(
    {'technology', 'role', 'framework_id', 'class_name', 'tag_name', 'attributes'}
)


class _ProxyRegistration:
    __slots__ = ('criteria', 'proxy_cls')

    def __init__(
        self, proxy_cls: type[AdapterProxy], criteria: dict[str, object]
    ) -> None:
        self.proxy_cls = proxy_cls
        self.criteria = criteria

    def __repr__(self) -> str:
        return f'_ProxyRegistration({self.proxy_cls.__name__}, {self.criteria!r})'


class PatternProxyFactory:
    """Module-level registry that maps adapters to proxy wrappers.

    Registrations are populated by the :func:`pattern_proxy_for`
    decorator. :meth:`find_proxy_for` returns the best-scoring proxy
    wrapped around the adapter, or the adapter itself when no
    registered proxy matches.

    The factory is intentionally **not** a global ``PatternRegistry``:
    it indexes proxy *classes* by adapter criteria; pattern identity
    itself stays adapter-local (see design section 5.4).
    """

    _registrations: list[_ProxyRegistration] = []

    @classmethod
    def register(
        cls, proxy_cls: type[AdapterProxy], criteria: dict[str, object]
    ) -> None:
        """Register ``proxy_cls`` with the given matching criteria.

        Called by :func:`pattern_proxy_for`; not normally invoked
        directly. Re-registering a class is idempotent — the previous
        entry is replaced.
        """
        if not isinstance(proxy_cls, type) or not issubclass(proxy_cls, AdapterProxy):
            raise TypeError(
                f'{proxy_cls!r} is not an AdapterProxy subclass'
            )
        cls._registrations = [
            entry for entry in cls._registrations if entry.proxy_cls is not proxy_cls
        ]
        cls._registrations.append(_ProxyRegistration(proxy_cls, dict(criteria)))

    @classmethod
    def unregister(cls, proxy_cls: type[AdapterProxy]) -> None:
        """Remove ``proxy_cls`` from the registry (no-op if absent).

        Useful in tests that register temporary proxies.
        """
        cls._registrations = [
            entry for entry in cls._registrations if entry.proxy_cls is not proxy_cls
        ]

    @classmethod
    def clear(cls) -> None:
        """Drop every registration. Intended for tests."""
        cls._registrations = []

    @classmethod
    def registrations(cls) -> Sequence[_ProxyRegistration]:
        """Read-only view of the current registrations (for diagnostics)."""
        return tuple(cls._registrations)

    @classmethod
    def find_proxy_for(cls, adapter: Adapter) -> AdapterFacade:
        """Wrap ``adapter`` in the highest-scoring registered proxy.

        Returns the adapter unchanged if no proxy has a positive score.
        Ties are broken by registration order: the earliest entry wins,
        consistent with the legacy ``AdapterProxyFactory`` behaviour.
        """
        if not cls._registrations:
            return adapter

        calculator = WeightCalculator(_AdapterCriteriaView(adapter))
        best_score = 0
        best_cls: type[AdapterProxy] | None = None
        for entry in cls._registrations:
            score = calculator.calculate(entry.criteria)
            if score > best_score:
                best_score = score
                best_cls = entry.proxy_cls

        if best_cls is None:
            return adapter
        return best_cls(adapter)


class _AdapterCriteriaView:
    """Adapt :class:`Adapter` to the :class:`WeightCalculator` Protocol.

    :class:`WeightCalculator` was built before :class:`Adapter` existed
    and consumes a structural :class:`AdapterLike` view (``technology``,
    ``role``, ``framework_id`` etc. as cached attributes plus
    ``attribute_value(name, namespace)``). This thin shim presents the
    calculator with exactly that surface without forcing the calculator
    to import :class:`Adapter`.
    """

    __slots__ = ('_adapter',)

    def __init__(self, adapter: Adapter) -> None:
        self._adapter = adapter

    @property
    def technology(self) -> Any:
        return self._adapter.technology

    @property
    def role(self) -> str:
        return self._adapter.role

    @property
    def supported_roles(self) -> 'set[RoleName]':
        return self._adapter.supported_roles

    @property
    def framework_id(self) -> 'FrameworkId':
        return self._adapter.framework_id

    @property
    def class_name(self) -> str:
        return self._adapter.class_name

    @property
    def tag_name(self) -> str:
        return self._adapter.tag_name

    @property
    def supported_patterns(self) -> 'list[PatternName]':
        return list(self._adapter.supported_pattern_names())

    def get_pattern(self, pattern_name: 'PatternName') -> Any:
        return self._adapter.get_pattern_by_name(pattern_name, raise_exception=False)

    def attribute_value(self, name: str, namespace: str = 'control') -> Any:
        try:
            return self._adapter.attribute_value(name, namespace)
        except KeyError:
            return None


def pattern_proxy_for(
    *,
    role: str | None = None,
    framework_id: str | None = None,
    class_name: str | re.Pattern[str] | None = None,
    tag_name: str | re.Pattern[str] | None = None,
    technology: type[Any] | None = None,
    attributes: dict[str | tuple[str, str], object] | None = None,
) -> Callable[[type[ProxyT]], type[ProxyT]]:
    """Class decorator: register an :class:`AdapterProxy` with match criteria.

    The accepted keywords mirror :class:`WeightCalculator`'s criteria
    dict. ``attributes`` keys may be bare strings (resolved in the
    default ``control`` namespace) or ``(namespace, name)`` tuples.

    Example::

        @pattern_proxy_for(role='Button')
        class ButtonProxy(AdapterProxy, patterns.Activatable):
            def activate(self) -> None: ...

        @pattern_proxy_for(
            role='Label',
            attributes={'ClassName': 'MyApp.FakeButton'},
        )
        class FakeButtonProxy(AdapterProxy, patterns.Activatable):
            def activate(self) -> None: ...
    """
    criteria: dict[str, object] = {
        'role': role,
        'framework_id': framework_id,
        'class_name': class_name,
        'tag_name': tag_name,
        'technology': technology,
        'attributes': attributes,
    }
    # Defensive: catch typos in future extensions.
    unknown = set(criteria) - _CRITERION_KEYS
    if unknown:  # pragma: no cover - guarded by the typed signature
        raise TypeError(f'unknown pattern_proxy_for criteria: {sorted(unknown)}')

    def decorator(cls: type[ProxyT]) -> type[ProxyT]:
        PatternProxyFactory.register(cls, criteria)
        return cls

    return decorator
