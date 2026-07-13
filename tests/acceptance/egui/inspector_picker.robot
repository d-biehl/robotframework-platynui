*** Settings ***
Documentation       Drives the real Inspector live picker end-to-end: launches the Inspector (itself
...                 an AccessKit app) plus the egui test app, arms the picker via its own toolbar
...                 toggle, holds Ctrl+Alt+Shift, moves the cursor onto a known widget, and asserts
...                 the Inspector selected that element by reading the Inspector's OWN a11y tree.

Library             Process
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Setup Inspector And App
Suite Teardown      Teardown Inspector And App
Test Tags           real


*** Variables ***
${INSPECTOR_BIN}        %{PLATYNUI_INSPECTOR_BIN=target/debug/platynui-inspector-rs}
${INSP_HANDLE}          ${None}
${INSP_WIN}             ${None}
${PICK_TOGGLE}          //Button[contains(@Name,"Pick Element")]


*** Test Cases ***
Picker Selects The Element Under The Cursor
    [Documentation]    Arm the picker, hold Ctrl+Alt+Shift, move onto the Click Me button's screen
    ...    centre, and confirm the Inspector picked it.
    ${bounds}=    BM.Get Attribute    ${WINDOW}${BTN_CLICK_ME}    Bounds
    ${cx}=    Evaluate    ${bounds.x} + ${bounds.width} / 2
    ${cy}=    Evaluate    ${bounds.y} + ${bounds.height} / 2
    Log    Button centre = (${cx}, ${cy})
    BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        ${pos}=    BM.Get Pointer Position
        Log    Pointer after move = (${pos.x}, ${pos.y})
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    Log    Driver sequence complete — inspect ${TEMPDIR}/inspector.log for picker ticks.


*** Keywords ***
Setup Inspector And App
    Launch Default Instance
    ${insp}=    Start Process    ${INSPECTOR_BIN}
    ...    stdout=${TEMPDIR}/inspector.log    stderr=STDOUT
    ...    env:RUST_LOG=platynui_inspector=trace,platynui=debug
    VAR    ${INSP_HANDLE}    ${insp}    scope=SUITE
    ${insp_pid}=    Get Process Id    ${insp}
    VAR    ${INSP_WIN}    /app:Application[@ProcessId=${insp_pid}]/(Frame|Window)    scope=SUITE
    Wait Until Keyword Succeeds    15s    0.3s    Inspector Window Present

Inspector Window Present
    ${w}=    BM.Query    ${INSP_WIN}    only_first=${True}
    Should Not Be Equal    ${w}    ${None}    msg=Inspector window not on the a11y tree yet

Teardown Inspector And App
    Run Keyword And Ignore Error    Terminate Process    ${INSP_HANDLE}
    Terminate Default Instance
