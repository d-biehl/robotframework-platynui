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
  empty ⇒ current behaviour", assertion "a").

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

import platynui_native as pn


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
    """(c) A top-level bucket whose value is not a dict is ignored, not an error."""
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


if __name__ == '__main__':
    import sys

    sys.exit('Please run this module with pytest.')
