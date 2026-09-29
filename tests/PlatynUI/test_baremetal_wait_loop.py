# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for the one polling loop behind every wait.

Each attempt of a wait looks up the ``Set Root`` root first, outside the errors that
``ignore_exceptions`` swallows, and the failure at the deadline describes the last attempt, with
the last swallowed error. What these tests need is something that changes between attempts: an
element that stops being valid, an error that stops. The mock provider never invalidates an
element and its tree never changes, so the mock-backed suite ``tests/BareMetal/wait_keywords.robot``
cannot show it; a fake runtime answers each evaluation by its query and its context element instead.
"""

import time
from collections.abc import Callable
from typing import Any
from unittest.mock import MagicMock

import pytest
from platynui_native import UiNode

from PlatynUI.BareMetal import (
    BareMetal,
    ElementNotFoundError,
    PinnedElementGoneError,
    RootNotFoundError,
    UiNodeDescriptor,
)

OWN_RUNTIME = 7
ROOT = '//control:Window[@Name="Main"]'
TARGET = './/control:Button[@Name="OK"]'


class FlakyBridgeError(Exception):
    """Stands for an error a provider or the XPath engine raises inside an attempt."""


def make_node(name: str) -> MagicMock:
    """A stand-in element. ``spec=UiNode`` keeps ``isinstance`` checks working."""
    node = MagicMock(spec=UiNode)
    node.owner_id = OWN_RUNTIME
    node.runtime_id = f'fake://{name}'
    node.is_valid.return_value = True
    node.describe.return_value = f'Window "{name}"'
    return node


class FakeRuntime:
    """Answers each evaluation by its query and its context element, and counts the evaluations."""

    instance_id = OWN_RUNTIME

    def __init__(self, answer: Callable[[str, Any], Any]) -> None:
        self._answer = answer
        self.calls: list[tuple[str, Any]] = []

    def evaluate_single(self, query: str, context: Any) -> Any:
        self.calls.append((query, context))
        return self._answer(query, context)

    def clear_cache(self) -> None:
        pass

    def is_context_dependent(self, query: str) -> bool:
        return query.startswith(('.', 'count(.'))


@pytest.fixture
def library() -> BareMetal:
    return BareMetal(use_mock=True, query_settings={'timeout': 0.05, 'retry_interval': 0.01})


def with_runtime(library: BareMetal, answer: Callable[[str, Any], Any]) -> FakeRuntime:
    """Swap in the fake runtime. ``runtime`` is a cached_property, so the instance dict wins;
    ``query_settings`` is left alone — outside a Robot run it falls back to the import defaults."""
    runtime = FakeRuntime(answer)
    library.__dict__['runtime'] = runtime
    return runtime


def with_root(monkeypatch: pytest.MonkeyPatch, binding: UiNodeDescriptor) -> None:
    """Make ``binding`` the root, as `Set Root` would; there is no Robot Framework variable to hold it."""
    monkeypatch.setattr(BareMetal, 'root', property(lambda self: binding.resolve(self, as_root=True)))


def test_an_attempt_that_completes_discards_the_remembered_error(library: BareMetal) -> None:
    raised = False

    def answer(query: str, context: Any) -> Any:
        nonlocal raised
        if not raised:
            raised = True
            raise FlakyBridgeError('the first attempt fails')
        return None

    with_runtime(library, answer)

    with pytest.raises(ElementNotFoundError) as caught:
        library.wait_until_exists(
            library.descriptor_from_query('//control:Button'), query_overrides={'ignore_exceptions': True}
        )

    assert str(caught.value).endswith('within timeout of 0.05 seconds.'), caught.value
    assert caught.value.__cause__ is None


def test_a_root_replaced_during_the_wait_is_followed(library: BareMetal, monkeypatch: pytest.MonkeyPatch) -> None:
    """The root's window closes and reopens: the next attempt looks the root up again."""
    closed, reopened, button = make_node('Main'), make_node('Main'), make_node('OK')
    roots = iter([closed, reopened])

    def answer(query: str, context: Any) -> Any:
        if query == ROOT:
            return next(roots, None)
        if context is closed:
            closed.is_valid.return_value = False
            return None
        return button if context is reopened else None

    with_runtime(library, answer)
    with_root(monkeypatch, UiNodeDescriptor(None, ROOT, is_root_binding=True))

    found = library.wait_until_exists(library.descriptor_from_query(TARGET), query_overrides={'timeout': 1})

    assert found is button


