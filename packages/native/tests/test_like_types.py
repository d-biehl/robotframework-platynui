import math
from typing import Any

from platynui_native import Point, Rect, Size


def is_num(x: Any) -> bool:
    return isinstance(x, (int, float)) and math.isfinite(float(x))


def test_point_equality_with_tuple_and_dict() -> None:
    p = Point(10, 20)
    assert p == (10, 20)
    assert p == {'x': 10, 'y': 20}
    assert p != (10, 21)
    assert p != {'x': 11, 'y': 20}


def test_point_from_like() -> None:
    p1 = Point.from_like((1, 2))
    assert p1.x == 1 and p1.y == 2
    p2 = Point.from_like({'x': 3, 'y': 4})
    assert p2.to_tuple() == (3.0, 4.0)


def test_point_unpacking() -> None:
    x, y = Point(1, 3)
    assert (x, y) == (1, 3)


def test_size_equality_with_tuple_and_dict() -> None:
    s = Size(100, 200)
    assert s == (100, 200)
    assert s == {'width': 100, 'height': 200}
    # tolerate short keys
    assert s == {'w': 100, 'h': 200}
    assert s != (101, 200)


def test_size_from_like() -> None:
    s1 = Size.from_like((5, 6))
    assert s1.width == 5 and s1.height == 6
    s2 = Size.from_like({'width': 7, 'height': 8})
    assert (s2.width, s2.height) == (7.0, 8.0)


def test_size_unpacking() -> None:
    w, h = Size(5, 6)
    assert (w, h) == (5, 6)


def test_rect_equality_with_tuple_and_dict() -> None:
    r = Rect(1, 2, 3, 4)
    assert r == (1, 2, 3, 4)
    assert r == {'x': 1, 'y': 2, 'width': 3, 'height': 4}
    assert r != (1, 2, 3, 5)


def test_rect_from_like() -> None:
    r1 = Rect.from_like((9, 8, 7, 6))
    assert r1.to_tuple() == (9.0, 8.0, 7.0, 6.0)
    r2 = Rect.from_like({'x': 2, 'y': 3, 'width': 4, 'height': 5})
    assert r2.left() == 2 and r2.top() == 3


def test_rect_unpacking() -> None:
    x, y, w, h = Rect(1, 2, 3, 4)
    assert (x, y, w, h) == (1, 2, 3, 4)
