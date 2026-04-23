# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false
#
# The native-pattern wrappers and pattern builders below intentionally
# reach into ``UiNodeAdapter._node`` to read the underlying native node.
# They are part of the same module / cooperating implementation, so the
# protected-access diagnostic is noise here.

"""Native-backed :class:`Adapter` implementation (design doc §A.4a).

:class:`UiNodeAdapter` wraps a single ``platynui_native.UiNode`` and
exposes it through the Python :class:`~PlatynUI.core.adapter.Adapter`
contract. It is the *only* production adapter in PlatynUI — the design
treats the native UI tree as the sole technology backend (see
``docs/python-library-design.md`` §A.4 / §A.8). Variations needed for
tests (stubs, spies, scripted behaviour) are layered on top via
:class:`~PlatynUI.core.adapter_proxy.AdapterProxy` overlays, not via
alternative adapter classes.

Pattern resolution maps Reverse-DNS pattern identifiers to thin Python
wrappers around the corresponding native pattern objects. Currently only
:class:`~PlatynUI.core.patterns.Focusable` has a native counterpart; the
remaining capability patterns will be wired up in later migration phases
once the native side exposes them.

Design note — why the wrappers exist
====================================
The Rust core deliberately splits a single Python pattern across two
mechanisms (see the original ``docs/patterns.md`` design memo, in this
repository's git history at commit ``4d36c43``):

* **RuntimePatterns** (e.g. ``FocusableAction``, ``WindowSurfaceActions``)
  carry *actions* — methods that need platform-specific code and may
  fail at runtime (UIA ``SetFocus``, AT-SPI ``grabFocus``, X11 grabs,
  …). They live behind the ``UiPattern`` trait and are obtained via
  ``UiNode::pattern::<T>()``.
* **ClientPatterns** (the much larger set, e.g. ``IsFocused``,
  ``IsSelected``, ``Bounds``) carry *state* — values that every
  provider exposes the same way: as ``UiAttribute`` entries readable
  via ``UiNode::attribute(name, namespace)``.

Python users want a *single* pattern object that bundles both halves
(``Focusable.is_focused`` *and* ``Focusable.focus()``). The native
wrappers below are the seam where the two Rust worlds get fused into
that unified Python view; they are not boilerplate around a single
underlying object.
"""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from typing import TYPE_CHECKING, ClassVar

import platynui_native as _pn

from ..adapter import Adapter
from ..patterns.base import PatternBase
from ..patterns.focusable import Focusable
from ..runtime import runtime
from ..technology import Technology

if TYPE_CHECKING:
    from ..types import FrameworkId, PatternName, RoleName

__all__ = ['UiNodeAdapter', 'UiNodeTechnology']


class UiNodeTechnology(Technology):
    """Singleton marker for the native ``platynui_native`` adapter family."""

    _instance: ClassVar['UiNodeTechnology | None'] = None

    def __new__(cls) -> 'UiNodeTechnology':
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance


# Pre-built singleton so adapters do not pay the __new__ check on every
# .technology access.
_TECHNOLOGY: UiNodeTechnology = UiNodeTechnology()


# ----------------------------------------------------------------------
# Native pattern wrappers
# ----------------------------------------------------------------------


class _NativeFocusable(Focusable):
    """Bridge ``platynui_native.Focusable`` (action) + node attribute (state).

    The native ``Focusable`` pattern only carries the runtime *action*
    ``focus()``; the corresponding *state* ``IsFocused`` lives in the
    node's attribute space (see the module docstring for the rationale
    behind that split). This wrapper is the seam that fuses both into
    the single Python :class:`~PlatynUI.core.patterns.Focusable`
    contract — it is not an indirection over a self-contained object.

    ``IsFocused`` is read from whichever namespace the underlying node
    advertises (``control`` for windows/buttons, ``item`` for list /
    tree items, …), so the wrapper inspects ``node.namespace`` instead
    of hard-coding ``"control"``.
    """

    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Focusable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    def is_focused(self) -> bool:
        # Focusable lives in whichever namespace the underlying node
        # uses (control for widgets like Window/Button, item for
        # ListItem/TreeItem, etc.). Mirror the node's own namespace so
        # we read the right attribute on every kind of focusable.
        node = self._adapter._node
        try:
            value = node.attribute('IsFocused', node.namespace.as_str())
        except _pn.AttributeNotFoundError:
            return False
        return bool(value)

    def focus(self) -> None:
        self._native.focus()


# Reverse-DNS → builder. Builders take the adapter and return a fresh
# pattern instance, or ``None`` if the native side cannot satisfy the
# request for this particular node.
def _build_focusable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Focusable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Focusable):  # defensive: native API is dynamically typed
        return None
    return _NativeFocusable(adapter, native)


_NATIVE_PATTERN_BUILDERS: dict[str, object] = {
    Focusable.pattern_name: _build_focusable,
}


