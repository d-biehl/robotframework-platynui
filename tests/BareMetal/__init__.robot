*** Settings ***
Documentation       Mock-backed BareMetal suites — they drive the
...                 ``PlatynUI.BareMetal`` library against the built-in mock tree
...                 (``use_mock=${True}``), so they are deterministic, need no
...                 display, and run on any OS.
...
...                 BUILD REQUIREMENT: the native module must be built WITH the
...                 mock-provider feature (``just build-native-mock``); a non-mock
...                 build cannot create the mock Runtime, so the imports fail. This
...                 is the counterpart to the ``real``-tagged egui acceptance lane.
...
...                 All tests here are tagged ``mock`` and selected by the ``mock``
...                 profile in robot.toml (``robotcode --profile mock run``, or
...                 ``just test-baremetal``).

Test Tags           mock
