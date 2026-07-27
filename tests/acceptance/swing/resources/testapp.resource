*** Settings ***
Documentation     Launch and tear down Swing fixture instances (apps/test-app-swing) from Robot
...               Framework, so each suite owns the instance(s) it needs — the top-level suite launches
...               nothing.
...
...               The fixture is plain Java 8 run straight from its class directory (the Gradle build's
...               classes output). The directory and the launch JVM (the provisioned Java 8 by default)
...               come from PLATYNUI_TEST_APP_SWING_CLASSES / PLATYNUI_TEST_APP_SWING_JAVA (prepared by
...               the test-acceptance-windows recipe; set them yourself when running outside it, PATH
...               ``java`` is the fallback launcher). Every launch passes
...               -Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge,
...               enabling the Access Bridge for that process only — no jabswitch, no machine state.
...
...               ``Launch Default Swing Instance`` pins its instance as the suite-wide query root
...               (``Set Root`` on ``/app:Application[@ProcessId=...]``), so suite locators are
...               relative (``.//``) and address controls by their explicit unique accessibleName —
...               JAB has no AutomationId equivalent, so ``@Name`` is the locator contract. The main
...               window's ``@Name`` is its (per-suite unique) launch title, exposed as
...               ``${SWING_TITLE}``, so parallel instances never cross-talk.
...
...               THE AGENT IS SWITCHED OFF HERE, deliberately. These suites are *about* the Access
...               Bridge: they assert ``@Technology = "JAB"``, the renderer-aliased cell names, and the
...               bridge's role quirks. Since ``provider-java-swing`` the in-JVM agent is the preferred
...               backend and ``providers.java.agent.auto_attach`` defaults to on, so a Swing JVM does
...               not *stay* agent-less — the provider injects one and serves the better representation.
...               A suite that wants the bridge therefore has to say so, rather than relying on an
...               absence the provider is actively removing. Agent-backed behaviour is covered by its
...               own suites and by the real-provider live fixture
...               (``crates/provider-java/tests/live_fixture.rs``).
Resource          swing_env.resource
Library           PlatynUI.BareMetal
...               config={'providers': {'java': {'agent': {'enabled': False}}}}
...               AS    BM


*** Variables ***
# Handle/title of the instance started by Launch Default Swing Instance.
${SWING_APP_HANDLE}     ${None}
${SWING_TITLE}          ${None}


*** Keywords ***
Launch Swing Test App
    [Documentation]    Start one fixture instance with a unique window title and wait until its main
    ...    window — pinned by the launched ProcessId — is on the accessibility tree with
    ...    ``@Technology = "JAB"``. Extra CLI args are forwarded. Returns the Process handle; the
    ...    query root is untouched (use Launch Default Swing Instance for the pinned suite instance).
    [Arguments]    ${title}    @{extra_args}
    ${handle}=    Start Swing Fixture Process    ${title}    @{extra_args}
    ${pid}=    Get Process Id    ${handle}
    BM.Wait Until Exists    /app:Application[@ProcessId=${pid}]//Window[@Name="${title}"]
    ...    query_overrides={'timeout': ${APP_WAIT_TIMEOUT}}
    RETURN    ${handle}

Launch Default Swing Instance
    [Documentation]    Suite Setup: check prerequisites, launch one fixture instance with a
    ...    suite-unique title, and pin it as the suite-wide query root (``Set Root`` on its
    ...    ``app:Application`` node). Exposes ``${SWING_TITLE}`` (the window's @Name) and the Process
    ...    handle at SUITE scope.
    [Arguments]    ${title}
    Require Swing Prerequisites
    ${handle}=    Launch Swing Test App    ${title}
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
    VAR    ${SWING_TITLE}    ${title}    scope=SUITE
    VAR    ${SWING_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Swing Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Swing
    ...    Instance (ignoring errors for instances that already exited, e.g. after a close test).
    ...    ``kill=True`` because a JVM ignores the "graceful" CTRL_BREAK (it just dumps threads),
    ...    so the default path always burns the Process library's 30 s terminate timeout.
    Run Keyword And Ignore Error    Terminate Process    ${SWING_APP_HANDLE}    kill=${True}
