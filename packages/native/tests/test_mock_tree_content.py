import pytest
from platynui_native import AttributeNotFoundError, Point, Rect, Runtime, UiNode


def test_mock_windows_and_buttons(rt_mock_platform: Runtime) -> None:
    # Check that key windows exist; total may include expose_flat duplicates
    windows = [n for n in rt_mock_platform.evaluate('//control:Window') if isinstance(n, UiNode)]
    window_names = {w.name for w in windows}
    assert {'Operations Console', 'Detail View', 'Settings'}.issubset(window_names)

    ok_btn = rt_mock_platform.evaluate_single("//control:Button[@Name='OK']")
    assert isinstance(ok_btn, UiNode)
    assert ok_btn.role == 'Button'
    # Check deterministic attributes from mock tree
    bounds = ok_btn.attribute('Bounds', 'control')
    assert isinstance(bounds, Rect)
    assert bounds.to_tuple() == (140.0, 620.0, 120.0, 32.0)
    ap = ok_btn.attribute('ActivationPoint', 'control')
    assert isinstance(ap, Point)
    assert ap.to_tuple() == (200.0, 636.0)
    assert ok_btn.attribute('MyProperty', 'control') == 'My Value'


def test_mock_description_attribute(rt_mock_platform: Runtime) -> None:
    # The OK button carries an accessible description in the mock tree.
    by_desc = rt_mock_platform.evaluate_single("//control:Button[@Description='Confirms the operation']")
    assert isinstance(by_desc, UiNode)
    assert by_desc.name == 'OK'
    assert by_desc.attribute('Description', 'control') == 'Confirms the operation'
    assert by_desc.description == 'Confirms the operation'


def test_mock_description_absent(rt_mock_platform: Runtime) -> None:
    # The Cancel button has no description: attribute absent, getter is None.
    cancel = rt_mock_platform.evaluate_single("//control:Button[@Name='Cancel']")
    assert isinstance(cancel, UiNode)
    assert cancel.description is None
    with pytest.raises(AttributeNotFoundError):
        cancel.attribute('Description', 'control')


def test_mock_lists_and_items(rt_mock_platform: Runtime) -> None:
    task_items = [n for n in rt_mock_platform.evaluate('//item:ListItem') if isinstance(n, UiNode)]
    assert {n.name for n in task_items} >= {
        'Analyze Project Status',
        'Run Tests',
        'Generate Report',
        'Validate Results',
    }

    tree_items = [n for n in rt_mock_platform.evaluate('//item:TreeItem') if isinstance(n, UiNode)]
    assert {n.name for n in tree_items} >= {
        'Dashboard',
        'Overview',
        'Metrics',
        'Reports',
        'Täglich',
        'Monatlich',
        'Jährlich',
    }
