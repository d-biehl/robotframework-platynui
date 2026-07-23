*** Settings ***
Documentation       Qt Quick/QML acceptance suites — they drive the REAL platform
...                 runtime against apps/test-app-qml (the first full instance of
...                 the fixture blueprint, dev-docs/testing-strategy.md §5.1):
...                 UIA on Windows, AT-SPI on Linux.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the
...                 mock-provider feature (``just build-native``). A mock-provider
...                 build makes ``Runtime()`` resolve the built-in mock tree
...                 instead of the real provider, so these tests fail.
...
...                 APP ENVIRONMENT: the fixture's interpreter + entrypoint are
...                 prepared and handed over via ``PLATYNUI_TEST_APP_QML_PYTHON``
...                 / ``PLATYNUI_TEST_APP_QML_MAIN`` by
...                 scripts/platynui-robot-session.sh (Linux) or the
...                 ``test-acceptance-windows`` recipe (Windows). Robot Framework
...                 launches the interpreter DIRECTLY (never via ``uv run``, which
...                 spawns Python as a child whose PID differs from uv's and would
...                 break the ``@ProcessId`` window pinning).
...
...                 catalog.robot is the thin onboarding suite over the shared
...                 catalog keywords (tests/acceptance/resources/catalog.resource);
...                 the other suites cover QML-specific behavior (popup modes, the
...                 in-scene modal dialog, the custom-controls chapter).
...
...                 All tests here are tagged ``real`` so the run matches the
...                 build. This top-level suite launches nothing: each child suite
...                 starts and tears down the instance(s) it needs (pinned by
...                 ProcessId), so every suite begins from a known state.

Test Tags           real
