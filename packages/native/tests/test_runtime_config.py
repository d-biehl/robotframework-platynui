"""Tests for the construction-time ``config`` dict on the native ``Runtime``.

These exercise the Python ``dict`` -> ``RuntimeConfig`` parser and the
``Runtime(config)`` binding (§7 of the ``per-runtime-platform-sessions`` change).

What the mock can and cannot show
---------------------------------
The *mock* platform backend deliberately ignores config VALUES (it has no real
connection), so these tests assert only what the mock can observe:

* a ``platform.backend='mock'`` config selects the mock backend and the runtime
  is usable (assertion "b");
* unknown/foreign ids and keys, and a non-dict section, are tolerated — no error
  (assertion "c");
* empty buckets / empty sub-dicts are no-ops (the observable slice of "absent or
  empty ⇒ current behaviour", assertion "a");
* what the binding cannot pass on to any component — another top-level key, a
  bucket that is not a dict, a non-string key, a value of an unsupported type —
  is reported as a warning, delivered before the constructor returns (spec:
  *runtime-session-config*, *diagnostic-logging*).

Real value overrides (``platform.x11.display``, ``providers.atspi.bus_address``)
change *which* live session is connected; they are verified in the real X11 /
AT-SPI acceptance lane, not here. The Python->Rust value-type parsing (including
the bool-before-int rule) is covered by construction not raising on every leaf
type below; its behavioural effect likewise surfaces only against a real backend.

Why every case forces ``backend='mock'``
-----------------------------------------
Only the mock backend is constructible headless. A real backend connects to its
display server when its platform bundle is built, so an auto-detected
``Runtime()`` / ``Runtime({})`` needs a live session and is out of scope for a
headless unit test. The cases below therefore force the mock backend and touch
only *platform* operations (pointer, desktop info) — never a tree query, which
would reach the lazily connected real provider that ``discover()`` also finds.
"""

import logging
import pathlib
from collections.abc import Generator
from typing import Any

import platynui_native as pn
import pytest

# The binding's own records; the runtime and the components log under other names.
BINDING_LOGGER = 'platynui.native.native.runtime'


