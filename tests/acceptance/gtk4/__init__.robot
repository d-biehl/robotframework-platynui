*** Settings ***
Documentation       GTK 4 acceptance suites — they drive the REAL platform runtime against
...                 apps/test-app-gtk4 (a blueprint fixture, dev-docs/testing-strategy.md §5.1)
...                 through GTK 4's own AT-SPI implementation, on both Linux lanes.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the mock-provider
...                 feature (``just build-native``). A mock-provider build makes ``Runtime()``
...                 resolve the built-in mock tree instead of the real provider.
...
...                 APP ENVIRONMENT: the fixture's interpreter + entrypoint are handed over via
...                 ``PLATYNUI_TEST_APP_GTK4_PYTHON`` / ``PLATYNUI_TEST_APP_GTK4_MAIN`` by
...                 scripts/platynui-robot-session.sh. PyGObject is a Linux dev dependency of the
...                 project venv; GTK 4 and its typelib (``gir1.2-gtk-4.0``) come from the system.
...
...                 All tests here are tagged ``acceptance`` (the test level), ``real`` (the build
...                 requirement) and ``platform:linux``: GTK is tested on Linux only, where both
...                 sessions run it natively. Tests that open a popover
...                 (menus, the context menu, the drop-down) additionally carry
...                 ``requires:gtk-4.18`` — older GTK 4 releases leave a popover's offset out of
...                 their accessible tree.
...                 This top-level suite launches nothing: each child suite starts and tears down
...                 the instance it needs (pinned by ProcessId).

Test Tags           acceptance    real    platform:linux
