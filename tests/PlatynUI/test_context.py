# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnusedClass=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal state (``_locator``, ``_adapter``,
# ``_context_children``) and define page-object subclasses purely for
# their registration side effect; both look unused to pyright.

"""Unit tests for ``PlatynUI.core.context``.

Covers the registration paths (`@context` decorator and class-keyword
form), `ContextFactory` weighting, locator inheritance, adapter
caching, parent/children traversal, and search delegation through
`adapter_factory`.
"""

import warnings
from collections.abc import Generator
from unittest.mock import MagicMock

import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapter_factory import AdapterFactory, adapter_factory
from PlatynUI.core.context import (
    ContextBase,
    ContextFactory,
    UnknownContext,
    context,
)
from PlatynUI.core.exceptions import (
    AdapterNotFoundError,
    DuplicateRegistrationWarning,
    MultipleElementsFoundError,
    NoLocatorDefinedError,
)
from PlatynUI.core.locator import Locator

# ----------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _isolate_registry() -> Generator[None]:
    """Snapshot and restore `ContextFactory.registered_contexts` per test."""
    saved = list(ContextFactory.registered_contexts)
    try:
        yield
    finally:
        ContextFactory.registered_contexts[:] = saved


def _make_adapter(
    *,
    role: str = 'Button',
    name: str = '',
    class_name: str = '',
    framework_id: str = '',
    runtime_id: str = 'rid',
    valid: bool = True,
    parent: Adapter | None = None,
    children: list[Adapter] | None = None,
    supported_roles: set[str] | None = None,
) -> Adapter:
    """Build a `MagicMock(spec=Adapter)` populated with the given fields."""
    a = MagicMock(spec=Adapter)
    a.role = role
    a.name = name
    a.class_name = class_name
    a.framework_id = framework_id
    a.runtime_id = runtime_id
    a.valid = valid
    a.tag_name = ''
    a.parent = parent
    a.children = children or []
    a.supported_roles = supported_roles or {role}
    a.supported_patterns = MagicMock(return_value=set())
    a.attribute_value = MagicMock(return_value=None)
    a.attribute_names = MagicMock(return_value=set())
    a.attributes = MagicMock(return_value=iter(()))
    a.technology = MagicMock()
    return a


