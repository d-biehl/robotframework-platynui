# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false, reportUnusedFunction=false, reportUnnecessaryTypeIgnoreComment=false
#
# Tests verify internal state (call counts on stub patterns) and pass
# private predicates to ``ensure_that``. Pytest fixtures look unused to
# pyright; the ``_ui_helpers`` import only needs the ignore under mypy.

"""Unit tests for ``PlatynUI.ui.buttons``."""

from collections.abc import Iterator

import pytest
from _ui_helpers import (  # type: ignore[import-not-found]
    ActivatableStub,
    ElementStub,
    FocusableStub,
    HasUserInputStub,
    ReadableStub,
    TextContentStub,
    ToggleableStub,
    make_adapter,
)

from PlatynUI.core import patterns
from PlatynUI.core.adapter import Adapter
from PlatynUI.core.exceptions import CannotEnsureError, PatternNotSupportedError
from PlatynUI.core.patterns import ToggleState
from PlatynUI.core.settings import Settings
from PlatynUI.ui.buttons import AbstractButton, Button, CheckBox

# ---------------------------------------------------------------------------
# Common fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _fast_settings() -> Iterator[None]:
    """Shrink ensure timeouts so failing predicates do not stall the suite."""
    with Settings(
        ensure_timeout=0.05,
        ensure_delay=0.0,
        exists_timeout=0.05,
        wait_for_timeout=0.05,
        wait_for_delay=0.0,
    ):
        yield


def _button_adapter(
    *,
    role: str = 'Button',
    extra: dict[type, object] | None = None,
    is_focused: bool = True,
) -> Adapter:
    """Build a button adapter parented to an active Window/Desktop chain.

    The window-parent has ``HasUserInput=True`` and a focused
    ``Focusable`` so the button's ``_application_is_ready`` and
    ``_toplevel_parent_is_active`` predicates pass without further
    setup.
    """
    desktop = make_adapter(role='Desktop')
    window = make_adapter(
        role='Window',
        parent=desktop,
        pattern_map={
            patterns.Element: ElementStub(),
            patterns.HasUserInput: HasUserInputStub(True),
            patterns.Focusable: FocusableStub(is_focused=is_focused),
        },
    )
    pmap: dict[type, object] = {patterns.Element: ElementStub()}
    if extra:
        pmap.update(extra)
    return make_adapter(  # type: ignore[no-any-return]
        role=role, parent=window, pattern_map=pmap,
    )


# ---------------------------------------------------------------------------
# AbstractButton — registration opt-out, text-property
# ---------------------------------------------------------------------------


def test_abstract_button_is_not_auto_registered() -> None:
    """`register=False` keeps the abstract base out of the context registry."""
    from PlatynUI.core.context import ContextFactory

    abstract_adapter = make_adapter(role='AbstractButton')
    cls = ContextFactory().find_context_class_for(abstract_adapter)
    assert cls is not AbstractButton


def test_button_text_returns_text_content_pattern_value() -> None:
    adapter = _button_adapter(extra={patterns.TextContent: TextContentStub('OK')})
    assert Button(adapter=adapter).text == 'OK'


def test_button_text_defaults_to_empty_string_when_pattern_missing() -> None:
    assert Button(adapter=_button_adapter()).text == ''


# ---------------------------------------------------------------------------
# Button.activate() — Activatable pattern path
# ---------------------------------------------------------------------------


def test_button_activate_invokes_activatable_pattern() -> None:
    activatable = ActivatableStub()
    adapter = _button_adapter(extra={patterns.Activatable: activatable})
    Button(adapter=adapter).activate()
    assert activatable.activate_calls == 1


def test_button_activate_runs_predicates_before_pattern_call() -> None:
    """`_element_is_enabled` must hold before `Activatable.activate` fires."""
    activatable = ActivatableStub()
    element = ElementStub(is_enabled=False)
    adapter = _button_adapter(
        extra={patterns.Element: element, patterns.Activatable: activatable},
    )
    with pytest.raises(CannotEnsureError):
        Button(adapter=adapter).activate()
    assert activatable.activate_calls == 0


def test_button_activate_raises_when_activatable_pattern_missing() -> None:
    """`activate` requires the provider to expose `Activatable`."""
    with pytest.raises(PatternNotSupportedError):
        Button(adapter=_button_adapter()).activate()


def test_button_registered_with_role_button() -> None:
    """`@__init_subclass__` auto-registration must use the class name."""
    from PlatynUI.core.context import ContextFactory

    cls = ContextFactory().find_context_class_for(_button_adapter())
    assert cls is Button


# ---------------------------------------------------------------------------
# CheckBox — state read-back
# ---------------------------------------------------------------------------


