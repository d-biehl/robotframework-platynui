*** Settings ***
Documentation       An application node that a suite holds reports its process's end (spec
...                 *application-node-validity*).
...
...                 A scoped root pinned to ``/app:Application[@ProcessId=…]`` is reused only while it
...                 reports itself valid. Once its process has ended, the next keyword looks the root
...                 up again and fails with ``RootNotFoundError``, which names the root, instead of
...                 waiting out its timeout for a target under a dead application. That holds for a
...                 wait that ignores errors (``ignore_exceptions``) as well: the root's lookup is not
...                 one of the attempts the call may swallow. A captured application node is gone for
...                 ``Wait Until Gone`` once its process has ended.
...
...                 Each test launches an instance of its own and ends it, so no other suite loses its
...                 application. On Windows this exercises UI Automation's application node, which
...                 answers from the process's pid and start time; on the Linux lanes it exercises
...                 AT-SPI's application accessible, which reports itself gone once the application
...                 no longer answers over D-Bus.

Resource            resources/testapp.resource


*** Test Cases ***
A Root Pinned To An Ended Application Is Looked Up Again
    ${handle}=    Launch Test App    PlatynUI App Root Exit    com.platynui.test.approot
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL
    BM.Wait Until Exists    ./(Frame|Window)
    Terminate App    ${handle}
    # At the scope level, so that the root's own lookup is short as well.
    BM.Set Query Settings    {'timeout': 2}    scope=LOCAL
    Run Keyword And Expect Error    STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}]', was not found
    ...    BM.Get Attribute Value    ./(Frame|Window)    Name
    [Teardown]    Run Keyword And Ignore Error    Terminate App    ${handle}

A Root Of An Ended Application Is Not Swallowed By Ignore Exceptions
    [Documentation]    The wait ignores errors for its own target and would wait 10 s for it. The
    ...    root's lookup runs with the 2 s of the scope instead, and its failure ends the wait: the
    ...    window under the dead application is neither reported gone nor still present.
    ${handle}=    Launch Test App    PlatynUI App Root Ignore    com.platynui.test.approotignore
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL
    BM.Wait Until Exists    ./(Frame|Window)
    Terminate App    ${handle}
    BM.Set Query Settings    {'timeout': 2}    scope=LOCAL
    Run Keyword And Expect Error    STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}]', was not found
    ...    BM.Wait Until Gone    ./(Frame|Window)    query_overrides={'timeout': 10, 'ignore_exceptions': True}
    [Teardown]    Run Keyword And Ignore Error    Terminate App    ${handle}

A Captured Application Node Is Gone Once Its Process Ended
    ${handle}=    Launch Test App    PlatynUI App Node Exit    com.platynui.test.appnode
    ${pid}=    Get Process Id    ${handle}
    ${application}=    BM.Query    /app:Application[@ProcessId=${pid}]    only_first=${True}
    Should Not Be Equal    ${application}    ${None}
    Terminate App    ${handle}
    BM.Wait Until Gone    ${application}    query_overrides={'timeout': 10}
    [Teardown]    Run Keyword And Ignore Error    Terminate App    ${handle}
