# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for ``PlatynUI.core.runtime``.

The accessor is a process-wide singleton, so most tests instantiate a
fresh ``Runtime()`` (== ``_RuntimeAccessor``) instead of touching the
module-level ``runtime`` object.  That keeps tests independent and
avoids cross-test pollution.  A handful of dedicated tests exercise the
module-level singleton itself.
"""

from unittest.mock import MagicMock

import pytest

from PlatynUI.core.runtime import Runtime, runtime

# ----------------------------------------------------------------------
# Module-level singleton smoke tests
# ----------------------------------------------------------------------


def test_module_singleton_is_runtime_instance() -> None:
    """``runtime`` is an instance of the public ``Runtime`` class."""
    assert isinstance(runtime, Runtime)


def test_runtime_class_alias_matches_singleton_type() -> None:
    """The exported ``Runtime`` symbol is the singleton's class."""
    assert type(runtime) is Runtime


# ----------------------------------------------------------------------
# Initial state
# ----------------------------------------------------------------------


def test_fresh_accessor_is_not_initialised() -> None:
    """A new accessor reports no instance and is not sealed."""
    rt = Runtime()
    assert rt.is_initialised() is False
    assert rt.is_sealed() is False


# ----------------------------------------------------------------------
# Lazy default build
# ----------------------------------------------------------------------


def test_current_lazy_creates_default(monkeypatch: pytest.MonkeyPatch) -> None:
    """The first ``current`` access builds via the default factory."""
    sentinel = MagicMock(name='DefaultNativeRuntime')
    factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    rt = Runtime()
    assert rt.is_initialised() is False
    obtained = rt.current
    factory.assert_called_once_with()
    assert obtained is sentinel
    assert rt.is_initialised() is True
    assert rt.is_sealed() is True


def test_current_caches_built_instance(monkeypatch: pytest.MonkeyPatch) -> None:
    """Subsequent ``current`` reads return the same cached instance."""
    sentinel = MagicMock(name='DefaultNativeRuntime')
    factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', factory)

    rt = Runtime()
    first = rt.current
    second = rt.current
    assert first is second
    factory.assert_called_once_with()


# ----------------------------------------------------------------------
# Variant selection (use_*)
# ----------------------------------------------------------------------


def test_use_factory_replaces_builder_before_seal() -> None:
    """``use_factory`` swaps the builder; ``current`` invokes the new one."""
    sentinel = MagicMock(name='Custom')
    factory = MagicMock(return_value=sentinel)

    rt = Runtime()
    rt.use_factory(factory)
    assert rt.is_initialised() is False  # not built yet

    obtained = rt.current
    factory.assert_called_once_with()
    assert obtained is sentinel


def test_use_factory_can_be_called_multiple_times_before_seal() -> None:
    """The most recent ``use_*`` call wins until ``current`` seals it."""
    first_factory = MagicMock(return_value=MagicMock(name='first'))
    second_sentinel = MagicMock(name='second')
    second_factory = MagicMock(return_value=second_sentinel)

    rt = Runtime()
    rt.use_factory(first_factory)
    rt.use_factory(second_factory)

    assert rt.current is second_sentinel
    first_factory.assert_not_called()
    second_factory.assert_called_once_with()


def test_use_default_resets_to_default_builder(monkeypatch: pytest.MonkeyPatch) -> None:
    """``use_default`` undoes a prior variant choice before seal."""
    sentinel = MagicMock(name='Default')
    default_factory = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native, 'Runtime', default_factory)

    rt = Runtime()
    rt.use_factory(MagicMock(return_value=MagicMock(name='custom')))
    rt.use_default()

    assert rt.current is sentinel
    default_factory.assert_called_once_with()


def test_use_mock_selects_mock_builder(monkeypatch: pytest.MonkeyPatch) -> None:
    """``use_mock`` routes through ``Runtime.new_with_mock``."""
    sentinel = MagicMock(name='MockRuntime')
    new_with_mock = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native.Runtime, 'new_with_mock', new_with_mock)

    rt = Runtime()
    rt.use_mock()
    obtained = rt.current

    new_with_mock.assert_called_once_with()
    assert obtained is sentinel


# ----------------------------------------------------------------------
# Sealing
# ----------------------------------------------------------------------


def test_use_default_after_seal_raises(monkeypatch: pytest.MonkeyPatch) -> None:
    """``use_default`` rejects calls after ``current`` has been read."""
    monkeypatch.setattr(
        'platynui_native.Runtime',
        MagicMock(return_value=MagicMock()),
    )

    rt = Runtime()
    _ = rt.current  # seal
    with pytest.raises(RuntimeError, match='already initialised'):
        rt.use_default()


