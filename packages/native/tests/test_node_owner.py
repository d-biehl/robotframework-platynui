"""Every node carries the identity of the runtime that produced it.

A `UiNode` handle is only meaningful to the runtime whose provider connection created it.
Nothing about the node itself reveals that: `runtime_id` is provider-stable, so two runtimes
driving the same session hand out equal ids for the same element. The owner stamp is the only
way a caller holding a node can tell which runtime it belongs to.
"""

from collections.abc import Generator

import platynui_native as pn
import pytest


@pytest.fixture
def second_mock_runtime() -> Generator[pn.Runtime, None, None]:
    runtime = pn.Runtime.new_with_mock()
    try:
        yield runtime
    finally:
        runtime.shutdown()


def test_runtimes_have_distinct_instance_ids(
    rt_mock_platform: pn.Runtime, second_mock_runtime: pn.Runtime
) -> None:
    assert rt_mock_platform.instance_id != second_mock_runtime.instance_id


def test_node_reports_its_producing_runtime(rt_mock_platform: pn.Runtime) -> None:
    node = rt_mock_platform.evaluate_single('//control:Window[@Name="Operations Console"]', None)
    assert isinstance(node, pn.UiNode)
    assert node.owner_id == rt_mock_platform.instance_id


def test_equal_element_from_two_runtimes_differs_only_in_owner(
    rt_mock_platform: pn.Runtime, second_mock_runtime: pn.Runtime
) -> None:
    """The case a Python-side check cannot see: same element, same runtime_id, different owner."""
    query = '//control:Window[@Name="Operations Console"]'
    first = rt_mock_platform.evaluate_single(query, None)
    second = second_mock_runtime.evaluate_single(query, None)
    assert isinstance(first, pn.UiNode)
    assert isinstance(second, pn.UiNode)

    assert first.runtime_id == second.runtime_id
    assert first.owner_id != second.owner_id


def test_navigation_preserves_the_owner(rt_mock_platform: pn.Runtime) -> None:
    """A node reached by walking the tree belongs to the same runtime as the node walked from."""
    owner = rt_mock_platform.instance_id
    window = rt_mock_platform.evaluate_single('//control:Window[@Name="Operations Console"]', None)
    assert isinstance(window, pn.UiNode)

    child = next(iter(window.children()))
    assert child.owner_id == owner

    parent = child.parent()
    assert parent is not None
    assert parent.owner_id == owner

    assert all(a.owner_id == owner for a in window.ancestors())
    assert all(a.owner_id == owner for a in window.ancestors_including_self())
    assert window.top_level_or_self().owner_id == owner


def test_evaluate_iter_preserves_the_owner(rt_mock_platform: pn.Runtime) -> None:
    nodes = [item for item in rt_mock_platform.evaluate_iter('//control:Window', None) if isinstance(item, pn.UiNode)]
    assert nodes
    assert all(node.owner_id == rt_mock_platform.instance_id for node in nodes)


def test_runtime_produced_nodes_carry_the_owner(rt_mock_platform: pn.Runtime) -> None:
    owner = rt_mock_platform.instance_id
    desktop = rt_mock_platform.desktop_node()
    assert desktop.owner_id == owner

    window = rt_mock_platform.evaluate_single('//control:Window[@Name="Operations Console"]', None)
    assert isinstance(window, pn.UiNode)
    top_level = rt_mock_platform.top_level_window_for(window)
    if top_level is not None:
        assert top_level.owner_id == owner