@pytest.fixture
def caplog_warnings(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> Generator[pytest.LogCaptureFixture, None, None]:
    """Native warnings at the default level, starting from an empty queue."""
    monkeypatch.delenv('RUST_LOG', raising=False)
    monkeypatch.delenv('PLATYNUI_LOG_LEVEL', raising=False)
    pn.set_log_level(None)
    pn.flush_logs()
    caplog.set_level(logging.WARNING, logger='platynui.native')
    caplog.clear()
    yield caplog
    pn.flush_logs()


def warnings_of_construction(caplog: pytest.LogCaptureFixture, config: dict[Any, Any]) -> list[str]:
    """The binding's warnings for ``config``, as delivered when the constructor has returned."""
    runtime = pn.Runtime(config)
    try:
        return [r.getMessage() for r in caplog.records if r.name == BINDING_LOGGER and r.levelno == logging.WARNING]
    finally:
        runtime.shutdown()


def _assert_mock_platform(runtime: pn.Runtime) -> None:
    """Assert the mock platform backend is the one that answered.

    Provable headless and independent of any ambient ``DISPLAY``: the mock
    desktop reports a distinctive technology id, and a mock platform device call
    succeeds where a real backend would have failed to connect at construction.
    """
    info = runtime.desktop_info()
    assert info['technology'] == 'MockPlatform'
    position = runtime.pointer_position()
    assert hasattr(position, 'x')
    assert hasattr(position, 'y')


def test_mock_backend_config_selects_mock_platform() -> None:
    """(b) A ``platform.backend='mock'`` config selects the mock backend."""
    runtime = pn.Runtime({'platform': {'backend': 'mock'}})
    try:
        _assert_mock_platform(runtime)
    finally:
        runtime.shutdown()


def test_empty_buckets_and_subdicts_are_tolerated() -> None:
    """(a) Empty ``providers`` bucket and an empty backend sub-dict are no-ops."""
    runtime = pn.Runtime({'platform': {'backend': 'mock', 'x11': {}}, 'providers': {}})
    try:
        _assert_mock_platform(runtime)
    finally:
        runtime.shutdown()


def test_unknown_and_foreign_keys_are_tolerated() -> None:
    """(c) Unclaimed ids/keys are ignored, and every leaf value type parses.

    A portable dict may carry every OS's block plus provider settings; an id or
    key no registered component claims (a foreign-OS block, a typo) is ignored,
    never an error. The mixed leaf values (str, int, float, bool, nested dict,
    list) exercise the whole ``dict`` -> ``ConfigValue`` parser without raising.
    """
    config = {
        'platform': {
            'backend': 'mock',
            'windows': {'highlight_color': '#ff0000'},  # foreign-OS block
            'x11': {'display': ':99'},  # unused when backend=mock
            'wayland': {'scale': 2, 'hidpi': True},  # int + bool leaves
            'bogus': {'nested': {'k': [1, 2.5, 'x', False]}},  # nested map + list
        },
        'providers': {
            'atspi': {'bus_address': 'unix:path=/nope'},  # unused for the mock
            'typo_provider': {'flag': True},  # unclaimed id
        },
    }
    runtime = pn.Runtime(config)
    try:
        _assert_mock_platform(runtime)
    finally:
        runtime.shutdown()


def test_non_dict_top_level_section_is_ignored() -> None:
    """(c) A top-level bucket whose value is not a dict is ignored with a warning, not an error."""
    runtime = pn.Runtime({'platform': {'backend': 'mock'}, 'providers': 'not-a-dict'})
    try:
        _assert_mock_platform(runtime)
    finally:
        runtime.shutdown()


def test_new_with_mock_still_works() -> None:
    """Regression: the no-arg mock convenience is unchanged by the config binding.

    ``new_with_mock`` yields the queryable in-memory mock *tree* (mock provider),
    which the config path above deliberately does not — that path discovers the
    real providers and only forces the mock *platform*.
    """
    runtime = pn.Runtime.new_with_mock()
    try:
        result = runtime.evaluate_single('/')
        assert isinstance(result, pn.UiNode)
        assert result.role == 'Desktop'
    finally:
        runtime.shutdown()


def test_a_config_the_binding_can_pass_on_warns_nothing(caplog_warnings: pytest.LogCaptureFixture) -> None:
    config = {
        'platform': {'backend': 'mock', 'wayland': {'scale': 2, 'hidpi': True, 'ratio': 1.5}},
        'providers': {'bogus': {'nested': {'k': [1, 2.5, 'x', False, ('t',)]}}},
    }
    assert warnings_of_construction(caplog_warnings, config) == []


def test_a_misspelled_bucket_warns_naming_it_and_the_buckets(caplog_warnings: pytest.LogCaptureFixture) -> None:
    config = {'platform': {'backend': 'mock'}, 'platfrom': {'x11': {'display': ':1'}}}
    [warning] = warnings_of_construction(caplog_warnings, config)
    assert 'platfrom' in warning
    assert 'platform' in warning.replace('platfrom', '')
    assert 'providers' in warning


def test_a_bucket_that_is_not_a_dict_warns_naming_it_and_its_type(caplog_warnings: pytest.LogCaptureFixture) -> None:
    [warning] = warnings_of_construction(caplog_warnings, {'platform': {'backend': 'mock'}, 'providers': 'atspi'})
    assert 'providers' in warning
    assert 'found=str' in warning


def test_a_key_that_is_not_a_string_warns_naming_its_path_and_type(
    caplog_warnings: pytest.LogCaptureFixture,
) -> None:
    [warning] = warnings_of_construction(caplog_warnings, {'platform': {'backend': 'mock', 1: {}}})
    assert 'key=platform.1' in warning
    assert 'found=int' in warning


def test_a_value_of_an_unsupported_type_warns_naming_its_path_and_type(
    caplog_warnings: pytest.LogCaptureFixture,
) -> None:
    display = pathlib.Path('/tmp/display')
    config = {'platform': {'backend': 'mock', 'x11': {'display': display}}}
    [warning] = warnings_of_construction(caplog_warnings, config)
    assert 'key=platform.x11.display' in warning
    assert f'found={type(display).__name__}' in warning
    assert 'Path' in warning


def test_a_list_element_of_an_unsupported_type_warns_naming_its_index(
    caplog_warnings: pytest.LogCaptureFixture,
) -> None:
    config = {'platform': {'backend': 'mock'}, 'providers': {'bogus': {'hosts': ['a', object()]}}}
    [warning] = warnings_of_construction(caplog_warnings, config)
    assert 'key=providers.bogus.hosts[1]' in warning
    assert 'found=object' in warning


if __name__ == '__main__':
    import sys

    sys.exit('Please run this module with pytest.')
