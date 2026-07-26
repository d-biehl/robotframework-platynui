# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Process-wide PlatynUI runtime accessor.

The runtime is exposed as a singleton (`runtime`). Callers select
the variant before first use via `use_default`, `use_mock`, or
`use_factory`, then read the built instance through `current`.
Tests install a scoped alternative through `override` (or
`override_with_mock`).
"""

from collections.abc import Callable, Generator
from contextlib import contextmanager
from threading import RLock
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from platynui_native import Runtime as _NativeRuntime

__all__ = ['Runtime', 'runtime']


_Builder = Callable[[], '_NativeRuntime']


def _default_builder() -> '_NativeRuntime':
    """Build a default runtime via auto-discovery."""
    # Local import keeps module import cheap and avoids a hard
    # dependency at import time (helpful for docs and type-only tooling).
    from platynui_native import Runtime as NativeRuntime

    return NativeRuntime()


def _mock_builder() -> '_NativeRuntime':
    """Build a runtime backed by the bundled mock provider."""
    from platynui_native import Runtime as NativeRuntime

    return NativeRuntime.new_with_mock()


def _shutdown_quietly(instance: '_NativeRuntime | None') -> None:
    """Call ``shutdown()`` on ``instance``, swallowing any exception."""
    if instance is None:
        return
    try:
        instance.shutdown()
    except Exception:  # noqa: BLE001, S110 (best-effort teardown; never propagate)
        pass


class _RuntimeAccessor:
    """Hold the process-wide `Runtime` singleton, exposed as `runtime`."""

    def __init__(self) -> None:
        self._builder: _Builder = _default_builder
        self._instance: _NativeRuntime | None = None
        # LIFO stack of (builder, instance) snapshots, populated by
        # ``override(...)`` and consumed on exit.
        self._stack: list[tuple[_Builder, _NativeRuntime | None]] = []
        self._lock = RLock()

    # ------------------------------------------------------------------
    # State inspection
    # ------------------------------------------------------------------

    def is_initialised(self) -> bool:
        """Return whether a runtime instance has been built.

        ``use_*`` calls do not count; the runtime is built lazily on
        first `current` access.
        """
        with self._lock:
            return self._instance is not None

    def is_sealed(self) -> bool:
        """Return whether variant selection is no longer permitted.

        The accessor seals as soon as `current` materialises an
        instance. Inside an `override` block, the override
        instance is sealed too; release the block to restore the
        previous (possibly unsealed) state.
        """
        with self._lock:
            return self._instance is not None

    # ------------------------------------------------------------------
    # Consumption
    # ------------------------------------------------------------------

    @property
    def current(self) -> '_NativeRuntime':
        """Return the active runtime, building it on first use.

        The first read evaluates the currently selected builder
        (`use_default` is the implicit fallback) and seals the
        accessor. After sealing, `use_default`, `use_mock`,
        and `use_factory` raise `RuntimeError`; install
        alternative runtimes through `override` instead.
        """
        with self._lock:
            if self._instance is None:
                self._instance = self._builder()
            return self._instance

    # ------------------------------------------------------------------
    # Variant selection (only valid before sealing)
    # ------------------------------------------------------------------

    def use_default(self) -> None:
        """Select the auto-discovery runtime as the next build.

        Equivalent to omitting any ``use_*`` call. Useful when a
        previous variant choice (e.g. in test setup) needs to be
        explicitly undone before the first `current` access.
        """
        self._set_builder(_default_builder)

    def use_mock(self) -> None:
        """Select the mock-provider runtime as the next build.

        Requires the native extension to be built with the
        ``mock-provider`` Cargo feature; otherwise the build raises
        `ProviderError`.
        """
        self._set_builder(_mock_builder)

    def use_factory(self, factory: _Builder) -> None:
        """Select a custom builder callable as the next build.

        ``factory`` is invoked at most once (on first `current`
        access) and must return a `Runtime`
        instance. Use this for embedding scenarios that need bespoke
        provider configurations not covered by `use_default` or
        `use_mock`.
        """
        self._set_builder(factory)

    def _set_builder(self, builder: _Builder) -> None:
        with self._lock:
            if self._instance is not None:
                raise RuntimeError(
                    'runtime already initialised; use override() instead',
                )
            self._builder = builder

    # ------------------------------------------------------------------
    # Test override (always permitted, scope-bound)
    # ------------------------------------------------------------------

    @contextmanager
    def override(self, factory: _Builder) -> Generator['_NativeRuntime']:
        """Activate an alternative runtime for the current scope.

        ``factory`` is a zero-argument callable that produces the
        override `Runtime`. It runs once on
        enter; the previous builder and instance are pushed onto a
        LIFO stack. On exit, ``shutdown()`` is invoked on the override
        instance (exceptions swallowed) and the previous state is
        restored.

        To override with an already-built instance, wrap it::

            with rt.override(lambda: existing_runtime):
                ...

        The explicit-callable contract avoids ambiguity: any object
        could be callable (e.g. test mocks), so the override API
        never guesses whether ``factory`` is a builder or an instance.

        Nested overrides are supported and follow LIFO semantics.
        """
        with self._lock:
            previous = (self._builder, self._instance)
            self._stack.append(previous)
            instance = factory()

            def _override_builder(_inst: '_NativeRuntime' = instance) -> '_NativeRuntime':
                return _inst

            self._builder = _override_builder
            self._instance = instance

        try:
            yield instance
        finally:
            with self._lock:
                # Defensive: stack should still hold our snapshot.
                # If a buggy caller manipulated it, restore as best
                # we can without raising.
                snapshot = self._stack.pop() if self._stack else previous
                _shutdown_quietly(self._instance)
                self._builder, self._instance = snapshot

    @contextmanager
    def override_with_mock(self) -> Generator['_NativeRuntime']:
        """Activate a mock-backed runtime for the current scope.

        Wraps `override` and builds a
        `new_with_mock` instance on
        entry. Intended for unit tests; requires the ``mock-provider``
        Cargo feature in the native extension.
        """
        with self.override(_mock_builder) as instance:
            yield instance


# Public alias mirroring the singleton type so consumers can spell out
# ``PlatynUI.core.runtime.Runtime`` in type annotations if they need to.
Runtime = _RuntimeAccessor

#: Process-wide runtime singleton. Read ``runtime.current`` to get the
#: underlying `Runtime`.
runtime: _RuntimeAccessor = _RuntimeAccessor()
