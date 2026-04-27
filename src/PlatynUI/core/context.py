# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Context base class and registry.

`ContextBase` is the root of every PlatynUI context: it holds a
`Locator`, a parent context, and the cached `Adapter` resolved through
`adapter_factory`. `ContextFactory` keeps the registry of context
classes annotated with `@context` and picks the best-matching subclass
for a freshly resolved adapter via `WeightCalculator`.

Use `@context(role=..., framework_id=...)` as a class decorator, or
the equivalent class-keyword form ``class Button(ContextBase,
role='Button'): ...``; both routes share the same registration path.
"""

import re
import warnings
import weakref
from collections.abc import Callable, Iterator
from typing import TYPE_CHECKING, Any, ClassVar, Literal, Self, overload

from ._criteria import criteria_equal
from .adapter import Adapter
from .adapter_factory import adapter_factory
from .adapter_proxy import AdapterCriteriaView
from .ensure import ensure_that
from .exceptions import DuplicateRegistrationWarning, NoLocatorDefinedError, PlatynUIFatalError
from .locator import Locator, LocatorScope
from .predicate import predicate
from .settings import Settings
from .weight_calculator import WeightCalculator

if TYPE_CHECKING:
    from types import TracebackType

__all__ = [
    'ContextBase',
    'ContextFactory',
    'UnknownContext',
    'context',
]


class ContextBase:
    """Context base.

    Subclasses receive a class-level default `Locator` either from the
    `@context` decorator or from the equivalent class-keyword form
    ``class Foo(ContextBase, role='X')``. Each instance owns a
    locator (merged from the class default), an optional parent
    context, and a cached resolved `Adapter`. Adapter access goes
    through `ensure_that` so transient resolution failures are
    retried within ``Settings.ensure_timeout``.
    """

    #: Default role used when the locator does not set one. Filled by
    #: `@context` / class-kwargs from the explicit ``role=`` or, as a
    #: fallback, from the class name.
    default_role: ClassVar[str | None] = None

    #: Default XPath prefix applied when the locator opts in via
    #: ``use_default_prefix=True``. Filled by `@context` from
    #: ``prefix=``.
    default_prefix: ClassVar[str | None] = None

    #: Class-level locator template. `__init__` clones it and merges
    #: instance overrides on top via `Locator.copy_from`.
    _locator: ClassVar[Locator | None] = None

    # Per-instance state. Type-annotated here for static checkers.
    _instance_locator: Locator | None
    _adapter: Adapter | None
    _context_children: 'weakref.WeakSet[ContextBase]'
    __context_parent: 'ContextBase | None'

    def __init__(
        self,
        locator: Locator | None = None,
        *,
        context_parent: 'ContextBase | None' = None,
        adapter: Adapter | None = None,
    ) -> None:
        self._context_children = weakref.WeakSet()

        if locator is not None:
            self._instance_locator = locator.copy_from(self._locator)
        elif self._locator is not None:
            self._instance_locator = self._locator.copy()
        else:
            self._instance_locator = None

        self.__context_parent = None
        self.context_parent = context_parent
        self._adapter = adapter

    def __init_subclass__(
        cls,
        *,
        role: str | None = None,
        framework_id: str | None = None,
        class_name: str | None = None,
        tag_name: str | None = None,
        attributes: dict[str | tuple[str, str], str | re.Pattern[str]] | None = None,
        prefix: str | None = None,
        register: bool | None = None,
        **kwargs: Any,
    ) -> None:
        """Wire the class-keyword form into the same registry as `@context`.

        Concrete subclasses register automatically with their class name as
        ``role`` (`__name__`). Pass ``register=False`` to opt out for
        abstract intermediate classes; subclasses that still carry
        unbound ``__abstractmethods__`` are skipped automatically.
        Any of ``role``, ``framework_id``, ``class_name``, ``tag_name``,
        ``attributes`` or ``prefix`` overrides the matching default.
        """
        super().__init_subclass__(**kwargs)
        if register is False:
            return
        if getattr(cls, '__abstractmethods__', frozenset[str]()):
            return
        _register_context_class(
            cls,
            role=role,
            framework_id=framework_id,
            class_name=class_name,
            tag_name=tag_name,
            attributes=attributes,
            prefix=prefix,
        )

    # ------------------------------------------------------------------
    # Repr
    # ------------------------------------------------------------------

    def __repr__(self) -> str:
        return f'{type(self).__name__}(locator={self._instance_locator!r})'

    def full_repr(self) -> str:
        """Return ``parent.full_repr() + '.' + repr(self)`` if there is a parent."""
        rendered = repr(self)
        if self.__context_parent is not None:
            return self.__context_parent.full_repr() + '.' + rendered
        return rendered

    # ------------------------------------------------------------------
    # Locator
    # ------------------------------------------------------------------

    @property
    def locator(self) -> Locator:
        """The merged instance locator. Raises if none was provided."""
        if self._instance_locator is None:
            raise NoLocatorDefinedError(f'no locator defined for {self!r}')
        return self._instance_locator

    @locator.setter
    def locator(self, value: Locator) -> None:
        self.invalidate()
        self._instance_locator = value

    # ------------------------------------------------------------------
    # Parent / children registry
    # ------------------------------------------------------------------

    @property
    def context_parent(self) -> 'ContextBase | None':
        """The owning context node, or ``None`` if detached."""
        return self.__context_parent

    @context_parent.setter
    def context_parent(self, value: 'ContextBase | None') -> None:
        previous = self.__context_parent
        if previous is not None:
            previous._context_children.discard(self)
        self.__context_parent = value
        self.invalidate()
        if value is not None:
            value._context_children.add(self)

    # ------------------------------------------------------------------
    # Adapter resolution
    # ------------------------------------------------------------------

    @property
    def adapter(self) -> Adapter:
        """The resolved `Adapter`. Forces an `ensure_that` cycle."""
        self.ensure_that(self._adapter_exists)
        result = self.get_adapter(raise_exception=True)
        if result is None:
            raise PlatynUIFatalError(f'adapter for {self!r} resolved to None')
        return result

    @adapter.setter
    def adapter(self, value: Adapter) -> None:
        self._adapter = value

    def get_adapter(
        self,
        *,
        timeout: float | None = None,
        raise_exception: bool = True,
    ) -> Adapter | None:
        """Resolve the adapter, returning ``None`` instead of raising on failure."""
        self.ensure_that(
            self._adapter_exists,
            timeout=timeout,
            raise_exception=raise_exception,
        )
        try:
            return self._try_get_adapter(raise_exception)
        except BaseException:
            if raise_exception:
                raise
            return None

    def _try_get_adapter(self, raise_exception: bool = False) -> Adapter | None:
        if self._adapter is not None and not self._adapter.valid:
            self.invalidate()
        if self._adapter is None:
            self._adapter = self._get_adapter(raise_exception)
        return self._adapter

    def _get_adapter(self, raise_exception: bool) -> Adapter | None:
        self.ensure_that(self._parent_exists)
        if self._instance_locator is None:
            return None
        parent_adapter = (
            self.__context_parent.adapter if self.__context_parent is not None else None
        )
        if parent_adapter is None:
            return None
        return adapter_factory.current.find_one(
            parent_adapter,
            self._instance_locator,
            default_role=type(self).default_role,
            default_prefix=type(self).default_prefix,
        )

    def invalidate(self) -> None:
        """Drop the cached adapter and cascade through child contexts."""
        for child in list(self._context_children):
            child.invalidate()
        self._adapter = None

    @property
    def is_valid(self) -> bool:
        """Whether the cached adapter is present and still alive."""
        return self._adapter is not None and self._adapter.valid

    def exists(
        self,
        *,
        timeout: float | None = None,
        raise_exception: bool = False,
    ) -> bool:
        """Return whether the adapter resolves within ``Settings.exists_timeout``."""
        if timeout is None:
            timeout = Settings.current().exists_timeout
        return self.ensure_that(
            self._adapter_exists,
            timeout=timeout,
            raise_exception=raise_exception,
        )

    # ------------------------------------------------------------------
    # Predicates / verification
    # ------------------------------------------------------------------

    @predicate('{0} exists')
    def _adapter_exists(self, raise_exception: bool = False) -> bool:
        self.ensure_that(self._parent_exists)
        a = self._try_get_adapter(raise_exception)
        return a is not None and a.valid

    @predicate('parent for {0} exists')
    def _parent_exists(self) -> bool:
        if self.__context_parent is None:
            return True
        return self.__context_parent._adapter_exists(True)

    def ensure_that(
        self,
        *predicates: Callable[[], bool] | None,
        timeout: float | None = None,
        raise_exception: bool | None = None,
    ) -> bool:
        """Verify ``predicates`` for this context, invalidating between retries."""
        return ensure_that(
            self,
            *predicates,
            timeout=timeout,
            raise_exception=raise_exception,
            failed_func=self.invalidate,
        )

    # ------------------------------------------------------------------
    # Property pass-through to the adapter
    # ------------------------------------------------------------------

    @property
    def name(self) -> str:
        return self.adapter.name

    @property
    def class_name(self) -> str:
        return self.adapter.class_name

    @property
    def tag_name(self) -> str:
        return self.adapter.tag_name

    @property
    def role(self) -> str:
        return self.adapter.role

    @property
    def supported_roles(self) -> set[str]:
        return self.adapter.supported_roles

    @property
    def supported_patterns(self) -> set[Any]:
        return self.adapter.supported_patterns()

    @property
    def framework_id(self) -> str:
        return self.adapter.framework_id

    @property
    def runtime_id(self) -> Any:
        """The adapter's runtime id, or ``self`` when no adapter is resolved."""
        self.ensure_that(self._adapter_exists, raise_exception=False)
        if not self.is_valid:
            return self
        return self.adapter.runtime_id

    # ------------------------------------------------------------------
    # Generic attribute reads
    # ------------------------------------------------------------------

    def attribute_names(self, namespace: str | None = None) -> set[str]:
        return self.adapter.attribute_names(namespace)

    def attribute_value(self, name: str, namespace: str = 'control') -> object:
        return self.adapter.attribute_value(name, namespace)

    def attributes(self) -> Iterator[tuple[str, str, object]]:
        return self.adapter.attributes()

    # ------------------------------------------------------------------
    # Context-manager (no-op convenience)
    # ------------------------------------------------------------------

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        exc_type: 'type[BaseException] | None',
        exc_val: BaseException | None,
        exc_tb: 'TracebackType | None',
    ) -> Literal[False]:
        return False

    # ------------------------------------------------------------------
    # Element search
    # ------------------------------------------------------------------

    def get[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
        scope: LocatorScope | None = None,
    ) -> T:
        """Resolve a single child context.

        ``locator`` overrides the per-class default of ``ctx``.
        ``scope`` overrides the locator's axis for one call (used by
        `get_child`, `ancestor`, ...).
        """
        return self._resolve_one(ctx, locator=locator, scope=scope)

    def get_one[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
        scope: LocatorScope | None = None,
    ) -> T:
        """Resolve exactly one child; raise if zero or more than one match."""
        from .exceptions import (
            AdapterNotFoundError,
            MultipleElementsFoundError,
        )

        results = self.get_all(ctx, locator=locator, scope=scope)
        if not results:
            raise AdapterNotFoundError(
                f'no element matching {ctx.__name__} found below {self!r}',
            )
        if len(results) > 1:
            raise MultipleElementsFoundError(
                f'expected exactly one {ctx.__name__} below {self!r}, '
                f'got {len(results)}',
            )
        return results[0]

    def get_all[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
        scope: LocatorScope | None = None,
    ) -> list[T]:
        """Resolve every matching child as a list."""
        return list(self.iter_all(ctx, locator=locator, scope=scope))

    def iter_all[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
        scope: LocatorScope | None = None,
    ) -> Iterator[T]:
        """Yield every matching child as a fresh context instance."""
        effective = self._effective_locator(ctx, locator, scope)
        adapters = adapter_factory.current.find_all(
            self.adapter,
            effective,
            default_role=ctx.default_role,
            default_prefix=ctx.default_prefix,
        )
        for a in adapters:
            chosen = ContextFactory.find_context_class_for(a, ctx)
            instance = chosen(effective.copy(), context_parent=self, adapter=a)
            if isinstance(instance, ctx):
                yield instance

    def get_child[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
    ) -> T:
        """Resolve a single direct child (``scope='children'``)."""
        return self.get(ctx, locator=locator, scope='children')

    def get_children[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
    ) -> list[T]:
        """Resolve every direct child (``scope='children'``)."""
        return self.get_all(ctx, locator=locator, scope='children')

    def ancestor[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
    ) -> T:
        """Resolve a single ancestor (``scope='ancestor'``)."""
        return self.get(ctx, locator=locator, scope='ancestor')

    def ancestors[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None = None,
    ) -> list[T]:
        """Resolve every ancestor (``scope='ancestor'``)."""
        return self.get_all(ctx, locator=locator, scope='ancestor')

    # ------------------------------------------------------------------
    # Iteration over children
    # ------------------------------------------------------------------

    def __iter__(self) -> Iterator['ContextBase']:
        adapter = self.adapter
        if not adapter.valid:
            return
        for child_adapter in adapter.children:
            chosen = ContextFactory.find_context_class_for(child_adapter)
            yield chosen(context_parent=self, adapter=child_adapter)

    @property
    def children(self) -> list['ContextBase']:
        """All direct child contexts as fresh wrappers."""
        return list(self)

    @property
    def parent(self) -> 'ContextBase | None':
        """The wrapped parent of this element, or ``None`` at the root."""
        adapter = self.adapter
        if not adapter.valid:
            return None
        parent_adapter = adapter.parent
        if parent_adapter is None:
            return None
        chosen = ContextFactory.find_context_class_for(parent_adapter)
        return chosen(context_parent=self.__context_parent, adapter=parent_adapter)

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    def _resolve_one[T: 'ContextBase'](
        self,
        ctx: type[T],
        *,
        locator: Locator | None,
        scope: LocatorScope | None,
    ) -> T:
        effective = self._effective_locator(ctx, locator, scope)
        return ctx(effective, context_parent=self)

    def _effective_locator(
        self,
        ctx: 'type[ContextBase]',
        locator: Locator | None,
        scope: LocatorScope | None,
    ) -> Locator:
        if locator is not None:
            base = locator.copy()
        elif ctx._locator is not None:
            base = ctx._locator.copy()
        else:
            raise NoLocatorDefinedError(
                f'{ctx.__name__} has no class-level locator; pass locator= explicitly',
            )
        if scope is not None:
            base.scope = scope
        return base