def test_use_mock_after_seal_raises(monkeypatch: pytest.MonkeyPatch) -> None:
    """``use_mock`` rejects calls after sealing."""
    monkeypatch.setattr(
        'platynui_native.Runtime',
        MagicMock(return_value=MagicMock()),
    )

    rt = Runtime()
    _ = rt.current
    with pytest.raises(RuntimeError, match='already initialised'):
        rt.use_mock()


def test_use_factory_after_seal_raises(monkeypatch: pytest.MonkeyPatch) -> None:
    """``use_factory`` rejects calls after sealing."""
    monkeypatch.setattr(
        'platynui_native.Runtime',
        MagicMock(return_value=MagicMock()),
    )

    rt = Runtime()
    _ = rt.current
    with pytest.raises(RuntimeError, match='already initialised'):
        rt.use_factory(MagicMock())


# ----------------------------------------------------------------------
# Override context manager
# ----------------------------------------------------------------------


def test_override_with_builder_activates_and_restores() -> None:
    """``override`` activates the built instance and restores on exit."""
    rt = Runtime()
    override_instance = MagicMock(name='Override')

    with rt.override(lambda: override_instance) as active:
        assert active is override_instance
        assert rt.current is override_instance
        assert rt.is_sealed() is True

    # After exit: snapshot restored — original (unsealed) state.
    assert rt.is_initialised() is False
    assert rt.is_sealed() is False
    override_instance.shutdown.assert_called_once_with()


def test_override_invokes_builder_once_on_enter() -> None:
    """The builder is invoked exactly once when the context is entered."""
    rt = Runtime()
    sentinel = MagicMock(name='Built')
    builder = MagicMock(return_value=sentinel)

    with rt.override(builder) as active:
        assert active is sentinel
        assert rt.current is sentinel
    builder.assert_called_once_with()
    sentinel.shutdown.assert_called_once_with()


def test_override_works_after_seal(monkeypatch: pytest.MonkeyPatch) -> None:
    """``override`` is permitted even after the accessor is sealed."""
    base = MagicMock(name='Base')
    monkeypatch.setattr('platynui_native.Runtime', MagicMock(return_value=base))

    rt = Runtime()
    assert rt.current is base  # seal with default

    override_instance = MagicMock(name='Override')
    with rt.override(lambda: override_instance):
        assert rt.current is override_instance

    # Original instance restored, no shutdown on the base instance.
    assert rt.current is base
    base.shutdown.assert_not_called()
    override_instance.shutdown.assert_called_once_with()


def test_override_supports_nesting() -> None:
    """Nested ``override`` blocks restore in LIFO order."""
    rt = Runtime()
    outer = MagicMock(name='Outer')
    inner = MagicMock(name='Inner')

    with rt.override(lambda: outer):
        assert rt.current is outer
        with rt.override(lambda: inner):
            assert rt.current is inner
        assert rt.current is outer
        inner.shutdown.assert_called_once_with()
        outer.shutdown.assert_not_called()

    assert rt.is_initialised() is False
    outer.shutdown.assert_called_once_with()


def test_override_swallows_shutdown_exceptions() -> None:
    """A failing ``shutdown`` on the override does not propagate."""
    rt = Runtime()
    override_instance = MagicMock(name='Override')
    override_instance.shutdown.side_effect = RuntimeError('boom')

    with rt.override(lambda: override_instance):
        pass  # exit must not raise

    assert rt.is_initialised() is False


def test_override_with_mock_builds_via_new_with_mock(monkeypatch: pytest.MonkeyPatch) -> None:
    """``override_with_mock`` calls ``Runtime.new_with_mock`` on enter."""
    sentinel = MagicMock(name='MockRuntime')
    new_with_mock = MagicMock(return_value=sentinel)
    import platynui_native

    monkeypatch.setattr(platynui_native.Runtime, 'new_with_mock', new_with_mock)

    rt = Runtime()
    with rt.override_with_mock() as active:
        new_with_mock.assert_called_once_with()
        assert active is sentinel
        assert rt.current is sentinel

    sentinel.shutdown.assert_called_once_with()


def test_override_restores_previous_variant_choice() -> None:
    """After an override, an earlier ``use_*`` choice is preserved."""
    rt = Runtime()
    pre_choice = MagicMock(return_value=MagicMock(name='PreChoice'))
    rt.use_factory(pre_choice)

    with rt.override(lambda: MagicMock(name='Override')):
        pass

    # Builder restored: ``current`` should now invoke the pre-override
    # factory, not fall back to the default.
    _ = rt.current
    pre_choice.assert_called_once_with()
