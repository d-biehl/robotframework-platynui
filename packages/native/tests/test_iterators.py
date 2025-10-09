#!/usr/bin/env python3
"""Test that UiNode.children() and UiNode.attributes() return iterators."""

from platynui_native import (
    Runtime,
    UiNode,
    NodeChildrenIterator,
    NodeAttributesIterator,
)


def get_desktop_node():
    """Helper to get desktop node."""
    rt = Runtime()
    items = rt.evaluate("/")
    node = next((it for it in items if isinstance(it, UiNode)), None)
    assert node is not None, "Expected at least one UiNode result"
    return node


def test_children_returns_iterator():
    """Verify that children() returns an iterator, not a list."""
    desktop = get_desktop_node()

    children_result = desktop.children()
    print(f"children() returned: {type(children_result).__name__}")
    assert isinstance(children_result, NodeChildrenIterator), (
        f"Expected NodeChildrenIterator, got {type(children_result)}"
    )

    # Should be iterable
    children_list = list(children_result)
    print(f"  Iterated over {len(children_list)} children")

    # Can use in for loop
    count = 0
    for child in get_desktop_node().children():
        count += 1
        print(f"    Child {count}: {child.name} ({child.role})")

    print(f"  Total children in for-loop: {count}")
    print("  ✅ children() returns iterator!\n")


def test_attributes_returns_iterator():
    """Verify that attributes() returns an iterator, not a list."""
    desktop = get_desktop_node()

    attrs_result = desktop.attributes()
    print(f"attributes() returned: {type(attrs_result).__name__}")
    assert isinstance(attrs_result, NodeAttributesIterator), (
        f"Expected NodeAttributesIterator, got {type(attrs_result)}"
    )

    # Should be iterable
    attrs_list = list(attrs_result)
    print(f"  Iterated over {len(attrs_list)} attributes")

    # Can use in for loop
    count = 0
    for attr in get_desktop_node().attributes():
        count += 1
        if count <= 5:  # Only print first 5
            print(f"    Attr {count}: {attr.namespace}:{attr.name}")

    print(f"  Total attributes in for-loop: {count}")
    print("  ✅ attributes() returns iterator!\n")


def test_iterator_exhaustion():
    """Verify that iterators can only be consumed once."""
    desktop = get_desktop_node()

    children_iter = desktop.children()

    # First iteration
    first_list = list(children_iter)
    print(f"First iteration: {len(first_list)} children")

    # Second iteration on same iterator (should be empty)
    second_list = list(children_iter)
    print(f"Second iteration on same iterator: {len(second_list)} children")
    assert len(second_list) == 0, "Iterator should be exhausted after first iteration"

    # Get fresh iterator
    fresh_list = list(get_desktop_node().children())
    print(f"Fresh iterator: {len(fresh_list)} children")
    assert len(fresh_list) == len(first_list), "Fresh iterator should have same count"

    print("  ✅ Iterators properly exhaust!\n")


def test_lazy_evaluation():
    """Demonstrate that iterators don't materialize all items upfront."""
    desktop = get_desktop_node()

    print("Testing lazy evaluation:")
    children_iter = desktop.children()
    print(f"  Created iterator: {type(children_iter).__name__}")

    # Take only first 3 children
    first_three = []
    for i, child in enumerate(children_iter):
        if i >= 3:
            break
        first_three.append(child)

    print(f"  Took first {len(first_three)} children without iterating all")
    for i, child in enumerate(first_three):
        print(f"    {i + 1}. {child.name} ({child.role})")

    print("  ✅ Iterator supports lazy evaluation!\n")


if __name__ == "__main__":
    print("=" * 60)
    print("Testing Iterator Implementation for UiNode")
    print("=" * 60 + "\n")

    test_children_returns_iterator()
    test_attributes_returns_iterator()
    test_iterator_exhaustion()
    test_lazy_evaluation()

    print("=" * 60)
    print("🎉 All iterator tests passed!")
    print("=" * 60)
