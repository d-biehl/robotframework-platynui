#!/usr/bin/env python3
"""Quick type check for iterator imports."""

from platynui_native import (
    NodeChildrenIterator,
    NodeAttributesIterator,
    UiNode,
)


# Type hints should work
def process_children(node: UiNode) -> None:
    children_iter: NodeChildrenIterator = node.children()
    for child in children_iter:
        print(f"Child: {child.name}")


def process_attributes(node: UiNode) -> None:
    attrs_iter: NodeAttributesIterator = node.attributes()
    for attr in attrs_iter:
        print(f"Attribute: {attr.namespace}:{attr.name}")


# Check that classes are importable
print(f"✅ NodeChildrenIterator: {NodeChildrenIterator}")
print(f"✅ NodeAttributesIterator: {NodeAttributesIterator}")
print("✅ All iterator types are properly exported!")
