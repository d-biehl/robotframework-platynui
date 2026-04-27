# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""`Application` identity-container context."""

import logging
import os
import sys
import time
from typing import TYPE_CHECKING

from ..core.context import ContextBase
from ..core.settings import Settings

if TYPE_CHECKING:
    from .window import Window

__all__ = ['Application']


_LOGGER = logging.getLogger('platynui.ui.application')


class Application(ContextBase):
    """Context representing a running application as the parent of its windows."""

    default_role = 'Application'
    default_prefix = 'app'

    @property
    def process_id(self) -> int:
        """The OS process ID of the application."""
        value = self.attribute_value('ProcessId', namespace='app')
        if not isinstance(value, int):
            raise TypeError(
                f'expected int for ProcessId, got {type(value).__name__}',
            )
        return value

    @property
    def process_name(self) -> str:
        """The OS process name of the application."""
        value = self.attribute_value('ProcessName', namespace='app')
        if not isinstance(value, str):
            raise TypeError(
                f'expected str for ProcessName, got {type(value).__name__}',
            )
        return value

    def is_ready(self) -> bool:
        """Whether the application is ready to accept user input."""
        return True

    def exit(self, timeout: float | None = None) -> None:
        """Exit the application, attempting graceful shutdown then killing."""
        if timeout is None:
            timeout = Settings.current().application_exit_timeout
        try:
            self._request_exit()
        except Exception as exc:  # graceful path may legitimately fail
            _LOGGER.debug('graceful exit failed for %r: %s', self, exc)
        self._force_exit(timeout)
        self.invalidate()

    # ------------------------------------------------------------------
    # Override hooks
    # ------------------------------------------------------------------

    def _request_exit(self) -> None:
        """Graceful shutdown. Default closes all top-level windows."""
        for window in self._top_level_windows():
            try:
                window.close()
            except Exception as exc:
                _LOGGER.debug('failed to close %r: %s', window, exc)

    def _force_exit(self, timeout: float) -> None:
        """Poll the process, kill after ``timeout`` seconds."""
        try:
            pid = self.process_id
        except Exception:
            # Adapter is gone — application has already exited.
            return
        if pid <= 0:
            return
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if not _process_alive(pid):
                return
            time.sleep(0.1)
        _LOGGER.warning(
            'application %r did not exit within %.1fs; killing pid=%d',
            self,
            timeout,
            pid,
        )
        _kill_process(pid)

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    def _top_level_windows(self) -> list['Window']:
        """Collect every direct-child `Window` context."""
        from .window import Window

        result: list[Window] = []
        for child in self.children:
            if isinstance(child, Window):
                result.append(child)
        return result


# ----------------------------------------------------------------------
# Process-control helpers
# ----------------------------------------------------------------------


def _process_alive(pid: int) -> bool:
    """Return whether the given OS process is still alive."""
    if sys.platform == 'win32':
        return _process_alive_windows(pid)
    return _process_alive_posix(pid)


def _process_alive_posix(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        # Process exists but we cannot signal it.
        return True
    return True


def _process_alive_windows(pid: int) -> bool:  # pragma: no cover - platform-specific
    import ctypes
    from ctypes import wintypes

    PROCESS_QUERY_LIMITED_INFORMATION = 0x1000  # noqa: N806 (Win32 constant)
    STILL_ACTIVE = 259  # noqa: N806 (Win32 constant)

    kernel32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return False
    try:
        exit_code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
            return False
        return exit_code.value == STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def _kill_process(pid: int) -> None:
    """Forcefully terminate the given OS process."""
    if sys.platform == 'win32':
        _kill_process_windows(pid)
        return
    import signal

    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        return
    except PermissionError as exc:
        _LOGGER.error('permission denied killing pid=%d: %s', pid, exc)


def _kill_process_windows(pid: int) -> None:  # pragma: no cover - platform-specific
    import ctypes

    PROCESS_TERMINATE = 0x0001  # noqa: N806 (Win32 constant)

    kernel32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
    handle = kernel32.OpenProcess(PROCESS_TERMINATE, False, pid)
    if not handle:
        return
    try:
        kernel32.TerminateProcess(handle, 1)
    finally:
        kernel32.CloseHandle(handle)
