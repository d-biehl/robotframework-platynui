# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Mouse and keyboard wrappers for UI elements.

Two abstract proxy classes form the public surface:

- `MouseProxy` — element-relative pointer actions (move, press,
  release, click, double-click) with anchor- and offset-based targeting.
- `KeyboardProxy` — keyboard input accepting sequence strings
  like ``"hello<Enter>"`` or ``"<Ctrl+Shift+T>"``.

Concrete implementations (`AdapterMouseProxy`,
`AdapterKeyboardProxy`) attach a proxy to a UI adapter; custom
subclasses can override `before_action` /
`after_action` to inject pre- and post-conditions
(focus, scroll-into-view, verification).
"""

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Callable, TypeAlias, override

from platynui_native import PointerButton

from . import patterns
from .runtime import runtime
from .types import Point, Rect

if TYPE_CHECKING:
    from .adapter_proxy import AdapterFacade

__all__ = [
    'AdapterKeyboardProxy',
    'AdapterMouseProxy',
    'Anchor',
    'KeyboardAction',
    'KeyboardProxy',
    'MouseAction',
    'MouseButton',
    'MouseProxy',
    'VirtualPoint',
]


#: Pointer button identifier. Alias for `PointerButton`
#: providing the standard members ``LEFT``, ``RIGHT``, ``MIDDLE``, ``X1``, ``X2``.
MouseButton: TypeAlias = PointerButton

_LOGGER = logging.getLogger('platynui.devices')


# ----------------------------------------------------------------------
# Action enums
# ----------------------------------------------------------------------


class MouseAction(str, Enum):
    """Identifies which mouse action triggered a ``before``/``after`` hook."""

    MOVE = 'move'
    PRESS = 'press'
    RELEASE = 'release'
    CLICK = 'click'
    DOUBLE_CLICK = 'double_click'

    def __str__(self) -> str:
        return self.value


class KeyboardAction(str, Enum):
    """Identifies which keyboard action triggered a ``before``/``after`` hook."""

    TYPE = 'type'
    PRESS = 'press'
    RELEASE = 'release'

    def __str__(self) -> str:
        return self.value


# ----------------------------------------------------------------------
# VirtualPoint and Anchor
# ----------------------------------------------------------------------


@dataclass(frozen=True)
class VirtualPoint:
    """A named function that derives a `Point` from a `Rect`.

    Use this to define custom anchors beyond the nine predefined ones in
    `Anchor`. The ``func`` receives the element's bounding box and
    must return an absolute desktop point.

    Example::

        quarter = VirtualPoint('quarter', lambda r: Point(
            r.x + r.width / 4, r.y + r.height / 4))
        proxy.click(pos=quarter)
    """

    name: str
    func: Callable[[Rect], Point]

    def resolve(self, rect: Rect) -> Point:
        """Compute the absolute point for the given bounding box."""
        return self.func(rect)

    def __repr__(self) -> str:  # pragma: no cover - trivial
        return f'VirtualPoint({self.name!r})'


def _anchor_top(rect: Rect) -> Point:
    return Point(rect.x + rect.width / 2, rect.y)


def _anchor_top_right(rect: Rect) -> Point:
    return Point(rect.right(), rect.y)


def _anchor_left(rect: Rect) -> Point:
    return Point(rect.x, rect.y + rect.height / 2)


def _anchor_right(rect: Rect) -> Point:
    return Point(rect.right(), rect.y + rect.height / 2)


def _anchor_bottom_left(rect: Rect) -> Point:
    return Point(rect.x, rect.bottom())


def _anchor_bottom(rect: Rect) -> Point:
    return Point(rect.x + rect.width / 2, rect.bottom())


def _anchor_bottom_right(rect: Rect) -> Point:
    return Point(rect.right(), rect.bottom())


class Anchor:
    """Predefined `VirtualPoint` instances for the nine standard
    positions inside an element's bounding box.

    Pass any of these as the ``pos`` argument to `MouseProxy`
    actions to target a specific corner, edge midpoint, or the centre::

        proxy.click(pos=Anchor.TOP_RIGHT)
        proxy.move_to(Anchor.CENTER, x=5, y=-2)  # 5px right, 2px up of center
    """

    TOP_LEFT = VirtualPoint('top_left', lambda r: r.position())
    TOP = VirtualPoint('top', _anchor_top)
    TOP_RIGHT = VirtualPoint('top_right', _anchor_top_right)
    LEFT = VirtualPoint('left', _anchor_left)
    CENTER = VirtualPoint('center', lambda r: r.center())
    RIGHT = VirtualPoint('right', _anchor_right)
    BOTTOM_LEFT = VirtualPoint('bottom_left', _anchor_bottom_left)
    BOTTOM = VirtualPoint('bottom', _anchor_bottom)
    BOTTOM_RIGHT = VirtualPoint('bottom_right', _anchor_bottom_right)


# ----------------------------------------------------------------------
# MouseProxy
# ----------------------------------------------------------------------


class MouseProxy(ABC):
    """Element-relative mouse interface.

    Subclass and implement `base_rect` to bind the proxy to an
    element. All actions accept the same targeting arguments:

    - ``pos`` selects the base point inside `base_rect`:

      * ``None`` (default) — use `default_click_position`
      * `Anchor` / `VirtualPoint` — anchor inside the box
      * `Point` — offset from the box's top-left corner

    - ``x`` / ``y`` add a final pixel offset on top of the base point.

    Override `before_action` / `after_action` to plug in
    pre- and post-conditions (focus, visibility checks, verification).

    Example::

        class MyProxy(MouseProxy):
            @property
            def base_rect(self) -> Rect:
                return self._element.bounds

        MyProxy().click()                              # centre
        MyProxy().click(pos=Anchor.TOP_RIGHT, x=-3)    # 3px left of top-right
        MyProxy().click(button=MouseButton.RIGHT)      # right-click centre
    """

    @property
    @abstractmethod
    def base_rect(self) -> Rect:
        """Bounding box of the target element in absolute desktop coordinates."""

    @property
    def default_click_position(self) -> Point:
        """The point clicked when ``pos`` is ``None``. Defaults to the
        centre of `base_rect`; subclasses may override to use a
        more specific location."""
        return self.base_rect.center()

    # -- hooks ---------------------------------------------------------

    def before_action(self, action: MouseAction) -> None:
        """Hook called immediately before the runtime executes ``action``.

        No-op by default. Override to enforce pre-conditions such as
        bringing the window to the foreground or scrolling the element
        into view.
        """

    def after_action(self, action: MouseAction) -> None:
        """Hook called immediately after the runtime executes ``action``.

        No-op by default. Override to verify the action's effect.
        """

    # -- coordinate resolution ----------------------------------------

    def _resolve_point(
        self,
        pos: Point | VirtualPoint | None,
        x: float | None,
        y: float | None,
    ) -> Point:
        if pos is None:
            base = self.default_click_position
        elif isinstance(pos, VirtualPoint):
            base = pos.resolve(self.base_rect)
        else:
            base = self.base_rect.position() + pos
        if x or y:
            return base.translate(x or 0.0, y or 0.0)
        return base

    # -- actions -------------------------------------------------------

    def move_to(
        self,
        pos: Point | VirtualPoint | None = None,
        *,
        x: float | None = None,
        y: float | None = None,
    ) -> Point:
        """Move the pointer to the resolved target. Returns the absolute
        point the pointer was moved to."""
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.MOVE)
        runtime.current.pointer_move_to(target)
        self.after_action(MouseAction.MOVE)
        return target

    def press(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        """Move to the target and press ``button`` without releasing."""
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.PRESS)
        runtime.current.pointer_press(target, button)
        self.after_action(MouseAction.PRESS)

    def release(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        """Move to the target and release a previously pressed ``button``."""
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.RELEASE)
        runtime.current.pointer_release(target, button)
        self.after_action(MouseAction.RELEASE)

    def click(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        times: int = 1,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        """Click ``button`` ``times`` times at the resolved target.

        ``times > 1`` produces a multi-click respecting the platform's
        double-click interval.
        """
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.CLICK)
        if times == 1:
            runtime.current.pointer_click(target, button)
        else:
            runtime.current.pointer_multi_click(target, times, button)
        self.after_action(MouseAction.CLICK)

    def double_click(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        """Double-click ``button`` at the resolved target."""
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.DOUBLE_CLICK)
        runtime.current.pointer_multi_click(target, 2, button)
        self.after_action(MouseAction.DOUBLE_CLICK)


class AdapterMouseProxy(MouseProxy):
    """Standard `MouseProxy` bound to a UI adapter.

    Reads the bounding box from the adapter's ``Element`` pattern and
    determines the default click position from the adapter's patterns,
    preferring an explicit activation target over the element centre:

    1. Centre of ``ActivationTarget.activation_area`` if set.
    2. ``ActivationTarget.activation_point`` if the pattern is supported.
    3. ``Element.default_click_position`` otherwise.

    When the adapter exposes an ``ActivationTarget.activation_hint``,
    each action logs it on DEBUG via the ``platynui.devices`` logger.
    """

    def __init__(self, adapter: 'AdapterFacade') -> None:
        self._adapter = adapter

    @property
    @override
    def base_rect(self) -> Rect:
        return self._adapter.get_pattern(patterns.Element).bounds

    @property
    @override
    def default_click_position(self) -> Point:
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            target = self._adapter.get_pattern(patterns.ActivationTarget)
            if target.activation_area is not None:
                return target.activation_area.center()
            return target.activation_point
        return self._adapter.get_pattern(patterns.Element).default_click_position

    @override
    def before_action(self, action: MouseAction) -> None:
        if not _LOGGER.isEnabledFor(logging.DEBUG):
            return
        if not self._adapter.supports_pattern(patterns.ActivationTarget):
            return
        hint = self._adapter.get_pattern(patterns.ActivationTarget).activation_hint
        if hint:
            _LOGGER.debug('mouse %s: %s', action.value, hint)


# ----------------------------------------------------------------------
# KeyboardProxy
# ----------------------------------------------------------------------


class KeyboardProxy(ABC):
    """Keyboard input interface.

    All three methods accept a single sequence string with the same
    syntax: literal text plus angle-bracket key names and modifier
    combinations.

    Examples::

        proxy.type_keys('Hello, World!<Enter>')
        proxy.type_keys('<Ctrl+A><Delete>')
        proxy.press_keys('<Shift>')        # hold Shift down
        proxy.type_keys('abc')             # types ABC
        proxy.release_keys('<Shift>')      # release Shift

    Override `before_action` / `after_action` for hooks
    around each action (e.g. to ensure the target window has focus).
    """

    def before_action(self, action: KeyboardAction) -> None:
        """Hook called immediately before the runtime executes ``action``.
        No-op by default."""

    def after_action(self, action: KeyboardAction) -> None:
        """Hook called immediately after the runtime executes ``action``.
        No-op by default."""

    def type_keys(self, sequence: str) -> None:
        """Type ``sequence`` — press and release each key in order."""
        self.before_action(KeyboardAction.TYPE)
        runtime.current.keyboard_type(sequence)
        self.after_action(KeyboardAction.TYPE)

    def press_keys(self, sequence: str) -> None:
        """Press the keys in ``sequence`` without releasing them."""
        self.before_action(KeyboardAction.PRESS)
        runtime.current.keyboard_press(sequence)
        self.after_action(KeyboardAction.PRESS)

    def release_keys(self, sequence: str) -> None:
        """Release the keys in ``sequence`` (in reverse order)."""
        self.before_action(KeyboardAction.RELEASE)
        runtime.current.keyboard_release(sequence)
        self.after_action(KeyboardAction.RELEASE)


class AdapterKeyboardProxy(KeyboardProxy):
    """Standard `KeyboardProxy` bound to a UI adapter.

    The adapter reference is held so subclasses can implement focus
    and verification logic via `before_action` /
    `after_action`.
    """

    def __init__(self, adapter: 'AdapterFacade') -> None:
        self._adapter = adapter
