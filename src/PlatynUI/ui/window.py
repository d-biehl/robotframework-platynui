# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Window` context base for top-level windows."""

from ..core import patterns
from ..core.predicate import predicate
from ..core.settings import Settings
from ..core.types import Point, Size
from .control import Control

__all__ = ['Frame', 'Window']


class Window(Control):
    """Context base for top-level windows."""

    default_role = 'Window'

    @property
    def is_active(self) -> bool:
        """Whether the window is currently the active foreground window."""
        state = self.adapter.get_pattern(patterns.WindowState, raise_exception=False)
        return state.is_active if state is not None else False

    @property
    def is_topmost(self) -> bool:
        """Whether the window is currently in always-on-top mode."""
        state = self.adapter.get_pattern(patterns.WindowState, raise_exception=False)
        return state.is_topmost if state is not None else False

    @property
    def is_modal(self) -> bool:
        """Whether the window is modal (blocks input to other windows)."""
        state = self.adapter.get_pattern(patterns.WindowState, raise_exception=False)
        return state.is_modal if state is not None else False

    @property
    def is_minimized(self) -> bool:
        """Whether the window is currently minimized."""
        minimizable = self.adapter.get_pattern(patterns.Minimizable, raise_exception=False)
        return minimizable.is_minimized if minimizable is not None else False

    @property
    def is_maximized(self) -> bool:
        """Whether the window is currently maximized."""
        maximizable = self.adapter.get_pattern(patterns.Maximizable, raise_exception=False)
        return maximizable.is_maximized if maximizable is not None else False

    @property
    def title(self) -> str:
        """The window title (reads ``control:Name`` directly)."""
        return self.name

    @predicate('window {0} can be minimized')
    def _window_can_minimize(self) -> bool:
        pattern = self.adapter.get_pattern(patterns.Minimizable, raise_exception=False)
        return pattern is not None and pattern.can_minimize

    @predicate('window {0} can be maximized')
    def _window_can_maximize(self) -> bool:
        pattern = self.adapter.get_pattern(patterns.Maximizable, raise_exception=False)
        return pattern is not None and pattern.can_maximize

    @predicate('window {0} can be closed')
    def _window_can_close(self) -> bool:
        pattern = self.adapter.get_pattern(patterns.Closeable, raise_exception=False)
        return pattern is not None and pattern.can_close

    @predicate('window {0} can be moved')
    def _window_can_move(self) -> bool:
        pattern = self.adapter.get_pattern(patterns.Movable, raise_exception=False)
        return pattern is not None and pattern.can_move

    @predicate('window {0} can be resized')
    def _window_can_resize(self) -> bool:
        pattern = self.adapter.get_pattern(patterns.Resizable, raise_exception=False)
        return pattern is not None and pattern.can_resize

    @predicate('window {0} is minimized')
    def _window_is_minimized(self) -> bool:
        return self.is_minimized

    @predicate('window {0} is maximized')
    def _window_is_maximized(self) -> bool:
        return self.is_maximized

    @predicate('window {0} is not minimized and not maximized')
    def _window_is_restored(self) -> bool:
        return not self.is_minimized and not self.is_maximized

    @predicate('window {0} is gone')
    def _window_is_gone(self) -> bool:
        return not self.exists()

    def activate(self) -> None:
        """Bring the window to the foreground and give it focus."""
        self.ensure_that(self._application_is_ready)
        if self.is_active:
            return
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._toplevel_parent_is_active)

    def minimize(self) -> None:
        """Minimize the window."""
        self.ensure_that(self._application_is_ready, self._window_can_minimize)
        self.adapter.get_pattern(patterns.Minimizable).minimize()
        self.ensure_that(self._window_is_minimized)

    def maximize(self) -> None:
        """Maximize the window."""
        self.ensure_that(self._application_is_ready, self._window_can_maximize)
        self.adapter.get_pattern(patterns.Maximizable).maximize()
        self.ensure_that(self._window_is_maximized)

    def restore(self) -> None:
        """Restore the window from its minimized or maximized state."""
        self.ensure_that(self._application_is_ready)
        self.adapter.get_pattern(patterns.Restorable).restore()
        self.ensure_that(self._window_is_restored)

    def close(self, timeout: float | None = None) -> None:
        """Close the window and wait until it disappears."""
        self.ensure_that(self._application_is_ready, self._window_can_close)
        self.adapter.get_pattern(patterns.Closeable).close()
        wait = timeout if timeout is not None else Settings.current().window_close_timeout
        self.ensure_that(self._window_is_gone, timeout=wait)

    def move_to(self, point: Point) -> None:
        """Move the window so its top-left corner is at ``point``."""
        self.ensure_that(self._application_is_ready, self._window_can_move)
        self.adapter.get_pattern(patterns.Movable).move_to(point)

    def resize(self, size: Size) -> None:
        """Resize the window to ``size``."""
        self.ensure_that(self._application_is_ready, self._window_can_resize)
        self.adapter.get_pattern(patterns.Resizable).resize(size)


class Frame(Window):
    """Marker subclass of `Window` for toolkits that distinguish Frame from Window."""

    default_role = 'Frame'
