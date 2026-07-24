*** Settings ***
Documentation       Contract checks for the redesigned Inspector header, read from the Inspector's
...                 OWN a11y tree (the Inspector is itself an AccessKit app): every toolbar control
...                 is resolvable by its stable element ID in the icons-only default, the refresh
...                 controls left the search row, and the status bar keeps a persistent picker-state
...                 segment that coexists with the transient message slot. The suite relies on the
...                 icons-only default toolbar style and the default Ctrl+Alt+Shift activation
...                 combination, so the Inspector runs on hermetic settings
...                 (see resources/inspector.resource).

Resource            resources/testapp.resource
Resource            resources/inspector.resource

Suite Setup         Run Keywords    Launch Default Instance
...                     AND    Launch Inspector    inspector-toolbar-settings.ron
Suite Teardown      Run Keywords    Terminate Inspector    AND    Terminate Default Instance
Test Tags           real


*** Test Cases ***
Toolbar Controls Are Resolvable By Stable Id
    [Documentation]    Icons-only default: every toolbar control carries its stable @Id
    ...    (AccessKit author id → UIA AutomationId / AT-SPI AccessibleId), independent of
    ...    its visible label or icon.
    FOR    ${id}    IN    picker-toggle    refresh-node    refresh-subtree    highlight-node    always-on-top
        BM.Wait Until Exists    ${INSP_WIN}//*[@Id="${id}"]
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
    BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    TRY
        BM.Wait Until Exists    ${INSP_WIN}//*[contains(@Name,"armed") and contains(@Name,"Ctrl+Alt+Shift")]
    FINALLY
        BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    END

Completed Pick Announces The Picked Element
    [Documentation]    A completed pick produces a transient "Picked: …" message identifying the
    ...    element while the persistent armed segment stays visible — both segments at once.
    Lay Windows Out Side By Side    ${WINDOW}
    ${bounds}=    BM.Get Attribute    ${WINDOW}//*[@Id="btn-click-me"]    Bounds
    VAR    ${cx}    ${{ $bounds.x + $bounds.width / 2 }}
    VAR    ${cy}    ${{ $bounds.y + $bounds.height / 2 }}
    BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    TRY
        BM.Wait Until Exists    ${INSP_WIN}//*[contains(@Name,"Picked:") and contains(@Name,"Click Me")]
        # The persistent segment is still there alongside the transient message.
        BM.Wait Until Exists    ${INSP_WIN}//*[contains(@Name,"armed") and contains(@Name,"Ctrl+Alt+Shift")]
    FINALLY
        BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    END
