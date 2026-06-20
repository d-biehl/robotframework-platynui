"""platynui_native package.

This package provides Python bindings for PlatynUI's native Rust implementation.
All types and functions are directly exported from the native extension module.
"""

# Re-export everything from the native extension
from typing import Any, Literal, TypeAlias, TypedDict

from ._native import (
    Activatable,
    AttributeNotFoundError,
    Closeable,
    EvaluatedAttribute,
    EvaluationError,
    EvaluationIterator,
    Focusable,
    KeyboardError,
    KeyboardOverrides,
    KeyboardProfile,
    Maximizable,
    Minimizable,
    Movable,
    Namespace,
    NodeAttributesIterator,
    NodeChildrenIterator,
    PatternError,
    PlatynUiError,
    Point,
    PointerAccelerationProfile,
    PointerButton,
    PointerError,
    PointerMotionMode,
    PointerOverrides,
    PointerProfile,
    PointerSettings,
    ProviderError,
    Rect,
    Resizable,
    Responsive,
    Restorable,
    Runtime,
    RuntimeId,
    Size,
    TechnologyId,
    UiAttribute,
    UiNode,
)

# The pointer enums are built from Rust through the IntEnum functional API, which leaves them
# without a docstring; set one here so it shows in IDEs and in libdoc's data-type reference.
PointerButton.__doc__ = 'Which mouse button a pointer action uses.'
PointerMotionMode.__doc__ = (
    'The path the pointer follows to its target: ``DIRECT`` (jump straight there, no visible '
    'travel), ``LINEAR`` (a straight line), ``BEZIER`` (a curved line), ``OVERSHOOT`` (overshoot, '
    'then settle back) or ``JITTER`` (a straight line with a small wobble).'
)
PointerAccelerationProfile.__doc__ = (
    'The speed curve of a pointer move: ``CONSTANT`` (even speed), ``EASE_IN`` (start slow, speed '
    'up) or ``EASE_OUT`` (slow down towards the target).'
)

# ===== Type Aliases =====


# Like dictionaries for ergonomics
class PointDict(TypedDict):
    """A point, as a dict: ``x`` and ``y`` in pixels."""

    x: float
    y: float


class SizeDict(TypedDict):
    """A size, as a dict: ``width`` and ``height`` in pixels."""

    width: float
    height: float


class SizeShortDict(TypedDict):
    """A size, as a dict with short keys: ``w`` and ``h`` in pixels."""

    w: float
    h: float


class RectDict(TypedDict):
    """A rectangle, as a dict: top-left ``x``/``y`` plus ``width``/``height`` in pixels."""

    x: float
    y: float
    width: float
    height: float


PointLike: TypeAlias = Point | tuple[float, float] | PointDict
SizeLike: TypeAlias = Size | tuple[float, float] | SizeDict | SizeShortDict
RectLike: TypeAlias = Rect | tuple[float, float, float, float] | RectDict
OriginLike: TypeAlias = Literal['desktop'] | PointLike | RectLike
ScrollDeltaLike: TypeAlias = tuple[float, float]
PointerButtonLike: TypeAlias = PointerButton | int


class PointerOverridesDict(TypedDict, total=False):
    """Per-call pointer overrides, as a dict.

    The fields of ``PointerProfileDict`` plus the move ``origin``, but applied to a single pointer
    call instead of the whole session. All keys are optional: the keys you set are used for that
    call, and anything you omit falls back to the active pointer profile. All ``*_ms`` values are
    milliseconds, ``*_us`` microseconds.
    """

    origin: OriginLike
    motion: PointerMotionMode
    steps_per_pixel: float
    speed_factor: float
    acceleration_profile: PointerAccelerationProfile
    max_move_duration_ms: float
    move_time_per_pixel_us: float
    after_move_delay_ms: float
    after_input_delay_ms: float
    press_release_delay_ms: float
    after_click_delay_ms: float
    before_next_click_delay_ms: float
    multi_click_delay_ms: float
    overshoot_ratio: float
    overshoot_settle_steps: int
    curve_amplitude: float
    jitter_amplitude: float
    jitter_frequency: float
    ensure_move_position: bool
    ensure_move_threshold: float
    ensure_move_timeout_ms: float
    scroll_step: tuple[float, float]
    scroll_delay_ms: float


