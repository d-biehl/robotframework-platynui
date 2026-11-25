*** Settings ***
Documentation       Suites against apps/winforms-test-window — they drive the REAL Windows runtime
...                 (UI Automation) against a WinForms tab control on .NET Framework.
...
...                 The window is a helper, not a fixture of the blueprint: a PowerShell script that
...                 Windows PowerShell 5.1 runs, so it needs no build. Its tab headers have no window
...                 of their own, and UI Automation reports no runtime id for them (capability
...                 *uia-runtime-id*). A missing ``powershell.exe`` FAILS the suites with an actionable
...                 message — it never skips.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the mock-provider
...                 feature (``just build-native``).
...
...                 All tests are tagged ``acceptance``, ``real`` and ``platform:windows``, so only the
...                 ``real-windows`` lane profile selects them. This top-level suite launches nothing:
...                 each child suite starts and ends the window it needs, pinned by ProcessId.

Test Tags           acceptance    real    platform:windows
