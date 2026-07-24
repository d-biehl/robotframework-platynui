*** Settings ***
Documentation       Drives the real Inspector live picker end-to-end: launches the Inspector (itself
...                 an AccessKit app) plus the egui test app, arms the picker via its own toolbar
...                 toggle, holds Ctrl+Alt+Shift, moves the cursor onto a known widget, and asserts
...                 the Inspector selected that element by reading the Inspector's OWN a11y tree.
...                 The suite relies on the default Ctrl+Alt+Shift activation combination, so the
...                 Inspector runs on hermetic settings (see resources/inspector.resource).

Resource            resources/testapp.resource
Resource            resources/inspector.resource

Suite Setup         Run Keywords    Launch Default Instance
...                     AND    Launch Inspector    inspector-picker-settings.ron
Suite Teardown      Run Keywords    Terminate Inspector    AND    Terminate Default Instance
Test Tags           real


*** Test Cases ***
Picker Selects The Element Under The Cursor
    [Documentation]    Lay the Inspector out clear of the test app, arm the picker, hold
    ...    Ctrl+Alt+Shift over the Click Me button, and confirm the Inspector actually selected it by
    ...    reading the Inspector's OWN a11y tree: the button's subtree is not loaded until the pick
    ...    reveals and selects it.
    Lay Windows Out Side By Side    ${WINDOW}
    # The picked button's subtree is not revealed in the Inspector yet.
    ${before}=    BM.Query    ${INSP_WIN}//*[contains(@Name,"Click Me")]    only_first=${True}
    Should Be Equal    ${before}    ${None}    msg=Inspector already shows the button before picking
    ${bounds}=    BM.Get Attribute    ${WINDOW}//*[@Id="btn-click-me"]    Bounds
    VAR    ${cx}    ${{ $bounds.x + $bounds.width / 2 }}
    VAR    ${cy}    ${{ $bounds.y + $bounds.height / 2 }}
    Log    Button centre = (${cx}, ${cy})
    BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    # The pick revealed the button's ancestors and selected it — so it is now on
    # the Inspector's own a11y tree (its attribute panel / expanded tree row).
    BM.Wait Until Exists    ${INSP_WIN}//*[contains(@Name,"Click Me")]
