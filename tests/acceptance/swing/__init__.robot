*** Settings ***
Documentation       Java Swing acceptance suites — they drive the REAL Windows runtime
...                 against apps/test-app-swing through the Java Access Bridge (JAB)
...                 provider (`crates/provider-jab`, OpenSpec capability `jab-provider`).
...
...                 BUILD REQUIREMENT: the native module must be built WITHOUT the
...                 mock-provider feature (``just build-native``). A mock-provider
...                 build makes ``Runtime()`` resolve the built-in mock tree instead
...                 of the real providers, so these tests fail.
...
...                 APP ENVIRONMENT: apps/test-app-swing is plain Java 8 compiled by
...                 ``just build-test-app-swing``. The class directory (and optionally
...                 the java launcher) are handed over via
...                 ``PLATYNUI_TEST_APP_SWING_CLASSES`` / ``PLATYNUI_TEST_APP_SWING_JAVA``
...                 by the ``test-acceptance-windows`` recipe. Every suite skips with a
...                 clear message when no JDK or no built fixture is available, and on
...                 non-Windows platforms (the JAB channel is Windows-only; Linux Swing
...                 rides AT-SPI via java-atk-wrapper in a future lane).
...
...                 All tests are tagged ``real``. This top-level suite launches
...                 nothing: each child suite starts and tears down the instance(s) it
...                 needs, pinned by ProcessId — see resources/testapp.resource.

Test Tags           real
