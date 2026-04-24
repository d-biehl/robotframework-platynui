"""platynui_native package.

This package provides Python bindings for PlatynUI's native Rust implementation.
All types and functions are directly exported from the native extension module.
"""

# Re-export everything from the native extension
from typing import Any, Literal, TypedDict

from ._native import (
    AttributeNotFoundError,
    EvaluatedAttribute,
    EvaluationError,
    EvaluationIterator,
    Focusable,
    KeyboardError,
    KeyboardOverrides,
    KeyboardProfile,
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
class PointDict(TypedDict):
    x: float
    y: float


class SizeDict(TypedDict):
    width: float
    height: float


class SizeShortDict(TypedDict):
    w: float
    h: float


class RectDict(TypedDict):
    x: float
    y: float
    width: float
    height: float


type PointLike = Point | tuple[float, float] | PointDict
type SizeLike = Size | tuple[float, float] | SizeDict | SizeShortDict
type RectLike = Rect | tuple[float, float, float, float] | RectDict
type OriginLike = Literal['desktop'] | PointLike | RectLike
type ScrollDeltaLike = tuple[float, float]
type PointerButtonLike = PointerButton | int

class PointerOverridesDict(TypedDict, total=False):
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
    double_click_time_ms: float
    double_click_size: SizeLike
    default_button: PointerButtonLike


class PointerProfileDict(TypedDict, total=False):
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
    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


class KeyboardOverridesDict(TypedDict, total=False):
    press_delay_ms: float
    release_delay_ms: float
    between_keys_delay_ms: float
    chord_press_delay_ms: float
    chord_release_delay_ms: float
    after_sequence_delay_ms: float
    after_text_delay_ms: float


type PointerOverridesLike = PointerOverrides | PointerOverridesDict
type PointerSettingsLike = PointerSettings | PointerSettingsDict
type PointerProfileLike = PointerProfile | PointerProfileDict
type KeyboardOverridesLike = KeyboardOverrides | KeyboardOverridesDict
type KeyboardProfileLike = KeyboardProfile | KeyboardProfileDict

Primitive = bool | int | float | str | None
JSONLike = dict[str, Any] | list[Any]
UiValue = Primitive | Point | Size | Rect | JSONLike


# Explicit __all__ for better IDE support (will be populated by stub file)
__all__ = [
    'AttributeNotFoundError',
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
    'WindowSurface',
]