# ----------------------------------------------------------------------
# Adapter
# ----------------------------------------------------------------------


class UiNodeAdapter(Adapter):
    """Adapter backed by a single native ``UiNode``.

    Construct via :meth:`from_node` (or :meth:`create_root` for the
    desktop). The class is intentionally not a dataclass — it owns
    mutable per-instance state (the resolved-pattern cache inherited
    from :class:`Adapter`).

    The adapter does not hold a runtime reference. Operations that need
    one (e.g. delegating to native pointer/keyboard methods) read
    :data:`PlatynUI.core.runtime.runtime`.``current`` lazily — the
    process-wide singleton (design doc §A.5, Rev. 20).
    """

    pattern_name: ClassVar['PatternName'] = 'org.platynui.adapters.UiNode'

    def __init__(self, node: _pn.UiNode) -> None:
        super().__init__()
        self._node = node

    # ------------------------------------------------------------------
    # Construction helpers
    # ------------------------------------------------------------------

    @classmethod
    def from_node(cls, node: _pn.UiNode) -> 'UiNodeAdapter':
        """Wrap an arbitrary native node (used for parent / children walks)."""
        return cls(node)

    @classmethod
    def create_root(cls) -> 'UiNodeAdapter':
        """Wrap the desktop root of the active process-wide runtime."""
        return cls(runtime.current.desktop_node())

    # ------------------------------------------------------------------
    # Identity & lifetime
    # ------------------------------------------------------------------

    @property
    def valid(self) -> bool:
        return self._node.is_valid()

    @property
    def runtime_id(self) -> str:
        return self._node.runtime_id

    @property
    def technology(self) -> Technology:
        return _TECHNOLOGY

    # ------------------------------------------------------------------
    # Structural relationships
    # ------------------------------------------------------------------

    @property
    def parent(self) -> 'Adapter | None':
        parent_node = self._node.parent()
        if parent_node is None:
            return None
        return UiNodeAdapter.from_node(parent_node)

    @property
    def children(self) -> Sequence['Adapter']:
        return [UiNodeAdapter.from_node(child) for child in self._node.children()]

    # ------------------------------------------------------------------
    # Search criteria (consumed by WeightCalculator)
    # ------------------------------------------------------------------

    @property
    def name(self) -> str:
        return self._node.name

    @property
    def class_name(self) -> str:
        return self._safe_str_attr('ClassName', 'control')

    @property
    def role(self) -> str:
        return self._node.role

    @property
    def supported_roles(self) -> set['RoleName']:
        # The native side currently surfaces only the primary role.
        # SupportedRoles will be added once the native attribute group
        # exposes it; for now the primary role is the single entry.
        return {self._node.role}

    @property
    def framework_id(self) -> 'FrameworkId':
        return self._safe_str_attr('FrameworkId', 'native')

    def _safe_str_attr(self, name: str, namespace: str) -> str:
        try:
            value = self._node.attribute(name, namespace)
        except _pn.AttributeNotFoundError:
            return ''
        return '' if value is None else str(value)

    # ------------------------------------------------------------------
    # Attributes (namespaced)
    # ------------------------------------------------------------------

    def attribute_names(self, namespace: str | None = None) -> set[str]:
        if namespace is None:
            return {attr.name for attr in self._node.attributes()}
        return {attr.name for attr in self._node.attributes() if attr.namespace == namespace}

    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        try:
            return self._node.attribute(name, namespace)
        except _pn.AttributeNotFoundError as exc:
            raise KeyError(f'{namespace}:{name}') from exc

    def attributes(self) -> Iterator[tuple[str, str, object]]:
        for attr in self._node.attributes():
            yield (attr.namespace, attr.name, attr.value())

    # ------------------------------------------------------------------
    # Pattern discovery
    # ------------------------------------------------------------------

    def supported_pattern_names(self) -> set['PatternName']:
        return set(self._node.supported_patterns())

    def supported_patterns(self) -> set[type[PatternBase]]:
        names = self.supported_pattern_names()
        result: set[type[PatternBase]] = set()
        if Focusable.pattern_name in names:
            result.add(Focusable)
        return result

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        # A pattern is only truly supported when (a) the native node
        # advertises it AND (b) we have a Python wrapper for it.
        # Returning True without (b) would let get_pattern fail later.
        name = getattr(pattern_type, 'pattern_name', None)
        if not isinstance(name, str) or not name:
            return False
        if name not in _NATIVE_PATTERN_BUILDERS:
            return False
        try:
            return self._node.has_pattern(name)
        except _pn.PatternError:
            return False

    def _resolve_pattern(self, pattern_name: 'PatternName') -> PatternBase | None:
        builder = _NATIVE_PATTERN_BUILDERS.get(pattern_name)
        if builder is None:
            return None
        # Builders accept (UiNodeAdapter,) and return PatternBase | None.
        return builder(self)  # type: ignore[operator,no-any-return]
