*** Settings ***
Documentation       A captured element still raises its window after the snapshot it was found in has
...                 been discarded, through the Access Bridge. Two fixture instances are stacked at the
...                 same position. The suite root is pinned to the target instance's ``app:Application``
...                 node, so the root holds that node and nothing below it: once the snapshot is
...                 discarded, only the captured button itself keeps the way up to its window. The
...                 second instance only covers the first and is addressed absolutely by its ProcessId.
...                 The in-JVM agent's counterpart is agent_auto_activate.robot.

Resource            resources/testapp.resource

Suite Setup         Launch Target And Cover Instances
Suite Teardown      Terminate Target And Cover Instances

Test Tags           real


*** Variables ***
# Window locator and Process handle of the covering instance — assigned at suite scope by
# Launch Target And Cover Instances (Suite Setup). Declared here so they resolve statically.
${COVER_WINDOW}    ${None}
${COVER_HANDLE}    ${None}


*** Test Cases ***
A Captured Element Still Raises Its Window After The Snapshot Was Discarded
    [Documentation]    Every Query starts from a fresh snapshot, so by the time the captured button is
    ...    clicked, nothing but the button holds the nodes between it and the application node the
    ...    root pins. With the cover in front, the click lands only if auto_activate still finds the
    ...    button's window and raises it; the unchanged bounds show that the captured element is still
    ...    the same button. The JAB-reported bounds follow a move once Swing's event-dispatch thread
    ...    has processed it, hence the wait for the new origin before the button is captured.
    BM.Move Window    ${COVER_WINDOW}    ${180}    ${160}
    BM.Move Window    .//Window[@Name="${SWING_TITLE}"]    ${180}    ${160}
    BM.Wait Until Query    .//Window[@Name="${SWING_TITLE}"]/@Bounds.X    ==    ${180}
    BM.Wait Until Query    .//Window[@Name="${SWING_TITLE}"]/@Bounds.Y    ==    ${160}
    BM.Wait Until Exists    .//*[@Name="stage1-status-clicks-0"]
    ${button}=    BM.Query    .//*[@Name="stage1-button"]    only_first=${True}
    ${bounds}=    BM.Get Attribute    ${button}    Bounds
    # This Query discards the snapshot the button was found in.
    ${cover}=    BM.Query    ${COVER_WINDOW}    only_first=${True}
    BM.Activate Window    ${cover}
    BM.Wait Until Query    ${COVER_WINDOW}/@IsActive    ==    ${True}
    BM.Pointer Click    ${button}
    BM.Wait Until Query    .//Window[@Name="${SWING_TITLE}"]/@IsActive    ==    ${True}
    ...    assertion_message=the captured button's window was not raised
    BM.Wait Until Exists    .//*[@Name="stage1-status-clicks-1"]
    BM.Get Attribute    ${button}    Bounds    ==    ${bounds}


*** Keywords ***
Launch Target And Cover Instances
    [Documentation]    Launch the target instance, pinned as the suite root at its application node,
    ...    then a second instance that covers it, addressed by its own ProcessId.
    Launch Default Swing Instance    PlatynUI Swing AutoActivate
    VAR    ${title}    PlatynUI Swing AutoActivate Cover
    ${handle}=    Launch Swing Test App    ${title}
    ${pid}=    Get Process Id    ${handle}
    VAR    ${COVER_HANDLE}    ${handle}    scope=SUITE
    VAR    ${COVER_WINDOW}    /app:Application[@ProcessId=${pid}]//Window[@Name="${title}"]    scope=SUITE

Terminate Target And Cover Instances
    [Documentation]    ``kill=True`` for the cover too, for the reason Terminate Default Swing Instance
    ...    gives: a JVM ignores the "graceful" CTRL_BREAK.
    Terminate Default Swing Instance
    Run Keyword And Ignore Error    Terminate Process    ${COVER_HANDLE}    kill=${True}
