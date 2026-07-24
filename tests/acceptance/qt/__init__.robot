*** Settings ***
Documentation       Qt (PySide6) acceptance suites — they drive the REAL platform
...                 runtime against apps/test-app-qt: AT-SPI on Linux (X11 and the
...                 PlatynUI Wayland compositor) and UIA on Windows.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the
...                 mock-provider feature (``just build-native``). A mock-provider
...                 build makes ``Runtime()`` resolve the built-in mock tree
...                 instead of the real provider, so these tests fail.
...
...                 APP ENVIRONMENT: apps/test-app-qt is a standalone uv project.
...                 Its interpreter + entrypoint are prepared and handed over via
...                 ``PLATYNUI_TEST_APP_QT_PYTHON`` / ``PLATYNUI_TEST_APP_QT_MAIN``
...                 by scripts/platynui-robot-session.sh (Linux) or the
...                 ``test-acceptance-windows`` recipe (Windows). Robot Framework
...                 launches the interpreter DIRECTLY (never via ``uv run``, which
...                 spawns Python as a child whose PID differs from uv's and would
...                 break the ``@ProcessId`` window pinning).
...
...                 All tests here are tagged ``acceptance`` (the test level) and
...                 ``real`` (the build requirement) so the run matches the build.
...                 This top-level suite launches nothing: each child suite starts
...                 and tears down the instance(s) it needs (pinned by ProcessId),
...                 so every suite begins from a known state — see
...                 resources/testapp.resource.

Test Tags           acceptance    real
