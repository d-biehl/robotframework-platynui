*** Settings ***
Documentation       The application node the Java Access Bridge serves reports its process's end (spec
...                 *application-node-validity*): a root pinned to it is looked up again and fails with
...                 ``RootNotFoundError``, and a captured one is gone for ``Wait Until Gone``.
...
...                 Each test launches a fixture instance of its own and ends it. The agent is off, as
...                 for every suite based on ``testapp.resource``; ``agent_app_root_after_exit.robot``
...                 covers the agent-served application node.

Resource            resources/testapp.resource

Suite Setup         Require Swing Prerequisites


*** Test Cases ***
A Root Pinned To An Ended JAB Application Is Looked Up Again
    ${handle}=    Launch Swing Test App    PlatynUI Swing JAB Root Exit
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL
    BM.Wait Until Exists    ./Window
    Terminate Process    ${handle}    kill=${True}
    # At the scope level, so that the root's own lookup is short as well.
    BM.Set Query Settings    {'timeout': 2}    scope=LOCAL
    Run Keyword And Expect Error    STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}]', was not found
    ...    BM.Get Attribute    ./Window    Name
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}

A Captured JAB Application Node Is Gone Once Its Process Ended
    ${handle}=    Launch Swing Test App    PlatynUI Swing JAB Node Exit
    ${pid}=    Get Process Id    ${handle}
    ${application}=    BM.Query    /app:Application[@ProcessId=${pid}][@Technology="JAB"]    only_first=${True}
    Should Not Be Equal    ${application}    ${None}
    Terminate Process    ${handle}    kill=${True}
    BM.Wait Until Gone    ${application}    query_overrides={'timeout': 10}
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}
