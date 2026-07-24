*** Settings ***
Documentation       Window-capability patterns on JAB top-level nodes: they delegate to the runtime's
...                 Win32 window manager via ``native:NativeWindowHandle`` (Activatable, Movable,
...                 Closeable — the AT-SPI blueprint), so activate/move/close behave exactly like any
...                 other window.

Library             Process
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Window
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Activate Brings The Window To The Foreground
    BM.Activate Window    .//Window[@Name="${SWING_TITLE}"]
    BM.Get Attribute    .//Window[@Name="${SWING_TITLE}"]    IsActive    ==    ${True}

Move Repositions The Window To The Requested Origin
    [Documentation]    The JAB-reported bounds track the move once Swing's event-dispatch thread has
    ...    processed it, hence the polling read.
    BM.Move Window    .//Window[@Name="${SWING_TITLE}"]    ${180}    ${160}
    Wait Until Keyword Succeeds    5s    0.25s    Window Origin Should Be    ${180}    ${160}

Close Ends The Fixture Process And Removes The Window
    [Documentation]    The fixture uses EXIT_ON_CLOSE, so a Closeable-driven close must terminate the
    ...    JVM and the window must vanish from the tree on a later poll (asserted desktop-absolute:
    ...    the suite root's app node dies with the process). Runs last — it consumes the suite's
    ...    instance.
    BM.Close Window    .//Window[@Name="${SWING_TITLE}"]
    ${result}=    Wait For Process    ${SWING_APP_HANDLE}    timeout=10s    on_timeout=terminate
    Should Be Equal As Integers    ${result.rc}    0    msg=fixture should exit cleanly on window close
    BM.Wait Until Gone    /Window[@Name="${SWING_TITLE}"]


*** Keywords ***
Window Origin Should Be
    [Documentation]    Predicate for Wait Until Keyword Succeeds: the window's top-left has reached
    ...    the requested origin.
    [Arguments]    ${x}    ${y}
    ${bounds}=    BM.Get Attribute    .//Window[@Name="${SWING_TITLE}"]    Bounds
    Should Be Equal As Numbers    ${bounds.x}    ${x}
    Should Be Equal As Numbers    ${bounds.y}    ${y}
