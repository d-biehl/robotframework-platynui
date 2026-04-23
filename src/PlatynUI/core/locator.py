# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Declarative XPath locator (design document section A.6).

A :class:`Locator` describes how to find an element relative to a parent
node. It does not perform the lookup itself — that is done by the runtime
via the Rust XPath engine. ``Locator.to_xpath`` produces a single XPath
2.0 expression suitable for ``runtime.evaluate``.

The ``LocatorScope`` axis vocabulary is exposed as a ``Literal`` type
alias rather than an ``Enum`` so that page-object authors can write
``scope="descendants"`` directly.

There are **three ways** to specify an attribute predicate, and they may
be freely mixed (but the same attribute may only be set via one of them
— see :meth:`Locator.__init__` for the conflict rules):

1. **Reserved snake_case convenience fields** — ``name``, ``id``,
   ``class_name``, ``role``, ``runtime_id``, ``framework_id`` — are
   typed dataclass-style parameters mapped to their PascalCase XPath
   form (``Locator(name="OK")`` → ``[@Name="OK"]``). This is a closed
   set; new attributes are *not* added here.

2. **Free-form** ``attributes`` **dict** — keys are taken verbatim as
   attribute names (no case conversion). Bare strings sit in the
   ``default_attribute_namespace`` (``"control"`` unless overridden);
   tuple keys ``(namespace, name)`` are explicit.

3. **Free-form keyword arguments** — any kwarg that is *not* a reserved
   field name is interpreted as a free-form attribute. ``Locator(
   AutomationId="x")`` → ``[@AutomationId="x"]``. To address a
   non-default namespace via kwarg, use the double-underscore separator
   ``ns__name``: ``Locator(native__HWND=0xABCD)`` →
   ``[@native:HWND="..."]``.

   The kwarg name is taken verbatim — no PascalCase enforcement, no
   case conversion. ``Locator(foo="x")`` → ``[@foo="x"]``. Authors are
   responsible for following the project convention that adapter
   attribute names are PascalCase (see ``crates/core/src/ui/attributes.rs``).
