# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnusedClass=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests touch internal slots (``_context``, ``_has_full_context``,
# ``_locator``) and define context subclasses solely for their
# `@context` registration side effect; both look unused to pyright.

"""Unit tests for ``PlatynUI.core.descriptor``."""

from collections.abc import Generator
from unittest.mock import MagicMock

import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.context import (
    ContextBase,
    ContextFactory,
    UnknownContext,
    context,
)
from PlatynUI.core.descriptor import (
    ElementDescriptor,
    RootElementDescriptor,
    reset_root_element_storage,
    set_root_element_storage,
)
from PlatynUI.core.locator import Locator

# ----------------------------------------------------------------------
# Helpers (mirrors test_context.py patterns)
# ----------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _isolate_registry() -> Generator[None]:
    """Snapshot and restore `ContextFactory.registered_contexts` per test."""
    saved = list(ContextFactory.registered_contexts)
    try:
        yield
    finally:
        ContextFactory.registered_contexts[:] = saved


@pytest.fixture(autouse=True)
def _isolate_root_storage() -> Generator[None]:
    """Reset the root-element storage hook around each test."""
    try:
        yield
    finally:
        reset_root_element_storage()


def _make_adapter(
    *,
    role: str = 'Button',
    name: str = '',
    class_name: str = '',
    framework_id: str = '',
    runtime_id: str = 'rid',
    valid: bool = True,
) -> Adapter:
    a = MagicMock(spec=Adapter)
    a.role = role
    a.name = name
    a.class_name = class_name
    a.framework_id = framework_id
    a.runtime_id = runtime_id
    a.valid = valid
    a.tag_name = ''
    a.parent = None
    a.children = []
    a.supported_roles = {role}
    a.supported_patterns = MagicMock(return_value=set())
    a.attribute_value = MagicMock(return_value=None)
    a.attribute_names = MagicMock(return_value=set())
    a.attributes = MagicMock(return_value=iter(()))
    a.technology = MagicMock()
    return a


# ----------------------------------------------------------------------
# Construction and __call__ paths
# ----------------------------------------------------------------------


def test_init_with_context_caches_full_context() -> None:
    """Pre-supplied context is returned as-is and never re-resolved."""
    ctx = ContextBase(adapter=_make_adapter())
    desc: ElementDescriptor = ElementDescriptor(context=ctx)

    assert desc(full_context=False) is ctx
    assert desc(full_context=True) is ctx
    assert desc._has_full_context is True


def test_call_with_full_context_false_returns_bare_contextbase() -> None:
    """``full_context=False`` builds a plain `ContextBase` from the locator."""
    loc = Locator(role='Button')
    desc: ElementDescriptor = ElementDescriptor(loc)

    result = desc(full_context=False)

    assert type(result) is ContextBase
    assert result.locator is loc
    assert desc._has_full_context is False


def test_call_with_full_context_true_upgrades_to_subclass() -> None:
    """``full_context=True`` swaps the cached context for the registry pick."""

    @context(role='__test_button__')
    class _TestButton(ContextBase):
        pass

    adapter = _make_adapter(role='__test_button__')
    desc: ElementDescriptor = ElementDescriptor(Locator(role='__test_button__'))

    # Patch get_adapter so we don't hit adapter_factory.
    bare_holder: dict[str, ContextBase] = {}
    original_init = ContextBase.__init__

    def capture_init(self: ContextBase, *args: object, **kwargs: object) -> None:
        original_init(self, *args, **kwargs)  # type: ignore[arg-type]
        if type(self) is ContextBase and 'bare' not in bare_holder:
            bare_holder['bare'] = self
            self.get_adapter = MagicMock(return_value=adapter)  # type: ignore[method-assign]

    ContextBase.__init__ = capture_init  # type: ignore[method-assign]
    try:
        result = desc(full_context=True)
    finally:
        ContextBase.__init__ = original_init  # type: ignore[method-assign]

    assert isinstance(result, _TestButton)
    assert desc._has_full_context is True
    # Second call returns the cached upgraded context unchanged.
    assert desc(full_context=True) is result


