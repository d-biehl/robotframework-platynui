# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Declarative XPath locator for UI elements.

Three predicate sources may be freely mixed, but the same attribute
must not be set through more than one of them:

1. Reserved snake_case fields (``name``, ``id``, ``class_name``,
   ``role``, ``runtime_id``, ``framework_id``) map to their PascalCase
   XPath form: ``Locator(name="OK")`` renders ``[@Name="OK"]``.
2. The free-form ``attributes`` dict takes keys verbatim. Bare-string
   keys sit in ``default_attribute_namespace``; tuple keys
   ``(namespace, name)`` are explicit.
3. Free-form keyword arguments are interpreted as attributes:
   ``Locator(AutomationId="x")`` renders ``[@AutomationId="x"]``. Use
   the double-underscore separator for a non-default namespace:
   ``Locator(native__HWND=0xABCD)``.
"""

import collections.abc
import numbers
import re
import xml.sax.saxutils as xmlutils
from enum import Enum
from typing import Any, ClassVar, Literal, Self, cast

__all__ = ['DEFAULT_ATTRIBUTE_NAMESPACE', 'Locator', 'LocatorMethodDescriptor', 'LocatorScope', 'locator']


#: Default XPath namespace for free-form attribute keys. Attributes in
#: this namespace are emitted unprefixed; all others are emitted with
#: their prefix.
DEFAULT_ATTRIBUTE_NAMESPACE: str = 'control'


type LocatorScope = Literal[
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


#: Key type for the ``Locator.attributes`` mapping. Bare ``str`` keys
#: use the enclosing class' ``default_attribute_namespace``;
#: ``(namespace, name)`` tuple keys are explicit.
type AttributeKey = str | tuple[str, str]


def _xquery_repr(value: Any) -> str:
    """Format ``value`` as an XPath/XQuery literal."""
    if isinstance(value, re.Pattern):
        # Regex predicates are wrapped in matches() by the caller; this
        # helper only handles literal values.
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
    """Render an attribute reference, prefixing non-default namespaces."""
    if namespace == DEFAULT_ATTRIBUTE_NAMESPACE:
        return f'@{name}'
    return f'@{namespace}:{name}'


def _attribute_predicate(namespace: str, name: str, value: Any) -> str:
    """Render a single predicate body of the form ``@Name=...``."""
    rendered = _render_attribute_name(namespace, name)
    if isinstance(value, re.Pattern):
        return f'matches({rendered}, {xmlutils.quoteattr(cast(str, value.pattern))})'
    return f'{rendered}={_xquery_repr(value)}'


def _normalize_key(key: AttributeKey, default_namespace: str) -> tuple[str, str]:
    """Resolve a free-form attribute key into ``(namespace, name)``.

    Bare strings adopt ``default_namespace``; tuple keys are taken
    verbatim.
    """
    if isinstance(key, tuple):
        if len(key) != 2:
            raise ValueError(f'attribute key tuple must be (namespace, name); got {key!r}')
        namespace, name = key
        if not isinstance(namespace, str) or not isinstance(name, str):
            raise TypeError(f'attribute key tuple must be (str, str); got {key!r}')
        return namespace, name
    if isinstance(key, str):
        return default_namespace, key
    raise TypeError(f'attribute key must be str or (str, str) tuple; got {type(key).__name__}')


def _split_kwarg_name(kwarg: str) -> tuple[str, str]:
    """Split a kwarg name into ``(namespace, attribute_name)``.

    ``foo`` yields ``(DEFAULT_ATTRIBUTE_NAMESPACE, "foo")`` and
    ``ns__name`` yields ``("ns", "name")``. Reject names with more than
    one ``__`` separator: the intended namespace would be ambiguous.
    """
    parts = kwarg.split('__')
    if len(parts) == 1:
        return DEFAULT_ATTRIBUTE_NAMESPACE, kwarg
    if len(parts) == 2:
        namespace, name = parts
        if not namespace or not name:
            raise ValueError(f'kwarg attribute name {kwarg!r} has empty namespace or name')
        return namespace, name
    raise ValueError(
        f'kwarg attribute name {kwarg!r} contains multiple "__" separators; '
        f'use the attributes={{(ns, name): value}} dict for complex keys'
    )


# Names of typed Locator parameters that are not free-form attributes.
# Kept in sync with ``Locator.__slots__``; consulted by ``__init__`` to
# distinguish reserved kwargs from PascalCase attribute kwargs.
_RESERVED_FIELDS: frozenset[str] = frozenset(
    {
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
    }
)


# Mapping from snake_case convenience fields to their PascalCase XPath
# attribute name. Closed set, all rendered in the default ``control``
# namespace.
_SHORTHAND_TO_ATTR: dict[str, str] = {
    'id': 'Id',
    'name': 'Name',
    'class_name': 'ClassName',
    'runtime_id': 'RuntimeId',
    'framework_id': 'FrameworkId',
}


class Locator:
    """Build an XPath 2.0 expression for a UI element.

    When ``path`` is set, the XPath is taken verbatim (modulo an
    optional ``prefix``/``axis`` prefix). Otherwise the expression is
    composed from a node name (``node`` or ``role``), an axis
    (``axis`` or ``scope``), attribute predicates, and the optional
    qualifiers ``index`` and ``position``.

    Setting the same logical attribute through more than one of the
    three predicate sources (see module docstring) raises
    `TypeError`.
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
        self.custom_attributes = list(custom_attributes) if custom_attributes is not None else []

        # Track which (namespace, name) keys originate from which source
        # so we can produce a clear conflict error.
        sources: dict[tuple[str, str], str] = {}

        # 1. Reserved snake_case convenience fields → PascalCase in 'control'.
        for field_name, attr_name in _SHORTHAND_TO_ATTR.items():
            if getattr(self, field_name) is not None:
                sources[(DEFAULT_ATTRIBUTE_NAMESPACE, attr_name)] = f'reserved field {field_name}='

        # 2. attributes-Dict (already on self.attributes; verified for
        # well-formedness via _normalize_key, with the *constructor's*
        # default namespace 'control' — context overrides happen
        # later in to_xpath()).
        for raw_key in self.attributes:
            ns_name = _normalize_key(raw_key, DEFAULT_ATTRIBUTE_NAMESPACE)
            existing = sources.get(ns_name)
            if existing is not None:
                raise TypeError(_conflict_message(ns_name, existing, f'attributes[{raw_key!r}]'))
            sources[ns_name] = f'attributes[{raw_key!r}]'

        # 3. **extra_attributes — interpret kwarg name (with __ separator)
        # and merge into self.attributes as tuple keys.
        for kwarg, value in extra_attributes.items():
            namespace, attr_name = _split_kwarg_name(kwarg)
            ns_name = (namespace, attr_name)
            existing = sources.get(ns_name)
            if existing is not None:
                raise TypeError(_conflict_message(ns_name, existing, f'kwarg {kwarg}='))
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

        Set ``parent_is_root_like`` when the resolving parent is an
        ``Application`` or ``Desktop`` node; this picks ``children`` as
        the implicit scope instead of ``descendants``. ``default_role``
        and ``default_prefix`` provide fallbacks from the context
        class; ``default_prefix`` only applies when
        ``use_default_prefix`` is true. ``default_attribute_namespace``
        names the namespace used for bare-string keys in
        `attributes`.
        """
        if self.path is not None:
            return self.path

        # Build attribute predicate body.
        predicate_parts: list[str] = []

        # Standard shorthand attributes — always sit in the default
        # ``control`` namespace.
        for attr_name, value in self._standard_attributes():
            if value is not None:
                predicate_parts.append(_attribute_predicate(DEFAULT_ATTRIBUTE_NAMESPACE, attr_name, value))

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

    def copy_from(self, other: 'Locator | None') -> Self:
        """Inherit unset fields and merge attribute dicts from ``other``.

        Implements property/class-level locator inheritance: the child
        locator wins on conflict, the parent fills the gaps.
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

    def copy(self) -> 'Locator':
        """Return a copy with attribute and custom-attribute lists duplicated."""
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
        return all(getattr(self, slot) == getattr(other, slot) for slot in self.__slots__)

    # Locators are mutable (copy_from rewrites fields in place); make
    # them explicitly unhashable, mirroring @dataclass(eq=True).
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


