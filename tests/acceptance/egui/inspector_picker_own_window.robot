*** Settings ***
Documentation       The compositor's own-window exclusion, seen from the consumer it exists for: with the
...                 Inspector's own window laid OVER the test app, a live pick must resolve the widget
...                 BEHIND that window — never the Inspector's own UI.
...
...                 This is the case `inspector_picker.robot` deliberately avoids by laying the two windows
...                 side by side. Before the compositor skipped the asking process's own window, a pick here
...                 produced NO element at all: the compositor answered with the Inspector's own window and
...                 the provider discarded it as its own. That is the regression this suite guards, and it
...                 becomes the only guard once the provider-side own-process filter is dropped.
...
...                 platform:wayland — the exclusion lives in the PlatynUI compositor's `window_at_point`.
...                 The X11 window manager reaches the same answer through its own ownership decision, which
...                 is a separate concern (see the `x11-window-owner-identity` change).

Resource            resources/testapp.resource
Resource            resources/inspector.resource

Suite Setup         Run Keywords    Launch Default Instance
...                     AND    Launch Inspector    inspector-picker-own-window-settings.ron
Suite Teardown      Run Keywords    Terminate Inspector    AND    Terminate Default Instance
Test Tags           real    platform:wayland


*** Test Cases ***
A Pick Over The Inspectors Own Window Resolves The Window Behind It
    [Documentation]    Place the app, cover it — and its Click Me button — with the Inspector's own window,
    ...    arm the picker, hold Ctrl+Alt+Shift over the covered button, and confirm the Inspector selected
    ...    that button by reading the Inspector's OWN a11y tree: the button's subtree is not loaded until a
    ...    pick reveals and selects it.
    BM.Move And Resize Window    ${WINDOW}    20    20    640    480
    ${bounds}=    BM.Get Attribute    ${WINDOW}//*[@Id="btn-click-me"]    Bounds
    VAR    ${cx}    ${{ $bounds.x + $bounds.width / 2 }}
    VAR    ${cy}    ${{ $bounds.y + $bounds.height / 2 }}
    # Cover the app, and the button, with the Inspector's own window.
    BM.Move And Resize Window    ${INSP_WIN}    10    10    900    700
    Sleep    0.5s
    ${before}=    BM.Query    ${INSP_WIN}//*[contains(@Name,"Click Me")]    only_first=${True}
    Should Be Equal    ${before}    ${None}    msg=Inspector already shows the button before picking
    BM.Pointer Click    ${INSP_WIN}//*[@Id="picker-toggle"]
    TRY
        BM.Keyboard Press    ${None}    <Ctrl+Alt+Shift>
        BM.Pointer Move To    x=${cx}    y=${cy}    activate=${False}
        Sleep    1.5s
    FINALLY
        BM.Keyboard Release    ${None}    <Ctrl+Alt+Shift>
    END
    # The pick reached the window behind the Inspector's own: the button is now on
    # the Inspector's own a11y tree.
    BM.Wait Until Exists    ${INSP_WIN}//*[contains(@Name,"Click Me")]
