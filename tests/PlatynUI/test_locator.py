# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.locator``."""

import re
from typing import cast

import pytest

from PlatynUI.core import Locator, locator
from PlatynUI.core.locator import DEFAULT_ATTRIBUTE_NAMESPACE


def test_default_attribute_namespace_constant() -> None:
    assert DEFAULT_ATTRIBUTE_NAMESPACE == 'control'


def test_default_axis_is_descendants() -> None:
    assert Locator(role='Button').to_xpath() == './/Button'


def test_default_axis_children_when_parent_root_like() -> None:
    assert Locator(role='Button').to_xpath(parent_is_root_like=True) == 'Button'


def test_explicit_scope_overrides_default() -> None:
    assert Locator(role='Button', scope='children').to_xpath() == 'Button'
    assert Locator(role='Window', scope='root').to_xpath() == '/Window'
    assert Locator(role='X', scope='ancestor').to_xpath() == 'ancestor::X'


def test_default_role_used_when_node_unspecified() -> None:
    assert Locator().to_xpath(default_role='Button') == './/Button'


def test_node_overrides_role_and_default() -> None:
    assert Locator(node='custom', role='Button').to_xpath(default_role='X') == './/custom'


def test_path_taken_verbatim() -> None:
    assert Locator(path='/.').to_xpath() == '/.'
    # path wins over everything else.
    assert Locator(path='/Window/Button', role='ignored', scope='root').to_xpath() == '/Window/Button'


def test_attribute_shorthands_render_as_predicates() -> None:
    xpath = Locator(role='Button', name='OK', id='btn1').to_xpath()
    assert xpath == './/Button[@Id="btn1" and @Name="OK"]'


def test_free_form_attributes_default_namespace_is_unprefixed() -> None:
    """Bare-string keys default to the ``control`` namespace and emit unprefixed."""
    xpath = Locator(role='Edit', attributes={'AutomationId': 'user'}).to_xpath()
    assert xpath == './/Edit[@AutomationId="user"]'


def test_regex_attribute_uses_matches() -> None:
    xpath = Locator(role='Button', attributes={'Name': re.compile(r'OK.*')}).to_xpath()
    assert 'matches(@Name, "OK.*")' in xpath


def test_tuple_key_explicit_namespace_renders_with_prefix() -> None:
    xpath = Locator(role='Button', attributes={('native', 'HWND'): '0x12AB'}).to_xpath()
    assert xpath == './/Button[@native:HWND="0x12AB"]'


def test_tuple_key_with_control_namespace_renders_unprefixed() -> None:
    """Explicit control-namespace tuple key still emits without prefix."""
    xpath = Locator(role='Button', attributes={('control', 'AutomationId'): 'x'}).to_xpath()
    assert xpath == './/Button[@AutomationId="x"]'


def test_tuple_key_regex_with_namespace() -> None:
    xpath = Locator(role='X', attributes={('item', 'Name'): re.compile(r'foo.*')}).to_xpath()
    assert 'matches(@item:Name, "foo.*")' in xpath


def test_default_attribute_namespace_override_applies_to_bare_keys() -> None:
    """A context can lift the default namespace to e.g. ``item``."""
    xpath = Locator(role='Row', attributes={'Index': '3'}).to_xpath(default_attribute_namespace='item')
    assert xpath == './/Row[@item:Index="3"]'


def test_default_attribute_namespace_override_does_not_affect_tuple_keys() -> None:
    xpath = Locator(
        role='Row',
        attributes={'Index': '3', ('native', 'HWND'): '0x1'},
    ).to_xpath(default_attribute_namespace='item')
    assert '@item:Index="3"' in xpath
    assert '@native:HWND="0x1"' in xpath


def test_default_attribute_namespace_override_does_not_affect_shorthands() -> None:
    """Standard shorthands (Id/Name/...) always sit in control namespace."""
    xpath = Locator(role='Row', name='OK').to_xpath(default_attribute_namespace='item')
    assert xpath == './/Row[@Name="OK"]'


def test_invalid_tuple_key_length_rejected() -> None:
    with pytest.raises(ValueError, match='must be'):
        Locator(role='X', attributes={('a', 'b', 'c'): 'v'})  # type: ignore[dict-item]


def test_invalid_tuple_key_types_rejected() -> None:
    with pytest.raises(TypeError, match='must be'):
        Locator(role='X', attributes={(1, 'b'): 'v'})  # type: ignore[dict-item]


def test_custom_attributes_appended_raw() -> None:
    xpath = Locator(role='X', custom_attributes=["@Foo='bar'", 'position()=2']).to_xpath()
    assert "@Foo='bar'" in xpath
    assert 'position()=2' in xpath


def test_index_and_position() -> None:
    xpath = Locator(role='Button', position=3, index=1).to_xpath()
    assert xpath == './/Button[position()=3][1]'


def test_prefix_explicit() -> None:
    assert Locator(role='Button', prefix='app').to_xpath() == './/app:Button'


def test_prefix_default_when_requested() -> None:
    xpath = Locator(role='Button', use_default_prefix=True).to_xpath(default_prefix='app')
    assert xpath == './/app:Button'


