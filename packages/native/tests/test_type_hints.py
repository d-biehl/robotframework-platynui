#!/usr/bin/env python3
"""Quick type check for iterator imports."""

from platynui_native import (
    NodeChildrenIterator,
    NodeAttributesIterator,
    UiNode,
    Runtime,
)


# Type hints should work
def process_children(node: UiNode) -> None:
    children_iter: NodeChildrenIterator = node.children()
    for _ in children_iter:
        pass


def process_attributes(node: UiNode) -> None:
    attrs_iter: NodeAttributesIterator = node.attributes()
    for _ in attrs_iter:
        pass


def test_iterator_types_exported(rt_mock_platform: Runtime) -> None:
    node = rt_mock_platform.desktop_node()
    # Ensure functions accept the typed iterators without runtime issues
    process_children(node)
    process_attributes(node)
