*** Settings ***
Documentation       Contract checks for the redesigned Inspector header, read from the Inspector's
...                 OWN a11y tree (the Inspector is itself an AccessKit app): every toolbar control
...                 is resolvable by its stable element ID in the icons-only default, the refresh
...                 controls left the search row, and the status bar keeps a persistent picker-state
...                 segment that coexists with the transient message slot.

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
${PICK_TOGGLE}          //*[@Id="picker-toggle"]


*** Test Cases ***
Toolbar Controls Are Resolvable By Stable Id
    [Documentation]    Icons-only default: every toolbar control carries its stable @Id
    ...    (AccessKit author id → UIA AutomationId / AT-SPI AccessibleId), independent of
    ...    its visible label or icon.
    FOR    ${id}    IN    picker-toggle    refresh-node    refresh-subtree    highlight-node    always-on-top
        ${el}=    BM.Query    ${INSP_WIN}//*[@Id="${id}"]    only_first=${True}
        Should Not Be Equal    ${el}    ${None}    msg=toolbar control @Id="${id}" not on the a11y tree
    END

Refresh Controls Left The Search Row
    [Documentation]    The only Refresh controls are the toolbar buttons (addressed by @Id) — the
    ...    former search-row buttons are gone, so no Refresh-named button exists besides them.
    ${stray}=    BM.Query
    ...    ${INSP_WIN}//Button[contains(@Name,"Refresh") and not(@Id="refresh-node" or @Id="refresh-subtree")]
    ...    only_first=${True}
    Should Be Equal    ${stray}    ${None}    msg=unexpected Refresh control outside the toolbar

Persistent Segment Shows The Armed Picker State
    [Documentation]    Arming the picker via the toolbar toggle puts the armed state — including the
    ...    configured activation combination — on the status bar's persistent segment.
    BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    TRY
        Wait Until Keyword Succeeds    5s    0.3s    Inspector Shows Element
        ...    contains(@Name,"armed") and contains(@Name,"Ctrl+Alt+Shift")
    FINALLY
        BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    END

Completed Pick Announces The Picked Element
    [Documentation]    A completed pick produces a transient "Picked: …" message identifying the
    ...    element while the persistent armed segment stays visible — both segments at once.
    Lay Windows Out Side By Side
    ${bounds}=    BM.Get Attribute    ${WINDOW}${BTN_CLICK_ME}    Bounds
    ${cx}=    Evaluate    ${bounds.x} + ${bounds.width} / 2
    ${cy}=    Evaluate    ${bounds.y} + ${bounds.height} / 2
    BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    TRY
        Wait Until Keyword Succeeds    5s    0.3s    Inspector Shows Element
        ...    contains(@Name,"Picked:") and contains(@Name,"Click Me")
        # The persistent segment is still there alongside the transient message.
        Inspector Shows Element    contains(@Name,"armed") and contains(@Name,"Ctrl+Alt+Shift")
    FINALLY
        BM.Pointer Click    ${INSP_WIN}${PICK_TOGGLE}
    END


*** Keywords ***
Inspector Shows Element
    [Arguments]    ${predicate}
    ${el}=    BM.Query    ${INSP_WIN}//*[${predicate}]    only_first=${True}
    Should Not Be Equal    ${el}    ${None}    msg=no element matching [${predicate}] on the Inspector's a11y tree

Lay Windows Out Side By Side
    [Documentation]    Place the test app on the left and the Inspector to its right so the Inspector
    ...    never covers the button (a hit-test over the Inspector's own window resolves nothing).
    BM.Move And Resize Window    ${WINDOW}    20    20    640    480
    ${app}=    BM.Get Attribute    ${WINDOW}    Bounds
    ${insp_x}=    Evaluate    int(${app.x} + ${app.width} + 20)
    BM.Move Window    ${INSP_WIN}    ${insp_x}    20
    Sleep    0.5s

Setup Inspector And App
    Launch Default Instance
    # Hermetic settings: the suite relies on the icons-only default toolbar style
    # and the default Ctrl+Alt+Shift activation combination, so the Inspector must
    # not load the user's real settings file.
    VAR    ${settings}    ${OUTPUT DIR}${/}inspector-toolbar-settings.ron
    Remove File    ${settings}
    ${insp}=    Start Process    ${INSPECTOR_BIN}
    ...    stdout=${TEMPDIR}/inspector-toolbar.log    stderr=STDOUT
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