def test_axis_overrides_scope() -> None:
    xpath = Locator(role='X', axis='self::').to_xpath()
    assert xpath == 'self::X'


def test_copy_from_inherits_unset_fields() -> None:
    parent = Locator(role='Button', framework_id='WPF', attributes={'K': 'V'})
    child = Locator(name='OK', attributes={'K2': 'V2'})
    child.copy_from(parent)
    assert child.role == 'Button'
    assert child.framework_id == 'WPF'
    assert child.name == 'OK'  # child wins
    assert child.attributes == {'K2': 'V2', 'K': 'V'}


def test_copy_from_merges_tuple_and_string_keys() -> None:
    parent = Locator(role='X', attributes={('native', 'HWND'): '0x1', 'A': 'a'})
    child = Locator(attributes={'A': 'override'})
    child.copy_from(parent)
    # child wins on conflict, parent fills the gaps
    assert child.attributes['A'] == 'override'
    assert child.attributes[('native', 'HWND')] == '0x1'


def test_copy_from_does_not_overwrite() -> None:
    parent = Locator(role='Button')
    child = Locator(role='Edit')
    child.copy_from(parent)
    assert child.role == 'Edit'


def test_copy_is_independent() -> None:
    original = Locator(role='X', attributes={'k': 'v'})
    clone = original.copy()
    clone.attributes['k2'] = 'v2'
    assert 'k2' not in original.attributes


# --- Free-form PascalCase kwargs --------------------------------------------


def test_pascal_case_kwarg_renders_in_default_namespace() -> None:
    xpath = Locator(role='Button', AutomationId='num5').to_xpath()
    assert xpath == './/Button[@AutomationId="num5"]'


def test_pascal_case_kwarg_takes_value_verbatim() -> None:
    # No case conversion: the kwarg name lands as-is.
    xpath = Locator(role='X', foo='bar').to_xpath()
    assert xpath == './/X[@foo="bar"]'


def test_kwarg_namespace_separator_emits_prefix() -> None:
    xpath = Locator(role='Window', native__HWND='0xABCD').to_xpath()
    assert xpath == './/Window[@native:HWND="0xABCD"]'


def test_kwarg_with_multiple_separators_rejected() -> None:
    with pytest.raises(ValueError, match='multiple "__" separators'):
        Locator(role='X', a__b__c='v')


def test_kwarg_with_empty_namespace_or_name_rejected() -> None:
    with pytest.raises(ValueError, match='empty namespace or name'):
        Locator(role='X', __HWND='v')
    with pytest.raises(ValueError, match='empty namespace or name'):
        Locator(role='X', native__='v')


def test_kwarg_and_dict_can_coexist_for_different_attributes() -> None:
    xpath = Locator(
        role='Button',
        AutomationId='x',
        attributes={'IsEnabled': 'true'},
    ).to_xpath()
    assert xpath == './/Button[@IsEnabled="true" and @AutomationId="x"]'


# --- Conflict detection ------------------------------------------------------


def test_conflict_snake_case_field_and_kwarg() -> None:
    with pytest.raises(TypeError, match='@Name'):
        Locator(role='X', name='A', Name='B')


def test_conflict_snake_case_field_and_attributes_dict() -> None:
    with pytest.raises(TypeError, match='@Name'):
        Locator(role='X', name='A', attributes={'Name': 'B'})


def test_conflict_kwarg_and_attributes_dict() -> None:
    with pytest.raises(TypeError, match='@AutomationId'):
        Locator(
            role='X',
            AutomationId='A',
            attributes={'AutomationId': 'B'},
        )


def test_conflict_after_namespace_normalization() -> None:
    # Bare-string 'Name' normalizes to ('control', 'Name'), which is the
    # same key as the explicit ('control', 'Name') tuple.
    with pytest.raises(TypeError, match='@Name'):
        Locator(
            role='X',
            attributes={'Name': 'A', ('control', 'Name'): 'B'},
        )


def test_conflict_kwarg_with_separator_vs_tuple_dict() -> None:
    with pytest.raises(TypeError, match='@native:HWND'):
        Locator(
            role='Window',
            native__HWND='A',
            attributes={('native', 'HWND'): 'B'},
        )


def test_no_conflict_when_namespaces_differ() -> None:
    # Same attribute name in two different namespaces is fine.
    xpath = Locator(
        role='X',
        attributes={'Foo': 'A', ('native', 'Foo'): 'B'},
    ).to_xpath()
    assert xpath == './/X[@Foo="A" and @native:Foo="B"]'


def test_reserved_field_only_snake_case() -> None:
    # 'Path' (capital P) is NOT the reserved 'path' field — it becomes a
    # free-form @Path attribute.
    xpath = Locator(role='X', Path='/some/path').to_xpath()
    assert xpath == './/X[@Path="/some/path"]'


# ---------------------------------------------------------------------------
# @locator decorator (design document §A.6 / §7.1)
# ---------------------------------------------------------------------------


