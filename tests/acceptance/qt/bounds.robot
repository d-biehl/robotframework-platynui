*** Settings ***
Documentation       Regression for the window-bounds bug where child dialogs parented to the main
...                 window reported the MAIN WINDOW's bounds instead of their own.
...
...                 Root cause (X11/AT-SPI): the dialogs are separate top-level windows sharing the
...                 app's PID; the window manager disambiguated candidates by matching the accessible
...                 name against the X11 window title. The dialogs deliberately have
...                 accessibleName ("child-dialog-N") != windowTitle ("Dialog N"), so the match failed
...                 and the code fell back to the first candidate — the main window — handing every
...                 dialog the main window's 900x640 bounds. The fix correlates the node's AT-SPI screen
...                 extents with each candidate's geometry instead.
...
...                 The tests read only PlatynUI attributes, so they are decisive on X11, the Wayland
...                 compositor, and Windows alike: each dialog must report its own (small, distinct)
...                 client rect, matching the AT-SPI screen extents.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Qt Instance
Suite Teardown      Terminate Default Qt Instance

Test Tags           real


*** Variables ***
# Client sizes are pixel-exact on X11; a few px of tolerance covers WM/rounding differences across
# platforms while staying far below the gap to the 900x640 main window.
${SIZE_TOL}         ${8}
# @Bounds (from the window manager) vs the AT-SPI screen extents (from the toolkit) describe the same
# client rect for a correctly resolved window, so they must agree to within a couple of pixels.
${EXTENTS_TOL}      ${4}


*** Test Cases ***
Main Window And Child Dialogs Are Exposed
    [Documentation]    Sanity: the main window and all three child dialogs resolve on the tree.
    Node Is Present    ${APP}${MAIN_WINDOW}
    Node Is Present    ${APP}${DIALOG_1}
    Node Is Present    ${APP}${DIALOG_2}
    Node Is Present    ${APP}${DIALOG_3}

Main Window Bounds Match Its Designed Size
    [Documentation]    Anchor the comparison baseline: the main window itself resolves to its own
    ...    ~900x640 bounds (if this were wrong, the dialog comparisons below would be meaningless).
    ${b}=    Get Bounds    ${APP}${MAIN_WINDOW}
    Should Be True    abs($b.width - 900) <= 40 and abs($b.height - 640) <= 40
    ...    msg=main window bounds ${b.width}x${b.height} are not ~900x640

Child Dialog Bounds Are Their Own Not The Main Window's
    [Documentation]    Each dialog must report its own client rect. Before the fix every dialog reported
    ...    the main window's size (900x640) and position; here we assert each dialog is smaller than the
    ...    main window, matches its designed size, and sits at a different position.
    ${main}=    Get Bounds    ${APP}${MAIN_WINDOW}
    FOR    ${dialog}    ${ew}    ${eh}    IN
    ...    ${DIALOG_1}    ${260}    ${180}
    ...    ${DIALOG_2}    ${300}    ${220}
    ...    ${DIALOG_3}    ${220}    ${160}
        ${b}=    Get Bounds    ${APP}${dialog}
        Should Be True    $b.width < $main.width and $b.height < $main.height
        ...    msg=${dialog} reports the main window size (${main.width}x${main.height}) — the resolve_window bug
        Should Be True    abs($b.width - ${ew}) <= ${SIZE_TOL} and abs($b.height - ${eh}) <= ${SIZE_TOL}
        ...    msg=${dialog} client size ${b.width}x${b.height} does not match the designed ~${ew}x${eh}
    END
    # Absolute position is deliberately NOT asserted here: it is meaningless on Wayland (a client cannot
    # know its global position and the compositor stacks all windows at the origin). Position correctness
    # on X11/Windows is covered by "Dialog Bounds Match The AT-SPI Screen Extents" below, which compares
    # each dialog's window-manager @Bounds against its independent AT-SPI screen extents.

Child Dialogs Have Distinct Bounds
    [Documentation]    Guards against a regression where every dialog resolves to the SAME window (each
    ...    would then be small enough to pass the test above yet still be wrong). The dialogs have
    ...    distinct designed sizes, so their bounds must differ pairwise.
    ${b1}=    Get Bounds    ${APP}${DIALOG_1}
    ${b2}=    Get Bounds    ${APP}${DIALOG_2}
    ${b3}=    Get Bounds    ${APP}${DIALOG_3}
    Bounds Differ    ${b1}    ${b2}
    Bounds Differ    ${b1}    ${b3}
    Bounds Differ    ${b2}    ${b3}

Dialog Bounds Match The AT-SPI Screen Extents
    [Documentation]    Correctness cross-check from an independent source: @Bounds (derived by the
    ...    window manager) must match the dialog's AT-SPI screen extents (reported by the toolkit) to
    ...    within a couple of pixels. Catches wrong-window selection and any offset error that the
    ...    size-based checks above would miss.
    FOR    ${dialog}    IN    ${DIALOG_1}    ${DIALOG_2}    ${DIALOG_3}
        Bounds Agree With Screen Extents    ${APP}${dialog}    ${EXTENTS_TOL}    ${dialog} @Bounds vs AT-SPI extents
    END


*** Keywords ***
Bounds Differ
    [Documentation]    Assert two Bounds rects are not identical (position or size differs).
    [Arguments]    ${a}    ${b}
    Should Be True    $a.x != $b.x or $a.y != $b.y or $a.width != $b.width or $a.height != $b.height
    ...    msg=bounds are identical (${a}) — dialogs resolved to the same window
