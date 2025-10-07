#!/usr/bin/env python3
"""Test new Runtime methods: evaluate_single() and providers()."""

from platynui_native import Runtime, UiNode

def test_evaluate_single():
    """Test that evaluate_single returns only the first result."""
    rt = Runtime()

    # Test with XPath that returns nodes
    result = rt.evaluate_single("/")
    print(f"evaluate_single('/') returned: {type(result).__name__}")

    if isinstance(result, UiNode):
        print(f"  Single node: {result.name} ({result.role})")
        assert result.role == "Desktop"
    else:
        print(f"  Result: {result}")

    # Test with no results
    no_result = rt.evaluate_single("//NonExistentElement")
    print(f"\nevaluate_single('//NonExistentElement') returned: {no_result}")
    assert no_result is None

    print("  ✅ evaluate_single() works correctly!\n")

def test_providers():
    """Test that providers() returns provider information."""
    rt = Runtime()

    providers = rt.providers()
    print(f"providers() returned {len(providers)} provider(s):")

    for i, provider in enumerate(providers, 1):
        print(f"  {i}. {provider['display_name']}")
        print(f"     ID: {provider['id']}")
        print(f"     Technology: {provider['technology']}")
        print(f"     Kind: {provider['kind']}")

    # Verify structure
    assert isinstance(providers, list)
    if providers:
        assert 'id' in providers[0]
        assert 'display_name' in providers[0]
        assert 'technology' in providers[0]
        assert 'kind' in providers[0]

    print("\n  ✅ providers() works correctly!\n")

def test_evaluate_iter():
    """Test that evaluate_iter returns an iterator."""
    rt = Runtime()

    # Test with XPath that returns nodes
    result_iter = rt.evaluate_iter("/")
    print(f"evaluate_iter('/') returned: {type(result_iter).__name__}")

    # Check that it's an iterator
    assert hasattr(result_iter, '__iter__')
    assert hasattr(result_iter, '__next__')

    # Consume the iterator
    results = list(result_iter)
    print(f"  Iterator yielded {len(results)} result(s)")

    if results:
        first = results[0]
        print(f"  First result: {type(first).__name__}")
        if isinstance(first, UiNode):
            print(f"    Node: {first.name} ({first.role})")
            assert first.role == "Desktop"

    # Test with no results
    empty_iter = rt.evaluate_iter("//NonExistentElement")
    empty_results = list(empty_iter)
    print(f"\nevaluate_iter('//NonExistentElement') yielded {len(empty_results)} result(s)")
    assert len(empty_results) == 0

    print("\n  ✅ evaluate_iter() works correctly!\n")

if __name__ == "__main__":
    print("=" * 60)
    print("Testing New Runtime Methods")
    print("=" * 60 + "\n")

    test_evaluate_single()
    test_providers()

    print("=" * 60)
    print("🎉 All new Runtime methods work!")
    print("=" * 60)
