*** Settings ***
Documentation     Launch and tear down Swing fixture instances served through the **in-JVM agent**,
...               the counterpart to ``testapp.resource``.
...
...               The only real difference is the ``PlatynUI.BareMetal`` import: the agent is left at
...               its default (on), so the provider injects one into the fixture JVM and serves the
...               better representation. That is also why the wait is keyed on
...               ``@Technology = "JavaAgent"`` rather than on the window alone — a Swing window is
...               reachable through the Access Bridge from the moment it exists, and a suite about
...               the agent must not start asserting before the takeover has happened.
...
...               Everything else — where the fixture is, how it is started, what makes it a hard
...               prerequisite — is shared through ``swing_env.resource``.
Resource          swing_env.resource
Library           PlatynUI.BareMetal    AS    BM


*** Variables ***
# Injecting an agent and completing its handshake takes longer than finding a window.
${AGENT_WAIT_TIMEOUT}         45
# Handle/title of the instance started by Launch Default Swing Agent Instance.
${SWING_AGENT_APP_HANDLE}     ${None}
${SWING_AGENT_TITLE}          ${None}


*** Keywords ***
Launch Swing Agent Test App
    [Documentation]    Start one fixture instance with a unique window title and wait until the in-JVM
    ...    agent serves its main window — pinned by the launched ProcessId. Returns the Process handle;
    ...    the query root is untouched (use Launch Default Swing Agent Instance for the pinned suite
    ...    instance). ``java``, extra CLI args and further named arguments are forwarded to
    ...    ``Start Swing Fixture Process``.
    [Arguments]    ${title}    @{extra_args}    ${java}=${SWING_JAVA}    &{process_options}
    ${handle}=    Start Swing Fixture Process    ${title}    @{extra_args}    java=${java}    &{process_options}
    ${pid}=    Get Process Id    ${handle}
    BM.Wait Until Exists
    ...    /app:Application[@ProcessId=${pid}]//Window[@Name="${title}"][@Technology="JavaAgent"]
    ...    query_overrides={'timeout': ${AGENT_WAIT_TIMEOUT}}
    RETURN    ${handle}

Launch Default Swing Agent Instance
    [Documentation]    Suite Setup: check prerequisites, launch one fixture instance with a
    ...    suite-unique title, wait until the in-JVM agent serves its window, and pin the instance as
    ...    the suite-wide query root. Exposes ``${SWING_AGENT_TITLE}`` (the window's @Name) and the
    ...    Process handle at SUITE scope.
    ...
    ...    The root names the agent's technology as well: while an agent session is degraded its
    ...    application node is invalid, and a root pinned by the pid alone would be looked up again
    ...    as the Access Bridge's application node, which goes empty once the agent serves again.
    [Arguments]    ${title}
    Require Swing Prerequisites
    ${handle}=    Launch Swing Agent Test App    ${title}
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]    scope=SUITE
    VAR    ${SWING_AGENT_TITLE}    ${title}    scope=SUITE
    VAR    ${SWING_AGENT_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Swing Agent Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Swing
    ...    Agent Instance. ``kill=True`` because a JVM ignores the "graceful" CTRL_BREAK (it just
    ...    dumps threads), so the default path always burns the Process library's 30 s timeout.
    Run Keyword And Ignore Error    Terminate Process    ${SWING_AGENT_APP_HANDLE}    kill=${True}
