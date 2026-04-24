# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false
#
# The native-pattern wrappers and pattern builders below deliberately
# reach into ``UiNodeAdapter._node`` to read the underlying native node.
# They are part of the same cooperating implementation, so the
# protected-access diagnostic is noise here.

"""Native-backed `Adapter` for the platform UI tree.

`UiNodeAdapter` wraps a single native UI node and exposes it
through the Python `Adapter` contract.
It is the only production adapter; test variations (stubs, spies,
scripted behaviour) are layered on top through
`AdapterProxy` overlays.

Pattern access combines two underlying mechanisms into the single
Python pattern view users expect:

* Actions, e.g. ``Focusable.focus()``: platform-specific calls that
  may fail at runtime.
* State, e.g. ``Focusable.is_focused``: values read from the node's
  attribute space.

Currently only `Focusable` is wired
up; the remaining capability patterns follow as the native side
exposes them.
"""

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
    """Marker singleton identifying the native UI-tree technology."""

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
    """Combine the native focus action with the ``IsFocused`` state attribute.

    The native focus pattern carries only the ``focus()`` action; the
    matching ``IsFocused`` state lives in the node's attribute space.
    This wrapper exposes both as a single
    `Focusable` object.

    ``IsFocused`` is read from whichever namespace the underlying node
    advertises (``control`` for windows and buttons, ``item`` for list
    or tree items, ...), so the wrapper mirrors the node's own
    namespace instead of hard-coding one.
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


# Reverse-DNS to builder. Each builder takes the adapter and returns a
# fresh pattern instance, or ``None`` if the native side cannot satisfy
# the request for this particular node.
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
    """Adapter backed by a single native UI node.

    Construct via `from_node`, or `create_root` for the
    desktop. Not a dataclass: each instance owns mutable state
    (the resolved-pattern cache inherited from `Adapter`).

    Operations that need the active runtime read it lazily from the
    process-wide `runtime` singleton; the
    adapter itself does not hold a runtime reference.
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
        """Wrap an arbitrary native node, e.g. from a parent or child walk."""
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
        # Only the primary role is currently exposed by the underlying
        # node; additional roles will be returned once the attribute
        # surface advertises them.
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