def test_finding_the_root_does_not_use_up_the_target_s_timeout(monkeypatch: pytest.MonkeyPatch) -> None:
    """The root keeps its own timeout: a root that appears late leaves the target its whole timeout."""
    library = BareMetal(use_mock=True, query_settings={'timeout': 2, 'retry_interval': 0.01})
    window, button = make_node('Main'), make_node('OK')
    root_appears = time.monotonic() + 0.5

    def answer(query: str, context: Any) -> Any:
        now = time.monotonic()
        if query == ROOT:
            return window if now >= root_appears else None
        return button if now >= root_appears + 0.1 else None

    with_runtime(library, answer)
    with_root(monkeypatch, UiNodeDescriptor(None, ROOT, is_root_binding=True))

    found = library.wait_until_exists(library.descriptor_from_query(TARGET), query_overrides={'timeout': 0.3})

    assert found is button


def test_a_root_whose_lookup_raises_quotes_the_root_s_last_error(monkeypatch: pytest.MonkeyPatch) -> None:
    """The root's own settings ignore errors here: its lookup waits them out and quotes the last one."""
    library = BareMetal(
        use_mock=True, query_settings={'timeout': 0.05, 'retry_interval': 0.01, 'ignore_exceptions': True}
    )

    def answer(query: str, context: Any) -> Any:
        if query == ROOT:
            raise FlakyBridgeError('the root cannot be read')
        return None

    with_runtime(library, answer)
    with_root(monkeypatch, UiNodeDescriptor(None, ROOT, is_root_binding=True))

    with pytest.raises(RootNotFoundError) as caught:
        library.wait_until_exists(library.descriptor_from_query(TARGET))

    assert str(caught.value).endswith(
        f'{TARGET!r} was not evaluated. The last error was: FlakyBridgeError: the root cannot be read'
    ), caught.value


def root_that_goes_away(button: MagicMock | None) -> Callable[[str, Any], Any]:
    """The root is found once; its window closes after the first attempt and never comes back."""
    window = make_node('Main')
    roots = iter([window])

    def answer(query: str, context: Any) -> Any:
        if query == ROOT:
            return next(roots, None)
        window.is_valid.return_value = False
        return button

    return answer


def test_a_root_that_goes_away_ends_wait_until_exists(library: BareMetal, monkeypatch: pytest.MonkeyPatch) -> None:
    with_runtime(library, root_that_goes_away(None))
    with_root(monkeypatch, UiNodeDescriptor(None, ROOT, is_root_binding=True))

    with pytest.raises(RootNotFoundError, match=r'within timeout of 0\.05 seconds; .* was not evaluated\.$'):
        library.wait_until_exists(
            library.descriptor_from_query(TARGET), query_overrides={'timeout': 1, 'ignore_exceptions': True}
        )


def test_a_pinned_root_that_stops_being_valid_ends_the_wait(
    library: BareMetal, monkeypatch: pytest.MonkeyPatch
) -> None:
    window = make_node('Main')

    def answer(query: str, context: Any) -> Any:
        window.is_valid.return_value = False
        return None

    with_runtime(library, answer)
    with_root(monkeypatch, UiNodeDescriptor(window, None, is_root_binding=True))

    with pytest.raises(PinnedElementGoneError):
        library.wait_until_exists(
            library.descriptor_from_query(TARGET), query_overrides={'timeout': 1, 'ignore_exceptions': True}
        )


def test_the_failure_is_chained_to_the_last_swallowed_error(library: BareMetal) -> None:
    attempts = 0

    def answer(query: str, context: Any) -> Any:
        nonlocal attempts
        attempts += 1
        raise FlakyBridgeError(f'attempt {attempts} failed')

    with_runtime(library, answer)

    with pytest.raises(ElementNotFoundError) as caught:
        library.wait_until_exists(
            library.descriptor_from_query('//control:Button'), query_overrides={'ignore_exceptions': True}
        )

    assert attempts > 1
    assert isinstance(caught.value.__cause__, FlakyBridgeError)
    assert str(caught.value.__cause__) == f'attempt {attempts} failed'
    assert str(caught.value).endswith(f'The last error was: FlakyBridgeError: attempt {attempts} failed')
