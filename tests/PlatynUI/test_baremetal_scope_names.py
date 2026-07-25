# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportPrivateUsage=false

"""Unit tests for how ``BareMetal`` derives its scoped-variable names.

The names must be stable across the per-suite instances Robot Framework creates for a
suite-scoped library, and distinct for every registered library name — see the
``baremetal-library-instances`` capability. The lookup reads Robot Framework internals,
so these tests pin the exact contract it relies on, including that it must not touch
``TestLibrary.instance`` (a property that instantiates on access).
"""

from typing import Any

import pytest

from PlatynUI.BareMetal import (
    PLATYNUI_QUERY_SETTINGS,
    PLATYNUI_ROOT_DESCRIPTOR,
    BareMetal,
)


class FakeTestLibrary:
    """Stand-in for ``robot.running.testlibraries.TestLibrary``.

    ``instance`` raises: the real property *creates* the library instance when the slot
    is empty, so any lookup that touches it would instantiate every library in the suite.
    """

    def __init__(self, code: type, instance: object | None) -> None:
        self.code = code
        self._instance = instance

    @property
    def instance(self) -> object:
        raise AssertionError('the name lookup must read _instance, never the instantiating property')


class FakeNamespace:
    def __init__(self, libraries: dict[str, FakeTestLibrary]) -> None:
        self._kw_store = type('KwStore', (), {'libraries': libraries})()


class FakeContext:
    def __init__(self, namespace: FakeNamespace) -> None:
        self.namespace = namespace


@pytest.fixture
def library() -> BareMetal:
    return BareMetal(use_mock=True)


def _with_context(monkeypatch: pytest.MonkeyPatch, libraries: dict[str, FakeTestLibrary]) -> None:
    contexts: Any = type('Contexts', (), {'current': FakeContext(FakeNamespace(libraries))})()
    monkeypatch.setattr('PlatynUI.BareMetal.EXECUTION_CONTEXTS', contexts)


def test_default_import_keeps_the_documented_names(monkeypatch: pytest.MonkeyPatch, library: BareMetal) -> None:
    _with_context(monkeypatch, {'PlatynUI.BareMetal': FakeTestLibrary(BareMetal, library)})

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'
    assert library._query_settings_variable_name() == f'${{{PLATYNUI_QUERY_SETTINGS}}}'


def test_alias_gets_a_suffix(monkeypatch: pytest.MonkeyPatch, library: BareMetal) -> None:
    _with_context(monkeypatch, {'BM': FakeTestLibrary(BareMetal, library)})

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}_BM}}'
    assert library._query_settings_variable_name() == f'${{{PLATYNUI_QUERY_SETTINGS}_BM}}'


@pytest.mark.parametrize(
    ('alias', 'expected'),
    [
        ('bm', 'BM'),
        ('App 2', 'APP_2'),
        ('my.app', 'MY_APP'),
        ('a-b', 'A_B'),
    ],
)
def test_alias_is_normalized_to_a_legal_variable_name(
    monkeypatch: pytest.MonkeyPatch, library: BareMetal, alias: str, expected: str
) -> None:
    """Anything outside [A-Z0-9_] would collide with Robot's extended variable syntax."""
    _with_context(monkeypatch, {alias: FakeTestLibrary(BareMetal, library)})

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}_{expected}}}'


def test_other_instances_of_the_same_library_are_not_confused(
    monkeypatch: pytest.MonkeyPatch, library: BareMetal
) -> None:
    other = BareMetal(use_mock=True)
    _with_context(
        monkeypatch,
        {
            'A': FakeTestLibrary(BareMetal, other),
            'B': FakeTestLibrary(BareMetal, library),
            'C': FakeTestLibrary(BareMetal, None),
        },
    )

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}_B}}'
    assert other._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}_A}}'


def test_unregistered_instance_falls_back_to_the_bare_name(
    monkeypatch: pytest.MonkeyPatch, library: BareMetal
) -> None:
    """No name to derive from (imported but not this instance) must not raise."""
    _with_context(monkeypatch, {'Other': FakeTestLibrary(BareMetal, None)})

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'


def test_without_an_execution_context_falls_back_to_the_bare_name(
    monkeypatch: pytest.MonkeyPatch, library: BareMetal
) -> None:
    contexts: Any = type('Contexts', (), {'current': None})()
    monkeypatch.setattr('PlatynUI.BareMetal.EXECUTION_CONTEXTS', contexts)

    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'


def test_the_name_is_resolved_once_per_instance(monkeypatch: pytest.MonkeyPatch, library: BareMetal) -> None:
    """The lookup walks the namespace; a keyword-rate walk would be wasteful."""
    libraries = {'BM': FakeTestLibrary(BareMetal, library)}
    _with_context(monkeypatch, libraries)

    first = library._root_variable_name()
    libraries.clear()
    assert library._root_variable_name() == first


def test_differently_configured_instances_have_different_fingerprints() -> None:
    """Same registered name, different import arguments — the mismatch a name cannot catch."""
    mock_lib = BareMetal(use_mock=True)
    same = BareMetal(use_mock=True)
    other = BareMetal(use_mock=True, auto_activate=False)

    assert mock_lib._import_fingerprint == same._import_fingerprint
    assert mock_lib._import_fingerprint != other._import_fingerprint
