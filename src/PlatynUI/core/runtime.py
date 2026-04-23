# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Process-wide PlatynUI runtime accessor.

The PlatynUI library wraps a single :class:`platynui_native.Runtime` per
process.  Provider cache, pointer/keyboard profiles and the desktop view
all belong to that runtime; adapters, device proxies, future Robot
keywords, the BareMetal helper and the inspector all share *the same*
instance.  Threading the runtime through every constructor would clash
with the Robot Framework keyword idiom (no per-call context) and bloat
the user-facing API.

This module exposes the runtime as a singleton (:data:`runtime`) with
three clearly separated responsibilities:

1. **Variant selection** — before first use, callers may pick *which*
   runtime should be built (default, mock-provider, custom factory).
   Methods: :meth:`Runtime.use_default`, :meth:`Runtime.use_mock`,
   :meth:`Runtime.use_factory`.
2. **Consumption** — every component reads :attr:`Runtime.current` and
   gets the same, once-built instance.  The first read seals the
   accessor; further variant selection is rejected.
3. **Test override** — tests install an alternative runtime via the
   context manager :meth:`Runtime.override` (or
   :meth:`Runtime.override_with_mock`), which guarantees restore of the
   previous state on exit.

There is intentionally **no** raw setter that would inject an external
native runtime into the singleton.  Every path goes through either a
variant choice or a scope-bound override — that rules out forgotten
resets by construction and prevents accidental swaps during live
operations through sealing.

Thread safety
-------------

The accessor is guarded by a re-entrant lock.  The underlying
:class:`platynui_native.Runtime` is internally backed by a Rust
``Mutex`` and is safe to share across Python threads.  Pointer and
keyboard methods target absolute screen coordinates and the OS event
queue, so an override mid-session does not invalidate
:class:`platynui_native.UiNode` instances obtained earlier — those keep
referencing their original provider tree.

Tests
-----

Mock-based tests install a mock-backed runtime through the override
context manager — no manual setting, no forgotten reset::

    @pytest.fixture
    def native_runtime():
        from PlatynUI.core import runtime
        with runtime.override_with_mock() as rt:
            yield rt
"""

from __future__ import annotations

from contextlib import contextmanager
from threading import RLock
from typing import TYPE_CHECKING, Callable, Iterator

if TYPE_CHECKING:
    from platynui_native import Runtime as _NativeRuntime

__all__ = ['Runtime', 'runtime']


_Builder = Callable[[], '_NativeRuntime']


def _default_builder() -> _NativeRuntime:
    """Build a default runtime via auto-discovery."""
    # Local import keeps module import cheap and avoids a hard
    # dependency at import time (helpful for docs and type-only tooling).
    from platynui_native import Runtime as NativeRuntime

    return NativeRuntime()


def _mock_builder() -> _NativeRuntime:
    """Build a runtime backed by the bundled mock provider."""
    from platynui_native import Runtime as NativeRuntime

    return NativeRuntime.new_with_mock()


def _shutdown_quietly(instance: _NativeRuntime | None) -> None:
    """Call ``shutdown()`` on *instance*, swallowing any exception."""
    if instance is None:
        return
    try:
        instance.shutdown()
    except Exception:  # best-effort teardown; never propagate
        pass


class _RuntimeAccessor:
    """Singleton holder for the process-wide :class:`platynui_native.Runtime`.

    Exposed module-side as :data:`runtime`; instantiated exactly once.
    See module docstring for the design rationale and lifecycle rules.
    """

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
        """Return ``True`` iff a runtime instance has been built.

        Pure ``use_*`` calls do *not* count — the runtime is built
        lazily on first :attr:`current` access.
        """
        with self._lock:
            return self._instance is not None

    def is_sealed(self) -> bool:
        """Return ``True`` iff variant selection is no longer permitted.

        The accessor seals as soon as :attr:`current` materialises an
        instance.  Inside an :meth:`override` block, the override
        instance is sealed too — release the block to restore the
        previous (possibly unsealed) state.
        """
        with self._lock:
            return self._instance is not None

    # ------------------------------------------------------------------
    # Consumption
    # ------------------------------------------------------------------

    @property
    def current(self) -> _NativeRuntime:
        """Return the active runtime, building it on first use.

        The first read evaluates the currently selected builder
        (:meth:`use_default` is the implicit fallback) and seals the
        accessor.  After sealing, :meth:`use_default`, :meth:`use_mock`
        and :meth:`use_factory` raise :class:`RuntimeError`; install
        alternative runtimes through :meth:`override` instead.
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

        Equivalent to *not* calling any ``use_*`` method.  Still useful
        when a previous variant choice (e.g. in test setup) needs to be
        explicitly undone before the first :attr:`current` access.
        """
        self._set_builder(_default_builder)

    def use_mock(self) -> None:
        """Select the mock-provider runtime as the next build.

        Requires the native extension to be built with the
        ``mock-provider`` Cargo feature; otherwise the build raises
        :class:`platynui_native.ProviderError`.
        """
        self._set_builder(_mock_builder)

    def use_factory(self, factory: _Builder) -> None:
        """Select a custom builder callable as the next build.

        ``factory`` is invoked at most once (on first :attr:`current`
        access) and must return a :class:`platynui_native.Runtime`
        instance.  Use this for embedding scenarios that need bespoke
        provider configurations not covered by :meth:`use_default` or
        :meth:`use_mock`.
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
    def override(self, factory: _Builder) -> Iterator[_NativeRuntime]:
        """Temporarily activate an alternative runtime for the scope.

        ``factory`` is a zero-argument callable that produces the
        override :class:`platynui_native.Runtime`.  It is invoked once
        on enter; the previous builder *and* instance are pushed onto
        a LIFO stack.  On exit, ``shutdown()`` is invoked on the
        override instance (exceptions swallowed) and the previous
        state is restored.

        To override with an already-built instance, wrap it::

            with rt.override(lambda: existing_runtime):
                ...

        This explicit-callable contract avoids ambiguity: any object
        could be callable (e.g. test mocks), so the override API never
        guesses whether ``factory`` is a builder or an instance.

        Nested overrides are supported and follow LIFO semantics.
        """
        with self._lock:
            previous = (self._builder, self._instance)
            self._stack.append(previous)
            instance = factory()

            def _override_builder(_inst: _NativeRuntime = instance) -> _NativeRuntime:
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
    def override_with_mock(self) -> Iterator[_NativeRuntime]:
        """Temporarily activate a mock-backed runtime for the scope.

        Convenience wrapper around :meth:`override` that builds a
        :func:`platynui_native.Runtime.new_with_mock` instance on
        entry.  Intended for unit tests; requires the ``mock-provider``
        Cargo feature in the native extension.
        """
        with self.override(_mock_builder) as instance:
            yield instance


# Public alias mirroring the singleton type so consumers can spell out
# ``PlatynUI.core.runtime.Runtime`` in type annotations if they need to.
Runtime = _RuntimeAccessor

#: Process-wide runtime singleton.  Use ``runtime.current`` to access
#: the underlying :class:`platynui_native.Runtime`.
runtime: _RuntimeAccessor = _RuntimeAccessor()