class PointerSettingsDict(TypedDict, total=False):
    """Pointer click semantics, as a dict.

    Defines how clicks are interpreted: the double-click time window
    (``double_click_time_ms``) and position tolerance (``double_click_size``, in pixels), and the
    ``default_button`` a click uses when none is named. All keys are optional; only the keys you set
    change, the rest keep the runtime defaults.
    """

    double_click_time_ms: float
    double_click_size: SizeLike
    default_button: PointerButtonLike


class PointerProfileDict(TypedDict, total=False):
    """Pointer movement and click pacing, as a dict.

    Shapes how the pointer travels to a target — the ``motion`` path, ``speed_factor``, overshoot,
    curve and jitter — and how moves and repeated clicks are paced. All ``*_ms`` values are
    milliseconds, ``*_us`` microseconds. All keys are optional; only the keys you set change, the
    rest keep the runtime defaults. The matching per-call overrides type is ``PointerOverridesDict``.
    """

    motion: PointerMotionMode
    steps_per_pixel: float
    max_move_duration_ms: float
    speed_factor: float
    acceleration_profile: PointerAccelerationProfile
    overshoot_ratio: float
    overshoot_settle_steps: int
    curve_amplitude: float
    jitter_amplitude: float
    jitter_frequency: float
    after_move_delay_ms: float
    after_input_delay_ms: float
    press_release_delay_ms: float
    after_click_delay_ms: float
    before_next_click_delay_ms: float
    multi_click_delay_ms: float
    ensure_move_position: bool
    ensure_move_threshold: float
    ensure_move_timeout_ms: float
    scroll_step: tuple[float, float]
    scroll_delay_ms: float
    move_time_per_pixel_us: float


class KeyboardProfileDict(TypedDict, total=False):
    """Keyboard timing, as a dict.

    Sets the delays (in milliseconds) around key presses and releases, between keystrokes, inside
    modifier chords, and after a whole sequence. All keys are optional; only the keys you set
    change, the rest keep the runtime defaults. The matching per-call overrides type is
    ``KeyboardOverridesDict``.
    """

    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


class KeyboardOverridesDict(TypedDict, total=False):
    """Per-call keyboard overrides, as a dict.

    The timing fields of ``KeyboardProfileDict``, but applied to a single keyboard call instead of
    the whole session. All keys are optional: the keys you set are used for that call, and anything
    you omit falls back to the active keyboard profile.
    """

    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


PointerOverridesLike: TypeAlias = PointerOverrides | PointerOverridesDict
PointerSettingsLike: TypeAlias = PointerSettings | PointerSettingsDict
PointerProfileLike: TypeAlias = PointerProfile | PointerProfileDict
KeyboardOverridesLike: TypeAlias = KeyboardOverrides | KeyboardOverridesDict
KeyboardProfileLike: TypeAlias = KeyboardProfile | KeyboardProfileDict

Primitive: TypeAlias = bool | int | float | str | None
JSONLike: TypeAlias = dict[str, Any] | list[Any]
UiValue: TypeAlias = Primitive | Point | Size | Rect | JSONLike


# Explicit __all__ for better IDE support (will be populated by stub file)
__all__ = [
    'Activatable',
    'AttributeNotFoundError',
    'Closeable',
    'EvaluatedAttribute',
    'EvaluationError',
    'EvaluationIterator',
    'Focusable',
    'KeyboardError',
    'KeyboardOverrides',
    'KeyboardOverridesDict',
    'KeyboardOverridesLike',
    'KeyboardProfile',
    'KeyboardProfileDict',
    'KeyboardProfileLike',
    'Maximizable',
    'Minimizable',
    'Movable',
    'Namespace',
    'NodeAttributesIterator',
    'NodeChildrenIterator',
    'OriginLike',
    'PatternError',
    'PlatynUiError',
    'Point',
    'PointDict',
    'PointLike',
    'PointerAccelerationProfile',
    'PointerButton',
    'PointerError',
    'PointerMotionMode',
    'PointerOverrides',
    'PointerOverridesDict',
    'PointerOverridesLike',
    'PointerProfile',
    'PointerProfileDict',
    'PointerProfileLike',
    'PointerSettings',
    'PointerSettingsDict',
    'PointerSettingsLike',
    'ProviderError',
    'Rect',
    'RectDict',
    'RectLike',
    'Resizable',
    'Responsive',
    'Restorable',
    'Runtime',
    'RuntimeId',
    'ScrollDeltaLike',
    'Size',
    'SizeDict',
    'SizeLike',
    'SizeShortDict',
    'TechnologyId',
    'UiAttribute',
    'UiNode',
    'UiValue',
]