class UnknownContext(ContextBase, register=False):
    """Fallback context class for adapters with no registered match."""


class ContextFactory:
    """Registry that maps adapters to the best-matching `ContextBase` subclass."""

    class Entry:
        """Single registry record: a context class plus its match criteria."""

        __slots__ = ('context_type', 'criteria')

        def __init__(
            self,
            context_type: 'type[ContextBase]',
            criteria: dict[str, object],
        ) -> None:
            self.context_type = context_type
            self.criteria = criteria

    registered_contexts: ClassVar[list['ContextFactory.Entry']] = []

    @classmethod
    def register_context(
        cls,
        context_type: 'type[ContextBase]',
        criteria: dict[str, object],
    ) -> None:
        """Append ``(context_type, criteria)`` to the registry.

        Emits a `DuplicateRegistrationWarning` when a *different* class has
        already been registered with criteria that compare equal (after
        normalising `re.Pattern` to ``(pattern, flags)``). Re-registering
        the same class with the same criteria is silent.
        """
        new_criteria = dict(criteria)
        for entry in cls.registered_contexts:
            if not criteria_equal(entry.criteria, new_criteria):
                continue
            if entry.context_type is context_type:
                return
            warnings.warn(
                f'{context_type.__module__}.{context_type.__qualname__} '
                f'registers with the same criteria {new_criteria!r} as '
                f'{entry.context_type.__module__}.{entry.context_type.__qualname__}; '
                f'matches will be ambiguous.',
                DuplicateRegistrationWarning,
                stacklevel=3,
            )
            break
        cls.registered_contexts.append(cls.Entry(context_type, new_criteria))

    @overload
    @classmethod
    def find_context_class_for(
        cls,
        adapter: Adapter,
        context_type: 'type[ContextBase]',
    ) -> 'type[ContextBase]': ...

    @overload
    @classmethod
    def find_context_class_for(
        cls,
        adapter: Adapter,
        context_type: None = None,
    ) -> 'type[ContextBase]': ...

    @classmethod
    def find_context_class_for(
        cls,
        adapter: Adapter,
        context_type: 'type[ContextBase] | None' = None,
    ) -> 'type[ContextBase]':
        """Return ``context_type`` if given, else the highest-weighted match.

        Returns `UnknownContext` when every entry scores zero.
        """
        if context_type is not None:
            return context_type

        calculator = WeightCalculator(AdapterCriteriaView(adapter))
        best_weight = 0
        best_class: 'type[ContextBase]' = UnknownContext
        for entry in cls.registered_contexts:
            weight = calculator.calculate(entry.criteria)
            if weight > best_weight:
                best_weight = weight
                best_class = entry.context_type
        return best_class


