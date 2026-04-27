# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0
#
# mypy: disable-error-code="type-abstract"

"""Unit tests for ``PlatynUI.core.devices``.

The module under test is a *thin* coordinate-resolving wrapper around
the Rust runtime.  Tests therefore install a ``MagicMock`` runtime via
``runtime.override(...)`` and assert which Rust methods got called with
which absolute coordinates — there is no need (and no value) in
exercising the real OS pointer subsystem here.
"""

import logging
from collections.abc import Iterator
from typing import Any
from unittest.mock import MagicMock

import pytest

from PlatynUI.core import patterns
from PlatynUI.core.devices import (
    AdapterMouseProxy,
    Anchor,
    KeyboardAction,
    KeyboardProxy,
    MouseAction,
    MouseButton,
    MouseProxy,
    VirtualPoint,
)
from PlatynUI.core.runtime import runtime
from PlatynUI.core.types import Point, Rect

# ----------------------------------------------------------------------
# Helpers — no real adapter needed; we feed a stub object whose
# pattern-resolution methods return whatever the test wants.
# ----------------------------------------------------------------------


class _ElementStub(patterns.Element):
    def __init__(self, bounds: Rect) -> None:
        self._bounds = bounds

    @property
    def bounds(self) -> Rect:
        return self._bounds

    @property
    def is_visible(self) -> bool:
        return True

    @property
    def is_in_view(self) -> bool:
        return True

    @property
    def is_enabled(self) -> bool:
        return True


class _ActivationTargetStub(patterns.ActivationTarget):
    def __init__(
        self,
        point: Point,
        *,
        area: Rect | None = None,
        hint: str | None = None,
    ) -> None:
        self._point = point
        self._area = area
        self._hint = hint

    @property
    def activation_point(self) -> Point:
        return self._point

    @property
    def activation_area(self) -> Rect | None:
        return self._area

    @property
    def activation_hint(self) -> str | None:
        return self._hint


class _StubAdapter:
    """Minimal adapter shape consumed by :class:`AdapterMouseProxy`."""

    def __init__(
        self,
        element: patterns.Element,
        activation: patterns.ActivationTarget | None = None,
    ) -> None:
        self._element = element
        self._activation = activation

    def supports_pattern(self, pattern_type: type) -> bool:
        if pattern_type is patterns.Element:
            return True
        if pattern_type is patterns.ActivationTarget:
            return self._activation is not None
        return False

    def get_pattern(self, pattern_type: type, *, raise_exception: bool = True) -> Any:
        if pattern_type is patterns.Element:
            return self._element
        if pattern_type is patterns.ActivationTarget and self._activation is not None:
            return self._activation
        if raise_exception:
            raise LookupError(pattern_type)
        return None


@pytest.fixture
def native_runtime() -> Iterator[MagicMock]:
    """Install a ``MagicMock`` runtime so device proxies talk to a stub."""
    fake = MagicMock(name='FakeNativeRuntime')
    with runtime.override(lambda: fake):
        yield fake


# ----------------------------------------------------------------------
# VirtualPoint and Anchor geometry
# ----------------------------------------------------------------------


@pytest.mark.parametrize(
    ('anchor', 'expected'),
    [
        (Anchor.TOP_LEFT, Point(10.0, 20.0)),
        (Anchor.TOP, Point(60.0, 20.0)),
        (Anchor.TOP_RIGHT, Point(110.0, 20.0)),
        (Anchor.LEFT, Point(10.0, 45.0)),
        (Anchor.CENTER, Point(60.0, 45.0)),
        (Anchor.RIGHT, Point(110.0, 45.0)),
        (Anchor.BOTTOM_LEFT, Point(10.0, 70.0)),
        (Anchor.BOTTOM, Point(60.0, 70.0)),
        (Anchor.BOTTOM_RIGHT, Point(110.0, 70.0)),
    ],
)
def test_anchor_resolves_against_rect(anchor: VirtualPoint, expected: Point) -> None:
    rect = Rect(10.0, 20.0, 100.0, 50.0)
    assert anchor.resolve(rect) == expected


# ----------------------------------------------------------------------
# MouseProxy._resolve_point — the heart of the module
# ----------------------------------------------------------------------


