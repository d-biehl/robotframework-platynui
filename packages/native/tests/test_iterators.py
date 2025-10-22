#!/usr/bin/env python3
"""Test that UiNode.children() and UiNode.attributes() return iterators."""

from platynui_native import (
    NodeAttributesIterator,
    NodeChildrenIterator,
    Runtime,
    UiNode,
)


def get_desktop_node(rt_mock_platform: Runtime) -> UiNode:
    """Helper to get desktop node."""
    items = rt_mock_platform.evaluate('/')
    node = next((it for it in items if isinstance(it, UiNode)), None)
    assert node is not None, 'Expected at least one UiNode result'
    return node


def test_children_returns_iterator(rt_mock_platform: Runtime) -> None:
    """Verify that children() returns an iterator, not a list."""
    desktop = get_desktop_node(rt_mock_platform)

    children_result = desktop.children()
    assert isinstance(children_result, NodeChildrenIterator), (
        f'Expected NodeChildrenIterator, got {type(children_result)}'
    )

    # Should be iterable
    first_list = list(children_result)
    assert isinstance(first_list, list)

    # Can use in for loop
    count = 0
    for child in get_desktop_node(rt_mock_platform).children():
        count += 1
        assert isinstance(child, UiNode)
    assert count >= 1


def test_attributes_returns_iterator(rt_mock_platform: Runtime) -> None:
    """Verify that attributes() returns an iterator, not a list."""
    desktop = get_desktop_node(rt_mock_platform)

    attrs_result = desktop.attributes()
    assert isinstance(attrs_result, NodeAttributesIterator), (
        f'Expected NodeAttributesIterator, got {type(attrs_result)}'
    )

    # Should be iterable
    first_attrs = list(attrs_result)
    assert isinstance(first_attrs, list)

    # Can use in for loop
    count = 0
    for attr in get_desktop_node(rt_mock_platform).attributes():
        count += 1
        # Minimal shape check
        assert hasattr(attr, 'namespace') and hasattr(attr, 'name')
    assert count >= 1


def test_iterator_exhaustion(rt_mock_platform: Runtime) -> None:
    """Verify that iterators can only be consumed once."""
    desktop = get_desktop_node(rt_mock_platform)

    children_iter = desktop.children()

    # First iteration
    first_list = list(children_iter)
    assert len(first_list) >= 1

    # Second iteration on same iterator (should be empty)
    second_list = list(children_iter)
    assert len(second_list) == 0, 'Iterator should be exhausted after first iteration'

    # Get fresh iterator
    fresh_list = list(get_desktop_node(rt_mock_platform).children())
    assert len(fresh_list) == len(first_list), 'Fresh iterator should have same count'


def test_lazy_evaluation(rt_mock_platform: Runtime) -> None:
    """Demonstrate that iterators don't materialize all items upfront."""
    desktop = get_desktop_node(rt_mock_platform)

    children_iter = desktop.children()

    # Take only first 3 children
    first_three = []
    for i, child in enumerate(children_iter):
        if i >= 3:
            break
        first_three.append(child)

    assert 0 < len(first_three) <= 3
    for node in first_three:
        assert isinstance(node, UiNode)


if __name__ == '__main__':
    pass