"""

from __future__ import annotations

import collections.abc
import numbers
import re
import xml.sax.saxutils as xmlutils
from enum import Enum
from typing import Any, ClassVar, Literal, TypeAlias, cast

from typing_extensions import Self

__all__ = ['DEFAULT_ATTRIBUTE_NAMESPACE', 'Locator', 'LocatorMethodDescriptor', 'LocatorScope', 'locator']


#: XPath default namespace for free-form attribute keys.
#: Mirrors :rust:`Namespace::Control` (``crates/core/src/ui/namespace.rs``).
#: Attributes in this namespace are emitted *unprefixed* in the generated
#: XPath; all other namespaces are emitted with their prefix.
DEFAULT_ATTRIBUTE_NAMESPACE: str = 'control'


LocatorScope: TypeAlias = Literal[
    'root',
    'descendants',
    'children',
    'parent',
    'ancestor',
    'ancestor-or-self',
    'descendants-or-self',
    'following',
    'following-sibling',
    'preceding',
    'preceding-sibling',
]


_XPATH_AXIS: dict[str, str] = {
    'root': '/',
    'descendants': './/',
    'children': '',
    'parent': 'parent::',
    'ancestor': 'ancestor::',
    'ancestor-or-self': 'ancestor-or-self::',
    'descendants-or-self': 'descendant-or-self::',
    'following': 'following::',
    'following-sibling': 'following-sibling::',
    'preceding': 'preceding::',
    'preceding-sibling': 'preceding-sibling::',
}


#: Type of the public ``Locator.attributes`` mapping. ``str`` keys use the
#: enclosing class' ``default_attribute_namespace``; ``(namespace, name)``
#: tuple keys are explicit.
AttributeKey: TypeAlias = 'str | tuple[str, str]'


def _xquery_repr(value: Any) -> str:
    """Format ``value`` as an XPath/XQuery literal."""
    if isinstance(value, re.Pattern):
        # The caller is expected to wrap regex predicates in matches() —
        # this helper only handles literal values.
        return xmlutils.quoteattr(cast(str, value.pattern))
    if isinstance(value, Enum):
        return '"' + str(value.value) + '"'
    if isinstance(value, bool):
        return repr(value).lower() + '()'
    if isinstance(value, str):
        return xmlutils.quoteattr(value)
    if isinstance(value, numbers.Number):
        return repr(value)
    if isinstance(value, dict):
        return '(' + ', '.join(_xquery_repr(item) for item in value.items()) + ')'
    if isinstance(value, collections.abc.Iterable):
        return '(' + ', '.join(_xquery_repr(item) for item in value) + ')'
    return xmlutils.quoteattr(repr(value))


def _render_attribute_name(namespace: str, name: str) -> str:
    """Render an attribute reference with namespace prefix when needed."""
    if namespace == DEFAULT_ATTRIBUTE_NAMESPACE:
        return f'@{name}'
    return f'@{namespace}:{name}'


def _attribute_predicate(namespace: str, name: str, value: Any) -> str:
    """Render a single ``[@Name=...]`` (or ``[@ns:Name=...]``) predicate body."""
    rendered = _render_attribute_name(namespace, name)
    if isinstance(value, re.Pattern):
        return f'matches({rendered}, {xmlutils.quoteattr(cast(str, value.pattern))})'
    return f'{rendered}={_xquery_repr(value)}'


def _normalize_key(key: AttributeKey, default_namespace: str) -> tuple[str, str]:
    """Resolve a free-form attribute key into ``(namespace, name)``.

    Bare strings are placed in ``default_namespace``; tuple keys are
    taken verbatim.
    """
    if isinstance(key, tuple):
        if len(key) != 2:
            raise ValueError(
                f'attribute key tuple must be (namespace, name); got {key!r}'
            )
        namespace, name = key
        if not isinstance(namespace, str) or not isinstance(name, str):
            raise TypeError(
                f'attribute key tuple must be (str, str); got {key!r}'
            )
        return namespace, name
    if isinstance(key, str):
        return default_namespace, key
    raise TypeError(
        f'attribute key must be str or (str, str) tuple; got {type(key).__name__}'
    )


def _split_kwarg_name(kwarg: str) -> tuple[str, str]:
    """Split a kwarg name into ``(namespace, attribute_name)``.

    ``foo`` → ``(DEFAULT_ATTRIBUTE_NAMESPACE, "foo")``;
    ``ns__name`` → ``("ns", "name")``. Multiple ``__`` separators are
    rejected because the intended namespace would be ambiguous.
    """
    parts = kwarg.split('__')
    if len(parts) == 1:
        return DEFAULT_ATTRIBUTE_NAMESPACE, kwarg
    if len(parts) == 2:
        namespace, name = parts
        if not namespace or not name:
            raise ValueError(
                f'kwarg attribute name {kwarg!r} has empty namespace or name'
            )
        return namespace, name
    raise ValueError(
        f'kwarg attribute name {kwarg!r} contains multiple "__" separators; '
        f'use the attributes={{(ns, name): value}} dict for complex keys'
    )


# Names of typed Locator parameters that are *not* free-form attributes.
# Used to separate reserved kwargs from PascalCase attribute kwargs in
# ``Locator.__init__``. Must stay in sync with ``Locator.__slots__``.
_RESERVED_FIELDS: frozenset[str] = frozenset({
    'path',
    'node',
    'prefix',
    'use_default_prefix',
    'axis',
    'scope',
    'index',
    'position',
    'name',
    'id',
    'class_name',
    'role',
    'runtime_id',
    'framework_id',
    'attributes',
    'custom_attributes',
})


# Mapping from snake_case convenience fields to their PascalCase XPath
# attribute name. The closed set of standard shorthand attributes, all
# rendered in the default ``control`` namespace.
_SHORTHAND_TO_ATTR: dict[str, str] = {
    'id': 'Id',
    'name': 'Name',
    'class_name': 'ClassName',
    'runtime_id': 'RuntimeId',
    'framework_id': 'FrameworkId',
}


class Locator:
    """Declarative XPath locator builder.

    Either ``path`` is set (then the XPath is taken verbatim, modulo the
    optional ``prefix``/``axis`` prefix), or the locator is composed from
    a node name (``node``/``role``), an axis (``axis``/``scope``), a set
    of attribute predicates, and optional positional qualifiers
    (``index``, ``position``).

    Attribute predicates can come from three sources (see module docstring
    for the full convention). Setting the same logical attribute via more
    than one source raises ``TypeError`` — there is no precedence rule,
    conflicts are surfaced loudly.
    """

    __slots__ = (
        'attributes',
        'axis',
        'class_name',
        'custom_attributes',
        'framework_id',
        'id',
        'index',
        'name',
        'node',
        'path',
        'position',
        'prefix',
        'role',
        'runtime_id',
        'scope',
        'use_default_prefix',
    )

    # Type annotations for static checkers / IDEs.
    path: str | None
    node: str | None
    prefix: str | None
    use_default_prefix: bool
    axis: str | None
    scope: LocatorScope | None
    index: int | None
    position: int | None
    name: str | None
    id: str | None
    class_name: str | None
    role: str | None
    runtime_id: str | None
    framework_id: str | None
    attributes: dict[AttributeKey, str | re.Pattern[str]]
    custom_attributes: list[str]

    # Exposed for introspection / tests.
    RESERVED_FIELDS: ClassVar[frozenset[str]] = _RESERVED_FIELDS

    def __init__(
        self,
        *,
        path: str | None = None,
        node: str | None = None,
        prefix: str | None = None,
        use_default_prefix: bool = False,
        axis: str | None = None,
        scope: LocatorScope | None = None,
        index: int | None = None,
        position: int | None = None,
        name: str | None = None,
        id: str | None = None,
        class_name: str | None = None,
        role: str | None = None,
        runtime_id: str | None = None,
        framework_id: str | None = None,
        attributes: dict[AttributeKey, str | re.Pattern[str]] | None = None,
        custom_attributes: list[str] | None = None,
        **extra_attributes: str | re.Pattern[str],
    ) -> None:
        self.path = path
        self.node = node
        self.prefix = prefix
        self.use_default_prefix = use_default_prefix
        self.axis = axis
        self.scope = scope
        self.index = index
        self.position = position
        self.name = name
        self.id = id
        self.class_name = class_name
        self.role = role
        self.runtime_id = runtime_id
        self.framework_id = framework_id
        self.attributes = dict(attributes) if attributes is not None else {}
        self.custom_attributes = (
            list(custom_attributes) if custom_attributes is not None else []
        )

        # Track which (namespace, name) keys originate from which source
        # so we can produce a clear conflict error.
        sources: dict[tuple[str, str], str] = {}

        # 1. Reserved snake_case convenience fields → PascalCase in 'control'.
        for field_name, attr_name in _SHORTHAND_TO_ATTR.items():
            if getattr(self, field_name) is not None:
                sources[(DEFAULT_ATTRIBUTE_NAMESPACE, attr_name)] = (
                    f'reserved field {field_name}='
                )

        # 2. attributes-Dict (already on self.attributes; verified for
        # well-formedness via _normalize_key, with the *constructor's*
        # default namespace 'control' — page-object overrides happen
        # later in to_xpath()).
        for raw_key in self.attributes:
            ns_name = _normalize_key(raw_key, DEFAULT_ATTRIBUTE_NAMESPACE)
            existing = sources.get(ns_name)
            if existing is not None:
                raise TypeError(
                    _conflict_message(
                        ns_name, existing, f'attributes[{raw_key!r}]'
                    )
                )
            sources[ns_name] = f'attributes[{raw_key!r}]'

        # 3. **extra_attributes — interpret kwarg name (with __ separator)
        # and merge into self.attributes as tuple keys.
        for kwarg, value in extra_attributes.items():
            namespace, attr_name = _split_kwarg_name(kwarg)
            ns_name = (namespace, attr_name)
            existing = sources.get(ns_name)
            if existing is not None:
                raise TypeError(
                    _conflict_message(ns_name, existing, f'kwarg {kwarg}=')
                )
            sources[ns_name] = f'kwarg {kwarg}='
            # Store as tuple key so the rendering pass uses the explicit
            # namespace, ignoring whatever default is in effect later.
            self.attributes[(namespace, attr_name)] = value

    def to_xpath(
        self,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
        default_attribute_namespace: str = DEFAULT_ATTRIBUTE_NAMESPACE,
    ) -> str:
        """Render this locator as an XPath 2.0 expression.

        Args:
            parent_is_root_like: ``True`` when the resolving parent is an
                ``Application`` or ``Desktop`` node. Influences the
                default scope when none is explicitly set.
            default_role: Fallback node name from the page-object class.
            default_prefix: Fallback namespace prefix from the page-object
                class; only used when ``use_default_prefix`` is ``True``.
            default_attribute_namespace: Namespace used for bare-string
                keys in :attr:`attributes`. Page-object classes pass
                their own ``default_attribute_namespace`` class attribute
                here (default ``"control"``).
        """
        if self.path is not None:
            return self.path

        # Build attribute predicate body.
        predicate_parts: list[str] = []

        # Standard shorthand attributes — always sit in the default
        # ``control`` namespace.
        for attr_name, value in self._standard_attributes():
            if value is not None:
                predicate_parts.append(
                    _attribute_predicate(DEFAULT_ATTRIBUTE_NAMESPACE, attr_name, value)
                )

        for raw_key, value in self.attributes.items():
            namespace, name = _normalize_key(raw_key, default_attribute_namespace)
            predicate_parts.append(_attribute_predicate(namespace, name, value))

        for raw in self.custom_attributes:
            if raw:
                predicate_parts.append(raw)

        if self.position is not None:
            predicate_parts.append(f'position()={self.position}')

        node_name = self.node or self.role or default_role or '*'

        if self.prefix is not None:
            node_name = f'{self.prefix}:{node_name}'
        elif self.use_default_prefix and default_prefix:
            node_name = f'{default_prefix}:{node_name}'

        # Axis / scope.
        if self.axis is not None:
            axis_prefix = self.axis
        else:
            scope = self.scope
            if scope is None:
                scope = 'children' if parent_is_root_like else 'descendants'
            axis_prefix = _XPATH_AXIS[scope]

        result = axis_prefix + node_name

        if predicate_parts:
            result += '[' + ' and '.join(predicate_parts) + ']'

        if self.index is not None:
            result += f'[{self.index}]'

        return result

    def copy_from(self, other: "Locator | None") -> Self:
        """Inherit unset fields and merge attribute dicts from ``other``.

        Used for property/class-level locator inheritance: child locator
        wins on conflict, parent fills the gaps.
        """
        if other is None:
            return self

        for attr_name in (
            'path',
            'node',
            'prefix',
            'axis',
            'scope',
            'index',
            'position',
            'name',
            'id',
            'class_name',
            'role',
            'runtime_id',
            'framework_id',
        ):
            if getattr(self, attr_name) is None:
                setattr(self, attr_name, getattr(other, attr_name))

        if not self.use_default_prefix:
            self.use_default_prefix = other.use_default_prefix

        for key, value in other.attributes.items():
            self.attributes.setdefault(key, value)

        for raw in other.custom_attributes:
            if raw not in self.custom_attributes:
                self.custom_attributes.append(raw)

        return self

    def copy(self) -> "Locator":
        """Return a shallow-ish copy (attribute and custom-attribute lists copied)."""
        return Locator(
            path=self.path,
            node=self.node,
            prefix=self.prefix,
            use_default_prefix=self.use_default_prefix,
            axis=self.axis,
            scope=self.scope,
            index=self.index,
            position=self.position,
            name=self.name,
            id=self.id,
            class_name=self.class_name,
            role=self.role,
            runtime_id=self.runtime_id,
            framework_id=self.framework_id,
            attributes=dict(self.attributes),
            custom_attributes=list(self.custom_attributes),
        )

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Locator):
            return NotImplemented
        return all(
            getattr(self, slot) == getattr(other, slot) for slot in self.__slots__
        )

    # Locators are mutable (copy_from rewrites fields in place) — explicitly
    # unhashable, mirroring the previous @dataclass(eq=True) default.
    __hash__ = None  # type: ignore[assignment]

    def __repr__(self) -> str:
        parts = [
            f'{slot}={getattr(self, slot)!r}'
            for slot in self.__slots__
            if getattr(self, slot) not in (None, False, [], {})
        ]
        return f'Locator({", ".join(parts)})'

    def _standard_attributes(self) -> list[tuple[str, Any]]:
        # Order matters for stable XPath output.
        return [
            ('Id', self.id),
            ('Name', self.name),
            ('ClassName', self.class_name),
            ('RuntimeId', self.runtime_id),
            ('FrameworkId', self.framework_id),
        ]


def _conflict_message(
    ns_name: tuple[str, str], existing_source: str, new_source: str
) -> str:
    namespace, name = ns_name
    if namespace == DEFAULT_ATTRIBUTE_NAMESPACE:
        rendered = f'@{name}'
    else:
        rendered = f'@{namespace}:{name}'
    return (
        f'conflicting attribute {rendered}: set via both '
        f'{existing_source} and {new_source}; use exactly one source'
    )


class LocatorMethodDescriptor:
    """Descriptor produced by ``@locator(...)`` on a method/property.

    Phase 2 stub. The full method-form decorator requires
    :class:`ContextBase.get` (Phase 3) to resolve the locator against the
    owning context using the method's return-type annotation. Until then,
    accessing such an attribute raises :class:`NotImplementedError`.

    The descriptor still stores the locator and the wrapped function so
    that page-object code can be authored today and will work once
    Phase 3 lands without source changes.
    """

    __slots__ = ('__locator__', '__wrapped__', 'attr_name')

    __locator__: Locator
    __wrapped__: collections.abc.Callable[..., Any]
    attr_name: str | None

    def __init__(
        self,
        loc: Locator,
        func: collections.abc.Callable[..., Any],
    ) -> None:
        self.__locator__ = loc
        self.__wrapped__ = func
        self.attr_name = getattr(func, '__name__', None)

    def __set_name__(self, owner: type, name: str) -> None:
        self.attr_name = name

    def __get__(self, instance: object, owner: type | None = None) -> Any:
        if instance is None:
            return self
        attr = self.attr_name or '<unknown>'
        raise NotImplementedError(
            f"@locator method form for {owner.__name__ if owner else '?'}.{attr} "
            'requires ContextBase.get() — implemented in Phase 3 of the '
            'Python migration. Use @locator only as a class decorator for now.'
        )


def locator(
    *,
    path: str | None = None,
    node: str | None = None,
    prefix: str | None = None,
    use_default_prefix: bool = False,
    axis: str | None = None,
    scope: LocatorScope | None = None,
    index: int | None = None,
    position: int | None = None,
    name: str | None = None,
    id: str | None = None,
    class_name: str | None = None,
    role: str | None = None,
    runtime_id: str | None = None,
    framework_id: str | None = None,
    attributes: dict[AttributeKey, str | re.Pattern[str]] | None = None,
    custom_attributes: list[str] | None = None,
    **extra_attributes: str | re.Pattern[str],
) -> collections.abc.Callable[[Any], Any]:
    """Decorator form of :class:`Locator`.

    Two usage forms are supported:

    1. **Class decorator** — attaches the locator as the ``__locator__``
       class attribute and returns the class unchanged::

           @locator(name="Calculator")
           class CalculatorWindow(Window):
               ...

    2. **Method/property decorator (Phase-3 stub)** — wraps the method in
       a descriptor that stores the locator. Accessing the attribute on
       an instance currently raises :class:`NotImplementedError`; the
       full resolution path (read return-type annotation, call
       ``ContextBase.get``) lands with Phase 3::

           class CalculatorWindow(Window):
               @property
               @locator(AutomationId="num5Button")
               def n5(self) -> Button: ...

    Keyword arguments mirror :meth:`Locator.__init__` exactly. To call
    :class:`Locator` directly without the decorator wrapping, use the
    class — :func:`locator` is *not* a transparent alias.
    """
    loc = Locator(
        path=path,
        node=node,
        prefix=prefix,
        use_default_prefix=use_default_prefix,
        axis=axis,
        scope=scope,
        index=index,
        position=position,
        name=name,
        id=id,
        class_name=class_name,
        role=role,
        runtime_id=runtime_id,
        framework_id=framework_id,
        attributes=attributes,
        custom_attributes=custom_attributes,
        **extra_attributes,
    )

    def _apply(target: Any) -> Any:
        if isinstance(target, type):
            # Class decorator form: attach and return unchanged. Use setattr
            # to avoid type-checker friction over an attribute the class
            # author has not declared statically.
            setattr(target, '__locator__', loc)
            return target
        if callable(target):
            # Method/property decorator form: wrap in stub descriptor.
            return LocatorMethodDescriptor(loc, target)
        raise TypeError(
            f'@locator can only decorate classes or callables, '
            f'got {type(target).__name__}'
        )

    return _apply
