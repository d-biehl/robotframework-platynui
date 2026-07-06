# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for the BareMetal ``Pointer Scroll`` keyword.

The keyword is a thin wrapper over ``Runtime.pointer_scroll``; its own logic is the
``direction`` + ``ticks`` → native ``(horizontal, vertical)`` delta mapping and the
move-over-the-target-before-scrolling ordering. Both are exercised here against a
``MagicMock`` runtime, so no display or real pointer device is involved — the effect against
a real scrollable container is covered by the egui acceptance suite. One notch is 120 units;
a *negative* component scrolls the visually intended way on each axis, so ``DOWN`` and
``RIGHT`` are negative (see ``_SCROLL_AXIS_SIGN`` in the library).
"""

from typing import Any, cast
from unittest.mock import MagicMock, call

import pytest
from platynui_native import Point, PointerOverridesLike

from PlatynUI.BareMetal import BareMetal

_WHEEL = 120.0

# (direction, single-tick (horizontal, vertical) delta)
_DIRECTIONS = [
    ('DOWN', (0.0, -_WHEEL)),
    ('UP', (0.0, _WHEEL)),
    ('LEFT', (_WHEEL, 0.0)),
    ('RIGHT', (-_WHEEL, 0.0)),
]


@pytest.fixture
def scroll() -> tuple[BareMetal, MagicMock]:
    """A BareMetal instance whose runtime is a fake, with window activation disabled.

    Assigning through ``__dict__`` shadows the ``runtime`` cached-property with the mock, so the
    keyword talks to the fake instead of building a native runtime.
    """
    library = BareMetal(auto_activate=False)
    fake = MagicMock(name='FakeRuntime')
    library.__dict__['runtime'] = fake
    return library, fake


def test_default_is_one_notch_down(scroll: tuple[BareMetal, MagicMock]) -> None:
    library, runtime = scroll
    library.pointer_scroll()
    runtime.pointer_move_to.assert_not_called()
    runtime.pointer_scroll.assert_called_once_with((0.0, -_WHEEL), None)


@pytest.mark.parametrize(('direction', 'unit_delta'), _DIRECTIONS)
def test_direction_and_ticks_map_to_delta(
    scroll: tuple[BareMetal, MagicMock],
    direction: str,
    unit_delta: tuple[float, float],
) -> None:
    library, runtime = scroll
    library.pointer_scroll(direction=cast(Any, direction), ticks=3)
    horizontal, vertical = unit_delta
    runtime.pointer_scroll.assert_called_once_with((horizontal * 3, vertical * 3), None)


def test_no_target_scrolls_at_current_position(scroll: tuple[BareMetal, MagicMock]) -> None:
    library, runtime = scroll
    library.pointer_scroll(None, direction=cast(Any, 'UP'))
    runtime.pointer_move_to.assert_not_called()
    runtime.pointer_scroll.assert_called_once_with((0.0, _WHEEL), None)


def test_target_moves_over_point_before_scrolling(scroll: tuple[BareMetal, MagicMock]) -> None:
    library, runtime = scroll
    library.pointer_scroll(x=400.0, y=300.0, direction=cast(Any, 'DOWN'))
    # The move must come first (the wheel acts on the widget under the cursor), then the scroll.
    assert runtime.mock_calls == [
        call.pointer_move_to(Point(400.0, 300.0), None),
        call.pointer_scroll((0.0, -_WHEEL), None),
    ]


def test_overrides_are_forwarded_to_both_calls(scroll: tuple[BareMetal, MagicMock]) -> None:
    library, runtime = scroll
    overrides: PointerOverridesLike = {'scroll_delay_ms': 5.0}
    library.pointer_scroll(x=10.0, y=20.0, direction=cast(Any, 'RIGHT'), ticks=2, overrides=overrides)
    runtime.pointer_move_to.assert_called_once_with(Point(10.0, 20.0), overrides)
    runtime.pointer_scroll.assert_called_once_with((-2 * _WHEEL, 0.0), overrides)


def test_invalid_direction_raises_before_scrolling(scroll: tuple[BareMetal, MagicMock]) -> None:
    library, runtime = scroll
    with pytest.raises(ValueError, match='Invalid scroll direction'):
        library.pointer_scroll(direction=cast(Any, 'DIAGONAL'))
    runtime.pointer_scroll.assert_not_called()
