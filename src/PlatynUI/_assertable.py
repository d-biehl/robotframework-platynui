"""Decorators for enriching Robot Framework keywords with assertions."""

import inspect
from collections.abc import Callable
from typing import Any, cast

from assertionengine import AssertionOperator, verify_assertion

_ASSERTION_PARAM_SPECS: tuple[tuple[str, Any], ...] = (
    ('assertion_operator', AssertionOperator | None),
    ('assertion_expected', Any),
    ('assertion_message', str | None),
)

_ASSERTION_ANNOTATIONS: dict[str, Any] = {
    'assertion_operator': AssertionOperator | None,
    'assertion_expected': Any,
    'assertion_message': str | None,
}

_ASSERTION_ROBOT_TYPES: dict[str, str] = {
    'assertion_operator': 'AssertionOperator | None',
    'assertion_expected': 'Any',
    'assertion_message': 'str | None',
}


def _build_assertion_parameters() -> list[inspect.Parameter]:
    """Create fresh Parameter objects describing assertion arguments."""
    return [
        inspect.Parameter(name, inspect.Parameter.POSITIONAL_OR_KEYWORD, default=None, annotation=annotation)
        for name, annotation in _ASSERTION_PARAM_SPECS
    ]


def _derive_source_metadata(callable_obj: Callable[..., Any]) -> tuple[str | None, int | None]:
    """Best-effort retrieval of the callable's source file and first line number."""
    try:
        filename = inspect.getsourcefile(callable_obj)
    except (TypeError, OSError):
        filename = None

    code_obj = getattr(callable_obj, '__code__', None)
    lineno = getattr(code_obj, 'co_firstlineno', None)

    return filename, lineno


def _copy_function_metadata(
    source: Callable[..., Any],
    target: Callable[..., Any],
    signature: inspect.Signature,
) -> None:
    """Mirror metadata needed by Robot Framework from source to target callable."""
    proxy = cast(Any, target)
    proxy.__name__ = getattr(source, '__name__', proxy.__name__)
    proxy.__qualname__ = getattr(source, '__qualname__', proxy.__qualname__)
    proxy.__doc__ = getattr(source, '__doc__', proxy.__doc__)
    proxy.__module__ = getattr(source, '__module__', proxy.__module__)

    annotations = dict(getattr(source, '__annotations__', {}))
    annotations.update(_ASSERTION_ANNOTATIONS)
    proxy.__annotations__ = annotations

    proxy.__signature__ = signature

    proxy.robot_name = getattr(source, 'robot_name', None)
    proxy.robot_tags = getattr(source, 'robot_tags', ())

    source_file = getattr(source, 'robot_source', None)
    source_line = getattr(source, 'robot_lineno', None)
    derived_file, derived_line = _derive_source_metadata(source)

    proxy.robot_source = source_file if source_file is not None else derived_file
    proxy.robot_lineno = source_line if source_line is not None else derived_line

    original_types = getattr(source, 'robot_types', {})
    types_copy: dict[str, Any]
    if isinstance(original_types, dict):
        types_copy = {**cast(dict[str, Any], original_types)}
    else:
        types_copy = {}
    types_copy.update(_ASSERTION_ROBOT_TYPES)
    proxy.robot_types = types_copy

    proxy.PLATYNUI_ASSERTABLE_FIELD = True


def assertable(func: Callable[..., Any]) -> Callable[..., Any]:
    """Decorator that adds assertion functionality to keywords.

    This decorator adds three optional parameters to the decorated function:
    - assertion_operator: The comparison operator to use
    - assertion_expected: The expected value to compare against
    - assertion_message: Custom message for assertion failures

    The decorator works by modifying the function signature and Robot Framework
    attributes to include the assertion parameters.
    """
    sig = inspect.signature(func)
    new_sig = sig.replace(parameters=[*sig.parameters.values(), *_build_assertion_parameters()])

    def wrapper(*args: Any, **kwargs: Any) -> Any:
        bound_arguments = new_sig.bind_partial(*args, **kwargs)
        assertion_operator = bound_arguments.arguments.pop('assertion_operator', None)
        assertion_expected = bound_arguments.arguments.pop('assertion_expected', None)
        assertion_message = bound_arguments.arguments.pop('assertion_message', None)

        result = func(*bound_arguments.args, **bound_arguments.kwargs)

        if assertion_operator is not None:
            verify_assertion(
                result,
                assertion_operator,
                assertion_expected,
                assertion_message
            )

        return result

    _copy_function_metadata(func, wrapper, new_sig)

    return wrapper