def _conflict_message(ns_name: tuple[str, str], existing_source: str, new_source: str) -> str:
    namespace, name = ns_name
    if namespace == DEFAULT_ATTRIBUTE_NAMESPACE:
        rendered = f'@{name}'
    else:
        rendered = f'@{namespace}:{name}'
    return f'conflicting attribute {rendered}: set via both {existing_source} and {new_source}; use exactly one source'


class LocatorMethodDescriptor:
    """Wrap a callable with an attached locator.

    Returned by `locator` when applied to a method or property.
    Attribute access on an instance raises `NotImplementedError`:
    method-form resolution (using the return-type annotation to locate
    the target element) is not yet implemented. Use `locator`
    only as a class decorator until then.
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
            f'@locator method form for {owner.__name__ if owner else "?"}.{attr} '
            'is not yet implemented. Use @locator only as a class decorator for now.'
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
    """Attach a `Locator` to a class, method, or property.

    As a class decorator, store the locator on ``__locator__`` and
    return the class unchanged::

        @locator(name="Calculator")
        class CalculatorWindow(Window):
            ...

    As a method or property decorator, return a
    `LocatorMethodDescriptor` carrying the locator. Method-form
    resolution is not yet implemented; accessing the attribute raises
    `NotImplementedError`. Context code can be written today
    and will work once method-form lands without source changes::

        class CalculatorWindow(Window):
            @property
            @locator(AutomationId="num5Button")
            def n5(self) -> Button: ...

    Keyword arguments mirror `__init__`.
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
        raise TypeError(f'@locator can only decorate classes or callables, got {type(target).__name__}')

    return _apply
