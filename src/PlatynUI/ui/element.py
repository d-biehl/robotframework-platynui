# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""`Element` page-object base for visible UI elements."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Final, override

from ..core import patterns
from ..core.context import ContextBase
from ..core.devices import (
    AdapterKeyboardProxy,
    AdapterMouseProxy,
    KeyboardAction,
    MouseAction,
)
from ..core.predicate import predicate
from ..core.runtime import runtime
from ..core.types import Rect

if TYPE_CHECKING:
    from .application import Application
    from .window import Window

__all__ = ['Element']


# Sentinel for the lazy `_application` cache slot. ``None`` is a valid
# resolved value (no enclosing Application), so a separate marker is needed.
_UNRESOLVED: Final = object()


class _ElementMouseProxy(AdapterMouseProxy):
    """Mouse proxy that ensures the owning element is interactable before each action."""

    def __init__(self, element: 'Element') -> None:
        super().__init__(element.adapter)
        self._element = element

    @override
    def before_action(self, action: MouseAction) -> None:
        super().before_action(action)
        self._element.ensure_that(
            self._element._toplevel_parent_is_active,
            self._element._element_is_in_view,
            self._element._element_is_enabled,
        )


class _ElementKeyboardProxy(AdapterKeyboardProxy):
    """Keyboard proxy that ensures the owning element is in view before each action.

    `Element` does not require focus for keyboard input; that
    constraint is added by `Control._ControlKeyboardProxy`.
    """

    def __init__(self, element: 'Element') -> None:
        super().__init__(element.adapter)
        self._element = element

    @override
    def before_action(self, action: KeyboardAction) -> None:
        super().before_action(action)
        self._element.ensure_that(
            self._element._toplevel_parent_is_active,
            self._element._element_is_visible,
            self._element._element_is_in_view,
        )