def test_call_falls_back_to_unknown_context_when_no_match() -> None:
    """No registered class matches → `UnknownContext` instance."""
    adapter = _make_adapter(role='Whatever')
    desc: ElementDescriptor = ElementDescriptor(Locator(role='Whatever'))

    original_init = ContextBase.__init__

    def capture_init(self: ContextBase, *args: object, **kwargs: object) -> None:
        original_init(self, *args, **kwargs)  # type: ignore[arg-type]
        if type(self) is ContextBase:
            self.get_adapter = MagicMock(return_value=adapter)  # type: ignore[method-assign]

    ContextBase.__init__ = capture_init  # type: ignore[method-assign]
    try:
        result = desc(full_context=True)
    finally:
        ContextBase.__init__ = original_init  # type: ignore[method-assign]

    assert isinstance(result, UnknownContext)


def test_call_when_get_adapter_returns_none_keeps_bare_context() -> None:
    """Adapter resolution failing without raise leaves the bare context cached."""
    desc: ElementDescriptor = ElementDescriptor(Locator(role='Missing'))

    original_init = ContextBase.__init__

    def capture_init(self: ContextBase, *args: object, **kwargs: object) -> None:
        original_init(self, *args, **kwargs)  # type: ignore[arg-type]
        if type(self) is ContextBase:
            self.get_adapter = MagicMock(return_value=None)  # type: ignore[method-assign]

    ContextBase.__init__ = capture_init  # type: ignore[method-assign]
    try:
        result = desc(full_context=True)
    finally:
        ContextBase.__init__ = original_init  # type: ignore[method-assign]

    assert type(result) is ContextBase
    assert desc._has_full_context is False


def test_explicit_context_type_overrides_registry_pick() -> None:
    """Passing ``context_type=...`` skips the weight calculation."""

    @context(role='__test_button__')
    class _TestButton(ContextBase):
        pass

    class _TestForced(ContextBase):
        pass

    adapter = _make_adapter(role='__test_button__')
    desc: ElementDescriptor = ElementDescriptor(
        Locator(role='__test_button__'), context_type=_TestForced
    )

    original_init = ContextBase.__init__

    def capture_init(self: ContextBase, *args: object, **kwargs: object) -> None:
        original_init(self, *args, **kwargs)  # type: ignore[arg-type]
        if type(self) is ContextBase:
            self.get_adapter = MagicMock(return_value=adapter)  # type: ignore[method-assign]

    ContextBase.__init__ = capture_init  # type: ignore[method-assign]
    try:
        result = desc(full_context=True)
    finally:
        ContextBase.__init__ = original_init  # type: ignore[method-assign]

    assert isinstance(result, _TestForced)
    assert not isinstance(result, _TestButton)


def test_parent_descriptor_is_resolved_for_child_context() -> None:
    """Parent descriptor resolves to a context used as the child's parent."""
    parent_ctx = ContextBase(adapter=_make_adapter(role='Window'))
    parent_desc: ElementDescriptor = ElementDescriptor(context=parent_ctx)
    child_desc: ElementDescriptor = ElementDescriptor(
        Locator(role='Button'), parent=parent_desc
    )

    result = child_desc(full_context=False)

    assert result.context_parent is parent_ctx


def test_repr_shows_context_when_present() -> None:
    """``__repr__`` mentions the cached context if any, otherwise the locator."""
    ctx = ContextBase(adapter=_make_adapter())
    desc: ElementDescriptor = ElementDescriptor(context=ctx)
    assert 'ElementDescriptor for' in repr(desc)
    assert repr(ctx) in repr(desc)

    loc_desc: ElementDescriptor = ElementDescriptor(Locator(role='Button'))
    assert 'ElementDescriptor for' in repr(loc_desc)


# ----------------------------------------------------------------------
# convert
# ----------------------------------------------------------------------


def test_convert_from_contextbase_wraps_directly() -> None:
    """`convert` for an already-resolved context wraps without locator."""
    ctx = ContextBase(adapter=_make_adapter())
    desc = ElementDescriptor.convert(ctx)
    assert desc(full_context=False) is ctx


