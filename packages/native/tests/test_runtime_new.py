#!/usr/bin/env python3
"""Test new Runtime methods: evaluate_single() and providers()."""

from platynui_native import UiNode, Runtime


def test_evaluate_single(rt_mock_platform: Runtime) -> None:
    """Test that evaluate_single returns only the first result."""

    # Test with XPath that returns nodes
    result = rt_mock_platform.evaluate_single("/")

    if isinstance(result, UiNode):
        assert result.role == "Desktop"

    # Test with no results
    no_result = rt_mock_platform.evaluate_single("//NonExistentElement")
    assert no_result is None



def test_providers(rt_mock_platform: Runtime) -> None:
    """Test that providers() returns provider information."""
    providers = rt_mock_platform.providers()

    for _ in enumerate(providers, 1):
        pass

    # Verify structure
    assert isinstance(providers, list)
    if providers:
        assert "id" in providers[0]
        assert "display_name" in providers[0]
        assert "technology" in providers[0]
        assert "kind" in providers[0]



def test_evaluate_iter(rt_mock_platform: Runtime) -> None:
    """Test that evaluate_iter returns an iterator."""
    # Test with XPath that returns nodes
    result_iter = rt_mock_platform.evaluate_iter("/")

    # Check that it's an iterator
    assert hasattr(result_iter, "__iter__")
    assert hasattr(result_iter, "__next__")

    # Consume the iterator
    results = list(result_iter)

    if results:
        first = results[0]
        if isinstance(first, UiNode):
            assert first.role == "Desktop"

    # Test with no results
    empty_iter = rt_mock_platform.evaluate_iter("//NonExistentElement")
    empty_results = list(empty_iter)
    assert len(empty_results) == 0



if __name__ == "__main__":
    # Execute via pytest to ensure fixtures are applied
    import sys

    sys.exit("Please run this module with pytest.")
