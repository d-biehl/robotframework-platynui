# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false
#
# The pattern-resolution algorithm below uses cross-tool ``# type: ignore``
# directives that mypy needs (TypeVar narrowing through ``isinstance`` on
# an unrelated base class is not supported) but pyright considers
# unnecessary. Disabling the diagnostic locally is cleaner than littering
# every line with paired ``# pyright: ignore`` annotations.

"""Adapter abstract base class (design document section A.4).

An :class:`Adapter` is what an adapter backend (Rust, JSON-RPC, mock, ...)
hands to the PlatynUI core. Adapters expose identity, structural
relationships, search criteria for the :class:`WeightCalculator`, the
namespaced attribute space (mirroring Rust ``UiNode``), and
pattern discovery / resolution.

Pattern resolution follows a fixed four-step algorithm implemented once
on the ABC (template method). Concrete adapters only override the narrow
:meth:`Adapter._resolve_pattern` hook — see the class docstring for
details.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from collections.abc import Iterator, Sequence
from typing import TYPE_CHECKING, ClassVar, Literal, TypeVar, cast, overload

from .exceptions import (
    AdapterNotValidError,
    NotAPatternTypeError,
    PatternNotSupportedError,
)
from .patterns.base import PatternBase

if TYPE_CHECKING:
    from .technology import Technology
    from .types import FrameworkId, PatternName, RoleName

__all__ = ['Adapter']


P = TypeVar('P', bound=PatternBase)


class Adapter(ABC):
    """What the adapter layer (Rust, JSON-RPC, mock, ...) provides.

    Adapters are *not* :class:`PatternBase` subclasses; the
    :attr:`pattern_name` class attribute exists only as a stable
    Reverse-DNS identifier on the wire, symmetric to how patterns
    advertise themselves.

    Pattern resolution (:meth:`get_pattern`, :meth:`get_pattern_by_name`)
    is implemented once on this class and follows the four-step algorithm
    documented in design section 4.3 / A.4:

    1. ``self`` is an ``isinstance`` of the requested pattern type
       (Rust-style adapters that natively implement a pattern).
    2. The pattern instance is already cached in
       :attr:`_pattern_impls` (keyed by Reverse-DNS pattern name).
    3. The adapter-specific hook :meth:`_resolve_pattern` returns a
       fresh implementation; the framework caches the result.
    4. Otherwise raise :class:`PatternNotSupportedError` (or return
       ``None`` if ``raise_exception=False``).

    Concrete adapters override only :meth:`_resolve_pattern`. They may
    also override :meth:`supports_pattern` for cheaper short-circuits
    (e.g. Rust ``UiNode.has_pattern`` instead of building the full
    pattern set).

    Visual / state properties (``bounds``, ``is_visible``,
    ``is_enabled``, ``is_focused``, ...) are intentionally **not**
    adapter methods — they are reached through the relevant pattern
    (``Element``, ``Focusable``, ...).
    """

    pattern_name: ClassVar['PatternName'] = 'org.platynui.core.Adapter'

    def __init__(self) -> None:
        self._pattern_impls: dict[str, PatternBase] = {}

    # ------------------------------------------------------------------
    # Identity & lifetime
    # ------------------------------------------------------------------

    @property
    @abstractmethod
    def valid(self) -> bool:
        """``True`` while this adapter handle still refers to a live element."""

    @property
    @abstractmethod
    def runtime_id(self) -> str:
        """Stable, opaque identifier for this element within its adapter backend."""

    @property
    @abstractmethod
    def technology(self) -> 'Technology':
        """The technology marker that owns this adapter."""

    # ------------------------------------------------------------------
    # Structural relationships
    # ------------------------------------------------------------------

    @property
    @abstractmethod
    def parent(self) -> 'Adapter | None':
        """Parent adapter, or ``None`` for the root."""

    @property
    @abstractmethod
    def children(self) -> Sequence['Adapter']:
        """Direct child adapters in document order."""

    # ------------------------------------------------------------------
    # Search criteria (consumed by WeightCalculator)
    # ------------------------------------------------------------------

    @property
    @abstractmethod
    def name(self) -> str:
        """Accessible name (may be empty)."""

    @property
    @abstractmethod
    def class_name(self) -> str:
        """Native class / control type name (may be empty)."""

    @property
    def tag_name(self) -> str:
        """XML-style tag name; default empty (HTML/web adapters override)."""
        return ''

    @property
    @abstractmethod
    def role(self) -> str:
        """Primary role (PascalCase, e.g. ``"Button"``, ``"Window"``)."""

    @property
    @abstractmethod
    def supported_roles(self) -> set['RoleName']:
        """All roles this element conforms to (includes :attr:`role`)."""

    @property
    @abstractmethod
    def framework_id(self) -> 'FrameworkId':
        """UI framework identifier (``"WPF"``, ``"Qt"``, ``"Gtk"``, ...)."""

    # ------------------------------------------------------------------
    # Attributes (namespaced; symmetric to Rust UiNode)
    # ------------------------------------------------------------------

    @abstractmethod
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        """Return attribute names.

        With ``namespace=None`` returns names from **all** namespaces —
        primarily useful for inspector/debug tooling. With an explicit
        namespace (``"control"``, ``"item"``, ``"app"``, ``"native"``)
        only that namespace's names are returned.
        """

    @abstractmethod
    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        """Return the value of ``namespace:name`` for this element."""

    @abstractmethod
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        """Iterate ``(namespace, name, value)`` triples — mirrors Rust ``UiNode.attributes()``."""

    # ------------------------------------------------------------------
    # Pattern discovery
    # ------------------------------------------------------------------

    @abstractmethod
    def supported_patterns(self) -> set[type[PatternBase]]:
        """All Python pattern classes this adapter can resolve."""

    @abstractmethod
    def supported_pattern_names(self) -> set['PatternName']:
        """All Reverse-DNS pattern identifiers this adapter advertises.

        Superset of :meth:`supported_patterns`'s ``pattern_name``s — it
        also covers patterns whose Python class is not (yet) imported
        on this side, which matters for cross-runtime adapters.
        """

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        """Default: membership test against :meth:`supported_patterns`.

        Adapters may override with a cheaper check (e.g. Rust ``UiNode.has_pattern``).
        """
        return pattern_type in self.supported_patterns()

    # ------------------------------------------------------------------
    # Pattern resolution (template method; override _resolve_pattern only)
    # ------------------------------------------------------------------

    @abstractmethod
    def _resolve_pattern(self, pattern_name: 'PatternName') -> PatternBase | None:
        """Adapter-specific pattern lookup (step 3 of resolution).

        Return a freshly constructed :class:`PatternBase` implementation
        for ``pattern_name``, or ``None`` if this adapter cannot provide
        it. The framework handles the ``isinstance`` short-circuit, the
        cache, and error reporting around this hook — implementations
        must not cache results themselves.
        """

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
        """Resolve ``pattern_type`` for this adapter.

        Implements the four-step algorithm; see class docstring.
        """
        if not isinstance(pattern_type, type) or not issubclass(pattern_type, PatternBase):
            raise NotAPatternTypeError(f'{pattern_type!r} is not a PatternBase subclass')

        name = getattr(pattern_type, 'pattern_name', None)
        if not isinstance(name, str) or not name:
            raise NotAPatternTypeError(
                f'{pattern_type!r} has no pattern_name; declare a Reverse-DNS identifier'
            )

        # Step 1: self natively implements the pattern (e.g. a Rust-backed
        # adapter that subclasses both Adapter and a pattern ABC). mypy
        # rejects the isinstance() statically since Adapter does not
        # extend PatternBase, hence the unreachable suppression.
        if isinstance(self, pattern_type):
            return cast('P', self)  # type: ignore[unreachable]

        # Step 2: cached instance from a previous resolve. pyright does
        # not narrow TypeVars through isinstance on values typed as a
        # broader base class; the suppressions reflect that gap.
        cached = self._pattern_impls.get(name)
        if cached is not None and isinstance(cached, pattern_type):
            return cached  # pyright: ignore[reportReturnType]

        # Step 3: adapter-specific lookup; cache on success.
        if not self.valid:
            raise AdapterNotValidError(f'adapter {self.runtime_id!r} is no longer valid')
        resolved = self._resolve_pattern(name)
        if resolved is not None:
            if not isinstance(resolved, pattern_type):
                raise PatternNotSupportedError(
                    f'adapter resolved {name!r} to {type(resolved).__name__}, '
                    f'which is not a {pattern_type.__name__}'
                )
            self._pattern_impls[name] = resolved
            return resolved  # pyright: ignore[reportReturnType]

        # Step 4: not supported.
        if raise_exception:
            raise PatternNotSupportedError(
                f'adapter {self.runtime_id!r} does not support pattern {name!r}'
            )
        return None

    def get_pattern_by_name(
        self,
        pattern_name: 'PatternName',
        *,
        raise_exception: bool = True,
    ) -> PatternBase | None:
        """Resolve a pattern by its Reverse-DNS identifier.

        Useful for cross-runtime callers that do not import the
        Python pattern class. Resolution mirrors :meth:`get_pattern`
        but cannot perform the ``isinstance(self, ...)`` short-circuit
        because no class object is available.
        """
        if not isinstance(pattern_name, str) or not pattern_name:
            raise NotAPatternTypeError(f'{pattern_name!r} is not a valid pattern name')

        cached = self._pattern_impls.get(pattern_name)
        if cached is not None:
            return cached

        if not self.valid:
            raise AdapterNotValidError(f'adapter {self.runtime_id!r} is no longer valid')
        resolved = self._resolve_pattern(pattern_name)
        if resolved is not None:
            self._pattern_impls[pattern_name] = resolved
            return resolved

        if raise_exception:
            raise PatternNotSupportedError(
                f'adapter {self.runtime_id!r} does not support pattern {pattern_name!r}'
            )
        return None

    # ------------------------------------------------------------------
    # Identity helpers
    # ------------------------------------------------------------------

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Adapter):
            return NotImplemented
        return self.runtime_id == other.runtime_id

    def __hash__(self) -> int:
        return hash(self.runtime_id)

    def __repr__(self) -> str:  # pragma: no cover - debug aid
        try:
            return (
                f'<{type(self).__name__} runtime_id={self.runtime_id!r} '
                f'role={self.role!r} name={self.name!r}>'
            )
        except Exception:  # repr must never raise
            return f'<{type(self).__name__} (invalid)>'
