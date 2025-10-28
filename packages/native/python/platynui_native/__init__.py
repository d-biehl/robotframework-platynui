"""platynui_native package.

This package provides Python bindings for PlatynUI's native Rust implementation.
All types and functions are directly exported from the native extension module.
"""

# Re-export everything from the native extension
from typing import Any, Literal, TypeAlias, TypedDict

from ._native import (
    AttributeNotFoundError,
    EvaluatedAttribute,
    EvaluationError,
    EvaluationIterator,
    Focusable,
    KeyboardError,
    KeyboardOverrides,
    KeyboardSettings,
    Namespace,
    NodeAttributesIterator,
    NodeChildrenIterator,
    PatternError,
    PatternId,
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
    Runtime,
    RuntimeId,
    Size,
    TechnologyId,
    UiAttribute,
    UiNode,
    WindowSurface,
)

# ===== Type Aliases =====


# Like dictionaries for ergonomics
class _PointDict(TypedDict):
    x: float
    y: float


class _SizeDict(TypedDict):
    width: float
    height: float


class _SizeShortDict(TypedDict):
    w: float
    h: float


class _RectDict(TypedDict):
    x: float
    y: float
    width: float
    height: float


PointLike: TypeAlias = Point | tuple[float, float] | _PointDict
SizeLike: TypeAlias = Size | tuple[float, float] | _SizeDict | _SizeShortDict
RectLike: TypeAlias = Rect | tuple[float, float, float, float] | _RectDict
OriginLike = Literal['desktop'] | PointLike | RectLike
ScrollDeltaLike = tuple[float, float]


class _PointerOverridesDict(TypedDict, total=False):
    origin: OriginLike
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
    ensure_move_threshold: float
    ensure_move_timeout_ms: float
    scroll_step: tuple[float, float]
    scroll_delay_ms: float


class _PointerSettingsDict(TypedDict, total=False):
    double_click_time_ms: float
    double_click_size: SizeLike
    default_button: PointerButton


class _PointerProfileDict(TypedDict, total=False):
    motion: PointerMotionMode
    steps_per_pixel: float
    max_move_duration_ms: float
    speed_factor: float
    acceleration_profile: PointerAccelerationProfile
    overshoot_ratio: float
    overshoot_settle_steps: int
    curve_amplitude: float
    jitter_amplitude: float
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


class _KeyboardSettingsDict(TypedDict, total=False):
    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


class _KeyboardOverridesDict(TypedDict, total=False):
    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


PointerOverridesLike: TypeAlias = PointerOverrides | _PointerOverridesDict
PointerSettingsLike: TypeAlias = PointerSettings | _PointerSettingsDict
PointerProfileLike: TypeAlias = PointerProfile | _PointerProfileDict
KeyboardSettingsLike: TypeAlias = KeyboardSettings | _KeyboardSettingsDict
KeyboardOverridesLike: TypeAlias = KeyboardOverrides | _KeyboardOverridesDict

Primitive = bool | int | float | str | None
JSONLike = dict[str, Any] | list[Any]
UiValue = Primitive | Point | Size | Rect | JSONLike


# Explicit __all__ for better IDE support (will be populated by stub file)
__all__ = [
    # Exceptions
    'AttributeNotFoundError',
    'EvaluatedAttribute',
    'EvaluationError',
    'EvaluationIterator',
    'Focusable',
    'KeyboardError',
    'KeyboardOverrides',
    'KeyboardOverridesLike',
    'KeyboardOverridesLike',
    'KeyboardSettings',
    'KeyboardSettingsLike',
    'KeyboardSettingsLike',
    'Namespace',
    'NodeAttributesIterator',
    'NodeChildrenIterator',
    'OriginLike',
    'PatternError',
    'PatternId',
    'PlatynUiError',
    # Core types
    'Point',
    'PointLike',
    'PointerAccelerationProfile',
    'PointerButton',
    'PointerError',
    'PointerMotionMode',
    # Overrides
    'PointerOverrides',
    'PointerOverridesLike',
    'PointerProfile',
    'PointerProfileLike',
    'PointerSettings',
    'PointerSettingsLike',
    'ProviderError',
    'Rect',
    'RectLike',
    # Runtime
    'Runtime',
    'RuntimeId',
    'ScrollDeltaLike',
    'Size',
    'SizeLike',
    'TechnologyId',
    'UiAttribute',
    'UiNode',
    'UiValue',
    'WindowSurface',
]