class Element(ContextBase, register=False):
    """Page-object base for visible UI elements.

    Wraps the `Element` capability pattern and exposes element
    geometry, visibility, enabled and read-only state. Provides
    `mouse` and `keyboard` proxies that automatically wait
    for the element to be interactable.
    """

    default_prefix = 'element'

    # Per-instance lazy slots
    _mouse_proxy: _ElementMouseProxy | None
    _keyboard_proxy: _ElementKeyboardProxy | None
    _application_cache: 'Application | None | object'

    def __init__(
        self,
        locator: object | None = None,
        *,
        context_parent: 'ContextBase | None' = None,
        adapter: object | None = None,
    ) -> None:
        super().__init__(locator, context_parent=context_parent, adapter=adapter)  # type: ignore[arg-type]
        self._mouse_proxy = None
        self._keyboard_proxy = None
        self._application_cache = _UNRESOLVED

    @override
    def invalidate(self) -> None:
        super().invalidate()
        self._mouse_proxy = None
        self._keyboard_proxy = None
        self._application_cache = _UNRESOLVED

    # ------------------------------------------------------------------
    # Adapter pass-through properties
    # ------------------------------------------------------------------

    @property
    def bounds(self) -> Rect:
        """The element's screen rectangle in absolute pixels."""
        return self.adapter.get_pattern(patterns.Element).bounds

    @property
    def is_visible(self) -> bool:
        """Whether the element is currently rendered."""
        return self.adapter.get_pattern(patterns.Element).is_visible

    @property
    def is_enabled(self) -> bool:
        """Whether the element currently accepts user input."""
        return self.adapter.get_pattern(patterns.Element).is_enabled

    @property
    def is_in_view(self) -> bool:
        """Whether the element lies within its container's viewport."""
        return self.adapter.get_pattern(patterns.Element).is_in_view

    @property
    def is_readonly(self) -> bool:
        """Whether the element is read-only.

        Convenience shortcut over the `Readable` pattern; defaults
        to ``False`` when the adapter does not expose `Readable`.
        """
        readable = self.adapter.get_pattern(patterns.Readable, raise_exception=False)
        return readable.is_readonly if readable is not None else False

    # ------------------------------------------------------------------
    # Tree navigation
    # ------------------------------------------------------------------

    @property
    def top_level_parent(self) -> 'Element':
        """The outermost `Element` ancestor (direct child of `DesktopBase`).

        Returns `self` when the element is itself a top-level element.
        """
        from .desktopbase import DesktopBase

        current: Element = self
        while True:
            parent = current.parent
            if parent is None or isinstance(parent, DesktopBase):
                return current
            if not isinstance(parent, Element):
                return current
            current = parent

    @property
    def parent_window(self) -> 'Window | None':
        """The nearest `Window` ancestor, or ``None`` when there is none."""
        from .window import Window

        node = self.parent
        while node is not None:
            if isinstance(node, Window):
                return node
            node = node.parent
        return None

    # ------------------------------------------------------------------
    # Devices
    # ------------------------------------------------------------------

    @property
    def mouse(self) -> _ElementMouseProxy:
        """Mouse proxy bound to this element with interactability checks."""
        if self._mouse_proxy is None:
            self._mouse_proxy = self._create_mouse_proxy()
        return self._mouse_proxy

    @property
    def keyboard(self) -> _ElementKeyboardProxy:
        """Keyboard proxy bound to this element with in-view checks."""
        if self._keyboard_proxy is None:
            self._keyboard_proxy = self._create_keyboard_proxy()
        return self._keyboard_proxy

    def _create_mouse_proxy(self) -> _ElementMouseProxy:
        """Build the mouse proxy. Override to install a custom proxy."""
        return _ElementMouseProxy(self)

    def _create_keyboard_proxy(self) -> _ElementKeyboardProxy:
        """Build the keyboard proxy. Override to install a custom proxy."""
        return _ElementKeyboardProxy(self)

    # ------------------------------------------------------------------
    # Predicates
    # ------------------------------------------------------------------

    @predicate('application for {0} is ready')
    def _application_is_ready(self) -> bool:
        """Whether the owning application is ready to accept input.

        Combines the top-level adapter's `HasUserInput` pattern with
        an optional user-defined `Application.is_ready()` check. The
        check runs on the top-level parent and is delegated up the
        tree from non-top-level elements.
        """
        top = self.top_level_parent
        if self is not top:
            return top._application_is_ready()
        pattern = self.adapter.get_pattern(patterns.HasUserInput, raise_exception=False)
        pattern_says = pattern.accepts_user_input() if pattern is not None else None
        if self._application_cache is _UNRESOLVED:
            self._application_cache = self._resolve_application()
        app = self._application_cache
        from .application import Application

        user_says = app.is_ready() if isinstance(app, Application) else None
        return pattern_says is not False and user_says is not False

    @predicate('element {0} is visible')
    def _element_is_visible(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.is_visible

    @predicate('element {0} is in view')
    def _element_is_in_view(self) -> bool:
        self.ensure_that(self._element_is_visible)
        # Pragmatic: no BringIntoViewable pattern yet — honest fail
        # when out-of-view. A future BringIntoViewable.bring_into_view()
        # call goes here before the read.
        return self.is_in_view

    @predicate('element {0} is enabled')
    def _element_is_enabled(self) -> bool:
        self.ensure_that(self._element_is_visible)
        return self.is_enabled

    @predicate('element {0} is not readonly')
    def _element_is_not_readonly(self) -> bool:
        self.ensure_that(self._element_is_enabled)
        return not self.is_readonly

    @predicate('top-level parent of element {0} is active')
    def _toplevel_parent_is_active(self) -> bool:
        """Activate the top-level parent if it is not already active."""
        top = self.top_level_parent
        # Lazy import: Window/DesktopBase live in sibling modules.
        from .desktopbase import DesktopBase
        from .window import Window

        if isinstance(top, DesktopBase):
            return True
        if isinstance(top, Window):
            if top.is_active:
                return True
            top.activate()
            return top.is_active
        # Top-level is a plain Element — no activation contract; assume active.
        return True

    # ------------------------------------------------------------------
    # Convenience methods
    # ------------------------------------------------------------------

    def activate_parent_window(self) -> None:
        """Activate the nearest `Window` ancestor, if any."""
        pw = self.parent_window
        if pw is not None:
            pw.activate()

    def bring_to_view(self) -> bool:
        """Bring the element into view, activating its top-level parent first."""
        return self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
        )

    def highlight(self, duration: float = 3.0) -> None:
        """Flash the element's bounds for ``duration`` seconds."""
        self.ensure_that(self._element_is_in_view)
        runtime.current.highlight(rects=[self.bounds], duration_ms=int(duration * 1000))

    def get_screenshot(self) -> bytes:
        """Capture the element's bounds as PNG-encoded bytes."""
        self._before_get_screenshot()
        return runtime.current.screenshot(rect=self.bounds)

    def save_screenshot(self, path: str | Path) -> Path:
        """Capture the element's bounds and write the PNG to ``path``."""
        out = Path(path)
        out.write_bytes(self.get_screenshot())
        return out

    def _before_get_screenshot(self) -> None:
        """Hook called before screenshot capture; ensures the element is in view."""
        self.ensure_that(self._element_is_in_view)

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    def _resolve_application(self) -> 'Application | None':
        """Walk up the context tree to find the enclosing `Application`."""
        from .application import Application

        node = self.parent
        while node is not None:
            if isinstance(node, Application):
                return node
            node = node.parent
        return None
