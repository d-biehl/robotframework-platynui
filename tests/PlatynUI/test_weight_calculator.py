# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.weight_calculator``."""

import re
from typing import Any

from PlatynUI.core import WeightCalculator


class _MockTechnology:
    pass


class _OtherTechnology:
    pass


class _MockAdapter:
    """Minimal duck-typed adapter for weight-calculator tests.

    Mirrors the post-Phase-1 adapter shape: namespaced attribute reads
    via ``attribute_value(name, namespace)``; no ``Properties`` /
    ``NativeProperties`` patterns.
    """

    def __init__(
        self,
        *,
        technology: object | None = None,
        role: str | None = None,
        supported_roles: list[str] | None = None,
        framework_id: str | None = None,
        class_name: str | None = None,
        tag_name: str | None = None,
        attributes: dict[tuple[str, str], Any] | None = None,
    ) -> None:
        self.technology = technology if technology is not None else _MockTechnology()
        self.role = role
        self.supported_roles = supported_roles or []
        self.framework_id = framework_id
        self.class_name = class_name
        self.tag_name = tag_name
        self._attributes = attributes or {}
        self.supported_patterns: list[str] = []

    def get_pattern(self, pattern_name: str) -> Any:
        raise AssertionError(f'unexpected pattern {pattern_name!r}')

    def attribute_value(self, name: str, namespace: str = 'control') -> Any:
        return self._attributes.get((namespace, name))


def test_no_criteria_returns_zero() -> None:
    calc = WeightCalculator(_MockAdapter())
    assert calc.calculate({}) == 0


def test_technology_match() -> None:
    calc = WeightCalculator(_MockAdapter(technology=_MockTechnology()))
    assert calc.calculate({'technology': _MockTechnology}) == 100_000


def test_technology_mismatch_returns_zero() -> None:
    calc = WeightCalculator(_MockAdapter(technology=_MockTechnology()))
    assert calc.calculate({'technology': _OtherTechnology}) == 0


def test_role_exact_match() -> None:
    calc = WeightCalculator(_MockAdapter(role='Button'))
    assert calc.calculate({'role': 'Button'}) == 10_000


def test_role_supported_match_decays_by_index() -> None:
    calc = WeightCalculator(
        _MockAdapter(role='Control', supported_roles=['Control', 'Button', 'Toggle'])
    )
    # "Toggle" is at index 2 → 5000 - 2 = 4998
    assert calc.calculate({'role': 'Toggle'}) == 4998


def test_role_no_match_returns_zero() -> None:
    calc = WeightCalculator(_MockAdapter(role='Button', supported_roles=['Button']))
    assert calc.calculate({'role': 'Slider'}) == 0


def test_class_name_regex_match() -> None:
    calc = WeightCalculator(_MockAdapter(class_name='WindowsForms10.BUTTON.app.0.123abc'))
    assert calc.calculate({'class_name': re.compile(r'WindowsForms10\.BUTTON\..*')}) == 500


def test_attributes_default_namespace_string_keys() -> None:
    """Bare-string criteria keys default to the ``control`` namespace."""
    calc = WeightCalculator(
        _MockAdapter(
            role='Button',
            attributes={
                ('control', 'AutomationId'): 'btn1',
                ('control', 'Name'): 'OK',
            },
        )
    )
    weight = calc.calculate(
        {'role': 'Button', 'attributes': {'AutomationId': 'btn1', 'Name': 'OK'}}
    )
    assert weight == 10_000 + 200 + 200


def test_attributes_explicit_namespace_tuple_keys() -> None:
    """Tuple keys in criteria pick the namespace explicitly."""
    calc = WeightCalculator(
        _MockAdapter(attributes={('native', 'HWND'): '0x12AB'})
    )
    assert calc.calculate({'attributes': {('native', 'HWND'): '0x12AB'}}) == 200


def test_attribute_mismatch_returns_zero() -> None:
    calc = WeightCalculator(
        _MockAdapter(role='Button', attributes={('control', 'AutomationId'): 'x'})
    )
    assert calc.calculate({'attributes': {'AutomationId': 'y'}}) == 0


def test_default_attribute_namespace_override() -> None:
    """``WeightCalculator(default_attribute_namespace=...)`` lifts the default."""
    calc = WeightCalculator(
        _MockAdapter(attributes={('item', 'Index'): '3'}),
        default_attribute_namespace='item',
    )
    # Bare-string criterion key now resolves into the 'item' namespace.
    assert calc.calculate({'attributes': {'Index': '3'}}) == 200


class _CountingAdapter(_MockAdapter):
    """Adapter that counts ``attribute_value`` invocations."""

    def __init__(self, **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self.fetches = 0

    def attribute_value(self, name: str, namespace: str = 'control') -> Any:
        self.fetches += 1
        return super().attribute_value(name, namespace)


def test_caching_reuses_results() -> None:
    adapter = _CountingAdapter(role='Button', attributes={('control', 'k'): 'v'})

    calc = WeightCalculator(adapter)
    calc.calculate({'attributes': {'k': 'v'}})
    calc.calculate({'attributes': {'k': 'v'}})
    # Second invocation must hit the cache; only one adapter fetch.
    assert adapter.fetches == 1
