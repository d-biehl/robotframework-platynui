# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for the pattern ABCs in ``PlatynUI.core.patterns``.

The Python pattern set mirrors the Rust ``pattern_ids`` and attribute
groups one-to-one (``crates/core/src/ui/identifiers.rs``,
``crates/core/src/ui/attributes.rs``). Capability-group patterns
(``Element``, ``Activatable``, ``Toggleable``) bundle related attributes
and actions instead of splitting them into per-attribute markers.
"""

from typing import Any

import pytest

from PlatynUI.core.patterns import (
    Activatable,
    ActivationTarget,
    Clearable,
    Element,
    Focusable,
    PatternBase,
    Point,
    Rect,
    TextContent,
    TextEditable,
    Toggleable,
    ToggleState,
)

ALL_PATTERNS: list[type[PatternBase]] = [
    Activatable,
    ActivationTarget,
    Clearable,
    Element,
    Focusable,
    TextContent,
    TextEditable,
    Toggleable,
]


@pytest.mark.parametrize('pattern_cls', ALL_PATTERNS)
def test_each_pattern_has_reverse_dns_name(pattern_cls: type[PatternBase]) -> None:
    name = pattern_cls.pattern_name
    assert name.startswith('org.platynui.patterns.')
    assert name.endswith(pattern_cls.__name__)


@pytest.mark.parametrize('pattern_cls', ALL_PATTERNS)
def test_each_pattern_inherits_from_base(pattern_cls: type[PatternBase]) -> None:
    assert issubclass(pattern_cls, PatternBase)


@pytest.mark.parametrize('pattern_cls', ALL_PATTERNS)
def test_each_pattern_is_abstract(pattern_cls: type[PatternBase]) -> None:
    cls: Any = pattern_cls
    with pytest.raises(TypeError):
        cls()


def test_pattern_names_are_unique() -> None:
    names = [cls.pattern_name for cls in ALL_PATTERNS]
    assert len(names) == len(set(names))


def test_no_properties_pattern_exported() -> None:
    """``Properties``/``NativeProperties`` pattern was deliberately removed.

    Generic key/value reads go through ``adapter.attribute_value(name,
    namespace=...)`` instead. See design doc §A.4 / §5.
    """
    import PlatynUI.core.patterns as patterns_pkg

    assert not hasattr(patterns_pkg, 'Properties')
    assert not hasattr(patterns_pkg, 'NativeProperties')


def test_legacy_split_patterns_are_gone() -> None:
    """The pre-consolidation patterns (``HasBounds``, ``Visibility``,
    ``HasIsEnabled``, ``HasIsReadonly``, ``HasToggleState``,
    ``EditableText``) collapsed into capability-group patterns
    (``Element``, ``Toggleable``, ``TextEditable``)."""
    import PlatynUI.core.patterns as patterns_pkg

    for legacy in (
        'HasBounds',
        'Visibility',
        'HasIsEnabled',
        'HasIsReadonly',
        'HasToggleState',
        'EditableText',
    ):
        assert not hasattr(patterns_pkg, legacy), f'{legacy} should no longer be exported'


def test_pattern_names_match_rust_ids() -> None:
    """Reverse-DNS pattern names align with the Rust ``pattern_ids`` module
    (see ``crates/core/src/ui/identifiers.rs``).
    """
    expected = {
        Activatable: 'org.platynui.patterns.Activatable',
        ActivationTarget: 'org.platynui.patterns.ActivationTarget',
        Clearable: 'org.platynui.patterns.Clearable',
        Element: 'org.platynui.patterns.Element',
        Focusable: 'org.platynui.patterns.Focusable',
        TextContent: 'org.platynui.patterns.TextContent',
        TextEditable: 'org.platynui.patterns.TextEditable',
        Toggleable: 'org.platynui.patterns.Toggleable',
    }
    for cls, name in expected.items():
        assert cls.pattern_name == name


def test_concrete_activatable_implementation() -> None:
    class Btn(Activatable):
        def __init__(self) -> None:
            self.activated = False

        def activate(self) -> None:
            self.activated = True

        @property
        def is_activation_enabled(self) -> bool:
            return True

        @property
        def default_accelerator(self) -> str | None:
            return 'Enter'

    btn = Btn()
    btn.activate()
    assert btn.activated is True
    assert btn.is_activation_enabled is True
    assert btn.default_accelerator == 'Enter'


def test_concrete_element_implementation() -> None:
    class Box(Element):
        @property
        def bounds(self) -> Rect:
            return Rect(10.0, 20.0, 4.0, 8.0)

        @property
        def is_visible(self) -> bool:
            return True

        @property
        def is_in_view(self) -> bool:
            return True

        @property
        def is_enabled(self) -> bool:
            return True

    b = Box()
    assert b.bounds == Rect(10.0, 20.0, 4.0, 8.0)
    assert b.is_visible is True
    assert b.is_in_view is True
    assert b.is_enabled is True


def test_concrete_toggleable_implementation() -> None:
    class Check(Toggleable):
        def __init__(self) -> None:
            self._state = ToggleState.OFF

        def toggle(self) -> None:
            self._state = ToggleState.ON if self._state is ToggleState.OFF else ToggleState.OFF

        @property
        def state(self) -> ToggleState:
            return self._state

        @property
        def supports_three_state(self) -> bool:
            return False

    c = Check()
    assert c.state is ToggleState.OFF
    c.toggle()
    state_after_toggle: ToggleState = c.state
    assert state_after_toggle is ToggleState.ON
    assert c.supports_three_state is False


def test_concrete_focusable_implementation() -> None:
    class Entry(Focusable):
        def __init__(self) -> None:
            self._focused = False

        @property
        def is_focused(self) -> bool:
            return self._focused

        def focus(self) -> None:
            self._focused = True

    e = Entry()
    assert e.is_focused is False
    e.focus()
    assert e.is_focused is True


def test_concrete_text_editable_implementation() -> None:
    class Input(TextEditable):
        def __init__(self) -> None:
            self.value = ''

        def set_text(self, value: str) -> None:
            self.value = value

        @property
        def is_readonly(self) -> bool:
            return False

        @property
        def max_length(self) -> int | None:
            return 256

        @property
        def supports_password_mode(self) -> bool:
            return True

        @property
        def is_multi_line(self) -> bool:
            return False

    i = Input()
    i.set_text('hello')
    assert i.value == 'hello'
    assert i.max_length == 256
    assert i.supports_password_mode is True
    assert i.is_readonly is False


def test_concrete_text_content_implementation() -> None:
    class Label(TextContent):
        @property
        def text(self) -> str:
            return 'hi'

    lbl = Label()
    assert lbl.text == 'hi'


def test_concrete_clearable_implementation() -> None:
    class Field(Clearable):
        def __init__(self) -> None:
            self.cleared = False

        def clear(self) -> None:
            self.cleared = True

    f = Field()
    f.clear()
    assert f.cleared is True


def test_concrete_activation_target_implementation() -> None:
    """Adapter overrides may surface ``ActivationArea`` and
    ``ActivationHint``; the ABC defaults both to ``None``."""

    class Btn(ActivationTarget):
        @property
        def activation_point(self) -> Point:
            return Point(40.0, 25.0)

    class RichBtn(ActivationTarget):
        @property
        def activation_point(self) -> Point:
            return Point(40.0, 25.0)

        @property
        def activation_area(self) -> Rect | None:
            return Rect(30.0, 20.0, 20.0, 10.0)

        @property
        def activation_hint(self) -> str | None:
            return 'click ribbon expand chevron'

    minimal = Btn()
    assert minimal.activation_point == Point(40.0, 25.0)
    assert minimal.activation_area is None
    assert minimal.activation_hint is None

    rich = RichBtn()
    assert rich.activation_point == Point(40.0, 25.0)
    assert rich.activation_area == Rect(30.0, 20.0, 20.0, 10.0)
    assert rich.activation_hint == 'click ribbon expand chevron'


def test_toggle_state_values() -> None:
    assert {ToggleState.OFF, ToggleState.ON, ToggleState.INDETERMINATE} == set(ToggleState)


def test_point_and_rect_come_from_native_module() -> None:
    """Geometry primitives are re-exported from ``platynui_native`` so that
    Python and Rust share a single representation (no FFI conversion)."""
    assert Point.__module__ == 'platynui_native'
    assert Rect.__module__ == 'platynui_native'
