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
Launch Default Swing Agent Instance
    [Documentation]    Suite Setup: check prerequisites, launch one fixture instance with a
    ...    suite-unique title, wait until the in-JVM agent serves its window, and pin the instance as
    ...    the suite-wide query root. Exposes ``${SWING_AGENT_TITLE}`` (the window's @Name) and the
    ...    Process handle at SUITE scope.
    [Arguments]    ${title}
    Require Swing Prerequisites
    ${handle}=    Start Swing Fixture Process    ${title}
    ${pid}=    Get Process Id    ${handle}
    BM.Wait Until Exists
    ...    /app:Application[@ProcessId=${pid}]//Window[@Name="${title}"][@Technology="JavaAgent"]
    ...    query_overrides={'timeout': ${AGENT_WAIT_TIMEOUT}}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
    VAR    ${SWING_AGENT_TITLE}    ${title}    scope=SUITE
    VAR    ${SWING_AGENT_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Swing Agent Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Swing
    ...    Agent Instance. ``kill=True`` because a JVM ignores the "graceful" CTRL_BREAK (it just
    ...    dumps threads), so the default path always burns the Process library's 30 s timeout.
    Run Keyword And Ignore Error    Terminate Process    ${SWING_AGENT_APP_HANDLE}    kill=${True}
