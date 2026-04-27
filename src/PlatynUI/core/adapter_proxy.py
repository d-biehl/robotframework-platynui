# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false
#
# Same mypy/pyright TypeVar-narrowing tradeoff as in core/adapter.py:
# ``isinstance(self, pattern_type)`` is statically unreachable for mypy
# (AdapterProxy does not extend PatternBase) but is the deliberate
# step 1 of pattern resolution for proxies that mix in pattern ABCs.

"""Adapter proxy and pattern-proxy registry."""

import re
import warnings
from collections.abc import Callable, Iterator, Sequence
from typing import TYPE_CHECKING, Any, Literal, cast, overload, override

from ._criteria import criteria_equal
from .adapter import Adapter
from .exceptions import DuplicateRegistrationWarning, NotAPatternTypeError
from .patterns.base import PatternBase
from .weight_calculator import WeightCalculator

if TYPE_CHECKING:
    from .technology import Technology
    from .types import FrameworkId, PatternName, RoleName

__all__ = [
    'AdapterProxy',
    'PatternProxyFactory',
    'pattern_proxy_for',
]


# ---------------------------------------------------------------------------
# AdapterProxy
# ---------------------------------------------------------------------------


class AdapterProxy(Adapter):
    """Wrap an `Adapter` to add or override pattern implementations.

    Adapter-shaped properties delegate to the wrapped instance.
    Pattern resolution checks the proxy itself first (when it mixes
    in the requested pattern ABC), then falls back to the wrapped
    adapter.
    """

    def __init__(self, adapter: Adapter) -> None:
        if not isinstance(adapter, Adapter):
            raise TypeError(
                f'AdapterProxy requires an Adapter instance; got {type(adapter).__name__}'
            )
        super().__init__()
        self._adapter = adapter

    # ------------------------------------------------------------------
    # Wrapped adapter access
    # ------------------------------------------------------------------

    @property
    def adapter(self) -> Adapter:
        """The wrapped adapter."""
        return self._adapter

    # ------------------------------------------------------------------
    # Identity & lifetime — delegated
    # ------------------------------------------------------------------

    @property
    @override
    def valid(self) -> bool:
        return self._adapter.valid

    @property
    @override
    def runtime_id(self) -> str:
        return self._adapter.runtime_id

    @property
    @override
    def technology(self) -> 'Technology':
        return self._adapter.technology

    # ------------------------------------------------------------------
    # Structure — delegated
    # ------------------------------------------------------------------

    @property
    @override
    def parent(self) -> 'Adapter | None':
        return self._adapter.parent

    @property
    @override
    def children(self) -> Sequence['Adapter']:
        return self._adapter.children

    # ------------------------------------------------------------------
    # Search criteria — delegated
    # ------------------------------------------------------------------

    @property
    @override
    def name(self) -> str:
        return self._adapter.name

    @property
    @override
    def class_name(self) -> str:
        return self._adapter.class_name

    @property
    @override
    def tag_name(self) -> str:
        return self._adapter.tag_name

    @property
    @override
    def role(self) -> str:
        return self._adapter.role

    @property
    @override
    def supported_roles(self) -> 'set[RoleName]':
        return self._adapter.supported_roles

    @property
    @override
    def framework_id(self) -> 'FrameworkId':
        return self._adapter.framework_id

    # ------------------------------------------------------------------
    # Attributes — delegated
    # ------------------------------------------------------------------

    @override
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        return self._adapter.attribute_names(namespace)

    @override
    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        return self._adapter.attribute_value(name, namespace)

    @override
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        return self._adapter.attributes()

    # ------------------------------------------------------------------
    # Pattern discovery — union of proxy- and adapter-provided patterns
    # ------------------------------------------------------------------

    def _proxy_pattern_classes(self) -> set[type[PatternBase]]:
        """Return pattern ABCs mixed into the concrete proxy class via MRO."""
        return {
            base
            for base in type(self).__mro__
            if isinstance(base, type)
            and base is not PatternBase
            and issubclass(base, PatternBase)
        }

    @override
    def supported_patterns(self) -> set[type[PatternBase]]:
        return self._proxy_pattern_classes() | self._adapter.supported_patterns()

    @override
    def supported_pattern_names(self) -> 'set[PatternName]':
        proxy_names = {
            cls.pattern_name
            for cls in self._proxy_pattern_classes()
            if isinstance(getattr(cls, 'pattern_name', None), str) and cls.pattern_name
        }
        return proxy_names | self._adapter.supported_pattern_names()

    @override
    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        if isinstance(self, pattern_type):
            return True
        return self._adapter.supports_pattern(pattern_type)

    # ------------------------------------------------------------------
    # Pattern resolution — proxy first, then delegate
    # ------------------------------------------------------------------

    @override
    def _resolve_pattern(self, pattern_name: 'PatternName') -> PatternBase | None:
        """Delegate to the wrapped adapter.

        The proxy-first short-circuit is handled in `get_pattern` /
        `get_pattern_by_name`; by the time `_resolve_pattern` runs the
        proxy itself does not implement ``pattern_name``, so the
        wrapped adapter is the only remaining source.
        """
        return self._adapter._resolve_pattern(pattern_name)

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P]) -> P: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: Literal[True]) -> P: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: Literal[False]) -> P | None: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: bool) -> P | None: ...

    @override
    def get_pattern[P: PatternBase](
        self,
        pattern_type: type[P],
        *,
        raise_exception: bool = True,
    ) -> P | None:
        """Resolve ``pattern_type`` via the proxy first, then the adapter.

        See the class docstring for the resolution algorithm.
        """
        if not isinstance(pattern_type, type) or not issubclass(pattern_type, PatternBase):
            raise NotAPatternTypeError(f'{pattern_type!r} is not a PatternBase subclass')

        # Step 1: proxy itself implements the pattern via mix-in.
        if isinstance(self, pattern_type):
            return cast('P', self)  # type: ignore[unreachable]

        # Step 2: delegate to the wrapped adapter.
        return self._adapter.get_pattern(pattern_type, raise_exception=raise_exception)

    @override
    def get_pattern_by_name(
        self,
        pattern_name: 'PatternName',
        *,
        raise_exception: bool = True,
    ) -> PatternBase | None:
        """Resolve a pattern by Reverse-DNS identifier; check proxy first."""
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

    @override
    def __eq__(self, other: object) -> bool:
        if isinstance(other, AdapterProxy):
            return self._adapter == other._adapter
        if isinstance(other, Adapter):
            return self._adapter == other
        return NotImplemented

    @override
    def __hash__(self) -> int:
        return hash(self._adapter)

    @override
    def __repr__(self) -> str:
        return f'<{type(self).__name__} adapter={self._adapter!r}>'


