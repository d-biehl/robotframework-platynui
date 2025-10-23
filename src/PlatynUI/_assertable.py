from collections.abc import Callable
from typing import ParamSpec, TypeVar

P = ParamSpec('P')
T = TypeVar('T')

PLATYNUI_ASSERTABLE_FIELD = 'platynui_assertable'


def assertable(func: Callable[P, T]) -> Callable[P, T]:
    setattr(func, PLATYNUI_ASSERTABLE_FIELD, True)
    return func