# ----------------------------------------------------------------------
# @context decorator and shared registration helper
# ----------------------------------------------------------------------


def _register_context_class(
    cls: 'type[ContextBase]',
    *,
    role: str | None,
    framework_id: str | None,
    class_name: str | None,
    tag_name: str | None,
    attributes: dict[str | tuple[str, str], str | re.Pattern[str]] | None,
    prefix: str | None,
) -> None:
    """Set class defaults and `_locator`, then register in `ContextFactory`."""
    effective_role = role if role is not None else cls.__name__
    cls.default_role = effective_role
    if prefix is not None:
        cls.default_prefix = prefix

    cls._locator = Locator(  # pyright: ignore[reportPrivateUsage]
        role=effective_role,
        framework_id=framework_id,
        class_name=class_name,
        attributes=attributes,
        prefix=prefix,
        use_default_prefix=prefix is not None,
    )

    ContextFactory.register_context(
        cls,
        {
            'role': effective_role,
            'framework_id': framework_id,
            'class_name': class_name,
            'tag_name': tag_name,
            'attributes': attributes,
        },
    )


@overload
def context[T: ContextBase](cls: type[T], /) -> type[T]: ...


@overload
def context[T: ContextBase](
    *,
    role: str | None = None,
    framework_id: str | None = None,
    class_name: str | None = None,
    tag_name: str | None = None,
    attributes: dict[str | tuple[str, str], str | re.Pattern[str]] | None = None,
    prefix: str | None = None,
) -> Callable[[type[T]], type[T]]: ...


def context[T: ContextBase](
    cls: 'type[T] | None' = None,
    /,
    *,
    role: str | None = None,
    framework_id: str | None = None,
    class_name: str | None = None,
    tag_name: str | None = None,
    attributes: dict[str | tuple[str, str], str | re.Pattern[str]] | None = None,
    prefix: str | None = None,
) -> 'type[T] | Callable[[type[T]], type[T]]':
    """Register ``cls`` as a context class with match criteria.

    All criteria are forwarded to `WeightCalculator` and stored on the
    class as a default `Locator` (``cls._locator``); ``role`` also fills
    `default_role` (defaulting to the class name when omitted).
    Equivalent to the class-keyword form on `ContextBase`.
    """

    def decorate(target: 'type[T]') -> 'type[T]':
        _register_context_class(
            target,
            role=role,
            framework_id=framework_id,
            class_name=class_name,
            tag_name=tag_name,
            attributes=attributes,
            prefix=prefix,
        )
        return target

    if cls is not None:
        return decorate(cls)
    return decorate
