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
    Namespace,
    NodeAttributesIterator,
    NodeChildrenIterator,
    PatternError,
    PatternId,
    PlatynUiError,
    Point,
    PointerButton,
    PointerError,
    PointerOverrides,
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

Primitive = bool | int | float | str | None
JSONLike = dict[str, Any] | list[Any]
UiValue = Primitive | Point | Size | Rect | JSONLike

OriginLike = Literal['desktop'] | PointLike | RectLike
ScrollDeltaLike = tuple[float, float]

# Explicit __all__ for better IDE support (will be populated by stub file)
__all__ = [
    'AttributeNotFoundError',
    'EvaluatedAttribute',
    # Exceptions
    'EvaluationError',
    'EvaluationIterator',
    'Focusable',
    'KeyboardError',
    'KeyboardOverrides',
    'Namespace',
    'NodeAttributesIterator',
    'NodeChildrenIterator',
    'PatternError',
    'PatternId',
    'PlatynUiError',
    # Core types
    'Point',
    'PointLike',
    'PointerButton',
    'PointerError',
    # Overrides
    'PointerOverrides',
    'ProviderError',
    'Rect',
    'RectLike',
    # Runtime
    'Runtime',
    'RuntimeId',
    'Size',
    'SizeLike',
    'TechnologyId',
    'UiAttribute',
    'UiNode',
    'UiValue',
    'WindowSurface',
]
