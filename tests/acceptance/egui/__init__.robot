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
...                 The default app instance ("PlatynUI Test App") is launched
...                 here in Suite Setup and torn down in Suite Teardown, so the
...                 child suites can assume it is running. Suites that need more
...                 than one window (e.g. auto_activate) launch additional
...                 instances themselves via resources/testapp.resource.

Resource            resources/testapp.resource

Suite Setup         Launch Test App    PlatynUI Test App    com.platynui.test
Suite Teardown      Terminate Test Apps

Test Tags           real