class _BareMouseProxy(MouseProxy):
    """Trivial proxy with a fixed bounds for resolver tests."""

    def __init__(self, rect: Rect) -> None:
        self._rect = rect

    @property
    def base_rect(self) -> Rect:
        return self._rect


def test_resolve_none_uses_default_click_position(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(10.0, 20.0, 100.0, 50.0))
    # Default = bounds.center = (60, 45)
    assert proxy.move_to() == Point(60.0, 45.0)


def test_resolve_none_with_offsets(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(10.0, 20.0, 100.0, 50.0))
    assert proxy.move_to(x=5.0, y=-3.0) == Point(65.0, 42.0)


def test_resolve_virtual_point_uses_anchor(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(10.0, 20.0, 100.0, 50.0))
    assert proxy.move_to(Anchor.TOP_LEFT) == Point(10.0, 20.0)
    assert proxy.move_to(Anchor.TOP_LEFT, x=2.0, y=3.0) == Point(12.0, 23.0)


def test_resolve_concrete_point_is_relative_to_top_left(native_runtime: MagicMock) -> None:
    """A bare ``Point`` is treated as offset from the element's top-left."""
    proxy = _BareMouseProxy(Rect(10.0, 20.0, 100.0, 50.0))
    assert proxy.move_to(Point(7.0, 4.0)) == Point(17.0, 24.0)
    assert proxy.move_to(Point(7.0, 4.0), x=1.0, y=1.0) == Point(18.0, 25.0)


# ----------------------------------------------------------------------
# MouseProxy actions — delegate to runtime with absolute coords
# ----------------------------------------------------------------------


def test_move_to_calls_pointer_move_to(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(10.0, 20.0, 100.0, 50.0))
    target = proxy.move_to(Anchor.TOP_RIGHT)
    assert target == Point(110.0, 20.0)
    native_runtime.pointer_move_to.assert_called_once_with(Point(110.0, 20.0))


def test_click_default_button_is_left(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(0.0, 0.0, 50.0, 50.0))
    proxy.click()
    native_runtime.pointer_click.assert_called_once_with(Point(25.0, 25.0), MouseButton.LEFT)


def test_click_with_explicit_button_and_offset(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(0.0, 0.0, 50.0, 50.0))
    proxy.click(button=MouseButton.RIGHT, x=2.0, y=-2.0)
    native_runtime.pointer_click.assert_called_once_with(Point(27.0, 23.0), MouseButton.RIGHT)


def test_click_times_two_uses_multi_click(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(0.0, 0.0, 10.0, 10.0))
    proxy.click(times=3)
    native_runtime.pointer_click.assert_not_called()
    native_runtime.pointer_multi_click.assert_called_once_with(Point(5.0, 5.0), 3, MouseButton.LEFT)


def test_double_click_uses_multi_click_two(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(0.0, 0.0, 10.0, 10.0))
    proxy.double_click()
    native_runtime.pointer_multi_click.assert_called_once_with(Point(5.0, 5.0), 2, MouseButton.LEFT)


def test_press_and_release_pass_button(native_runtime: MagicMock) -> None:
    proxy = _BareMouseProxy(Rect(0.0, 0.0, 10.0, 10.0))
    proxy.press(button=MouseButton.MIDDLE)
    proxy.release(button=MouseButton.MIDDLE)
    native_runtime.pointer_press.assert_called_once_with(Point(5.0, 5.0), MouseButton.MIDDLE)
    native_runtime.pointer_release.assert_called_once_with(Point(5.0, 5.0), MouseButton.MIDDLE)


def test_before_after_action_invoked(native_runtime: MagicMock) -> None:
    """Hooks fire around every action with the matching ``MouseAction``."""
    seen: list[tuple[str, MouseAction]] = []

    class HookedProxy(_BareMouseProxy):
        def before_action(self, action: MouseAction) -> None:
            seen.append(('before', action))

        def after_action(self, action: MouseAction) -> None:
            seen.append(('after', action))

    HookedProxy(Rect(0.0, 0.0, 10.0, 10.0)).click()
    assert seen == [('before', MouseAction.CLICK), ('after', MouseAction.CLICK)]


