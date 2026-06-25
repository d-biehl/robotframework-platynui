"""Python value semantics of evaluated query results.

`UiNode` and `EvaluatedAttribute` are returned by `Runtime.evaluate` / `evaluate_single`.
Their truthiness and (for attributes) equality must reflect their meaning, so that
`if node:`, `IF ${result}` and the BareMetal wait keywords' truthy default behave
intuitively instead of being unconditionally `True` / `False`.
"""

from platynui_native import EvaluatedAttribute, Runtime, UiNode

OPS = "//control:Window[@Name='Operations Console']"


def test_uinode_bool_reflects_validity(rt_mock_platform: Runtime) -> None:
    node = rt_mock_platform.evaluate_single(OPS)
    assert isinstance(node, UiNode)
    assert node.is_valid() is True
    assert bool(node) is True
    assert bool(node) == node.is_valid()


def test_evaluated_attribute_truthy_value(rt_mock_platform: Runtime) -> None:
    attr = rt_mock_platform.evaluate_single(f'{OPS}/@Name')
    assert isinstance(attr, EvaluatedAttribute)
    assert attr.value == 'Operations Console'
    # Truthiness and comparisons delegate to the captured value.
    assert bool(attr) is True
    assert attr == 'Operations Console'
    assert not (attr != 'Operations Console')
    assert attr != 'Something Else'
    assert str(attr) == 'Operations Console'
    assert hash(attr) == hash('Operations Console')


def test_evaluated_attribute_falsy_value(rt_mock_platform: Runtime) -> None:
    # A fresh mock window is not maximized, so @IsMaximized is False.
    attr = rt_mock_platform.evaluate_single(f'{OPS}/@IsMaximized')
    assert isinstance(attr, EvaluatedAttribute)
    assert attr.value is False
    # The whole point: a falsy attribute is falsy, not unconditionally True.
    assert bool(attr) is False
    assert attr == attr.value
