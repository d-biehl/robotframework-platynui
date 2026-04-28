# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Lazy element reference used as Robot keyword argument type.

`ElementDescriptor` wraps either a `Locator` or an already-resolved
`ContextBase`. Calling the descriptor resolves it on demand and, when
``full_context=True``, upgrades the cached context to the best-matching
`ContextBase` subclass via `ContextFactory`.

`PatternT` is a phantom marker: ``ElementDescriptor[patterns.Activatable]``
registers as its own Robot converter without constraining the runtime
return type.

Root-element storage is swappable via `set_root_element_storage`; the
Robot library installs an override that reads and writes
``${PLATYNUI_ROOT_ELEMENT}``.
"""

from typing import TYPE_CHECKING, Any, Generic

from typing_extensions import TypeVar

from .context import ContextBase, ContextFactory
from .locator import Locator
from .patterns.base import PatternBase

if TYPE_CHECKING:
    from collections.abc import Callable

__all__ = [
    'ElementDescriptor',
    'PatternT',
    'RootElementDescriptor',
    'RootElementGetter',
    'RootElementSetter',
    'reset_root_element_storage',
    'set_root_element_storage',
]


PatternT = TypeVar('PatternT', bound=PatternBase, default=PatternBase)
"""Phantom marker tying a descriptor to a pattern interface.

Only used for static typing and Robot converter dispatch; the runtime
return type of `ElementDescriptor.__call__` is always `ContextBase`.
"""


# ----------------------------------------------------------------------
# Root-element storage hook
# ----------------------------------------------------------------------


RootElementGetter = 'Callable[[], ElementDescriptor[Any] | None]'
RootElementSetter = 'Callable[[ElementDescriptor[Any] | None], ElementDescriptor[Any] | None]'

_root_element_slot: 'ElementDescriptor[Any] | None' = None


def _default_get_root_element() -> 'ElementDescriptor[Any] | None':
    return _root_element_slot


def _default_set_root_element(
    element: 'ElementDescriptor[Any] | None',
) -> 'ElementDescriptor[Any] | None':
    global _root_element_slot
    previous = _root_element_slot
    _root_element_slot = element
    return previous


_get_root_element: 'Callable[[], ElementDescriptor[Any] | None]' = _default_get_root_element
_set_root_element: 'Callable[[ElementDescriptor[Any] | None], ElementDescriptor[Any] | None]' = (
    _default_set_root_element
)


def set_root_element_storage(
    getter: 'Callable[[], ElementDescriptor[Any] | None]',
    setter: ('Callable[[ElementDescriptor[Any] | None], ElementDescriptor[Any] | None]'),
) -> None:
    """Replace the root-element storage hook with a custom pair.

    The Robot library installs an override backed by
    `EXECUTION_CONTEXTS.current.variables` so each suite sees its own
    root. Tests and `BareMetal` use the default in-process slot.
    """
    global _get_root_element, _set_root_element
    _get_root_element = getter
    _set_root_element = setter


def reset_root_element_storage() -> None:
    """Restore the default in-process storage and clear the slot."""
    global _get_root_element, _set_root_element, _root_element_slot
    _get_root_element = _default_get_root_element
    _set_root_element = _default_set_root_element
    _root_element_slot = None


# ----------------------------------------------------------------------
# ElementDescriptor
# ----------------------------------------------------------------------


class ElementDescriptor(Generic[PatternT]):
    """Lazy reference to a UI element.

    Construct from a `Locator` (resolution is deferred until call) or
    from an already-resolved `ContextBase` (call returns it as-is).
    Calling the descriptor caches the result.
    """

    __slots__ = (
        '_context',
        '_context_type',
        '_has_full_context',
        '_locator',
        '_parent',
    )

    def __init__(
        self,
        locator: Locator | None = None,
        context_type: type[ContextBase] | None = None,
        parent: 'ElementDescriptor[Any] | None' = None,
        context: ContextBase | None = None,
    ) -> None:
        self._locator = locator
        self._context_type = context_type
        self._parent = parent
        self._context: ContextBase | None = context
        self._has_full_context = context is not None

    def __repr__(self) -> str:
        target = self._context if self._context is not None else self._locator
        return f'<ElementDescriptor for {target!r}>'

    def __call__(self, *, full_context: bool = True) -> ContextBase:
        """Resolve the descriptor and return the cached `ContextBase`.

        With ``full_context=True`` the returned context is an instance
        of the best-matching `ContextBase` subclass (chosen by
        `ContextFactory.find_context_class_for`); with ``False`` a bare
        `ContextBase` is returned for cheap property reads.
        """
        if self._context is None:
            parent_context = self._parent() if self._parent is not None else None
            self._context = ContextBase(self._locator, context_parent=parent_context)

        if full_context and not self._has_full_context:
            adapter = self._context.get_adapter()
            if adapter is not None:
                chosen = ContextFactory.find_context_class_for(adapter, self._context_type)
                self._context = chosen(
                    self._locator,
                    context_parent=self._context.context_parent,
                    adapter=adapter,
                )
                self._has_full_context = True

        return self._context

    @staticmethod
    def convert(value: 'str | ContextBase') -> 'ElementDescriptor[Any]':
        """Robot converter: build a descriptor from string or context."""
        if isinstance(value, ContextBase):
            return ElementDescriptor(context=value)
        return ElementDescriptor(
            Locator(path=value),
            parent=ElementDescriptor.get_root_element(),
        )

    @staticmethod
    def set_root_element(
        element: 'ElementDescriptor[Any] | None',
    ) -> 'ElementDescriptor[Any] | None':
        """Install ``element`` as the ambient root and return the previous value."""
        return _set_root_element(element)

    @staticmethod
    def get_root_element() -> 'ElementDescriptor[Any] | None':
        """Return the currently installed root element, or `None`."""
        return _get_root_element()


class RootElementDescriptor(ElementDescriptor[PatternT]):
    """Descriptor variant whose ``convert`` ignores the ambient root."""

    @staticmethod
    def convert(value: 'str | ContextBase') -> 'ElementDescriptor[Any]':
        """Build a descriptor from string or context, without inheriting a root."""
        if isinstance(value, ContextBase):
            return ElementDescriptor(context=value)
        return RootElementDescriptor(Locator(path=value))
