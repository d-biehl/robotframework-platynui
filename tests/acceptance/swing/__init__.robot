*** Settings ***
Documentation       Java Swing acceptance suites — they drive the REAL Windows runtime
...                 against apps/test-app-swing through the Java provider (OpenSpec
...                 capability `java-provider`).
...
...                 TWO BACKENDS, TWO RESOURCE FILES. Most suites here are about the
...                 Java Access Bridge (`crates/provider-java-jab`) and switch the
...                 in-JVM agent OFF via resources/testapp.resource, because the agent
...                 is the preferred backend and a Swing JVM does not otherwise stay
...                 agent-less. Suites about the agent itself use
...                 resources/testapp_agent.resource, which leaves it on and waits for
...                 `@Technology = "JavaAgent"`. Both share resources/swing_env.resource.
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
...                 by the ``test-acceptance-windows`` recipe, which builds the fixture
...                 as a hard prerequisite. A missing launcher or unbuilt fixture FAILS
...                 the suites with an actionable message — it never skips.
...
...                 All tests are tagged ``acceptance``, ``real``, and
...                 ``platform:windows`` (the JAB
...                 channel is Windows-only; Linux Swing rides AT-SPI via
...                 java-atk-wrapper in a future lane), so only the ``real-windows``
...                 lane profile selects them. This top-level suite launches nothing:
...                 each child suite starts and tears down the instance(s) it needs,
...                 pinned by ProcessId — see resources/testapp.resource.

Test Tags           acceptance    real    platform:windows