class _StubFactory(AdapterFactory):
    """`AdapterFactory` returning preconfigured adapter lists."""

    def __init__(self, *, results: list[Adapter] | None = None) -> None:
        self.results = results or []
        self.find_one_calls: list[tuple[Adapter, Locator]] = []
        self.find_all_calls: list[tuple[Adapter, Locator]] = []

    def find_one(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> Adapter | None:
        del parent_is_root_like, default_role, default_prefix
        self.find_one_calls.append((parent, locator))
        return self.results[0] if self.results else None

    def find_all(
        self,
        parent: Adapter,
        locator: Locator,
        *,
        parent_is_root_like: bool = False,
        default_role: str | None = None,
        default_prefix: str | None = None,
    ) -> list[Adapter]:
        del parent_is_root_like, default_role, default_prefix
        self.find_all_calls.append((parent, locator))
        return list(self.results)


# ----------------------------------------------------------------------
# Construction and locator handling
# ----------------------------------------------------------------------


def test_init_without_locator_or_class_default_has_no_locator() -> None:
    ctx = ContextBase()
    assert ctx._instance_locator is None
    with pytest.raises(NoLocatorDefinedError):
        _ = ctx.locator


def test_init_with_explicit_locator_keeps_it() -> None:
    loc = Locator(role='Button')
    ctx = ContextBase(loc)
    assert ctx.locator.role == 'Button'


def test_locator_setter_invalidates() -> None:
    a = _make_adapter()
    ctx = ContextBase(adapter=a)
    assert ctx._adapter is a
    ctx.locator = Locator(role='X')
    assert ctx._adapter is None


def test_repr_includes_locator() -> None:
    ctx = ContextBase(Locator(role='Button'))
    assert 'Button' in repr(ctx)


def test_full_repr_chains_parent() -> None:
    parent = ContextBase(Locator(role='Window'))
    child = ContextBase(Locator(role='Button'), context_parent=parent)
    assert child.full_repr().startswith(repr(parent))
    assert child.full_repr().endswith(repr(child))


# ----------------------------------------------------------------------
# Context-parent / children registry
# ----------------------------------------------------------------------


def test_context_parent_registers_in_weak_set() -> None:
    parent = ContextBase()
    child = ContextBase(context_parent=parent)
    assert child in parent._context_children


def test_context_parent_reassignment_moves_membership() -> None:
    p1 = ContextBase()
    p2 = ContextBase()
    child = ContextBase(context_parent=p1)
    child.context_parent = p2
    assert child not in p1._context_children
    assert child in p2._context_children


def test_invalidate_cascades_to_children() -> None:
    parent_adapter = _make_adapter()
    child_adapter = _make_adapter(role='Button')
    parent = ContextBase(adapter=parent_adapter)
    child = ContextBase(adapter=child_adapter, context_parent=parent)
    parent.invalidate()
    assert parent._adapter is None
    assert child._adapter is None


# ----------------------------------------------------------------------
# Adapter access
# ----------------------------------------------------------------------


def test_adapter_property_returns_preset_adapter() -> None:
    a = _make_adapter()
    ctx = ContextBase(adapter=a)
    assert ctx.adapter is a


def test_is_valid_reflects_adapter_state() -> None:
    a = _make_adapter()
    ctx = ContextBase(adapter=a)
    assert ctx.is_valid is True
    a.valid = False  # type: ignore[misc]
    assert ctx.is_valid is False


def test_property_passthrough_uses_adapter() -> None:
    a = _make_adapter(role='Button', name='OK', class_name='QPushButton')
    ctx = ContextBase(adapter=a)
    assert ctx.role == 'Button'
    assert ctx.name == 'OK'
    assert ctx.class_name == 'QPushButton'
    assert ctx.runtime_id == 'rid'


# ----------------------------------------------------------------------
# Context-manager
# ----------------------------------------------------------------------


def test_context_manager_returns_self_and_swallows_nothing() -> None:
    ctx = ContextBase(adapter=_make_adapter())
    with ctx as entered:
        assert entered is ctx
    # __exit__ returns False → exceptions would propagate
    with pytest.raises(RuntimeError), ctx:
        raise RuntimeError('boom')


# ----------------------------------------------------------------------
# Decorator and class-kwarg registration
# ----------------------------------------------------------------------


def test_decorator_sets_default_role_and_locator() -> None:
    @context(role='__test_button__')
    class MyButton(ContextBase):
        pass

    assert MyButton.default_role == '__test_button__'
    assert MyButton._locator is not None
    assert MyButton._locator.role == '__test_button__'


def test_decorator_defaults_role_to_class_name() -> None:
    @context()
    class TestDecoratorWindow(ContextBase):
        pass

    assert TestDecoratorWindow.default_role == 'TestDecoratorWindow'
    assert TestDecoratorWindow._locator is not None
    assert TestDecoratorWindow._locator.role == 'TestDecoratorWindow'


def test_decorator_sets_prefix_and_use_default_prefix() -> None:
    @context(role='Foo', prefix='item')
    class Item(ContextBase):
        pass

    assert Item.default_prefix == 'item'
    assert Item._locator is not None
    assert Item._locator.prefix == 'item'
    assert Item._locator.use_default_prefix is True


def test_class_kwargs_form_registers_too() -> None:
    class MyControl(ContextBase, role='Slider'):
        pass

    assert MyControl.default_role == 'Slider'
    assert MyControl._locator is not None
    assert MyControl._locator.role == 'Slider'
    assert any(
        e.context_type is MyControl
        for e in ContextFactory.registered_contexts
    )


def test_subclass_without_kwargs_is_not_registered() -> None:
    before = len(ContextFactory.registered_contexts)

    class Intermediate(ContextBase, register=False):
        pass

    assert len(ContextFactory.registered_contexts) == before
    assert Intermediate._locator is None


def test_subclass_without_kwargs_auto_registers_with_class_name() -> None:
    class AutoRegisteredCtx(ContextBase):
        pass

    assert AutoRegisteredCtx._locator is not None
    assert AutoRegisteredCtx.default_role == 'AutoRegisteredCtx'
    assert any(
        e.context_type is AutoRegisteredCtx
        for e in ContextFactory.registered_contexts
    )


def test_duplicate_criteria_emits_warning() -> None:
    @context(role='__test_dup__')
    class FirstDup(ContextBase):
        pass

    with pytest.warns(DuplicateRegistrationWarning, match='__test_dup__'):
        @context(role='__test_dup__')
        class SecondDup(ContextBase):
            pass

    # Both classes are registered; the warning does not block registration.
    assert any(e.context_type is FirstDup for e in ContextFactory.registered_contexts)
    assert any(e.context_type is SecondDup for e in ContextFactory.registered_contexts)


def test_re_registering_same_class_is_silent() -> None:
    @context(role='__test_reuse__')
    class ReuseCtx(ContextBase):
        pass

    with warnings.catch_warnings():
        warnings.simplefilter('error', DuplicateRegistrationWarning)
        # Re-registering the exact same class with the same criteria stays silent.
        ContextFactory.register_context(ReuseCtx, {'role': '__test_reuse__'})


def test_duplicate_criteria_warning_normalizes_regex() -> None:
    import re as _re

    @context(role='__test_regex_dup__', attributes={'Name': _re.compile(r'foo', _re.IGNORECASE)})
    class FirstRegex(ContextBase):
        pass

    with pytest.warns(DuplicateRegistrationWarning):
        @context(role='__test_regex_dup__', attributes={'Name': _re.compile(r'foo', _re.IGNORECASE)})
        class SecondRegex(ContextBase):
            pass

    assert any(e.context_type is FirstRegex for e in ContextFactory.registered_contexts)
    assert any(e.context_type is SecondRegex for e in ContextFactory.registered_contexts)


# ----------------------------------------------------------------------
# ContextFactory weighting
# ----------------------------------------------------------------------


def test_find_context_class_for_returns_explicit_type() -> None:
    @context(role='__test_button__')
    class B(ContextBase):
        pass

    a = _make_adapter(role='OtherRole')
    assert ContextFactory.find_context_class_for(a, B) is B


def test_find_context_class_for_picks_best_weight() -> None:
    @context(role='__test_button__')
    class GenericButton(ContextBase):
        pass

    @context(role='__test_button__', framework_id='Qt')
    class QtButton(ContextBase):
        pass

    a = _make_adapter(role='__test_button__', framework_id='Qt')
    assert ContextFactory.find_context_class_for(a) is QtButton


def test_find_context_class_for_returns_unknown_when_no_match() -> None:
    @context(role='__test_button__')
    class B(ContextBase):
        pass

    a = _make_adapter(role='__no_such_role_for_test__')
    assert ContextFactory.find_context_class_for(a) is UnknownContext


# ----------------------------------------------------------------------
# Locator inheritance via copy_from
# ----------------------------------------------------------------------


def test_instance_locator_inherits_class_default() -> None:
    @context(role='__test_button__', framework_id='Qt')
    class QtButton(ContextBase):
        pass

    instance = QtButton(Locator(name='OK'))
    assert instance.locator.role == '__test_button__'
    assert instance.locator.framework_id == 'Qt'
    assert instance.locator.name == 'OK'


def test_instance_locator_overrides_class_default() -> None:
    @context(role='__test_button__')
    class B(ContextBase):
        pass

    instance = B(Locator(role='ToggleButton'))
    assert instance.locator.role == 'ToggleButton'


# ----------------------------------------------------------------------
# Element search delegates to adapter_factory
# ----------------------------------------------------------------------


def test_get_returns_typed_context() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent_adapter = _make_adapter(role='Window')
    parent = ContextBase(adapter=parent_adapter)

    result = parent.get(Btn)
    assert isinstance(result, Btn)
    assert result.context_parent is parent
    assert result.locator.role == '__test_button__'


def test_get_with_explicit_locator_uses_it() -> None:
    parent = ContextBase(adapter=_make_adapter())
    custom = Locator(role='Custom', name='X')
    result = parent.get(ContextBase, locator=custom)
    assert result.locator.role == 'Custom'
    assert result.locator.name == 'X'


def test_get_without_class_default_or_locator_raises() -> None:
    parent = ContextBase(adapter=_make_adapter())
    with pytest.raises(NoLocatorDefinedError):
        parent.get(ContextBase)


def test_get_child_sets_children_scope() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent = ContextBase(adapter=_make_adapter())
    child = parent.get_child(Btn)
    assert child.locator.scope == 'children'


def test_ancestor_sets_ancestor_scope() -> None:
    @context(role='__test_ancestor_window__')
    class Win(ContextBase):
        pass

    inner = ContextBase(adapter=_make_adapter())
    res = inner.ancestor(Win)
    assert res.locator.scope == 'ancestor'


def test_iter_all_consults_adapter_factory() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent_adapter = _make_adapter(role='Window')
    parent = ContextBase(adapter=parent_adapter)

    a1 = _make_adapter(role='__test_button__', runtime_id='b1')
    a2 = _make_adapter(role='__test_button__', runtime_id='b2')
    stub = _StubFactory(results=[a1, a2])

    with adapter_factory.override(lambda: stub):
        results = parent.get_all(Btn)

    assert len(results) == 2
    assert all(isinstance(r, Btn) for r in results)
    assert {r.runtime_id for r in results} == {'b1', 'b2'}
    assert len(stub.find_all_calls) == 1
    called_parent, _ = stub.find_all_calls[0]
    assert called_parent is parent_adapter


def test_get_one_returns_single_match() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent = ContextBase(adapter=_make_adapter())
    only = _make_adapter(role='__test_button__')
    stub = _StubFactory(results=[only])

    with adapter_factory.override(lambda: stub):
        result = parent.get_one(Btn)

    assert isinstance(result, Btn)
    assert result.adapter is only


def test_get_one_raises_on_zero_matches() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent = ContextBase(adapter=_make_adapter())
    stub = _StubFactory(results=[])
    with adapter_factory.override(lambda: stub):
        with pytest.raises(AdapterNotFoundError):
            parent.get_one(Btn)


def test_get_one_raises_on_multiple_matches() -> None:
    @context(role='__test_button__')
    class Btn(ContextBase):
        pass

    parent = ContextBase(adapter=_make_adapter())
    stub = _StubFactory(
        results=[_make_adapter(role='__test_button__'), _make_adapter(role='__test_button__')],
    )
    with adapter_factory.override(lambda: stub):
        with pytest.raises(MultipleElementsFoundError):
            parent.get_one(Btn)


# ----------------------------------------------------------------------
# Iteration over children / parent
# ----------------------------------------------------------------------


def test_iter_yields_one_context_per_child_adapter() -> None:
    c1 = _make_adapter(role='A')
    c2 = _make_adapter(role='B')
    root = _make_adapter(role='Window', children=[c1, c2])
    ctx = ContextBase(adapter=root)
    children = list(ctx)
    assert len(children) == 2
    assert {c.role for c in children} == {'A', 'B'}


def test_children_property_returns_one_per_child_adapter() -> None:
    c1 = _make_adapter(role='A', runtime_id='c1')
    c2 = _make_adapter(role='B', runtime_id='c2')
    root = _make_adapter(role='Window', children=[c1, c2])
    ctx = ContextBase(adapter=root)
    children = ctx.children
    assert [c.adapter for c in children] == [c1, c2]


def test_parent_returns_wrapped_parent_adapter() -> None:
    parent_adapter = _make_adapter(role='Window')
    me_adapter = _make_adapter(role='Button', parent=parent_adapter)
    ctx = ContextBase(adapter=me_adapter)
    parent_ctx = ctx.parent
    assert parent_ctx is not None
    assert parent_ctx.adapter is parent_adapter


def test_parent_returns_none_at_root() -> None:
    root = _make_adapter(role='Desktop', parent=None)
    ctx = ContextBase(adapter=root)
    assert ctx.parent is None


# ----------------------------------------------------------------------
# Generic attribute reads
# ----------------------------------------------------------------------


def test_attribute_value_delegates_to_adapter() -> None:
    a = _make_adapter()
    a.attribute_value = MagicMock(return_value='hello')  # type: ignore[method-assign]
    ctx = ContextBase(adapter=a)
    assert ctx.attribute_value('Name') == 'hello'
    a.attribute_value.assert_called_once_with('Name', 'control')


def test_attributes_iter_delegates_to_adapter() -> None:
    a = _make_adapter()
    triples = [('control', 'Name', 'OK'), ('control', 'Role', 'Button')]
    a.attributes = MagicMock(return_value=iter(triples))  # type: ignore[method-assign]  # type: ignore[method-assign]
    ctx = ContextBase(adapter=a)
    assert list(ctx.attributes()) == triples


# ----------------------------------------------------------------------
# UnknownContext
# ----------------------------------------------------------------------


def test_unknown_context_is_a_context_base() -> None:
    assert issubclass(UnknownContext, ContextBase)
