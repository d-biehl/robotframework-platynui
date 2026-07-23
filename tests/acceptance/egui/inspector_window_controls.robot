*** Settings ***
Documentation       Contract checks for the Inspector's headerbar window controls on decoration-less
...                 Wayland sessions, read from and driven through the Inspector's OWN a11y tree: the
...                 Maximize/Restore and Close buttons resolve by their stable element IDs, empty
...                 menu-bar space acts as a compositor-side move grip (drag moves, double-click
...                 maximizes), and menu clicks keep working next to the grip. The test compositor
...                 implements the real xdg-toplevel move/maximize grabs, so effects are observable
...                 as window-bounds changes.

Library             OperatingSystem
Library             Process
Library             PlatynUI.BareMetal    AS    BM

Suite Setup         Setup Inspector
Suite Teardown      Teardown Inspector
# platform:wayland — headerbar window controls exist only on decoration-less
# Wayland sessions; on X11 (and Windows/macOS) the Inspector keeps native
# decorations and shows none of the elements this suite drives.
Test Tags           real    platform:wayland


*** Variables ***
${INSPECTOR_BIN}        ${{ os.environ.get("PLATYNUI_INSPECTOR_BIN") or os.path.abspath("target/debug/platynui-inspector-rs" + (".exe" if os.name == "nt" else "")) }}
${INSP_HANDLE}          ${None}
${INSP_WIN}             ${None}
${MAXIMIZE_BTN}         //*[@Id="window-maximize"]
${CLOSE_BTN}            //*[@Id="window-close"]


*** Test Cases ***
Window Buttons Are Resolvable By Stable Id
    [Documentation]    Exactly one Maximize and one Close button, each carrying its stable @Id
    ...    (AccessKit author id → UIA AutomationId / AT-SPI AccessibleId).
    FOR    ${id}    IN    window-maximize    window-close
        ${els}=    BM.Query    ${INSP_WIN}//*[@Id="${id}"]
        Length Should Be    ${els}    1    msg=expected exactly one element with @Id="${id}"
    END

Dragging Empty Menu-Bar Space Moves The Window
    [Documentation]    Press-and-drag on the empty space between the menus and the window buttons
    ...    starts a compositor-side interactive move — the window's screen rectangle follows.
    BM.Move Window    ${INSP_WIN}    60    60
    # The window must hold keyboard focus: egui-winit honors a StartDrag
    # request only on a focused window (its guard against an X11 input-grab
    # bug). A real user drags a window they clicked into anyway.
    BM.Activate Window    ${INSP_WIN}
    Sleep    0.3s
    ${before}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    ${x}    ${y}=    Empty Menu Bar Point
    Drag Pointer From    ${x}    ${y}
    Wait Until Keyword Succeeds    5s    0.3s    Window Moved From    ${before}

Maximize Button Toggles The Window State
    [Documentation]    The Maximize button grows the window to the output; the same control —
    ...    now presenting as Restore under the unchanged @Id — restores the previous size.
    ${before}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    BM.Pointer Click    ${INSP_WIN}${MAXIMIZE_BTN}
    Wait Until Keyword Succeeds    5s    0.3s    Window Grew Beyond    ${before}
    BM.Pointer Click    ${INSP_WIN}${MAXIMIZE_BTN}
    Wait Until Keyword Succeeds    5s    0.3s    Window Restored To    ${before}

Double-Click On Empty Menu-Bar Space Maximizes
    [Documentation]    Double-clicking the move grip toggles maximized, like a title bar would.
    ${before}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    ${x}    ${y}=    Empty Menu Bar Point
    # Two fast single clicks instead of Pointer Multi Click: the default
    # multi-click pacing (double_click_time/2 between clicks plus the click
    # delays) lands just outside egui's 300 ms double-click window.
    VAR    &{fast}    press_release_delay_ms=${30}    after_click_delay_ms=${20}
    ...    after_input_delay_ms=${0}    before_next_click_delay_ms=${0}    multi_click_delay_ms=${0}
    BM.Pointer Move To    x=${x}    y=${y}
    Sleep    0.3s
    BM.Pointer Click    x=${x}    y=${y}    overrides=${fast}
    BM.Pointer Click    x=${x}    y=${y}    overrides=${fast}
    Wait Until Keyword Succeeds    5s    0.3s    Window Grew Beyond    ${before}
    BM.Pointer Click    ${INSP_WIN}${MAXIMIZE_BTN}
    Wait Until Keyword Succeeds    5s    0.3s    Window Restored To    ${before}

