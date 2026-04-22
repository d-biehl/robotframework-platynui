# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Adapter weight calculator (design document section 11.4).

Used during ``@pattern_proxy_for`` resolution to score which proxy
overload best matches a given adapter. The legacy implementation
(``core/weight_calculator.py`` in the old project) is preserved 1:1 in
behaviour with one substantive change: the old twin criteria
``properties[k]==v`` / ``native_properties[k]==v`` are consolidated into
a single ``attributes[(ns, name)] == v`` criterion, mirroring the
namespaced ``UiAttribute`` model on the Rust side
(``crates/core/src/ui/{node,namespace}.rs``).

``adapter`` is typed via an :class:`AdapterLike` ``Protocol`` to avoid a
hard import dependency on ``core.adapter`` (which lands in Phase 2).

Weights:

* ``technology`` — 100 000
* ``role`` exact match — 10 000; entry in ``supported_roles`` — ``5000-i``
* ``framework_id`` — 1 000
* ``class_name`` — 500
* ``tag_name`` — 400
* each entry in ``attributes`` — 200

Any criterion provided but not satisfied causes ``calculate`` to return
``0`` (i.e. "no match").
"""

from __future__ import annotations

import re
from typing import Any, Protocol, cast, runtime_checkable

from .locator import DEFAULT_ATTRIBUTE_NAMESPACE
from .types import PatternName

__all__ = ['AdapterLike', 'WeightCalculator']


@runtime_checkable
class AdapterLike(Protocol):
    """Minimal adapter surface required by :class:`WeightCalculator`.

    The full adapter interface lives in ``core.adapter`` (Phase 2). Using
    a structural protocol keeps this module independent of that concrete
    type so it can be unit-tested with simple stubs.

    ``attribute_value`` mirrors the Rust ``UiNode::attribute(namespace,
    name)`` API and returns ``None`` for unknown / unsupported
    attributes (see §A.4).
    """

    @property
    def technology(self) -> Any: ...

    @property
    def supported_patterns(self) -> "list[PatternName]": ...

    def get_pattern(self, pattern_name: PatternName) -> Any: ...

    def attribute_value(
        self, name: str, namespace: str = DEFAULT_ATTRIBUTE_NAMESPACE
    ) -> Any: ...


#: A criterion attribute key. Bare strings live in
#: :data:`DEFAULT_ATTRIBUTE_NAMESPACE`; tuple keys are explicit.
AttributeKey = 'str | tuple[str, str]'


def _normalize_key(
    key: 'str | tuple[str, str]', default_namespace: str
) -> tuple[str, str]:
    """Resolve a free-form attribute key into ``(namespace, name)``.

    Bare strings are placed in ``default_namespace``; tuple keys are
    taken verbatim.
    """
    if isinstance(key, tuple):
        if len(key) != 2:
            raise ValueError(
                f'attribute key tuple must be (namespace, name); got {key!r}'
            )
        namespace, name = key
        if not isinstance(namespace, str) or not isinstance(name, str):
            raise TypeError(
                f'attribute key tuple must be (str, str); got {key!r}'
            )
        return namespace, name
    if isinstance(key, str):
        return default_namespace, key
    raise TypeError(
        f'attribute key must be str or (str, str) tuple; got {type(key).__name__}'
    )


class WeightCalculator:
    """Score how well an adapter matches a set of criteria."""

    def __init__(
        self,
        adapter: AdapterLike,
        *,
        default_attribute_namespace: str = DEFAULT_ATTRIBUTE_NAMESPACE,
    ) -> None:
        self.adapter = adapter
        self.default_attribute_namespace = default_attribute_namespace
        self._cache: dict[str, Any] = {}
        self._attributes_cache: dict[tuple[str, str], Any] = {}

    def cached(self, name: str) -> Any:
        """Memoised attribute access on the adapter."""
        if name not in self._cache:
            self._cache[name] = getattr(self.adapter, name, None)
        return self._cache[name]

    def attribute_cached(self, namespace: str, name: str) -> Any:
        """Memoised access to ``adapter.attribute_value(name, namespace)``."""
        key = (namespace, name)
        if key not in self._attributes_cache:
            self._attributes_cache[key] = self.adapter.attribute_value(
                name, namespace
            )
        return self._attributes_cache[key]

    @staticmethod
    def test_values(actual: Any, expected: Any) -> bool:
        """Compare ``actual`` against ``expected`` (regex- or equality-based)."""
        if isinstance(expected, re.Pattern):
            return expected.fullmatch(str(actual)) is not None
        return bool(actual == expected)

    def calculate(self, criteria: dict[str, object]) -> int:
        """Return the match weight, or ``0`` if any criterion fails."""
        weight = 0

        if criteria.get('technology') is not None:
            if self.test_values(type(self.adapter.technology), criteria['technology']):
                weight += 100_000
            else:
                return 0

        if criteria.get('role') is not None:
            if self.cached('role') == criteria['role']:
                weight += 10_000
            else:
                supported: list[Any] = list(self.cached('supported_roles') or [])
                try:
                    index = supported.index(criteria['role'])
                except ValueError:
                    return 0
                weight += 5_000 - index

        if criteria.get('framework_id') is not None:
            if self.test_values(self.cached('framework_id'), criteria['framework_id']):
                weight += 1_000
            else:
                return 0

        if criteria.get('class_name') is not None:
            if self.test_values(self.cached('class_name'), criteria['class_name']):
                weight += 500
            else:
                return 0

        if criteria.get('tag_name') is not None:
            if self.test_values(self.cached('tag_name'), criteria['tag_name']):
                weight += 400
            else:
                return 0

        attributes = criteria.get('attributes')
        if attributes is not None:
            for raw_key, expected in cast('dict[Any, Any]', attributes).items():
                namespace, name = _normalize_key(
                    raw_key, self.default_attribute_namespace
                )
                if self.test_values(self.attribute_cached(namespace, name), expected):
                    weight += 200
                else:
                    return 0

        return weight
