# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# mypy: disable-error-code="type-abstract"
# pyright: reportPrivateUsage=false, reportUnusedFunction=false
#
# Pattern-ABC classes are passed to ``get_pattern`` as public API; the
# unused-function diagnostic covers pytest fixtures, and the protected
# accesses target the resolved-pattern cache to verify caching.

"""Tests for :class:`PlatynUI.core.adapters.UiNodeAdapter`.

These tests exercise the native adapter end-to-end against the Rust
mock provider (``Runtime.new_with_mock()``). They cover:

* identity / lifetime mapping (``runtime_id``, ``valid``)
* structural traversal (``parent``, ``children``)
* search-criteria properties and defensive defaults for missing
  optional attributes (``ClassName``, ``FrameworkId``)
* namespaced attribute reads (including ``KeyError`` on missing names)
* native-pattern resolution for :class:`Focusable`
* :class:`UiNodeTechnology` singleton identity

The fixtures rely on the deterministic ``mock_tree.xml`` shipped with
``crates/provider-mock``; see ``crates/provider-mock/assets/mock_tree.xml``
for ground-truth values such as the ``Operations Console`` window and
the ``OK`` button.
"""

from __future__ import annotations

from collections.abc import Generator

import platynui_native as _pn
import pytest

from PlatynUI.core.adapter import Adapter
from PlatynUI.core.adapters import UiNodeAdapter, UiNodeTechnology
from PlatynUI.core.exceptions import PatternNotSupportedError
from PlatynUI.core.patterns import Element, Focusable
from PlatynUI.core.technology import Technology

# ----------------------------------------------------------------------
# Fixtures
# ----------------------------------------------------------------------


@pytest.fixture
def runtime() -> Generator[_pn.Runtime, None, None]:
    rt = _pn.Runtime.new_with_mock()
    try:
        yield rt
    finally:
        rt.shutdown()


@pytest.fixture
def desktop_adapter(runtime: _pn.Runtime) -> UiNodeAdapter:
    return UiNodeAdapter.create_root(runtime)


@pytest.fixture
def main_window_adapter(runtime: _pn.Runtime) -> UiNodeAdapter:
    node = runtime.evaluate_single("//control:Window[@Name='Operations Console']")
    assert isinstance(node, _pn.UiNode), 'mock tree must expose Operations Console window'
    return UiNodeAdapter.from_node(runtime, node)


@pytest.fixture
def ok_button_adapter(runtime: _pn.Runtime) -> UiNodeAdapter:
    node = runtime.evaluate_single("//control:Button[@Name='OK']")
    assert isinstance(node, _pn.UiNode), 'mock tree must expose OK button'
    return UiNodeAdapter.from_node(runtime, node)


@pytest.fixture
def focusable_listitem_adapter(runtime: _pn.Runtime) -> UiNodeAdapter:
    # First ListItem in the Task List — advertises the Focusable pattern.
    # ListItems live in the "item" namespace, so the Name predicate must
    # be explicitly qualified.
    node = runtime.evaluate_single("//item:ListItem[@item:Name='Analyze Project Status']")
    assert isinstance(node, _pn.UiNode), 'mock tree must expose Analyze Project Status item'
    return UiNodeAdapter.from_node(runtime, node)


# ----------------------------------------------------------------------
# Construction & identity
# ----------------------------------------------------------------------


def test_create_root_wraps_desktop_node(runtime: _pn.Runtime, desktop_adapter: UiNodeAdapter) -> None:
    assert isinstance(desktop_adapter, Adapter)
    assert desktop_adapter.role == 'Desktop'
    assert desktop_adapter.runtime_id == runtime.desktop_node().runtime_id


def test_runtime_id_matches_native_node(main_window_adapter: UiNodeAdapter) -> None:
    assert main_window_adapter.runtime_id == 'mock://window/main'


def test_valid_flag_reflects_node_state(desktop_adapter: UiNodeAdapter) -> None:
    assert desktop_adapter.valid is True


def test_technology_is_singleton(desktop_adapter: UiNodeAdapter, main_window_adapter: UiNodeAdapter) -> None:
    assert isinstance(desktop_adapter.technology, UiNodeTechnology)
    assert desktop_adapter.technology is main_window_adapter.technology
    assert UiNodeTechnology() is desktop_adapter.technology