def test_convert_from_string_builds_path_locator_with_root_parent() -> None:
    """`convert` for a string uses the ambient root as parent."""
    root_ctx = ContextBase(adapter=_make_adapter(role='Desktop'))
    root_desc: ElementDescriptor = ElementDescriptor(context=root_ctx)
    ElementDescriptor.set_root_element(root_desc)

    desc = ElementDescriptor.convert('//control:Button')
    result = desc(full_context=False)

    assert result.locator is not None
    assert result.locator.path == '//control:Button'
    assert result.context_parent is root_ctx


def test_convert_from_string_without_root_uses_none_parent() -> None:
    """Without a registered root the descriptor has no parent."""
    desc = ElementDescriptor.convert('//control:Button')
    result = desc(full_context=False)

    assert result.context_parent is None


# ----------------------------------------------------------------------
# RootElementDescriptor
# ----------------------------------------------------------------------


def test_rootelement_convert_ignores_ambient_root_for_strings() -> None:
    """`RootElementDescriptor.convert(str)` never inherits a parent."""
    other_ctx = ContextBase(adapter=_make_adapter(role='Desktop'))
    ElementDescriptor.set_root_element(ElementDescriptor(context=other_ctx))

    desc = RootElementDescriptor.convert('//control:Button')
    result = desc(full_context=False)

    assert isinstance(desc, RootElementDescriptor)
    assert result.context_parent is None


def test_rootelement_convert_from_context_returns_plain_descriptor() -> None:
    """`RootElementDescriptor.convert(ctx)` returns a plain `ElementDescriptor`."""
    ctx = ContextBase(adapter=_make_adapter())
    desc = RootElementDescriptor.convert(ctx)
    assert type(desc) is ElementDescriptor
    assert desc(full_context=False) is ctx


# ----------------------------------------------------------------------
# Root-element storage hook
# ----------------------------------------------------------------------


def test_default_root_storage_roundtrip() -> None:
    """Default in-process storage behaves as a single slot."""
    assert ElementDescriptor.get_root_element() is None

    ctx = ContextBase(adapter=_make_adapter())
    desc: ElementDescriptor = ElementDescriptor(context=ctx)

    previous = ElementDescriptor.set_root_element(desc)
    assert previous is None
    assert ElementDescriptor.get_root_element() is desc

    cleared = ElementDescriptor.set_root_element(None)
    assert cleared is desc
    assert ElementDescriptor.get_root_element() is None


def test_set_root_element_storage_overrides_hook() -> None:
    """Custom getter/setter replace the default slot."""
    store: dict[str, ElementDescriptor | None] = {'value': None}

    def getter() -> ElementDescriptor | None:
        return store['value']

    def setter(element: ElementDescriptor | None) -> ElementDescriptor | None:
        previous = store['value']
        store['value'] = element
        return previous

    set_root_element_storage(getter, setter)

    desc: ElementDescriptor = ElementDescriptor(
        context=ContextBase(adapter=_make_adapter())
    )
    ElementDescriptor.set_root_element(desc)

    assert store['value'] is desc
    assert ElementDescriptor.get_root_element() is desc


def test_reset_root_element_storage_restores_default_and_clears_slot() -> None:
    """`reset_root_element_storage` reverts to the default in-process slot."""
    desc: ElementDescriptor = ElementDescriptor(
        context=ContextBase(adapter=_make_adapter())
    )
    ElementDescriptor.set_root_element(desc)
    assert ElementDescriptor.get_root_element() is desc

    reset_root_element_storage()
    assert ElementDescriptor.get_root_element() is None


# ----------------------------------------------------------------------
# Generic alias / phantom PatternT
# ----------------------------------------------------------------------


def test_generic_alias_forwards_convert_to_origin() -> None:
    """`ElementDescriptor[X].convert` resolves to `ElementDescriptor.convert`."""
    from PlatynUI.core.patterns.base import PatternBase

    class FakePattern(PatternBase):
        pass

    alias = ElementDescriptor[FakePattern]
    ctx = ContextBase(adapter=_make_adapter())

    # _GenericAlias forwards attribute access to the origin class.
    desc = alias.convert(ctx)
    assert isinstance(desc, ElementDescriptor)
    assert desc(full_context=False) is ctx