Menu Clicks Are Not Stolen By The Grip
    [Documentation]    A menu next to the grip still opens on click — the grip only covers
    ...    genuinely empty space.
    BM.Pointer Click    ${INSP_WIN}//*[@Name="Help"]
    TRY
        Wait Until Keyword Succeeds    5s    0.3s    Inspector Shows Element
        ...    contains(@Name,"About PlatynUI Inspector")
    FINALLY
        BM.Keyboard Type    ${None}    <Escape>
    END

Close Button Removes The Window
    [Documentation]    Activating the Close button ends the Inspector — the window leaves the
    ...    accessibility tree. Runs last; the teardown tolerates the process being gone.
    BM.Pointer Click    ${INSP_WIN}${CLOSE_BTN}
    Wait Until Keyword Succeeds    10s    0.3s    Inspector Window Gone


*** Keywords ***
Empty Menu Bar Point
    [Documentation]    Screen point on the menu bar's empty middle: horizontally between the last
    ...    menu (Help) and the Maximize button, vertically centered on the menu row.
    ${help}=    BM.Get Attribute    ${INSP_WIN}//*[@Name="Help"]    Bounds
    ${max}=    BM.Get Attribute    ${INSP_WIN}${MAXIMIZE_BTN}    Bounds
    ${x}=    Evaluate    (${help.x} + ${help.width} + ${max.x}) / 2
    ${y}=    Evaluate    ${help.y} + ${help.height} / 2
    RETURN    ${x}    ${y}

Drag Pointer From
    [Documentation]    Press at the point, give the app a frame to see the pressed button and
    ...    request the interactive move (an instantaneous press-move-release would complete before
    ...    the app ever reacts), then drag down-right in two interpolated moves.
    [Arguments]    ${x}    ${y}
    BM.Pointer Press    x=${x}    y=${y}
    TRY
        Sleep    0.4s
        BM.Pointer Move To    x=${{ ${x} + 60 }}    y=${{ ${y} + 40 }}
        BM.Pointer Move To    x=${{ ${x} + 120 }}    y=${{ ${y} + 80 }}
        Sleep    0.2s
    FINALLY
        BM.Pointer Release
    END

Window Moved From
    [Arguments]    ${before}
    ${now}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    Should Be True    abs(${now.x} - ${before.x}) > 30 or abs(${now.y} - ${before.y}) > 30
    ...    msg=window did not move (before ${before}, now ${now})

Window Grew Beyond
    [Arguments]    ${before}
    ${now}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    Should Be True    ${now.width} > ${before.width}
    ...    msg=window did not maximize (before ${before}, now ${now})

Window Restored To
    [Arguments]    ${before}
    ${now}=    BM.Get Attribute    ${INSP_WIN}    Bounds
    Should Be True    ${now.width} == ${before.width} and ${now.height} == ${before.height}
    ...    msg=window did not restore (before ${before}, now ${now})

Inspector Shows Element
    [Arguments]    ${predicate}
    ${el}=    BM.Query    ${INSP_WIN}//*[${predicate}]    only_first=${True}
    Should Not Be Equal    ${el}    ${None}    msg=no element matching [${predicate}] on the Inspector's a11y tree

Inspector Window Gone
    ${w}=    BM.Query    ${INSP_WIN}    only_first=${True}
    Should Be Equal    ${w}    ${None}    msg=Inspector window still on the a11y tree

Setup Inspector
    # Hermetic settings: keep the user's real settings file out of the run.
    VAR    ${settings}    ${OUTPUT DIR}${/}inspector-window-controls-settings.ron
    Remove File    ${settings}
    ${insp}=    Start Process    ${INSPECTOR_BIN}
    ...    stdout=${TEMPDIR}/inspector-window-controls.log    stderr=STDOUT
    ...    env:RUST_LOG=platynui_inspector=trace,platynui=debug
    ...    env:PLATYNUI_INSPECTOR_SETTINGS_PATH=${settings}
    VAR    ${INSP_HANDLE}    ${insp}    scope=SUITE
    ${insp_pid}=    Get Process Id    ${insp}
    VAR    ${INSP_WIN}    /app:Application[@ProcessId=${insp_pid}]/(Frame|Window)    scope=SUITE
    Wait Until Keyword Succeeds    15s    0.3s    Inspector Window Present

Inspector Window Present
    ${w}=    BM.Query    ${INSP_WIN}    only_first=${True}
    Should Not Be Equal    ${w}    ${None}    msg=Inspector window not on the a11y tree yet

Teardown Inspector
    Run Keyword And Ignore Error    Terminate Process    ${INSP_HANDLE}
