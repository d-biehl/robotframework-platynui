*** Settings ***
Documentation       Process-level suites against apps/win32-test-window — they drive the REAL
...                 Windows runtime (UI Automation).
...
...                 The window is a helper, not a fixture of the blueprint: it has a process and a
...                 window, and no controls. Its bitness follows the build target, and the
...                 ``test-acceptance-windows`` recipe builds it for 32-bit x86
...                 (``just build-win32-test-window-x86``, which needs the Rust target
...                 ``i686-pc-windows-msvc``) and hands it over via
...                 ``PLATYNUI_WIN32_TEST_WINDOW_X86``. A missing binary FAILS the suites with an
...                 actionable message — it never skips.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the mock-provider
...                 feature (``just build-native``).
...
...                 All tests are tagged ``acceptance``, ``real`` and ``platform:windows``, so only the
...                 ``real-windows`` lane profile selects them. This top-level suite launches nothing:
...                 each child suite starts and ends the window it needs, pinned by ProcessId.

Test Tags           acceptance    real    platform:windows
