*** Settings ***
Documentation       Window-capability patterns on JAB top-level nodes: they delegate to the runtime's
...                 Win32 window manager via ``native:NativeWindowHandle`` (Activatable, Movable,
...                 Closeable — the AT-SPI blueprint), so activate/move/close behave exactly like any
...                 other window.

Library             Process
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Window
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Activate Brings The Window To The Foreground
    BM.Activate Window    ${APP}${WINDOW}
    BM.Get Attribute    ${APP}${WINDOW}    IsActive    ==    ${True}

Move Repositions The Window To The Requested Origin
    [Documentation]    The JAB-reported bounds track the move once Swing's event-dispatch thread has
    ...    processed it, hence the polling read.
    BM.Move Window    ${APP}${WINDOW}    ${180}    ${160}
    Wait Until Keyword Succeeds    5s    0.25s    Window Origin Should Be    ${180}    ${160}


*** Keywords ***
Window Origin Should Be
    [Arguments]    ${x}    ${y}
    ${bounds}=    BM.Get Attribute    ${APP}${WINDOW}    Bounds
    Should Be Equal As Numbers    ${bounds.x}    ${x}
    Should Be Equal As Numbers    ${bounds.y}    ${y}

Close Ends The Fixture Process And Removes The Window
    [Documentation]    The fixture uses EXIT_ON_CLOSE, so a Closeable-driven close must terminate the
    ...    JVM and the window must vanish from the tree on a later poll. Runs last — it consumes the
    ...    suite's instance.
    BM.Close Window    ${APP}${WINDOW}
    ${result}=    Wait For Process    ${SWING_APP_HANDLE}    timeout=10s    on_timeout=terminate
    Should Be Equal As Integers    ${result.rc}    0    msg=fixture should exit cleanly on window close
    Wait Until Keyword Succeeds    5s    0.25s    Node Is Absent    ${APP}${WINDOW}
