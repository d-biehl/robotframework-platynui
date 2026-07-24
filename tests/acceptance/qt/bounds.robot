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
...                 extents with each candidate's geometry instead. That divergence is the regression
...                 condition — it must not be "fixed" in the app.
...
...                 The tests read only PlatynUI ``@Bounds``, so they are decisive on X11, the Wayland
...                 compositor, and Windows alike: each dialog must report its own (small, distinct)
...                 client rect, not the main window's. Window locators keep the role union so they
...                 resolve the WINDOW node (whose bounds are under test), not an inner widget.

Resource            resources/testapp.resource

Suite Setup         Launch Qt Test App
Suite Teardown      Terminate Qt Instance

Test Tags           real


*** Variables ***
# Client sizes are pixel-exact on X11; a few px of tolerance covers WM/rounding differences across
# platforms while staying far below the gap to the 900x640 main window.
${SIZE_TOL}         ${8}


*** Test Cases ***
Main Window And Child Dialogs Are Exposed
    [Documentation]    Sanity (and the suite's synchronization point): the main window and all three
    ...    child dialogs — spawned one event-loop turn after the window shows — are on the tree.
    BM.Wait Until Exists    .//(Frame|Window)[@Name="main-window"]
    BM.Wait Until Exists    .//(Dialog|Window|Frame)[@Name="child-dialog-1"]
    BM.Wait Until Exists    .//(Dialog|Window|Frame)[@Name="child-dialog-2"]
    BM.Wait Until Exists    .//(Dialog|Window|Frame)[@Name="child-dialog-3"]

Main Window Bounds Match Its Designed Size
    [Documentation]    Anchor the comparison baseline: the main window itself resolves to its own
    ...    ~900x640 bounds (if this were wrong, the dialog comparisons below would be meaningless).
    ${b}=    BM.Get Attribute    .//(Frame|Window)[@Name="main-window"]    Bounds
    Should Be True    abs($b.width - 900) <= 40 and abs($b.height - 640) <= 40
    ...    msg=main window bounds ${b.width}x${b.height} are not ~900x640

Child Dialog Bounds Are Their Own Not The Main Window's
    [Documentation]    Each dialog must report its own client rect. Before the fix every dialog reported
    ...    the main window's size (900x640) and position; here we assert each dialog is smaller than the
    ...    main window and matches its designed size.
    ${main}=    BM.Get Attribute    .//(Frame|Window)[@Name="main-window"]    Bounds
    FOR    ${dialog}    ${ew}    ${eh}    IN
    ...    child-dialog-1    ${260}    ${180}
    ...    child-dialog-2    ${300}    ${220}
    ...    child-dialog-3    ${220}    ${160}
        ${b}=    BM.Get Attribute    .//(Dialog|Window|Frame)[@Name="${dialog}"]    Bounds
        Should Be True    $b.width < $main.width and $b.height < $main.height
        ...    msg=${dialog} reports the main window size (${main.width}x${main.height}) — the resolve_window bug
        Should Be True    abs($b.width - ${ew}) <= ${SIZE_TOL} and abs($b.height - ${eh}) <= ${SIZE_TOL}
        ...    msg=${dialog} client size ${b.width}x${b.height} does not match the designed ~${ew}x${eh}
    END
    # Absolute position is deliberately NOT asserted: it is meaningless on Wayland (a client cannot know
    # its global position and the compositor stacks all windows at the origin), and there is no
    # provider-independent ground truth to compare it against. Wrong-window selection — the actual
    # regression — is caught by the size and distinctness checks, which need only @Bounds.

Child Dialogs Have Distinct Bounds
    [Documentation]    Guards against a regression where every dialog resolves to the SAME window (each
    ...    would then be small enough to pass the test above yet still be wrong). The dialogs have
    ...    distinct designed sizes, so their bounds must differ pairwise.
    ${b1}=    BM.Get Attribute    .//(Dialog|Window|Frame)[@Name="child-dialog-1"]    Bounds
    ${b2}=    BM.Get Attribute    .//(Dialog|Window|Frame)[@Name="child-dialog-2"]    Bounds
    ${b3}=    BM.Get Attribute    .//(Dialog|Window|Frame)[@Name="child-dialog-3"]    Bounds
    Bounds Differ    ${b1}    ${b2}
    Bounds Differ    ${b1}    ${b3}
    Bounds Differ    ${b2}    ${b3}


*** Keywords ***
Bounds Differ
    [Documentation]    Assert two Bounds rects are not identical (position or size differs).
    [Arguments]    ${a}    ${b}
    Should Be True    $a.x != $b.x or $a.y != $b.y or $a.width != $b.width or $a.height != $b.height
    ...    msg=bounds are identical (${a}) — dialogs resolved to the same window
