"""platynui_native package.

This package provides Python bindings for PlatynUI's native Rust implementation.
All types and functions are directly exported from the native extension module.
"""

# Re-export everything from the native extension
from ._native import *  # noqa: F403

# Explicit __all__ for better IDE support (will be populated by stub file)
__all__ = [  # noqa: F405
    # Core types
    'Point',
    'Size',
    'Rect',
    'PatternId',
    'RuntimeId',
    'TechnologyId',
    'Namespace',
    # Runtime
    'Runtime',
    'UiNode',
    'NodeChildrenIterator',
    'NodeAttributesIterator',
    'EvaluationIterator',
    'UiAttribute',
    'EvaluatedAttribute',
    'Focusable',
    'WindowSurface',
    # Overrides
    'PointerOverrides',
    'KeyboardOverrides',
    'PointerButton',
    # Exceptions
    'EvaluationError',
    'ProviderError',
    'PointerError',
    'KeyboardError',
    'PatternError',
    # Mock providers (always available)
    'MOCK_PROVIDER',
    'MOCK_PLATFORM',
    'MOCK_HIGHLIGHT_PROVIDER',
    'MOCK_SCREENSHOT_PROVIDER',
    'MOCK_POINTER_DEVICE',
    'MOCK_KEYBOARD_DEVICE',
]
