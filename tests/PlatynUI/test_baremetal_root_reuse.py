# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""Unit tests for reusing the element a scoped root resolved to.

A root is looked up once per keyword *in addition* to the keyword's own target, which is the
one repetition a suite never asked for — so a root binding keeps its element while that element
is live. Target selectors keep being re-evaluated; that is the observation a keyword makes.

These are unit tests on purpose: the reuse is gated on ``UiNode.is_valid()``, and the mock
provider does not override it (the trait default is ``True``), so no mock-backed Robot suite can
exercise the invalidation path. The real proof that a dying root still re-resolves is
``tests/acceptance/swing/window.robot``, which closes the fixture process.
"""

from typing import Any
from unittest.mock import MagicMock

import pytest
from platynui_native import UiNode

from PlatynUI.BareMetal import BareMetal, PinnedElementGoneError, UiNodeDescriptor

OWN_RUNTIME = 7


def make_node(*, owner: int = OWN_RUNTIME, valid: bool = True, runtime_id: str = 'mock://desktop/1') -> MagicMock:
    """A stand-in element. ``spec=UiNode`` keeps ``isinstance`` checks working."""
    node = MagicMock(spec=UiNode)
    node.owner_id = owner
    node.runtime_id = runtime_id
    node.is_valid.return_value = valid
    return node


class FakeRuntime:
    """Counts the evaluations a resolve performs, so reuse is observable."""

    instance_id = OWN_RUNTIME

    def __init__(self, results: list[Any]) -> None:
        self._results = results
        self.calls: list[tuple[str, Any]] = []

    def evaluate_single(self, query: str, context: Any) -> Any:
        self.calls.append((query, context))
        return self._results.pop(0) if self._results else None

    def clear_cache(self) -> None:
        pass

    def is_context_dependent(self, query: str) -> bool:
        return query.startswith('.')


@pytest.fixture
def library() -> BareMetal:
    return BareMetal(use_mock=True, query_settings={'timeout': 0.05})


def with_runtime(library: BareMetal, *results: Any) -> FakeRuntime:
    """Swap in a counting runtime. ``runtime`` is a cached_property, so the instance dict wins;
    ``query_settings`` is left alone — outside a Robot run it falls back to the import defaults."""
    runtime = FakeRuntime(list(results))
    library.__dict__['runtime'] = runtime
    return runtime


def test_a_root_is_resolved_once_and_then_reused(library: BareMetal) -> None:
    first = make_node()
    runtime = with_runtime(library, first)
    root = UiNodeDescriptor(None, '//control:Window', is_root_binding=True)

    assert root.resolve(library, as_root=True) is first
    assert root.resolve(library, as_root=True) is first
    assert root.resolve(library, as_root=True) is first

    assert len(runtime.calls) == 1, f'the root must be looked up once, got {runtime.calls}'


def test_a_root_that_stopped_being_valid_is_resolved_again(library: BareMetal) -> None:
    """The promise that must not break: a root survives its window closing and reopening."""
    closed = make_node()
    reopened = make_node()
    runtime = with_runtime(library, closed, reopened)
    root = UiNodeDescriptor(None, '//control:Window', is_root_binding=True)

    assert root.resolve(library, as_root=True) is closed  # resolved while still live
    closed.is_valid.return_value = False  # its window went away

    assert root.resolve(library, as_root=True) is reopened
    assert len(runtime.calls) == 2


def test_a_root_reused_by_another_import_is_resolved_there(library: BareMetal) -> None:
    """A selector root may be handed to another import, which shares this object.

    Liveness alone would hand that import an element from a runtime it never bound to, so the
    owner is part of the question — and a selector can always be looked up again.
    """
    foreign = make_node(owner=8)
    own = make_node(owner=OWN_RUNTIME)
    runtime = with_runtime(library, own)
    root = UiNodeDescriptor(None, '//control:Window', is_root_binding=True)
    root._root_node = foreign  # as if the other import had resolved it

    assert root.resolve(library, as_root=True) is own, 'a foreign element must not be reused'
    assert len(runtime.calls) == 1


def test_a_target_selector_is_never_reused(library: BareMetal) -> None:
    """The counterpart: re-evaluating a target selector is the observation the keyword makes."""
    runtime = with_runtime(library, make_node(), make_node())
    target = library.descriptor_from_query('//control:Button')

    target.resolve(library)
    target.resolve(library)

    assert len(runtime.calls) == 2


def test_a_dead_capture_says_what_is_wrong(library: BareMetal) -> None:
    """Without a selector there is nothing to re-evaluate — the error has to say that."""
    with_runtime(library)
    dead = make_node(valid=False, runtime_id='mock://desktop/42')

    with pytest.raises(PinnedElementGoneError, match='mock://desktop/42'):
        UiNodeDescriptor(dead, None).resolve(library)