def _checkbox_adapter(
    *,
    state: ToggleState = ToggleState.OFF,
    is_readonly: bool = False,
    cycle: tuple[ToggleState, ...] | None = None,
    supports_three_state: bool = False,
) -> tuple[Adapter, ToggleableStub]:
    toggleable = ToggleableStub(
        state, cycle=cycle, supports_three_state=supports_three_state,
    )
    adapter = _button_adapter(
        role='CheckBox',
        extra={
            patterns.Toggleable: toggleable,
            patterns.Readable: ReadableStub(is_readonly=is_readonly),
        },
    )
    return adapter, toggleable


def test_checkbox_state_returns_toggleable_state() -> None:
    adapter, _ = _checkbox_adapter(state=ToggleState.ON)
    assert CheckBox(adapter=adapter).state is ToggleState.ON


def test_checkbox_is_checked_true_when_state_on() -> None:
    adapter, _ = _checkbox_adapter(state=ToggleState.ON)
    assert CheckBox(adapter=adapter).is_checked is True


def test_checkbox_is_checked_false_when_state_off() -> None:
    adapter, _ = _checkbox_adapter(state=ToggleState.OFF)
    assert CheckBox(adapter=adapter).is_checked is False


def test_checkbox_is_unchecked_true_when_state_off() -> None:
    adapter, _ = _checkbox_adapter(state=ToggleState.OFF)
    assert CheckBox(adapter=adapter).is_unchecked is True


def test_checkbox_is_unchecked_false_when_state_on() -> None:
    adapter, _ = _checkbox_adapter(state=ToggleState.ON)
    assert CheckBox(adapter=adapter).is_unchecked is False


def test_checkbox_state_raises_when_toggleable_pattern_missing() -> None:
    with pytest.raises(PatternNotSupportedError):
        _ = CheckBox(adapter=_button_adapter(role='CheckBox')).state


# ---------------------------------------------------------------------------
# CheckBox.toggle() / check() / uncheck() / set_state()
# ---------------------------------------------------------------------------


def test_checkbox_toggle_invokes_toggleable_once() -> None:
    adapter, toggleable = _checkbox_adapter(state=ToggleState.OFF)
    CheckBox(adapter=adapter).toggle()
    assert toggleable.toggle_calls == 1
    assert toggleable.state is ToggleState.ON


def test_checkbox_toggle_blocks_on_readonly_predicate() -> None:
    adapter, toggleable = _checkbox_adapter(state=ToggleState.OFF, is_readonly=True)
    with pytest.raises(CannotEnsureError):
        CheckBox(adapter=adapter).toggle()
    assert toggleable.toggle_calls == 0


def test_checkbox_check_no_op_when_already_on() -> None:
    adapter, toggleable = _checkbox_adapter(state=ToggleState.ON)
    CheckBox(adapter=adapter).check()
    assert toggleable.toggle_calls == 0


def test_checkbox_check_toggles_until_state_on() -> None:
    adapter, toggleable = _checkbox_adapter(state=ToggleState.OFF)
    CheckBox(adapter=adapter).check()
    assert toggleable.state is ToggleState.ON
    assert toggleable.toggle_calls == 1


def test_checkbox_uncheck_toggles_until_state_off() -> None:
    adapter, toggleable = _checkbox_adapter(state=ToggleState.ON)
    CheckBox(adapter=adapter).uncheck()
    assert toggleable.state is ToggleState.OFF
    assert toggleable.toggle_calls == 1


def test_checkbox_activate_means_check() -> None:
    """`CheckBox.activate()` is the user-intent „abhaken", not Toggleable.toggle."""
    adapter, toggleable = _checkbox_adapter(state=ToggleState.OFF)
    CheckBox(adapter=adapter).activate()
    assert toggleable.state is ToggleState.ON


def test_checkbox_set_state_handles_three_state_cycle() -> None:
    """Reach ``INDETERMINATE`` from ``ON`` in a tri-state toggle (one toggle call)."""
    cycle = (ToggleState.OFF, ToggleState.ON, ToggleState.INDETERMINATE)
    adapter, toggleable = _checkbox_adapter(
        state=ToggleState.ON, cycle=cycle, supports_three_state=True,
    )
    CheckBox(adapter=adapter).set_state(ToggleState.INDETERMINATE)
    assert toggleable.state is ToggleState.INDETERMINATE
    assert toggleable.toggle_calls == 1


def test_checkbox_set_state_terminates_when_cycle_skips_target() -> None:
    """Two-state cycle skipping ``INDETERMINATE`` returns after `len(ToggleState)` tries."""
    adapter, toggleable = _checkbox_adapter(state=ToggleState.OFF)
    CheckBox(adapter=adapter).set_state(ToggleState.INDETERMINATE)
    # Two-state cycle never reaches INDETERMINATE; loop bound by len(ToggleState)=3.
    assert toggleable.toggle_calls == 3
    assert toggleable.state is not ToggleState.INDETERMINATE


def test_checkbox_registered_with_role_checkbox() -> None:
    from PlatynUI.core.context import ContextFactory

    adapter, _ = _checkbox_adapter()
    cls = ContextFactory().find_context_class_for(adapter)
    assert cls is CheckBox
