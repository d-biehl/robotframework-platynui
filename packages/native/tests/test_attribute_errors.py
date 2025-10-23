import platynui_native as pn


def test_missing_attribute_raises_specific_error(rt_mock_platform: pn.Runtime) -> None:
    node = rt_mock_platform.desktop_node()
    try:
        node.attribute('DoesNotExist', 'control')
    except pn.AttributeNotFoundError as exc:
        msg = str(exc)
        assert 'attribute not found' in msg
        assert 'control:DoesNotExist' in msg
    else:
        raise AssertionError('AttributeNotFoundError was not raised')