def test_locator_is_a_decorator_factory_not_an_alias() -> None:
    # Regression guard: ``locator`` must NOT be the bare ``Locator`` class.
    # Calling it returns a decorator, not a Locator instance.
    # ``cast`` widens the comparison so neither mypy nor pyright complains.
    assert cast(object, locator) is not Locator
    decorator = locator(name='X')
    assert callable(decorator)
    assert not isinstance(decorator, Locator)


def test_class_decorator_attaches_locator_attribute() -> None:
    @locator(name='Calculator', role='Window')
    class CalculatorWindow:
        pass

    loc: Locator = getattr(CalculatorWindow, '__locator__')
    assert isinstance(loc, Locator)
    assert loc.name == 'Calculator'
    assert loc.role == 'Window'
    assert loc.to_xpath() == './/Window[@Name="Calculator"]'


def test_class_decorator_returns_class_unchanged() -> None:
    @locator(name='X')
    class Foo:
        bar = 42

    # Same class object — the decorator does not wrap or replace it.
    assert isinstance(Foo, type)
    assert Foo.__name__ == 'Foo'
    assert Foo.bar == 42
    assert Foo().bar == 42


def test_class_decorator_supports_free_form_kwargs() -> None:
    @locator(AutomationId='num5Button')
    class N5:
        pass

    loc: Locator = getattr(N5, '__locator__')
    assert loc.to_xpath() == './/*[@AutomationId="num5Button"]'


def test_method_decorator_returns_descriptor() -> None:
    from PlatynUI.core.locator import LocatorMethodDescriptor

    class Ctx:
        @locator(AutomationId='num5Button')
        def n5(self) -> object: ...

    raw = Ctx.__dict__['n5']
    assert isinstance(raw, LocatorMethodDescriptor)
    assert raw.__locator__.to_xpath() == './/*[@AutomationId="num5Button"]'


def test_method_form_bare_resolves_annotated_child() -> None:
    from PlatynUI.core.context import ContextBase

    class _Child(ContextBase, register=False): ...

    class _Win(ContextBase, register=False):
        @locator(AutomationId='num5Button')
        def n5(self) -> _Child:
            raise NotImplementedError  # body unused: the @locator descriptor resolves it

    win = _Win(Locator(path='/Win'))
    child = win.n5

    assert isinstance(child, _Child)
    assert child.context_parent is win
    assert child.locator.to_xpath() == './/*[@AutomationId="num5Button"]'


def test_method_form_property_resolves_annotated_child() -> None:
    from PlatynUI.core.context import ContextBase

    class _Child(ContextBase, register=False): ...

    class _Win(ContextBase, register=False):
        @property
        @locator(AutomationId='num6Button')
        def n6(self) -> _Child:
            raise NotImplementedError  # body unused: the @locator descriptor resolves it

    win = _Win(Locator(path='/Win'))
    child = win.n6

    assert isinstance(child, _Child)
    assert child.context_parent is win
    assert child.locator.to_xpath() == './/*[@AutomationId="num6Button"]'


def test_method_form_rejects_non_context_annotation() -> None:
    from PlatynUI.core.context import ContextBase

    class _Win(ContextBase, register=False):
        @locator(AutomationId='x')
        def thing(self) -> object: ...

    win = _Win(Locator(path='/Win'))
    with pytest.raises(TypeError, match='ContextBase subclass'):
        _ = win.thing


def test_method_form_rejects_non_context_instance() -> None:
    from PlatynUI.core.context import ContextBase

    class _Child(ContextBase, register=False): ...

    class _Plain:
        @locator(AutomationId='x')
        def child(self) -> _Child:
            raise NotImplementedError  # body unused: the @locator descriptor resolves it

    with pytest.raises(TypeError, match='requires a ContextBase instance'):
        _ = _Plain().child


def test_method_decorator_class_access_returns_descriptor() -> None:
    from PlatynUI.core.locator import LocatorMethodDescriptor

    class Ctx:
        @locator(name='X')
        def thing(self) -> object: ...

    # Accessing on the class (not an instance) yields the descriptor itself.
    obj = Ctx.__dict__['thing']
    assert isinstance(obj, LocatorMethodDescriptor)


def test_method_decorator_remembers_attribute_name() -> None:
    from PlatynUI.core.locator import LocatorMethodDescriptor

    class Ctx:
        @locator(name='X')
        def thing(self) -> object: ...

    desc = Ctx.__dict__['thing']
    assert isinstance(desc, LocatorMethodDescriptor)
    assert desc.attr_name == 'thing'


def test_decorator_rejects_non_class_non_callable() -> None:
    decorator = locator(name='X')
    with pytest.raises(TypeError, match='classes or callables'):
        decorator(42)


def test_decorator_kwargs_match_locator_constructor() -> None:
    # Same kwargs that work on Locator(...) must work on locator(...).
    @locator(
        role='Button',
        scope='descendants',
        index=2,
        attributes={('native', 'HWND'): '0xABCD'},
    )
    class Btn:
        pass

    loc: Locator = getattr(Btn, '__locator__')
    assert loc.role == 'Button'
    assert loc.index == 2
    assert ('native', 'HWND') in loc.attributes
