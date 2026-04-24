# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Locator-driven adapter resolution.

Defines the `AdapterFactory` interface, the default
`RuntimeAdapterFactory` backed by the native runtime, and the
process-wide `adapter_factory` singleton used by page objects to
look up child adapters.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from collections.abc import Callable, Generator
from contextlib import contextmanager
from threading import RLock
from typing import TYPE_CHECKING, override

import platynui_native as _pn

from .exceptions import InvalidResultTypeError
from .runtime import runtime

if TYPE_CHECKING:
    from .adapter import Adapter
    from .locator import Locator

__all__ = [
    'AdapterFactory',
    'RuntimeAdapterFactory',
    'adapter_factory',
]


class AdapterFactory(ABC):
    """Resolve a `Locator` against a parent `Adapter` to matching adapters."""

    @abstractmethod
    def find_one(
        self,
        parent: 'Adapter',
        locator: 'Locator',
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> 'Adapter | None':
        """Return the first adapter matching ``locator`` below ``parent``, or ``None``.

        ``parent_is_root_like`` switches the implicit XPath scope from
        ``descendants`` to ``children``. ``default_role`` and
        ``default_prefix`` fill in the locator's role and namespace
        when not set explicitly.
        """

    @abstractmethod
    def find_all(
        self,
        parent: 'Adapter',
        locator: 'Locator',
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> list['Adapter']:
        """Return every adapter matching ``locator`` below ``parent``."""


class RuntimeAdapterFactory(AdapterFactory):
    """`AdapterFactory` that evaluates locators through the native runtime.

    Renders the locator to XPath via ``Locator.to_xpath`` and runs it
    on `runtime.current`, wrapping each returned `UiNode` in a
    `UiNodeAdapter`. Stateless and thread-safe.
    """

    @override
    def find_one(
        self,
        parent: 'Adapter',
        locator: 'Locator',
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> 'Adapter | None':
        xpath = locator.to_xpath(
            parent_is_root_like=parent_is_root_like,
            default_role=default_role,
            default_prefix=default_prefix,
        )
        node = self._native_node(parent)
        result = runtime.current.evaluate_single(xpath, node)
        return self._wrap(result, xpath)

    @override
    def find_all(
        self,
        parent: 'Adapter',
        locator: 'Locator',
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> list['Adapter']:
        xpath = locator.to_xpath(
            parent_is_root_like=parent_is_root_like,
            default_role=default_role,
            default_prefix=default_prefix,
        )
        node = self._native_node(parent)
        results = runtime.current.evaluate(xpath, node)
        return [a for a in (self._wrap(r, xpath) for r in results) if a is not None]

    @staticmethod
    def _native_node(parent: 'Adapter') -> _pn.UiNode:
        """Return the native `UiNode` carried by ``parent``.

        Raises ``TypeError`` if the adapter does not expose a
        ``native_node`` of type `_pn.UiNode`.
        """
        node = getattr(parent, 'native_node', None)
        if not isinstance(node, _pn.UiNode):
            raise TypeError(
                f'{type(parent).__name__} does not expose a native UiNode',
            )
        return node

    @staticmethod
    def _wrap(result: object, xpath: str) -> 'Adapter | None':
        """Wrap a `UiNode` result in `UiNodeAdapter`; ``None`` stays ``None``.

        Raises `InvalidResultTypeError` for non-node results
        (`EvaluatedAttribute`, `UiValue`).
        """
        if result is None:
            return None
        if isinstance(result, _pn.UiNode):
            from .adapters.ui_node import UiNodeAdapter

            return UiNodeAdapter.from_node(result)
        raise InvalidResultTypeError(
            f'XPath {xpath!r} returned a non-node result of type '
            f'{type(result).__name__}',
        )


_Builder = Callable[[], AdapterFactory]


def _default_builder() -> AdapterFactory:
    """Build a fresh `RuntimeAdapterFactory`."""
    return RuntimeAdapterFactory()


class AdapterFactoryAccessor:
    """Process-wide holder for the active `AdapterFactory`.

    The factory is built lazily on first read of `current` and then
    sealed: `use_default` and `use_factory` raise once an instance
    exists. Use `override` to swap the factory inside a scope (e.g.
    for tests); nested overrides stack LIFO and unwind on exit.
    """

    def __init__(self) -> None:
        self._builder: _Builder = _default_builder
        self._instance: AdapterFactory | None = None
        self._stack: list[tuple[_Builder, AdapterFactory | None]] = []
        self._lock = RLock()

    # ------------------------------------------------------------------
    # State inspection
    # ------------------------------------------------------------------

    def is_initialised(self) -> bool:
        """Return ``True`` once the factory has been built."""
        with self._lock:
            return self._instance is not None

    def is_sealed(self) -> bool:
        """Return ``True`` once `use_default` / `use_factory` are locked."""
        with self._lock:
            return self._instance is not None

    # ------------------------------------------------------------------
    # Consumption
    # ------------------------------------------------------------------

    @property
    def current(self) -> AdapterFactory:
        """Return the active `AdapterFactory`, building it on first access."""
        with self._lock:
            if self._instance is None:
                self._instance = self._builder()
            return self._instance

    # ------------------------------------------------------------------
    # Variant selection (only valid before sealing)
    # ------------------------------------------------------------------

    def use_default(self) -> None:
        """Set the next-built factory to `RuntimeAdapterFactory`."""
        self._set_builder(_default_builder)

    def use_factory(self, factory: _Builder) -> None:
        """Set the next-built factory to ``factory()``."""
        self._set_builder(factory)

    def _set_builder(self, builder: _Builder) -> None:
        with self._lock:
            if self._instance is not None:
                raise RuntimeError(
                    'adapter_factory already initialised; use override() instead',
                )
            self._builder = builder

    # ------------------------------------------------------------------
    # Test override (always permitted, scope-bound)
    # ------------------------------------------------------------------

    @contextmanager
    def override(self, factory: _Builder) -> Generator[AdapterFactory]:
        """Replace the active factory with ``factory()`` for the scope.

        ``factory`` is called once on enter; the produced
        `AdapterFactory` is yielded. On exit the previous builder and
        instance are restored. Overrides may be nested.

        To override with an existing instance, wrap it in a lambda::

            with adapter_factory.override(lambda: existing_factory):
                ...
        """
        with self._lock:
            previous = (self._builder, self._instance)
            self._stack.append(previous)
            instance = factory()

            def _override_builder(_inst: AdapterFactory = instance) -> AdapterFactory:
                return _inst

            self._builder = _override_builder
            self._instance = instance

        try:
            yield instance
        finally:
            with self._lock:
                snapshot = self._stack.pop() if self._stack else previous
                self._builder, self._instance = snapshot


#: Process-wide `AdapterFactory` accessor. Read ``adapter_factory.current``
#: to obtain the active factory.
adapter_factory: AdapterFactoryAccessor = AdapterFactoryAccessor()
