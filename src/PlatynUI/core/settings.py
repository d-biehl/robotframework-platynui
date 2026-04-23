# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Process-wide settings singleton (design document section A.1).

``Settings`` is a frozen, slotted, keyword-only dataclass. Instances act as
context managers that temporarily replace the process-wide singleton:

>>> with Settings(ensure_timeout=30):
...     ensure_that(ctx, predicate)

For programmatic configuration without nesting, use
:meth:`Settings.set_current`. Settings are **not** thread-safe; this matches
the legacy implementation and is sufficient for ``pabot``-style parallelism
where each worker runs in its own process.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

__all__ = ['Settings']


_current: "Settings | None" = None
_stack: list['Settings'] = []


@dataclass(frozen=True, slots=True, kw_only=True)
class Settings:
    # Wait timings
    wait_for_timeout: float = 1.0
    wait_for_delay: float = 0.1
    ensure_timeout: float = 15.0
    ensure_delay: float = 0.1
    exists_timeout: float = 1.0
    window_close_timeout: float = 1.0

    # Keyboard
    input_after_input_delay: float = 0.001
    keyboard_after_press_key_delay: float = 0.01
    keyboard_after_release_key_delay: float = 0.01
    keyboard_after_press_release_delay: float = 0.05

    # Mouse
    mouse_before_next_click_delay_multiplicator: float = 1.5
    mouse_after_click_delay: float = 0.010
    mouse_multi_click_delay_multiplicator: float = 0.5
    mouse_press_release_delay: float = 0.010
    mouse_after_move_delay: float = 0.010
    mouse_move_delay: float = 0.001
    mouse_move_time: float = 0.2

    # Display / diagnostics
    display_screenshot_format: str = 'png'
    display_screenshot_quality: int = -1
    display_screenshot_basename: str = 'screenshot'
    element_highlight_time: float = 2.0
    element_highlight_ensure_timeout: float = 2.0

    @staticmethod
    def current() -> "Settings":
        """Return the active settings instance, creating a default if needed."""
        global _current
        if _current is None:
            _current = Settings()
        return _current

    @staticmethod
    def set_current(settings: "Settings") -> None:
        """Replace the process-wide singleton.

        Use within tests or library bootstrap. Within a ``with``-block,
        prefer the context-manager form so the previous value is restored.
        """
        global _current
        _current = settings

    def __enter__(self) -> "Settings":
        global _current
        _stack.append(_current if _current is not None else Settings())
        _current = self
        return self

    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> Literal[False]:
        global _current
        _current = _stack.pop()
        return False
