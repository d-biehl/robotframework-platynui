# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnnecessaryTypeIgnoreComment=false
#
# The pattern-resolution algorithm below uses ``# type: ignore`` directives
# that mypy needs (TypeVar narrowing through ``isinstance`` on an unrelated
# base class is not supported) but pyright considers unnecessary. Disabling
# the diagnostic locally is cleaner than pairing each line with its own
# ``# pyright: ignore``.

"""Adapter abstract base class.

Adapters expose a UI element to the PlatynUI core: identity, structural
relationships, search criteria for the
`WeightCalculator`, the
namespaced attribute space, and pattern discovery and resolution.

Pattern resolution follows a fixed four-step algorithm implemented
once on the ABC. Concrete adapters override only the narrow
`_resolve_pattern` hook.
"""

from abc import ABC, abstractmethod
from collections.abc import Iterator, Sequence
from typing import TYPE_CHECKING, ClassVar, Literal, cast, overload

from .exceptions import (
    AdapterNotValidError,
    NotAPatternTypeError,
    PatternNotSupportedError,
)
from .patterns.base import PatternBase

if TYPE_CHECKING:
    from .types import FrameworkId, PatternName, RoleName

__all__ = ['Adapter']


class Adapter(ABC):
    """Expose a UI element to the PlatynUI core.

    `get_pattern` and `get_pattern_by_name` resolve a pattern by:

    1. checking whether ``self`` is an instance of the pattern type,
    2. returning a cached implementation from `_pattern_impls`,
    3. asking `_resolve_pattern` to build a fresh one,
    4. or raising `PatternNotSupportedError` (or returning ``None``
       when ``raise_exception=False``).

    Concrete adapters override `_resolve_pattern` and optionally
    `supports_pattern` for a cheaper short-circuit.
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
        """Whether this handle still refers to a live element."""

    @property
    @abstractmethod
    def runtime_id(self) -> str:
        """A stable opaque identifier within this adapter's backend."""

    # ------------------------------------------------------------------
    # Structural relationships
    # ------------------------------------------------------------------

    @property
    @abstractmethod
    def parent(self) -> 'Adapter | None':
        """The parent adapter, or ``None`` for the root."""

    @property
    @abstractmethod
    def children(self) -> Sequence['Adapter']:
        """The direct child adapters in document order."""

    # ------------------------------------------------------------------
    # Search criteria (consumed by WeightCalculator)
    # ------------------------------------------------------------------

    @property
    @abstractmethod
    def name(self) -> str:
        """The accessible name; may be empty."""

    @property
    @abstractmethod
    def class_name(self) -> str:
        """The native class or control type name; may be empty."""

    @property
    def tag_name(self) -> str:
        """The XML-style tag name; empty unless overridden."""
        return ''

    @property
    @abstractmethod
    def role(self) -> str:
        """The primary role in PascalCase, for example ``"Button"``."""

    @property
    @abstractmethod
    def supported_roles(self) -> set['RoleName']:
        """All roles this element conforms to, including `role`."""

    @property
    @abstractmethod
    def framework_id(self) -> 'FrameworkId':
        """The UI framework identifier, e.g. ``"WPF"`` or ``"Qt"``."""

    # ------------------------------------------------------------------
    # Attributes (namespaced; symmetric to Rust UiNode)
    # ------------------------------------------------------------------

    @abstractmethod
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        """Return attribute names.

        Pass ``namespace=None`` to return names from every namespace
        (mainly for inspector or debug tooling). Pass an explicit
        namespace (``"control"``, ``"item"``, ``"app"``, ``"native"``)
        to restrict the result to that namespace.
        """

    @abstractmethod
    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        """Return the value of the ``namespace:name`` attribute."""

    @abstractmethod
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        """Yield ``(namespace, name, value)`` for every attribute."""

    # ------------------------------------------------------------------
    # Pattern discovery
    # ------------------------------------------------------------------

    @abstractmethod
    def supported_patterns(self) -> set[type[PatternBase]]:
        """Return every Python pattern class this adapter can resolve."""

    @abstractmethod
    def supported_pattern_names(self) -> set['PatternName']:
        """Return every Reverse-DNS pattern identifier this adapter advertises.

        Superset of the ``pattern_name`` values from
        `supported_patterns`: also covers patterns whose Python
        class is not (yet) imported on this side, which matters for
        cross-runtime adapters.
        """

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        """Return whether ``pattern_type`` is in `supported_patterns`.

        Adapters may override with a cheaper check that avoids building
        the full pattern set.
        """
        return pattern_type in self.supported_patterns()

    # ------------------------------------------------------------------
    # Pattern resolution (template method; override _resolve_pattern only)
    # ------------------------------------------------------------------

    @abstractmethod
    def _resolve_pattern(self, pattern_name: 'PatternName') -> PatternBase | None:
        """Construct the implementation for ``pattern_name``.

        Return a fresh `PatternBase` instance, or ``None`` when
        this adapter cannot provide it. The ``isinstance`` short-circuit,
        the cache, and error reporting are handled around this hook;
        implementations must not cache results themselves.
        """

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P]) -> P: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: Literal[True]) -> P: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: Literal[False]) -> P | None: ...

    @overload
    def get_pattern[P: PatternBase](self, pattern_type: type[P], *, raise_exception: bool) -> P | None: ...

    def get_pattern[P: PatternBase](
        self,
        pattern_type: type[P],
        *,
        raise_exception: bool = True,
    ) -> P | None:
        """Resolve ``pattern_type`` for this adapter.

        Run the four-step algorithm described in the class docstring.
        Raise `PatternNotSupportedError` when the pattern cannot
        be obtained, unless ``raise_exception`` is false (in which case
        return ``None``).
        """
        if not isinstance(pattern_type, type) or not issubclass(pattern_type, PatternBase):
            raise NotAPatternTypeError(f'{pattern_type!r} is not a PatternBase subclass')

        name = getattr(pattern_type, 'pattern_name', None)
        if not isinstance(name, str) or not name:
            raise NotAPatternTypeError(f'{pattern_type!r} has no pattern_name; declare a Reverse-DNS identifier')

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
                    f'adapter resolved {name!r} to {type(resolved).__name__}, which is not a {pattern_type.__name__}'
                )
            self._pattern_impls[name] = resolved
            return resolved  # pyright: ignore[reportReturnType]

        # Step 4: not supported.
        if raise_exception:
            raise PatternNotSupportedError(f'adapter {self.runtime_id!r} does not support pattern {name!r}')
        return None

    def get_pattern_by_name(
        self,
        pattern_name: 'PatternName',
        *,
        raise_exception: bool = True,
    ) -> PatternBase | None:
        """Resolve a pattern by its Reverse-DNS identifier.

        Behaves like `get_pattern` but skips the
        ``isinstance(self, ...)`` short-circuit because no class object
        is available. Useful for callers that do not import the Python
        pattern class.
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
            raise PatternNotSupportedError(f'adapter {self.runtime_id!r} does not support pattern {pattern_name!r}')
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
            return f'<{type(self).__name__} runtime_id={self.runtime_id!r} role={self.role!r} name={self.name!r}>'
        except Exception:  # repr must never raise
            return f'<{type(self).__name__} (invalid)>'
