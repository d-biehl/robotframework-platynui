*** Settings ***
Documentation       Drives the real Inspector live picker end-to-end: launches the Inspector (itself
...                 an AccessKit app) plus the egui test app, arms the picker via its own toolbar
...                 toggle, holds Ctrl+Alt+Shift, moves the cursor onto a known widget, and asserts
...                 the Inspector selected that element by reading the Inspector's OWN a11y tree.

Library             OperatingSystem
Library             Process
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Setup Inspector And App
Suite Teardown      Teardown Inspector And App
Test Tags           real


*** Variables ***
${INSPECTOR_BIN}        ${{ os.environ.get("PLATYNUI_INSPECTOR_BIN") or os.path.abspath("target/debug/platynui-inspector-rs" + (".exe" if os.name == "nt" else "")) }}
${INSP_HANDLE}          ${None}
${INSP_WIN}             ${None}
${PICK_TOGGLE}          //Button[contains(@Name,"Pick Element")]


*** Test Cases ***
Picker Selects The Element Under The Cursor
    [Documentation]    Lay the Inspector out clear of the test app (a hit-test over the Inspector's own
    ...    window resolves nothing — it is meant to inspect *other* windows), arm the picker, hold
    ...    Ctrl+Alt+Shift over the Click Me button, and confirm the Inspector actually selected it by
    ...    reading the Inspector's OWN a11y tree: the button's subtree is not loaded until the pick
    ...    reveals and selects it.
    Lay Windows Out Side By Side
    # The picked button's subtree is not revealed in the Inspector yet.
    ${before}=    BM.Query    ${INSP_WIN}//*[contains(@Name,"Click Me")]    only_first=${True}
    Should Be Equal    ${before}    ${None}    msg=Inspector already shows the button before picking
    ${bounds}=    BM.Get Attribute    ${WINDOW}${BTN_CLICK_ME}    Bounds
    ${cx}=    Evaluate    ${bounds.x} + ${bounds.width} / 2
    ${cy}=    Evaluate    ${bounds.y} + ${bounds.height} / 2
    Log    Button centre = (${cx}, ${cy})
    BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    # The pick revealed the button's ancestors and selected it — so it is now on
    # the Inspector's own a11y tree (its attribute panel / expanded tree row).
    Wait Until Keyword Succeeds    5s    0.3s    Inspector Shows Picked Button


*** Keywords ***
Lay Windows Out Side By Side
    [Documentation]    Place the test app on the left and the Inspector to its right so the Inspector
    ...    never covers the button. The Inspector x is derived from the app's actual bounds, so it
    ...    adapts to whatever size the toolkit grants.
    BM.Move And Resize Window    ${WINDOW}    20    20    640    480
    ${app}=    BM.Get Attribute    ${WINDOW}    Bounds
    ${insp_x}=    Evaluate    int(${app.x} + ${app.width} + 20)
    BM.Move Window    ${INSP_WIN}    ${insp_x}    20
    Sleep    0.5s

Inspector Shows Picked Button
    ${el}=    BM.Query    ${INSP_WIN}//*[contains(@Name,"Click Me")]    only_first=${True}
    Should Not Be Equal    ${el}    ${None}
    ...    msg=Inspector did not reveal/select the picked button in its own tree

Setup Inspector And App
    Launch Default Instance
    # Hermetic settings: the suite relies on the default Ctrl+Alt+Shift
    # activation combination, so the Inspector must not load the user's real
    # settings file (a reconfigured combination would break the pick).
    VAR    ${settings}    ${OUTPUT DIR}${/}inspector-picker-settings.ron
    Remove File    ${settings}
    ${insp}=    Start Process    ${INSPECTOR_BIN}
    ...    stdout=${TEMPDIR}/inspector.log    stderr=STDOUT
    ...    env:RUST_LOG=platynui_inspector=trace,platynui=debug
    ...    env:PLATYNUI_INSPECTOR_SETTINGS_PATH=${settings}
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
