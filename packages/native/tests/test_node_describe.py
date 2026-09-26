"""The one-line element description and the cheap ``repr`` (spec: *diagnostic-logging*, element form).

``describe()`` asks the provider for the name and the id; ``repr()`` and
``str()`` show only the runtime id, because Robot Framework converts every value
it assigns to text.
"""

import platynui_native as pn

OK_BUTTON = "//control:Button[@Name='OK']"


def ok_button(runtime: pn.Runtime) -> pn.UiNode:
    node = runtime.evaluate_single(OK_BUTTON)
    assert isinstance(node, pn.UiNode)
    return node


def test_the_description_of_the_mock_ok_button(rt_mock_platform: pn.Runtime) -> None:
    assert ok_button(rt_mock_platform).describe() == 'Button "OK"'


def test_repr_and_str_show_the_runtime_id(rt_mock_platform: pn.Runtime) -> None:
    node = ok_button(rt_mock_platform)
    assert node.runtime_id in repr(node)
    assert repr(node) == "UiNode(runtime_id='mock://button/ok')"
    assert str(node) == repr(node)
