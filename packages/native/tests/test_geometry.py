#!/usr/bin/env python3
"""Quick test of the newly exposed geometry methods."""

from platynui_native import Point, Size, Rect


def test_point_methods() -> None:
    p = Point(10.0, 20.0)

    p2 = p.with_x(15.0)
    assert p2.x == 15.0 and p2.y == 20.0

    p3 = p.with_y(25.0)
    assert p3.x == 10.0 and p3.y == 25.0

    p4 = p.translate(5.0, -3.0)
    assert p4.x == 15.0 and p4.y == 17.0

    assert p.is_finite()

    p5 = p + Point(1.0, 2.0)
    assert p5.x == 11.0 and p5.y == 22.0

    p6 = p - Point(1.0, 2.0)
    assert p6.x == 9.0 and p6.y == 18.0



def test_size_methods() -> None:
    s = Size(100.0, 50.0)

    assert s.area() == 5000.0

    assert not s.is_empty()

    empty = Size(0.0, 10.0)
    assert empty.is_empty()

    assert s.is_finite()

    s2 = s + Size(10.0, 5.0)
    assert s2.width == 110.0 and s2.height == 55.0

    s3 = s - Size(10.0, 5.0)
    assert s3.width == 90.0 and s3.height == 45.0

    s4 = s * 2.0
    assert s4.width == 200.0 and s4.height == 100.0

    s5 = s / 2.0
    assert s5.width == 50.0 and s5.height == 25.0



def test_rect_methods() -> None:
    r = Rect(10.0, 20.0, 100.0, 50.0)

    assert r.left() == 10.0
    assert r.top() == 20.0
    assert r.right() == 110.0
    assert r.bottom() == 70.0

    center = r.center()
    assert center.x == 60.0 and center.y == 45.0

    size = r.size()
    assert size.width == 100.0 and size.height == 50.0

    pos = r.position()
    assert pos.x == 10.0 and pos.y == 20.0

    # contains
    assert r.contains(Point(50.0, 40.0))
    assert not r.contains(Point(200.0, 200.0))

    # intersects
    r2 = Rect(50.0, 30.0, 100.0, 50.0)
    assert r.intersects(r2)

    # intersection
    inter = r.intersection(r2)
    assert inter is not None

    # union
    uni = r.union(r2)
    assert uni.left() == 10.0  # leftmost

    # translate
    r3 = r.translate(5.0, -3.0)
    assert r3.x == 15.0 and r3.y == 17.0

    # inflate/deflate
    r4 = r.inflate(10.0, 5.0)
    # inflate expands in all directions: x-dw, y-dh, width+2*dw, height+2*dh
    assert r4.x == 0.0 and r4.y == 15.0
    assert r4.width == 120.0 and r4.height == 60.0

    r5 = r.deflate(10.0, 5.0)
    # deflate shrinks in all directions: x+dw, y+dh, width-2*dw, height-2*dh
    assert r5.x == 20.0 and r5.y == 25.0
    assert r5.width == 80.0 and r5.height == 40.0

    assert not r.is_empty()

    # operators
    r6 = r + Point(5.0, 3.0)
    assert r6.x == 15.0 and r6.y == 23.0

    r7 = r - Point(5.0, 3.0)
    assert r7.x == 5.0 and r7.y == 17.0



if __name__ == "__main__":
    pass
