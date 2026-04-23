# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Process-wide PlatynUI runtime accessor.

The PlatynUI library wraps a single :class:`platynui_native.Runtime`
that owns the provider cache, pointer/keyboard profiles and desktop
view.  Most call sites (adapters, device proxies, future Robot keywords,
the BareMetal helper, the inspector) need *the same* runtime; passing
it explicitly through every constructor is impractical and clashes with
the Robot Framework keyword idiom (no per-call context).

This module therefore exposes the runtime through a singleton object,
``runtime``, with three operations:

* :attr:`Runtime.current` — lazy accessor; creates a default
  :class:`platynui_native.Runtime` on first use.
* :meth:`Runtime.set` — replace the active runtime (mainly for tests
  that want to substitute :func:`platynui_native.Runtime.new_with_mock`).
* :meth:`Runtime.reset` — drop the active runtime; the next access
  triggers a fresh lazy creation.

Thread safety
-------------

The accessor itself is guarded by a re-entrant lock; the underlying
:class:`platynui_native.Runtime` is internally backed by a Rust
``Mutex`` and is safe to share across Python threads.  Pointer and
keyboard methods are global (they target absolute screen coordinates
and the OS event queue), so swapping the runtime mid-session does not
invalidate :class:`platynui_native.UiNode` instances obtained earlier:
those continue to reference their original provider tree.

Tests
-----

Tests that need provider isolation should use a fixture that swaps in
``Runtime.new_with_mock()`` and resets afterwards::

    @pytest.fixture
    def mock_runtime():
        from PlatynUI.core.runtime import runtime
        from platynui_native import Runtime as NativeRuntime
        rt = NativeRuntime.new_with_mock()
        runtime.set(rt)
        try:
            yield rt
        finally:
            runtime.reset()
"""

from __future__ import annotations

from threading import RLock
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from platynui_native import Runtime as _NativeRuntime

__all__ = ['Runtime', 'runtime']


class _RuntimeAccessor:
    """Singleton holder for the process-wide :class:`platynui_native.Runtime`.

    Exposed module-side as :data:`runtime`; instantiated exactly once.
    """

    def __init__(self) -> None:
        self._instance: _NativeRuntime | None = None
        self._lock = RLock()

    @property
    def current(self) -> _NativeRuntime:
        """Return the active runtime, creating a default one on first use.

        The default constructor (``platynui_native.Runtime()``) discovers
        statically registered providers via ``inventory``.  Callers that
        need a different configuration (mock provider, custom factory)
        must :meth:`set` an instance before the first :attr:`current`
        access — or :meth:`reset` and re-:meth:`set`.
        """
        with self._lock:
            if self._instance is None:
                # Local import keeps module import cheap and avoids a
                # hard dependency at import time (helpful for docs and
                # type-only tooling).
                from platynui_native import Runtime as NativeRuntime

                self._instance = NativeRuntime()
            return self._instance

    def set(self, runtime: _NativeRuntime) -> None:
        """Replace the active runtime.

        Calling :meth:`set` does *not* shut down a previously installed
        runtime — the caller owns the lifecycle of the object it passes
        in.  Use :meth:`reset` to dispose of the current runtime via
        ``shutdown()`` before installing a new one.
        """
        with self._lock:
            self._instance = runtime

    def reset(self) -> None:
        """Drop the current runtime and call ``shutdown()`` on it.

        After :meth:`reset`, the next :attr:`current` access creates a
        fresh default runtime.  Tests typically call :meth:`reset` in a
        teardown fixture to leave the process in a clean state.
        """
        with self._lock:
            if self._instance is not None:
                try:
                    self._instance.shutdown()
                except Exception:  # best-effort teardown; never propagate
                    pass
                self._instance = None

    def is_initialised(self) -> bool:
        """Return ``True`` iff a runtime has been created or installed.

        Useful in diagnostic/teardown code that wants to avoid
        triggering a lazy construction just to check the state.
        """
        with self._lock:
            return self._instance is not None


# Public alias mirroring the singleton type so consumers can spell out
# ``PlatynUI.core.runtime.Runtime`` in type annotations if they need to.
Runtime = _RuntimeAccessor

#: Process-wide runtime singleton.  Use ``runtime.current`` to access
#: the underlying :class:`platynui_native.Runtime`.
runtime: _RuntimeAccessor = _RuntimeAccessor()
