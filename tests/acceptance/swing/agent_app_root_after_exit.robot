*** Settings ***
Documentation       The application node the in-JVM agent serves reports its process's end (spec
...                 *application-node-validity*): a root pinned to it is looked up again and fails with
...                 ``RootNotFoundError``, and a captured one is gone for ``Wait Until Gone``.
...
...                 Each test launches a fixture instance of its own and ends it. The agent node also
...                 turns invalid when its session closes or degrades; the unit tests of
...                 ``crates/provider-java`` cover those, since a lane cannot provoke them on demand.

Resource            resources/testapp_agent.resource

Suite Setup         Require Swing Prerequisites


*** Test Cases ***
A Root Pinned To An Ended Agent Application Is Looked Up Again
    ${handle}=    Launch Swing Agent Test App    PlatynUI Swing Agent Root Exit
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]    scope=LOCAL
    BM.Wait Until Exists    ./Window
    Terminate Process    ${handle}    kill=${True}
    # At the scope level, so that the root's own lookup is short as well.
    BM.Set Query Settings    {'timeout': 2}    scope=LOCAL
    Run Keyword And Expect Error
    ...    STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]', was not found
    ...    BM.Get Attribute    ./Window    Name
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}

A Captured Agent Application Node Is Gone Once Its Process Ended
    ${handle}=    Launch Swing Agent Test App    PlatynUI Swing Agent Node Exit
    ${pid}=    Get Process Id    ${handle}
    ${application}=    BM.Query    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]    only_first=${True}
    Should Not Be Equal    ${application}    ${None}
    Terminate Process    ${handle}    kill=${True}
    BM.Wait Until Gone    ${application}    query_overrides={'timeout': 10}
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}
