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

from types import SimpleNamespace
from typing import Any
from unittest.mock import MagicMock

import pytest
from platynui_native import UiNode

from PlatynUI.BareMetal import (
    PLATYNUI_QUERY_SETTINGS,
    PLATYNUI_ROOT_DESCRIPTOR,
    BareMetal,
    ForeignNodeError,
    UiNodeDescriptor,
    UnsharableRootError,
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


def test_a_resolved_name_is_looked_up_once_per_instance(
    monkeypatch: pytest.MonkeyPatch, library: BareMetal
) -> None:
    """The lookup walks the namespace; a keyword-rate walk would be wasteful."""
    libraries = {'BM': FakeTestLibrary(BareMetal, library)}
    _with_context(monkeypatch, libraries)

    first = library._root_variable_name()
    libraries.clear()
    assert library._root_variable_name() == first


@pytest.mark.parametrize('miss', ['no_context', 'not_registered'])
def test_a_failed_lookup_is_not_cached(monkeypatch: pytest.MonkeyPatch, library: BareMetal, miss: str) -> None:
    """A miss must not pin an aliased import to the default variable for the rest of its life.

    That would be the leak this naming exists to prevent, and a silent one: the import would then
    consistently read and write the variable another import owns.
    """
    if miss == 'no_context':
        contexts: Any = type('Contexts', (), {'current': None})()
        monkeypatch.setattr('PlatynUI.BareMetal.EXECUTION_CONTEXTS', contexts)
    else:
        _with_context(monkeypatch, {'Other': FakeTestLibrary(BareMetal, None)})
    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'

    _with_context(monkeypatch, {'BM': FakeTestLibrary(BareMetal, library)})
    assert library._root_variable_name() == f'${{{PLATYNUI_ROOT_DESCRIPTOR}_BM}}'
    assert library._query_settings_variable_name() == f'${{{PLATYNUI_QUERY_SETTINGS}_BM}}'


class FakeStore:
    """Stand-in for RF's ``VariableStore`` — its ``get`` takes a keyword default."""

    def get(self, name: str, default: Any = None) -> Any:
        return default


class RecordingVariables:
    """Records which ``VariableScopes`` setter a scope name is routed to."""

    def __init__(self) -> None:
        self.current = FakeStore()
        self._suite = FakeStore()
        self._test = FakeStore()
        self._global = FakeStore()
        self.calls: list[tuple[str, str, dict[str, Any]]] = []

    def set_local(self, name: str, value: Any) -> None:
        self.calls.append(('set_local', name, {}))

    def set_test(self, name: str, value: Any) -> None:
        self.calls.append(('set_test', name, {}))

    def set_suite(self, name: str, value: Any, **kwargs: Any) -> None:
        self.calls.append(('set_suite', name, kwargs))

    def set_global(self, name: str, value: Any) -> None:
        self.calls.append(('set_global', name, {}))


@pytest.fixture
def recorded(monkeypatch: pytest.MonkeyPatch) -> RecordingVariables:
    variables = RecordingVariables()
    context = SimpleNamespace(variables=variables, namespace=FakeNamespace({}))
    monkeypatch.setattr('PlatynUI.BareMetal.EXECUTION_CONTEXTS', type('Contexts', (), {'current': context})())
    return variables


# The mapping Robot Framework's own `VAR` syntax uses (`Var._get_scope`): TASK is an alias of
# TEST, SUITES is the suite scope extended to child suites, and nothing else widens a scope.
@pytest.mark.parametrize(
    ('scope', 'setter', 'kwargs'),
    [
        ('LOCAL', 'set_local', {}),
        ('TEST', 'set_test', {}),
        ('TASK', 'set_test', {}),
        ('SUITE', 'set_suite', {}),
        ('SUITES', 'set_suite', {'children': True}),
        ('GLOBAL', 'set_global', {}),
    ],
)
def test_each_scope_routes_to_robots_own_setter(
    library: BareMetal, recorded: RecordingVariables, scope: Any, setter: str, kwargs: dict[str, Any]
) -> None:
    library.set_root(UiNodeDescriptor(None, '//control:Window', is_root_binding=True), scope=scope)
    library.set_query_settings({'timeout': 1.0}, scope=scope)

    assert [call[0] for call in recorded.calls] == [setter, setter]
    assert [call[2] for call in recorded.calls] == [kwargs, kwargs], f'unexpected setter kwargs: {recorded.calls}'


def test_a_suite_scoped_value_is_not_pushed_to_child_suites(library: BareMetal, recorded: RecordingVariables) -> None:
    """``SUITE`` must stop at this suite; widening it is what ``SUITES`` is for.

    Pinned separately from the mapping table above because this is the boundary a "convenience"
    change would silently move.
    """
    library.set_root(UiNodeDescriptor(None, '//control:Window', is_root_binding=True), scope='SUITE')

    assert recorded.calls == [('set_suite', f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}', {})]


@pytest.mark.parametrize('scope', ['SUITES', 'GLOBAL'])
def test_a_pinned_root_is_rejected_at_cross_suite_scopes(
    library: BareMetal, recorded: RecordingVariables, scope: Any
) -> None:
    """A capture cannot be re-found in another suite's runtime, so it must not be stored there.

    Rejected where the choice is made, not on read in a suite that never set a root.
    """
    library.__dict__['runtime'] = SimpleNamespace(instance_id=7, is_context_dependent=lambda _q: False)
    node = MagicMock(spec=UiNode)
    node.owner_id = 7

    with pytest.raises(UnsharableRootError, match='pins an element'):
        library.set_root(UiNodeDescriptor(node, None), scope=scope)

    assert recorded.calls == [], 'nothing may be stored when the root is rejected'


@pytest.mark.parametrize('scope', ['SUITES', 'GLOBAL'])
def test_a_pinned_root_is_rejected_deeper_in_the_chain(
    library: BareMetal, recorded: RecordingVariables, scope: Any
) -> None:
    """A selector root may drill into a capture — then the chain is as unsharable as the capture."""
    library.__dict__['runtime'] = SimpleNamespace(instance_id=7, is_context_dependent=lambda _q: False)
    node = MagicMock(spec=UiNode)
    node.owner_id = 7
    drilled = UiNodeDescriptor(None, './/Dialog', parent=UiNodeDescriptor(node, None), is_root_binding=True)

    with pytest.raises(UnsharableRootError):
        library.set_root(drilled, scope=scope)


@pytest.mark.parametrize('scope', ['SUITES', 'GLOBAL'])
def test_a_selector_root_is_allowed_at_cross_suite_scopes(
    library: BareMetal, recorded: RecordingVariables, scope: Any
) -> None:
    library.set_root(UiNodeDescriptor(None, '//control:Window', is_root_binding=True), scope=scope)

    assert len(recorded.calls) == 1


@pytest.mark.parametrize('scope', ['LOCAL', 'TEST', 'SUITE', 'SUITES', 'GLOBAL'])
def test_a_restored_root_pinning_a_foreign_element_is_rejected(
    library: BareMetal, recorded: RecordingVariables, scope: Any
) -> None:
    """A root binding handed over from another import must not be stored, at any scope.

    Restoring a value this keyword returned skips the argument conversion that guards a raw
    element, so without this check the foreign handle would be stored and only fail on the next
    lookup — in a suite that has no idea where the root came from.
    """
    library.__dict__['runtime'] = SimpleNamespace(instance_id=7)
    foreign = MagicMock(spec=UiNode)
    foreign.owner_id = 8
    foreign.runtime_id = 'mock://desktop/1'

    with pytest.raises(ForeignNodeError, match='different library instance'):
        library.set_root(UiNodeDescriptor(foreign, None, is_root_binding=True), scope=scope)

    assert recorded.calls == [], 'nothing may be stored when the root is rejected'


def test_a_restored_selector_root_drilling_into_a_foreign_element_is_rejected(
    library: BareMetal, recorded: RecordingVariables
) -> None:
    """The chain again: the foreign element can sit in the parent of a selector root."""
    library.__dict__['runtime'] = SimpleNamespace(instance_id=7)
    foreign = MagicMock(spec=UiNode)
    foreign.owner_id = 8
    foreign.runtime_id = 'mock://desktop/1'
    drilled = UiNodeDescriptor(None, './/Dialog', parent=UiNodeDescriptor(foreign, None), is_root_binding=True)

    with pytest.raises(ForeignNodeError):
        library.set_root(drilled, scope='LOCAL')