# ---------------------------------------------------------------------------
# Pattern proxy registry & decorator
# ---------------------------------------------------------------------------


#: Criterion keys accepted by `pattern_proxy_for`. The set mirrors
#: `calculate`'s ``criteria`` dict so that proxy
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
    """Map adapters to registered proxy wrappers.

    The `pattern_proxy_for` decorator populates the registry.
    `find_proxy_for` returns the best-scoring proxy wrapped
    around the adapter, or the adapter itself when no registered
    proxy matches.
    """

    _registrations: list[_ProxyRegistration] = []

    @classmethod
    def register(
        cls, proxy_cls: type[AdapterProxy], criteria: dict[str, object]
    ) -> None:
        """Register ``proxy_cls`` with the given matching criteria.

        Re-registering an existing class replaces its previous entry.
        Emits a `DuplicateRegistrationWarning` when a *different* proxy
        class has already been registered with criteria that compare
        equal (after normalising `re.Pattern` to ``(pattern, flags)``).
        Normally invoked through `pattern_proxy_for` rather than
        directly.
        """
        if not isinstance(proxy_cls, type) or not issubclass(proxy_cls, AdapterProxy):
            raise TypeError(
                f'{proxy_cls!r} is not an AdapterProxy subclass'
            )
        new_criteria = dict(criteria)
        cls._registrations = [
            entry for entry in cls._registrations if entry.proxy_cls is not proxy_cls
        ]
        for entry in cls._registrations:
            if criteria_equal(entry.criteria, new_criteria):
                warnings.warn(
                    f'{proxy_cls.__module__}.{proxy_cls.__qualname__} '
                    f'registers with the same criteria {new_criteria!r} as '
                    f'{entry.proxy_cls.__module__}.{entry.proxy_cls.__qualname__}; '
                    f'matches will be ambiguous.',
                    DuplicateRegistrationWarning,
                    stacklevel=3,
                )
                break
        cls._registrations.append(_ProxyRegistration(proxy_cls, new_criteria))

    @classmethod
    def unregister(cls, proxy_cls: type[AdapterProxy]) -> None:
        """Remove ``proxy_cls`` from the registry; no-op if absent."""
        cls._registrations = [
            entry for entry in cls._registrations if entry.proxy_cls is not proxy_cls
        ]

    @classmethod
    def clear(cls) -> None:
        """Drop every registration. Intended for tests."""
        cls._registrations = []

    @classmethod
    def registrations(cls) -> Sequence[_ProxyRegistration]:
        """Return a read-only view of the current registrations."""
        return tuple(cls._registrations)

    @classmethod
    def find_proxy_for(cls, adapter: Adapter) -> Adapter:
        """Wrap ``adapter`` in the highest-scoring registered proxy.

        Return the adapter unchanged when no proxy has a positive
        score. Break ties by registration order; the earliest entry
        wins.
        """
        if not cls._registrations:
            return adapter

        calculator = WeightCalculator(AdapterCriteriaView(adapter))
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


class AdapterCriteriaView:
    """Present an `Adapter` through the `WeightCalculator` Protocol.

    `WeightCalculator` consumes a structural ``AdapterLike``
    view (``technology``, ``role``, ``framework_id`` and so on as
    cached attributes plus ``attribute_value(name, namespace)``).
    This shim exposes that exact surface without forcing the
    calculator to import `Adapter`.
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


def pattern_proxy_for[ProxyT: AdapterProxy](
    *,
    role: str | None = None,
    framework_id: str | None = None,
    class_name: str | re.Pattern[str] | None = None,
    tag_name: str | re.Pattern[str] | None = None,
    technology: type[Any] | None = None,
    attributes: dict[str | tuple[str, str], object] | None = None,
) -> Callable[[type[ProxyT]], type[ProxyT]]:
    """Register an `AdapterProxy` subclass with match criteria.

    The accepted keywords mirror `WeightCalculator`'s criteria
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
