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
through the Python `Adapter` contract. Pattern access combines
native action calls (e.g. ``Focusable.focus()``) with attribute
reads (e.g. ``Focusable.is_focused``).
"""

from collections.abc import Iterator, Sequence
from typing import TYPE_CHECKING, ClassVar, override

import platynui_native as _pn

from ..adapter import Adapter
from ..patterns.activation import Activatable
from ..patterns.activation_target import ActivationTarget
from ..patterns.base import PatternBase
from ..patterns.closeable import Closeable
from ..patterns.element import Element
from ..patterns.focusable import Focusable
from ..patterns.maximizable import Maximizable
from ..patterns.minimizable import Minimizable
from ..patterns.movable import Movable
from ..patterns.readable import Readable
from ..patterns.resizable import Resizable
from ..patterns.responsive import Responsive
from ..patterns.restorable import Restorable
from ..patterns.text import TextContent
from ..patterns.window_state import WindowState
from ..runtime import runtime
from ..types import Point, Rect, Size

if TYPE_CHECKING:
    from ..types import FrameworkId, PatternName, RoleName

__all__ = ['UiNodeAdapter']


# ----------------------------------------------------------------------
# Native pattern wrappers
# ----------------------------------------------------------------------


def _bool_attr(adapter: 'UiNodeAdapter', name: str, namespace: str = 'control') -> bool:
    node = adapter._node
    try:
        value = node.attribute(name, namespace)
    except _pn.AttributeNotFoundError:
        return False
    return bool(value)


class _NativeFocusable(Focusable):
    """`Focusable` implementation backed by the native focus action and ``IsFocused`` attribute."""

    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Focusable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def is_focused(self) -> bool:
        return _bool_attr(self._adapter, 'IsFocused')

    @override
    def focus(self) -> None:
        self._native.focus()


class _NativeActivatable(Activatable):
    """`Activatable` for top-level windows; delegates to the native activate action."""

    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Activatable) -> None:
        self._adapter = adapter
        self._native = native

    @override
    def activate(self) -> None:
        self._native.activate()

    @property
    @override
    def is_activation_enabled(self) -> bool:
        return _bool_attr(self._adapter, 'IsActivationEnabled')

    @property
    @override
    def default_accelerator(self) -> str | None:
        node = self._adapter._node
        try:
            value = node.attribute('DefaultAccelerator', node.namespace.as_str())
        except _pn.AttributeNotFoundError:
            return None
        if value is None or value == '':
            return None
        return str(value)


class _NativeWindowState(WindowState):
    """`WindowState` reads the ``IsActive``, ``IsTopmost`` and ``IsModal`` attributes."""

    __slots__ = ('_adapter',)

    def __init__(self, adapter: 'UiNodeAdapter') -> None:
        self._adapter = adapter

    @property
    @override
    def is_active(self) -> bool:
        return _bool_attr(self._adapter, 'IsActive')

    @property
    @override
    def is_topmost(self) -> bool:
        return _bool_attr(self._adapter, 'IsTopmost')

    @property
    @override
    def is_modal(self) -> bool:
        return _bool_attr(self._adapter, 'IsModal')


class _NativeElement(Element):
    """`Element` reads ``Bounds``, ``IsVisible``, ``IsInView`` and ``IsEnabled`` attributes."""

    __slots__ = ('_adapter',)

    def __init__(self, adapter: 'UiNodeAdapter') -> None:
        self._adapter = adapter

    @property
    @override
    def bounds(self) -> Rect:
        node = self._adapter._node
        try:
            value = node.attribute('Bounds', 'control')
        except _pn.AttributeNotFoundError:
            return Rect(0.0, 0.0, 0.0, 0.0)
        if not isinstance(value, Rect):  # defensive: native API is dynamically typed
            return Rect(0.0, 0.0, 0.0, 0.0)
        return value

    @property
    @override
    def is_visible(self) -> bool:
        return _bool_attr(self._adapter, 'IsVisible')

    @property
    @override
    def is_in_view(self) -> bool:
        return _bool_attr(self._adapter, 'IsInView')

    @property
    @override
    def is_enabled(self) -> bool:
        return _bool_attr(self._adapter, 'IsEnabled')


class _NativeActivationTarget(ActivationTarget):
    """`ActivationTarget` reads ``ActivationPoint``, ``ActivationArea``, ``ActivationHint`` attributes."""

    __slots__ = ('_adapter',)

    def __init__(self, adapter: 'UiNodeAdapter') -> None:
        self._adapter = adapter

    @property
    @override
    def activation_point(self) -> Point:
        node = self._adapter._node
        value = node.attribute('ActivationPoint', 'control')
        if not isinstance(value, Point):  # defensive: native API is dynamically typed
            raise TypeError(f'ActivationPoint attribute must be a Point, got {type(value).__name__}')
        return value

    @property
    @override
    def activation_area(self) -> Rect | None:
        node = self._adapter._node
        try:
            value = node.attribute('ActivationArea', 'control')
        except _pn.AttributeNotFoundError:
            return None
        if not isinstance(value, Rect):
            return None
        return value

    @property
    @override
    def activation_hint(self) -> str | None:
        node = self._adapter._node
        try:
            value = node.attribute('ActivationHint', 'control')
        except _pn.AttributeNotFoundError:
            return None
        if value is None or value == '':
            return None
        return str(value)


class _NativeReadable(Readable):
    """`Readable` reads the ``IsReadOnly`` attribute."""

    __slots__ = ('_adapter',)

    def __init__(self, adapter: 'UiNodeAdapter') -> None:
        self._adapter = adapter

    @property
    @override
    def is_readonly(self) -> bool:
        return _bool_attr(self._adapter, 'IsReadOnly')


class _NativeTextContent(TextContent):
    """`TextContent` reads the read-only ``control:Text`` attribute."""

    __slots__ = ('_adapter',)

    def __init__(self, adapter: 'UiNodeAdapter') -> None:
        self._adapter = adapter

    @property
    @override
    def text(self) -> str:
        return str(self._adapter.attribute_value('Text'))


class _NativeMinimizable(Minimizable):
    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Minimizable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def is_minimized(self) -> bool:
        return _bool_attr(self._adapter, 'IsMinimized')

    @property
    @override
    def can_minimize(self) -> bool:
        return _bool_attr(self._adapter, 'CanMinimize')

    @override
    def minimize(self) -> None:
        self._native.minimize()


class _NativeMaximizable(Maximizable):
    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Maximizable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def is_maximized(self) -> bool:
        return _bool_attr(self._adapter, 'IsMaximized')

    @property
    @override
    def can_maximize(self) -> bool:
        return _bool_attr(self._adapter, 'CanMaximize')

    @override
    def maximize(self) -> None:
        self._native.maximize()


class _NativeRestorable(Restorable):
    __slots__ = ('_native',)

    def __init__(self, native: _pn.Restorable) -> None:
        self._native = native

    @override
    def restore(self) -> None:
        self._native.restore()


class _NativeCloseable(Closeable):
    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Closeable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def can_close(self) -> bool:
        return _bool_attr(self._adapter, 'CanClose')

    @override
    def close(self) -> None:
        self._native.close()


class _NativeMovable(Movable):
    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Movable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def can_move(self) -> bool:
        return _bool_attr(self._adapter, 'CanMove')

    @override
    def move_to(self, point: Point) -> None:
        self._native.move_to(point.x, point.y)


class _NativeResizable(Resizable):
    __slots__ = ('_adapter', '_native')

    def __init__(self, adapter: 'UiNodeAdapter', native: _pn.Resizable) -> None:
        self._adapter = adapter
        self._native = native

    @property
    @override
    def can_resize(self) -> bool:
        return _bool_attr(self._adapter, 'CanResize')

    @override
    def resize(self, size: Size) -> None:
        self._native.resize(size.width, size.height)


class _NativeResponsive(Responsive):
    __slots__ = ('_native',)

    def __init__(self, native: _pn.Responsive) -> None:
        self._native = native

    @override
    def accepts_user_input(self) -> bool | None:
        return self._native.accepts_user_input()


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


def _build_activatable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Activatable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Activatable):
        return None
    return _NativeActivatable(adapter, native)


def _build_window_state(adapter: 'UiNodeAdapter') -> PatternBase | None:
    # No native pattern object — capability is derived from attribute presence.
    if not _has_attribute(adapter, 'IsActive'):
        return None
    return _NativeWindowState(adapter)


def _build_element(adapter: 'UiNodeAdapter') -> PatternBase | None:
    # No native pattern object — capability is derived from attribute presence.
    if not _has_attribute(adapter, 'Bounds'):
        return None
    return _NativeElement(adapter)


def _build_activation_target(adapter: 'UiNodeAdapter') -> PatternBase | None:
    # No native pattern object — `ActivationPoint` is mandatory; without
    # it the pattern cannot satisfy its contract.
    if not _has_attribute(adapter, 'ActivationPoint'):
        return None
    return _NativeActivationTarget(adapter)


def _build_readable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    # No native pattern object — capability is derived from attribute presence.
    if not _has_attribute(adapter, 'IsReadOnly'):
        return None
    return _NativeReadable(adapter)


def _build_textcontent(adapter: 'UiNodeAdapter') -> PatternBase | None:
    # No native pattern object — capability is derived from the presence of
    # the read-only `control:Text` attribute the providers synthesise.
    if not _has_attribute(adapter, 'Text'):
        return None
    return _NativeTextContent(adapter)


def _build_minimizable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Minimizable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Minimizable):
        return None
    return _NativeMinimizable(adapter, native)


def _build_maximizable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Maximizable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Maximizable):
        return None
    return _NativeMaximizable(adapter, native)


def _build_restorable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Restorable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Restorable):
        return None
    return _NativeRestorable(native)


def _build_closeable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Closeable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Closeable):
        return None
    return _NativeCloseable(adapter, native)


def _build_movable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Movable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Movable):
        return None
    return _NativeMovable(adapter, native)


def _build_resizable(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Resizable.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Resizable):
        return None
    return _NativeResizable(adapter, native)


def _build_responsive(adapter: 'UiNodeAdapter') -> PatternBase | None:
    try:
        native = adapter._node.get_pattern(Responsive.pattern_name)
    except _pn.PatternError:
        return None
    if not isinstance(native, _pn.Responsive):
        return None
    return _NativeResponsive(native)


def _has_attribute(adapter: 'UiNodeAdapter', name: str) -> bool:
    return any(attr.name == name for attr in adapter._node.attributes())


_NATIVE_PATTERN_BUILDERS: dict[str, object] = {
    Element.pattern_name: _build_element,
    ActivationTarget.pattern_name: _build_activation_target,
    Readable.pattern_name: _build_readable,
    TextContent.pattern_name: _build_textcontent,
    Focusable.pattern_name: _build_focusable,
    Activatable.pattern_name: _build_activatable,
    WindowState.pattern_name: _build_window_state,
    Minimizable.pattern_name: _build_minimizable,
    Maximizable.pattern_name: _build_maximizable,
    Restorable.pattern_name: _build_restorable,
    Closeable.pattern_name: _build_closeable,
    Movable.pattern_name: _build_movable,
    Resizable.pattern_name: _build_resizable,
    Responsive.pattern_name: _build_responsive,
}


# Pattern types that map directly to a same-named native pattern for
# `supported_patterns()` reporting. Attribute-only patterns (no native
# pattern object) are listed separately in `_ATTRIBUTE_ONLY_PATTERNS`
# with the sentinel attribute that proves the pattern is available.
_NATIVE_PATTERN_TYPES: tuple[type[PatternBase], ...] = (
    Focusable,
    Activatable,
    Minimizable,
    Maximizable,
    Restorable,
    Closeable,
    Movable,
    Resizable,
    Responsive,
)


# Attribute-only patterns: (pattern_type, sentinel_attribute_name).
# `supports_pattern`/`supported_patterns` treat these specially because
# the native side does not advertise them via `has_pattern`.
_ATTRIBUTE_ONLY_PATTERNS: tuple[tuple[type[PatternBase], str], ...] = (
    (Element, 'Bounds'),
    (ActivationTarget, 'ActivationPoint'),
    (Readable, 'IsReadOnly'),
    (TextContent, 'Text'),
    (WindowState, 'IsActive'),
)


# ----------------------------------------------------------------------
# Adapter
# ----------------------------------------------------------------------


class UiNodeAdapter(Adapter):
    """Adapter backed by a single native UI node.

    Construct via `from_node`, or `create_root` for the desktop.
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
    # Native-node access
    # ------------------------------------------------------------------

    @property
    def native_node(self) -> _pn.UiNode:
        """The wrapped native `_pn.UiNode`."""
        return self._node

    # ------------------------------------------------------------------
    # Identity & lifetime
    # ------------------------------------------------------------------

    @property
    @override
    def valid(self) -> bool:
        return self._node.is_valid()

    @property
    @override
    def runtime_id(self) -> str:
        return self._node.runtime_id

    # ------------------------------------------------------------------
    # Structural relationships
    # ------------------------------------------------------------------

    @property
    @override
    def parent(self) -> 'Adapter | None':
        parent_node = self._node.parent()
        if parent_node is None:
            return None
        return UiNodeAdapter.from_node(parent_node)

    @property
    @override
    def children(self) -> Sequence['Adapter']:
        return [UiNodeAdapter.from_node(child) for child in self._node.children()]

    # ------------------------------------------------------------------
    # Search criteria (consumed by WeightCalculator)
    # ------------------------------------------------------------------

    @property
    @override
    def name(self) -> str:
        return self._node.name

    @property
    @override
    def class_name(self) -> str:
        return self._safe_str_attr('ClassName', 'control')

    @property
    @override
    def role(self) -> str:
        return self._node.role

    @property
    @override
    def supported_roles(self) -> set['RoleName']:
        # Only the primary role is currently exposed by the underlying
        # node; additional roles will be returned once the attribute
        # surface advertises them.
        return {self._node.role}

    @property
    @override
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

    @override
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        if namespace is None:
            return {attr.name for attr in self._node.attributes()}
        return {attr.name for attr in self._node.attributes() if attr.namespace == namespace}

    @override
    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        try:
            return self._node.attribute(name, namespace)
        except _pn.AttributeNotFoundError as exc:
            raise KeyError(f'{namespace}:{name}') from exc

    @override
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        for attr in self._node.attributes():
            yield (attr.namespace, attr.name, attr.value())

    # ------------------------------------------------------------------
    # Pattern discovery
    # ------------------------------------------------------------------

    @override
    def supported_pattern_names(self) -> set['PatternName']:
        return set(self._node.supported_patterns())

    @override
    def supported_patterns(self) -> set[type[PatternBase]]:
        names = self.supported_pattern_names()
        result: set[type[PatternBase]] = {pt for pt in _NATIVE_PATTERN_TYPES if pt.pattern_name in names}
        for pattern_type, sentinel in _ATTRIBUTE_ONLY_PATTERNS:
            if _has_attribute(self, sentinel):
                result.add(pattern_type)
        return result

    @override
    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        # A pattern is only truly supported when (a) the native node
        # advertises it (or, for attribute-only patterns, exposes the
        # sentinel attribute) AND (b) we have a Python wrapper for it.
        # Returning True without (b) would let get_pattern fail later.
        name = getattr(pattern_type, 'pattern_name', None)
        if not isinstance(name, str) or not name:
            return False
        if name not in _NATIVE_PATTERN_BUILDERS:
            return False
        for attr_pattern, sentinel in _ATTRIBUTE_ONLY_PATTERNS:
            if pattern_type is attr_pattern:
                return _has_attribute(self, sentinel)
        try:
            return self._node.has_pattern(name)
        except _pn.PatternError:
            return False

    @override
    def _resolve_pattern(self, pattern_name: 'PatternName') -> PatternBase | None:
        builder = _NATIVE_PATTERN_BUILDERS.get(pattern_name)
        if builder is None:
            return None
        # Builders accept (UiNodeAdapter,) and return PatternBase | None.
        return builder(self)  # type: ignore[operator,no-any-return]
