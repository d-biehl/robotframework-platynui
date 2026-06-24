*** Settings ***
Documentation       egui acceptance suites — they drive the REAL platform
...                 runtime (AT-SPI) against apps/test-app-egui.
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT
...                 the mock-provider feature (``just build-native``). A
...                 mock-provider build makes ``Runtime()`` resolve the built-in
...                 mock tree instead of the real provider, so these tests fail.
...
...                 All tests here are tagged ``real`` so the run can be matched
...                 to the build (e.g. ``robotcode robot -i real`` on a non-mock
...                 build; mock-tree suites are tagged ``mock`` and need
...                 ``just build-native-mock``).
...
...                 This top-level suite launches nothing: each child suite starts
...                 and tears down the instance(s) it needs (pinned by ProcessId),
...                 so every suite begins from a known state — see
...                 resources/testapp.resource.

Test Tags           real