def test_technology_subclasses_base_marker(desktop_adapter: UiNodeAdapter) -> None:
    assert isinstance(desktop_adapter.technology, Technology)


def test_equality_uses_runtime_id(runtime: _pn.Runtime) -> None:
    a = UiNodeAdapter.create_root(runtime)
    b = UiNodeAdapter.create_root(runtime)
    assert a == b
    assert hash(a) == hash(b)
    assert a is not b  # distinct Python objects, equal by runtime_id


# ----------------------------------------------------------------------
# Structural traversal
# ----------------------------------------------------------------------


def test_parent_of_root_is_none(desktop_adapter: UiNodeAdapter) -> None:
    assert desktop_adapter.parent is None


def test_parent_returns_adapter(main_window_adapter: UiNodeAdapter) -> None:
    parent = main_window_adapter.parent
    assert parent is not None
    assert isinstance(parent, UiNodeAdapter)
    # The Operations Console window has expose_flat="true" in the mock
    # tree (see crates/provider-mock/assets/mock_tree.xml), so the
    # provider lifts it directly under the Desktop root.
    assert parent.role == 'Desktop'


def test_children_returns_adapters(main_window_adapter: UiNodeAdapter) -> None:
    children = main_window_adapter.children
    assert len(children) > 0
    assert all(isinstance(c, UiNodeAdapter) for c in children)


def test_children_round_trip_via_parent(main_window_adapter: UiNodeAdapter) -> None:
    children = list(main_window_adapter.children)
    assert children, 'window must have children for this test'
    first = children[0]
    parent_again = first.parent
    assert parent_again is not None
    assert parent_again == main_window_adapter


# ----------------------------------------------------------------------
# Search criteria
# ----------------------------------------------------------------------


def test_basic_attributes(main_window_adapter: UiNodeAdapter) -> None:
    assert main_window_adapter.name == 'Operations Console'
    assert main_window_adapter.role == 'Window'


def test_supported_roles_contains_primary(main_window_adapter: UiNodeAdapter) -> None:
    roles = main_window_adapter.supported_roles
    assert 'Window' in roles


def test_class_name_defaults_to_empty_when_missing(main_window_adapter: UiNodeAdapter) -> None:
    # The mock tree does not set ClassName for the window; the adapter
    # must surface that as an empty string instead of raising.
    assert main_window_adapter.class_name == ''


def test_framework_id_defaults_to_empty_when_missing(main_window_adapter: UiNodeAdapter) -> None:
    assert main_window_adapter.framework_id == ''


def test_tag_name_defaults_to_empty(main_window_adapter: UiNodeAdapter) -> None:
    # UiNodeAdapter does not synthesise an XML tag; the ABC default ('')
    # is the right behaviour for native UI trees.
    assert main_window_adapter.tag_name == ''


# ----------------------------------------------------------------------
# Attribute API
# ----------------------------------------------------------------------


def test_attribute_value_returns_native_value(ok_button_adapter: UiNodeAdapter) -> None:
    bounds = ok_button_adapter.attribute_value('Bounds', 'control')
    assert isinstance(bounds, _pn.Rect)
    assert bounds.to_tuple() == (140.0, 620.0, 120.0, 32.0)


def test_attribute_value_default_namespace_is_control(ok_button_adapter: UiNodeAdapter) -> None:
    # Mirror the ABC default; omitting ``namespace`` must hit "control".
    assert ok_button_adapter.attribute_value('MyProperty') == 'My Value'


def test_attribute_value_missing_raises_keyerror(main_window_adapter: UiNodeAdapter) -> None:
    with pytest.raises(KeyError):
        main_window_adapter.attribute_value('DoesNotExist', 'control')


def test_attribute_value_missing_uses_namespaced_key(main_window_adapter: UiNodeAdapter) -> None:
    with pytest.raises(KeyError) as exc_info:
        main_window_adapter.attribute_value('DoesNotExist', 'native')
    # Help debugging: the KeyError carries the qualified name.
    assert 'native:DoesNotExist' in str(exc_info.value)


