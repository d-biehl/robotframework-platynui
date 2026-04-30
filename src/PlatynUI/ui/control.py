# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnnecessaryTypeIgnoreComment=false

"""`Control` context base for focusable UI elements."""

from collections.abc import Iterator
from typing import TYPE_CHECKING, ForwardRef, override

from ..core import patterns
from ..core.devices import KeyboardAction
from ..core.locator import Locator
from ..core.predicate import predicate
from .element import Element, _ElementKeyboardProxy

if TYPE_CHECKING:
    from .item import Item

__all__ = ['Control', 'ItemContainer']


class _ControlKeyboardProxy(_ElementKeyboardProxy):
    """Keyboard proxy that ensures the owning control has focus before each action."""

    @override
    def before_action(self, action: KeyboardAction) -> None:
        super().before_action(action)
        self._element.ensure_that(self._element._control_has_focus)  # type: ignore[attr-defined]


class Control(Element, register=False):
    """Context base for focusable UI elements."""

    @property
    def has_focus(self) -> bool:
        """Whether the control currently has keyboard focus."""
        focusable = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        return focusable.is_focused if focusable is not None else False

    def focus(self) -> None:
        """Move keyboard focus to this control."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        focusable = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        if focusable is not None:
            focusable.focus()

    @predicate('control {0} has focus')
    def _control_has_focus(self) -> bool:
        if self.has_focus:
            return True
        self.focus()
        return self.has_focus

    @override
    def _create_keyboard_proxy(self) -> _ElementKeyboardProxy:
        return _ControlKeyboardProxy(self)


def _resolve_item_type(cls: type) -> 'type[Item]':
    """Walk the MRO and return the concrete `Item` subclass bound to `ItemContainer[I]`.

    Caches the result on the class for repeat lookups.
    """
    cached = cls.__dict__.get('_item_container_item_type')
    if cached is not None:
        return cached  # type: ignore[no-any-return]

    import typing as _typing

    from .item import Item as _Item

    for base in cls.__mro__:
        for orig in getattr(base, '__orig_bases__', ()):
            origin = _typing.get_origin(orig)
            if origin is None or not (isinstance(origin, type) and issubclass(origin, ItemContainer)):
                continue
            args = _typing.get_args(orig)
            if not args:
                continue
            arg = args[0]
            if isinstance(arg, str):
                arg = ForwardRef(arg)
            if isinstance(arg, ForwardRef):
                # Resolve the forward ref against the class's module globals.
                import sys

                module = sys.modules.get(cls.__module__)
                ns = getattr(module, '__dict__', {})
                arg = arg._evaluate(ns, None, recursive_guard=frozenset())
            if not isinstance(arg, type) or not issubclass(arg, _Item):
                raise TypeError(
                    f'ItemContainer type argument for {cls.__name__} must be a subclass of Item, got {arg!r}',
                )
            cls._item_container_item_type = arg  # type: ignore[attr-defined]
            return arg

    raise TypeError(f'{cls.__name__} does not parameterise ItemContainer[I]')


class ItemContainer[I: 'Item'](Control, register=False):
    """Generic base for control contexts that host `Item`-typed children.

    Concrete containers parameterise the generic with the item type they host:

    ```python
    class List(ItemContainer[ListItem]): ...
    ```

    The ``get_items``/``iter_items``/``get_item`` methods then delegate to
    ``self.get_all``/``iter_all``/``get`` with the resolved item type and
    ``scope='children'``.
    """

    def get_items(self, *, locator: Locator | None = None) -> list[I]:
        """Return every item directly contained by this container."""
        item_cls = _resolve_item_type(type(self))
        return self.get_all(item_cls, locator=locator, scope='children')  # type: ignore[arg-type]

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[I]:
        """Iterate over every item directly contained by this container."""
        item_cls = _resolve_item_type(type(self))
        return self.iter_all(item_cls, locator=locator, scope='children')  # type: ignore[arg-type]

    def get_item(self, *, locator: Locator | None = None) -> I:
        """Resolve a single item, raising if zero or multiple match."""
        item_cls = _resolve_item_type(type(self))
        return self.get(item_cls, locator=locator, scope='children')  # type: ignore[return-value]

    # ----- Selection (Read pattern at the container, Rev. 46/47) ------

    @property
    def can_select_multiple(self) -> bool:
        """Whether the container allows multi-selection."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Selection).can_select_multiple

    @property
    def is_selection_required(self) -> bool:
        """Whether at least one item must remain selected."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(patterns.Selection).is_selection_required

    def get_selected_items(self) -> list[I]:
        """Return the currently selected items as wrapped UI objects.

        Reads the container's `Selection` pattern, then wraps each
        adapter via `ContextFactory.find_context_class_for(adapter,
        item_cls)` so subclass-specific items (e.g. a custom
        `MyListItem`) are honoured.
        """
        from ..core.context import ContextFactory

        item_cls = _resolve_item_type(type(self))
        adapters = self.adapter.get_pattern(patterns.Selection).get_selected_adapters()
        return [
            ContextFactory.find_context_class_for(a, item_cls)(context_parent=self, adapter=a)  # type: ignore[misc]
            for a in adapters
        ]

    # ----- Selection convenience wrappers (resolve item + delegate) ---

    def select(self, *, locator: Locator | None = None) -> I:
        """Resolve a single item, call ``Item.select()``, return the item."""
        item = self.get_item(locator=locator)
        item.select()
        return item

    def deselect(self, *, locator: Locator | None = None) -> I:
        """Resolve a single item, call ``Item.deselect()`` (Deselectable), return it.

        Raises ``PatternNotSupportedError`` when the item adapter does
        not expose ``Deselectable``.
        """
        item = self.get_item(locator=locator)
        item.deselect()
        return item

    def add_to_selection(self, *, locator: Locator | None = None) -> I:
        """Resolve a single item, call ``Item.add_to_selection()`` (MultiSelectable), return it."""
        item = self.get_item(locator=locator)
        item.add_to_selection()
        return item

    def remove_from_selection(self, *, locator: Locator | None = None) -> I:
        """Resolve a single item, call ``Item.remove_from_selection()`` (MultiSelectable), return it."""
        item = self.get_item(locator=locator)
        item.remove_from_selection()
        return item

    def clear_selection(self) -> None:
        """Deselect all currently selected items.

        Iterates over the current selection and calls
        ``remove_from_selection()`` (= ``MultiSelectable``) on each
        item. Raises ``PatternNotSupportedError`` if the container
        does not expose `Selection` or its items do not support
        multi-selection.
        """
        for item in self.get_selected_items():
            item.remove_from_selection()