# ----------------------------------------------------------------------
# AdapterMouseProxy — fallback chain and logging
# ----------------------------------------------------------------------


def test_adapter_default_click_falls_back_to_element_when_no_activation() -> None:
    """Without ``ActivationTarget`` we fall back to ``Element.bounds.center()``."""
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    proxy = AdapterMouseProxy(_StubAdapter(elem))  # type: ignore[arg-type]
    assert proxy.default_click_position == Point(50.0, 25.0)


def test_adapter_default_click_uses_activation_point_when_no_area() -> None:
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    target = _ActivationTargetStub(Point(40.0, 25.0))
    proxy = AdapterMouseProxy(_StubAdapter(elem, target))  # type: ignore[arg-type]
    assert proxy.default_click_position == Point(40.0, 25.0)


def test_adapter_default_click_prefers_activation_area_center() -> None:
    """Fallback chain: ``activation_area.center()`` wins over
    ``activation_point`` when both are set."""
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    target = _ActivationTargetStub(
        Point(40.0, 25.0),
        area=Rect(60.0, 30.0, 20.0, 10.0),
    )
    proxy = AdapterMouseProxy(_StubAdapter(elem, target))  # type: ignore[arg-type]
    assert proxy.default_click_position == Point(70.0, 35.0)


def test_adapter_logs_activation_hint_on_debug(
    native_runtime: MagicMock,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """``before_action`` logs the hint at DEBUG level when present."""
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    target = _ActivationTargetStub(Point(40.0, 25.0), hint='ribbon expand chevron')
    proxy = AdapterMouseProxy(_StubAdapter(elem, target))  # type: ignore[arg-type]

    with caplog.at_level(logging.DEBUG, logger='platynui.devices'):
        proxy.click()

    matching = [r for r in caplog.records if 'ribbon expand chevron' in r.getMessage()]
    assert len(matching) == 1
    assert matching[0].levelno == logging.DEBUG
    assert 'click' in matching[0].getMessage()


def test_adapter_silent_when_hint_is_none(
    native_runtime: MagicMock,
    caplog: pytest.LogCaptureFixture,
) -> None:
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    target = _ActivationTargetStub(Point(40.0, 25.0))  # hint=None
    proxy = AdapterMouseProxy(_StubAdapter(elem, target))  # type: ignore[arg-type]

    with caplog.at_level(logging.DEBUG, logger='platynui.devices'):
        proxy.click()

    assert caplog.records == []


def test_adapter_silent_when_no_activation_target(
    native_runtime: MagicMock,
    caplog: pytest.LogCaptureFixture,
) -> None:
    elem = _ElementStub(Rect(0.0, 0.0, 100.0, 50.0))
    proxy = AdapterMouseProxy(_StubAdapter(elem))  # type: ignore[arg-type]

    with caplog.at_level(logging.DEBUG, logger='platynui.devices'):
        proxy.click()

    assert caplog.records == []


# ----------------------------------------------------------------------
# KeyboardProxy and AdapterKeyboardProxy
# ----------------------------------------------------------------------


class _BareKeyboardProxy(KeyboardProxy):
    pass


def test_keyboard_type_delegates_to_runtime(native_runtime: MagicMock) -> None:
    _BareKeyboardProxy().type_keys('hello<Enter>')
    native_runtime.keyboard_type.assert_called_once_with('hello<Enter>')


def test_keyboard_press_and_release_delegate(native_runtime: MagicMock) -> None:
    kb = _BareKeyboardProxy()
    kb.press_keys('<Control+A>')
    kb.release_keys('<Control+A>')
    native_runtime.keyboard_press.assert_called_once_with('<Control+A>')
    native_runtime.keyboard_release.assert_called_once_with('<Control+A>')


def test_keyboard_hooks_fire_around_actions(native_runtime: MagicMock) -> None:
    seen: list[tuple[str, KeyboardAction]] = []

    class Hooked(KeyboardProxy):
        def before_action(self, action: KeyboardAction) -> None:
            seen.append(('before', action))

        def after_action(self, action: KeyboardAction) -> None:
            seen.append(('after', action))

    Hooked().type_keys('x')
    assert seen == [('before', KeyboardAction.TYPE), ('after', KeyboardAction.TYPE)]
