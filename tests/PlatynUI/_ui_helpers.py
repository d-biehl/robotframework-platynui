# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Test helpers for `PlatynUI.ui` context tests.

Provides a ``make_adapter`` factory and concrete pattern stub classes
that satisfy the abstract pattern ABCs from
`PlatynUI.core.patterns`. Adapter pattern resolution is driven by an
explicit ``patterns`` mapping handed to ``make_adapter`` so each test
controls exactly which capabilities its element exposes.
"""

from typing import Any
from unittest.mock import MagicMock

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.types import Point, Rect, Size

__all__ = [
    'ActivatableStub',
    'ClearableStub',
    'CloseableStub',
    'ElementStub',
    'ExpandableStub',
    'FocusableStub',
    'HasEditorStub',
    'ItemContainerStub',
    'MaximizableStub',
    'MinimizableStub',
    'MovableStub',
    'ReadableStub',
    'ResizableStub',
    'ResponsiveStub',
    'RestorableStub',
    'SelectableStub',
    'TextContentStub',
    'TextEditableStub',
    'ToggleableStub',
    'WindowStateStub',
    'make_adapter',
]


# ---------------------------------------------------------------------------
# Pattern stubs — concrete classes so isinstance checks work and
# abstract methods stay satisfied.
# ---------------------------------------------------------------------------


class ElementStub(patterns.Element):
    """`patterns.Element` stub with mutable bounds/visibility/enabled state."""

    def __init__(
        self,
        *,
        bounds: Rect = Rect(0.0, 0.0, 100.0, 50.0),
        is_visible: bool = True,
        is_in_view: bool = True,
        is_enabled: bool = True,
    ) -> None:
        self._bounds = bounds
        self._visible = is_visible
        self._in_view = is_in_view
        self._enabled = is_enabled

    @property
    def bounds(self) -> Rect:
        return self._bounds

    @property
    def is_visible(self) -> bool:
        return self._visible

    @property
    def is_in_view(self) -> bool:
        return self._in_view

    @property
    def is_enabled(self) -> bool:
        return self._enabled


class FocusableStub(patterns.Focusable):
    """`patterns.Focusable` stub that records ``focus()`` calls."""

    def __init__(self, *, is_focused: bool = False) -> None:
        self._focused = is_focused
        self.focus_calls = 0

    @property
    def is_focused(self) -> bool:
        return self._focused

    def focus(self) -> None:
        self.focus_calls += 1
        self._focused = True


class ActivatableStub(patterns.Activatable):
    """`patterns.Activatable` stub that records ``activate()`` calls."""

    def __init__(
        self,
        *,
        is_activation_enabled: bool = True,
        default_accelerator: str | None = None,
    ) -> None:
        self._enabled = is_activation_enabled
        self._accel = default_accelerator
        self.activate_calls = 0

    def activate(self) -> None:
        self.activate_calls += 1

    @property
    def is_activation_enabled(self) -> bool:
        return self._enabled

    @property
    def default_accelerator(self) -> str | None:
        return self._accel


class MinimizableStub(patterns.Minimizable):
    """`patterns.Minimizable` stub with mutable state and call counter."""

    def __init__(self, *, is_minimized: bool = False, can_minimize: bool = True) -> None:
        self._is_minimized = is_minimized
        self._can_minimize = can_minimize
        self.minimize_calls = 0

    @property
    def is_minimized(self) -> bool:
        return self._is_minimized

    @property
    def can_minimize(self) -> bool:
        return self._can_minimize

    def minimize(self) -> None:
        self.minimize_calls += 1
        self._is_minimized = True


class MaximizableStub(patterns.Maximizable):
    """`patterns.Maximizable` stub with mutable state and call counter."""

    def __init__(self, *, is_maximized: bool = False, can_maximize: bool = True) -> None:
        self._is_maximized = is_maximized
        self._can_maximize = can_maximize
        self.maximize_calls = 0

    @property
    def is_maximized(self) -> bool:
        return self._is_maximized

    @property
    def can_maximize(self) -> bool:
        return self._can_maximize

    def maximize(self) -> None:
        self.maximize_calls += 1
        self._is_maximized = True


class RestorableStub(patterns.Restorable):
    """`patterns.Restorable` stub that records ``restore()`` calls."""

    def __init__(self) -> None:
        self.restore_calls = 0

    def restore(self) -> None:
        self.restore_calls += 1


class CloseableStub(patterns.Closeable):
    """`patterns.Closeable` stub that records ``close()`` calls."""

    def __init__(self, *, can_close: bool = True) -> None:
        self._can_close = can_close
        self.close_calls = 0

    @property
    def can_close(self) -> bool:
        return self._can_close

    def close(self) -> None:
        self.close_calls += 1


class MovableStub(patterns.Movable):
    """`patterns.Movable` stub recording the last move target."""

    def __init__(self, *, can_move: bool = True) -> None:
        self._can_move = can_move
        self.move_calls: list[Point] = []

    @property
    def can_move(self) -> bool:
        return self._can_move

    def move_to(self, point: Point) -> None:
        self.move_calls.append(point)


class ResizableStub(patterns.Resizable):
    """`patterns.Resizable` stub recording the last resize target."""

    def __init__(self, *, can_resize: bool = True) -> None:
        self._can_resize = can_resize
        self.resize_calls: list[Size] = []

    @property
    def can_resize(self) -> bool:
        return self._can_resize

    def resize(self, size: Size) -> None:
        self.resize_calls.append(size)


class ResponsiveStub(patterns.Responsive):
    """`patterns.Responsive` stub returning a fixed answer."""

    def __init__(self, answer: bool | None = True) -> None:
        self._answer = answer

    def accepts_user_input(self) -> bool | None:
        return self._answer


class WindowStateStub(patterns.WindowState):
    """`patterns.WindowState` stub with fixed active/topmost flags."""

    def __init__(self, *, is_active: bool = True, is_topmost: bool = False) -> None:
        self._active = is_active
        self._topmost = is_topmost

    @property
    def is_active(self) -> bool:
        return self._active

    @property
    def is_topmost(self) -> bool:
        return self._topmost


class ReadableStub(patterns.Readable):
    """`patterns.Readable` stub with a fixed read-only flag."""

    def __init__(self, *, is_readonly: bool = False) -> None:
        self._readonly = is_readonly

    @property
    def is_readonly(self) -> bool:
        return self._readonly


class TextContentStub(patterns.TextContent):
    """`patterns.TextContent` stub with a fixed text value."""

    def __init__(
        self,
        text: str = '',
        *,
        locale: str = '',
        is_truncated: bool = False,
    ) -> None:
        self._text = text
        self._locale = locale
        self._truncated = is_truncated

    @property
    def text(self) -> str:
        return self._text

    @property
    def locale(self) -> str:
        return self._locale

    @property
    def is_truncated(self) -> bool:
        return self._truncated


class TextEditableStub(patterns.TextEditable):
    """`patterns.TextEditable` stub recording the last ``set_text`` value."""

    def __init__(
        self,
        *,
        is_readonly: bool = False,
        max_length: int | None = None,
        supports_password_mode: bool = False,
        is_multi_line: bool = False,
    ) -> None:
        self._readonly = is_readonly
        self._max_length = max_length
        self._password = supports_password_mode
        self._multi_line = is_multi_line
        self.set_text_calls: list[str] = []

    def set_text(self, value: str) -> None:
        self.set_text_calls.append(value)

    @property
    def is_readonly(self) -> bool:
        return self._readonly

    @property
    def max_length(self) -> int | None:
        return self._max_length

    @property
    def supports_password_mode(self) -> bool:
        return self._password

    @property
    def is_multi_line(self) -> bool:
        return self._multi_line


class ClearableStub(patterns.Clearable):
    """`patterns.Clearable` stub that records ``clear()`` calls."""

    def __init__(self) -> None:
        self.clear_calls = 0

    def clear(self) -> None:
        self.clear_calls += 1


class ToggleableStub(patterns.Toggleable):
    """`patterns.Toggleable` stub with a mutable state cycle.

    Each ``toggle()`` call advances through the cycle handed to the
    constructor (default: two-state ``OFF`` → ``ON`` → ``OFF``). Tests
    can pass a three-state cycle to exercise tri-state behaviour.
    """

    def __init__(
        self,
        state: patterns.ToggleState = patterns.ToggleState.OFF,
        *,
        cycle: tuple[patterns.ToggleState, ...] | None = None,
        supports_three_state: bool = False,
    ) -> None:
        self._state = state
        self._cycle = cycle or (patterns.ToggleState.OFF, patterns.ToggleState.ON)
        self._three_state = supports_three_state
        self.toggle_calls = 0

    @property
    def state(self) -> patterns.ToggleState:
        return self._state

    @property
    def supports_three_state(self) -> bool:
        return self._three_state

    def toggle(self) -> None:
        self.toggle_calls += 1
        idx = self._cycle.index(self._state)
        self._state = self._cycle[(idx + 1) % len(self._cycle)]


class SelectableStub(patterns.Selectable):
    """`patterns.Selectable` stub that records ``select()`` calls."""

    def __init__(self, *, is_selected: bool = False) -> None:
        self._selected = is_selected
        self.select_calls = 0

    @property
    def is_selected(self) -> bool:
        return self._selected

    def select(self) -> None:
        self.select_calls += 1
        self._selected = True


class ExpandableStub(patterns.Expandable):
    """`patterns.Expandable` stub that records expand/collapse calls."""

    def __init__(self, *, is_expanded: bool = False, can_expand: bool = True) -> None:
        self._expanded = is_expanded
        self._can_expand = can_expand
        self.expand_calls = 0
        self.collapse_calls = 0

    @property
    def can_expand(self) -> bool:
        return self._can_expand

    @property
    def is_expanded(self) -> bool:
        return self._expanded

    def expand(self) -> None:
        self.expand_calls += 1
        self._expanded = True

    def collapse(self) -> None:
        self.collapse_calls += 1
        self._expanded = False


class HasEditorStub(patterns.HasEditor):
    """`patterns.HasEditor` stub that records lifecycle calls."""

    def __init__(self) -> None:
        self.open_calls = 0
        self.accept_calls = 0
        self.cancel_calls = 0

    def open_editor(self) -> None:
        self.open_calls += 1

    def accept(self) -> None:
        self.accept_calls += 1

    def cancel(self) -> None:
        self.cancel_calls += 1


class ItemContainerStub(patterns.ItemContainer):
    """`patterns.ItemContainer` stub.

    Each count is optional; unsupplied counts raise ``NotImplementedError``
    when accessed, matching the contract for concrete container roles.
    """

    def __init__(
        self,
        *,
        item_count: int | None = None,
        row_count: int | None = None,
        column_count: int | None = None,
    ) -> None:
        self._item_count = item_count
        self._row_count = row_count
        self._column_count = column_count

    @property
    def item_count(self) -> int:
        if self._item_count is None:
            raise NotImplementedError('item_count')
        return self._item_count

    @property
    def row_count(self) -> int:
        if self._row_count is None:
            raise NotImplementedError('row_count')
        return self._row_count

    @property
    def column_count(self) -> int:
        if self._column_count is None:
            raise NotImplementedError('column_count')
        return self._column_count


# ---------------------------------------------------------------------------
# Adapter factory
# ---------------------------------------------------------------------------


def make_adapter(
    *,
    role: str = 'Window',
    name: str = '',
    class_name: str = '',
    framework_id: str = '',
    runtime_id: str = 'rid',
    valid: bool = True,
    parent: Adapter | None = None,
    children: list[Adapter] | None = None,
    supported_roles: set[str] | None = None,
    pattern_map: dict[type, Any] | None = None,
    attributes: dict[tuple[str, str], object] | None = None,
) -> Adapter:
    """Build a ``MagicMock(spec=Adapter)`` driven by an explicit pattern map.

    ``pattern_map`` keys are pattern ABC types; values are stub
    instances. ``adapter.get_pattern(T, raise_exception=...)`` then
    returns ``pattern_map[T]`` or applies the standard not-supported
    behaviour. ``attributes`` keys are ``(name, namespace)`` pairs.
    """
    a = MagicMock(spec=Adapter)
    a.role = role
    a.name = name
    a.class_name = class_name
    a.framework_id = framework_id
    a.runtime_id = runtime_id
    a.valid = valid
    a.tag_name = ''
    a.parent = parent
    a.children = children or []
    a.supported_roles = supported_roles or {role}
    a.supported_patterns = MagicMock(return_value=set(pattern_map or {}))
    a.attribute_names = MagicMock(return_value=set())
    a.attributes = MagicMock(return_value=iter(()))

    pmap = pattern_map or {}

    def supports_pattern(pattern_type: type) -> bool:
        return pattern_type in pmap

    def get_pattern(pattern_type: type, *, raise_exception: bool = True) -> Any:
        if pattern_type in pmap:
            return pmap[pattern_type]
        if raise_exception:
            from PlatynUI.core.exceptions import PatternNotSupportedError

            raise PatternNotSupportedError(pattern_type.__name__)
        return None

    a.supports_pattern = MagicMock(side_effect=supports_pattern)
    a.get_pattern = MagicMock(side_effect=get_pattern)

    attrs = attributes or {}

    def attribute_value(name: str, namespace: str = 'control') -> object:
        return attrs.get((name, namespace))

    a.attribute_value = MagicMock(side_effect=attribute_value)
    return a