def test_attribute_names_filter_by_namespace(main_window_adapter: UiNodeAdapter) -> None:
    # mock_tree.xml: <attribute namespace="native" name="ProcessId" .../>
    native_names = main_window_adapter.attribute_names('native')
    assert 'ProcessId' in native_names
    control_names = main_window_adapter.attribute_names('control')
    assert 'ProcessId' not in control_names


def test_attribute_names_default_returns_all_namespaces(main_window_adapter: UiNodeAdapter) -> None:
    all_names = main_window_adapter.attribute_names(None)
    # Both namespaces must be represented somewhere in the union.
    assert 'ProcessId' in all_names
    # AutomationId is the explicit control-namespace attribute.
    assert 'AutomationId' in all_names


def test_attributes_yields_namespace_name_value_triples(main_window_adapter: UiNodeAdapter) -> None:
    triples = list(main_window_adapter.attributes())
    assert triples, 'window must have attributes'
    by_qual = {(ns, name): value for ns, name, value in triples}
    assert by_qual[('native', 'ProcessId')] == 4242
    assert by_qual[('control', 'AutomationId')] == 'MainWindow'


# ----------------------------------------------------------------------
# Pattern discovery
# ----------------------------------------------------------------------


def test_supported_pattern_names_includes_focusable(focusable_listitem_adapter: UiNodeAdapter) -> None:
    assert Focusable.pattern_name in focusable_listitem_adapter.supported_pattern_names()


def test_supported_patterns_returns_python_classes(focusable_listitem_adapter: UiNodeAdapter) -> None:
    classes = focusable_listitem_adapter.supported_patterns()
    assert Focusable in classes


def test_supports_pattern_true_for_focusable(focusable_listitem_adapter: UiNodeAdapter) -> None:
    assert focusable_listitem_adapter.supports_pattern(Focusable) is True


def test_supports_pattern_false_when_native_lacks_python_wrapper(
    main_window_adapter: UiNodeAdapter,
) -> None:
    # Element is advertised on most nodes but has no native Python
    # wrapper yet; supports_pattern must reflect that to keep
    # get_pattern's contract intact.
    assert main_window_adapter.supports_pattern(Element) is False


def test_get_pattern_focusable_returns_wrapper(focusable_listitem_adapter: UiNodeAdapter) -> None:
    pattern = focusable_listitem_adapter.get_pattern(Focusable)
    assert isinstance(pattern, Focusable)


def test_get_pattern_focusable_caches_instance(focusable_listitem_adapter: UiNodeAdapter) -> None:
    first = focusable_listitem_adapter.get_pattern(Focusable)
    second = focusable_listitem_adapter.get_pattern(Focusable)
    assert first is second


def test_focusable_is_focused_defaults_false_when_attribute_missing(
    focusable_listitem_adapter: UiNodeAdapter,
) -> None:
    pattern = focusable_listitem_adapter.get_pattern(Focusable)
    # mock tree does not set IsFocused on this list item.
    assert pattern.is_focused is False


def test_focusable_is_focused_reads_native_attribute(runtime: _pn.Runtime) -> None:
    # mock_tree.xml sets IsFocused=true on exactly one node (the OK
    # button). Find it by iterating instead of via XPath: the native
    # side stores IsFocused as a real bool, so a string-comparison
    # XPath predicate would not match.
    candidate: _pn.UiNode | None = None
    for node in runtime.evaluate('//*'):
        if not isinstance(node, _pn.UiNode):
            continue
        if Focusable.pattern_name not in node.supported_patterns():
            continue
        try:
            value = node.attribute('IsFocused', node.namespace.as_str())
        except _pn.AttributeNotFoundError:
            continue
        if value is True:
            candidate = node
            break
    assert candidate is not None, 'mock tree must contain a node with IsFocused=true'
    adapter = UiNodeAdapter.from_node(runtime, candidate)
    pattern = adapter.get_pattern(Focusable)
    assert pattern.is_focused is True


def test_get_pattern_unknown_raises(main_window_adapter: UiNodeAdapter) -> None:
    # Element has no native wrapper → resolution must fail.
    with pytest.raises(PatternNotSupportedError):
        main_window_adapter.get_pattern(Element)


def test_get_pattern_raise_exception_false_returns_none(main_window_adapter: UiNodeAdapter) -> None:
    assert main_window_adapter.get_pattern(Element, raise_exception=False) is None
