"""Native log levels requested by library instances, and delivery of native log records.

The native extension produces log records process-wide, and how detailed they
are is one process-wide setting. Each library instance may request a level;
while several requests are live, the most verbose one applies, and a request
ends when its library instance goes out of scope. Records reach Python
``logging`` — and through it the Robot Framework log — only when they are
delivered on the calling thread, which the libraries do around every keyword.
See ``dev-docs/python-bindings.md`` (Logging).
"""

import threading
from collections.abc import Callable
from types import ModuleType
from typing import Final

import platynui_native

#: The accepted levels, least verbose first.
NATIVE_LOG_LEVELS: Final = ('error', 'warn', 'info', 'debug', 'trace')


def validate_native_log_level(value: str) -> str:
    """Return ``value`` in canonical lower case, or raise naming the accepted levels."""
    level = value.lower()
    if level not in NATIVE_LOG_LEVELS:
        raise ValueError(f'native_log_level must be one of {", ".join(NATIVE_LOG_LEVELS)}, got {value!r}')
    return level


class LogLevelRequest:
    """One library instance's request; hand it back to withdraw it."""

    __slots__ = ('level',)

    def __init__(self, level: str) -> None:
        self.level = level


class NativeLogLevels:
    """The live requests and the level they add up to."""

    def __init__(self, native: ModuleType = platynui_native) -> None:
        self._native = native
        self._requests: list[LogLevelRequest] = []
        self._lock = threading.Lock()

    @property
    def effective(self) -> str | None:
        """The most verbose live request, ``None`` when there is none."""
        with self._lock:
            return self._effective()

    def request(self, level: str) -> LogLevelRequest:
        """Register a request for ``level`` and apply the resulting level."""
        request = LogLevelRequest(validate_native_log_level(level))
        set_log_level = self._set_log_level()
        with self._lock:
            self._requests.append(request)
            set_log_level(self._effective())
        return request

    def withdraw(self, request: LogLevelRequest) -> None:
        """End ``request`` and apply the level the remaining requests add up to."""
        with self._lock:
            if request not in self._requests:
                return
            self._requests.remove(request)
            self._set_log_level()(self._effective())

    def _effective(self) -> str | None:
        return max((r.level for r in self._requests), key=NATIVE_LOG_LEVELS.index, default=None)

    def _set_log_level(self) -> Callable[[str | None], None]:
        set_log_level: Callable[[str | None], None] | None = getattr(self._native, 'set_log_level', None)
        if set_log_level is None or not hasattr(self._native, 'flush_logs'):
            raise RuntimeError(
                'The installed platynui_native predates native logging (it has no set_log_level/flush_logs); '
                'rebuild the native module, for example with `just build-native`.'
            )
        return set_log_level


#: The process-wide registry the libraries use.
native_log_levels: Final = NativeLogLevels()


def flush_native_logs() -> None:
    """Deliver queued native log records to ``logging`` on the calling thread.

    Does nothing with an extension built before native logging existed.
    """
    flush_logs = getattr(platynui_native, 'flush_logs', None)
    if flush_logs is not None:
        flush_logs()


class NativeLogScope:
    """Library listener that ends an instance's level request when the instance goes out of scope.

    Robot Framework calls ``close`` when a suite-scoped library's suite ends; the
    listener then also delivers whatever the instance's runtime logged last.
    """

    ROBOT_LISTENER_API_VERSION = 3

    def __init__(self, request: LogLevelRequest | None) -> None:
        self._request = request

    def close(self) -> None:
        if self._request is not None:
            native_log_levels.withdraw(self._request)
            self._request = None
        flush_native_logs()
