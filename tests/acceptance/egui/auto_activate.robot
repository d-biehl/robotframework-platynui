*** Settings ***
Documentation       Real-lane proof of auto_activate against two overlapping egui instances: acting on
...                 a backgrounded window brings it to the front first, so pointer/keyboard input
...                 lands there, and the per-call ``activate`` override gates that raise. Also covers
...                 the window-control keywords used to arrange the windows (Activate Window, Move
...                 Window, Resize Window) and Take Screenshot of a raised element. Runs under both
...                 the Wayland compositor and X11/Xephyr.
...
...                 The two instances are the SAME program, so they are pinned by ``@ProcessId`` (the
...                 robust way to tell copies apart, per the library's "Targeting a specific
...                 application"): Suite Setup captures each launched PID and builds the instance roots
...                 ``app:Application[@ProcessId=...]//Frame``. Widgets are addressed by role + ``@Id``
...                 fragments from the page-object resource.

Resource            resources/testapp.resource

Suite Setup         Launch Both Instances
Suite Teardown      Terminate Both Instances

Test Tags           real


*** Variables ***
# Per-instance roots + handles + Beta's button locator — all assigned at suite scope by
# Launch Both Instances (Suite Setup). Declared here as placeholders so they resolve statically.
${ALPHA}        ${None}
${BETA}         ${None}
${ALPHA_H}      ${None}
${BETA_H}       ${None}
${BETA_BTN}     ${None}


*** Test Cases ***
Activate Window Switches The Active Window Exclusively
    [Documentation]    The real window manager keeps a single foreground window: activating one makes
    ...    it active and drops the other, observable through @IsActive.
    BM.Activate Window    ${ALPHA}
    BM.Get Attribute    ${ALPHA}    IsActive    ==    ${True}
    BM.Get Attribute    ${BETA}     IsActive    ==    ${False}
    BM.Activate Window    ${BETA}
    BM.Get Attribute    ${BETA}     IsActive    ==    ${True}
    BM.Get Attribute    ${ALPHA}    IsActive    ==    ${False}

Move And Resize Window Change The Window Bounds
    [Documentation]    The window-control keywords used to arrange the instances actually move and
    ...    resize a real window: resize sets the size exactly, move shifts the position. The compositor
    ...    applies both asynchronously — a move is composited server-side, but a resize is a full
    ...    configure/ack/commit round-trip with the client — so poll @Bounds until the change lands
    ...    rather than reading it once (a fixed settle sleep would be both flaky and needlessly slow).
    ${b0}=    Get Bounds    ${ALPHA}
    BM.Move Window    ${ALPHA}    ${260}    ${180}
    Wait Until Keyword Succeeds    3s    0.1s    Window Position Changed    ${ALPHA}    ${b0}
    BM.Resize Window    ${ALPHA}    ${640}    ${360}
    Wait Until Keyword Succeeds    3s    0.1s    Window Size Is    ${ALPHA}    ${640}    ${360}

Auto Activate Raises The Background Window For A Pointer Click
    [Documentation]    With auto_activate on (default), clicking an element in the backgrounded window
    ...    raises its window first, so the click lands there: @IsActive flips and the window's own
    ...    click counter increments.
    BM.Activate Window    ${ALPHA}
    BM.Get Attribute    ${BETA}    IsActive    ==    ${False}
    ${before}=    Get Click Count    ${BETA}
    BM.Pointer Click    ${BETA_BTN}
    Sleep    0.3s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${True}
    ${after}=    Get Click Count    ${BETA}
    Should Be Equal As Integers    ${after}    ${{ $before + 1 }}    msg=click did not land on the raised window

Activate False Leaves The Target Behind So The Click Misses It
    [Documentation]    The negative case: with activate=${False} the background window is not raised, so
    ...    a click at the target's coordinates hits the occluding foreground window instead — the target
    ...    stays inactive and its counter does not move. Proves the raise is what makes the input land.
    Stack Windows
    BM.Activate Window    ${ALPHA}
    ${beta_before}=    Get Click Count    ${BETA}
    BM.Pointer Click    ${BETA_BTN}    activate=${False}
    Sleep    0.3s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${False}
    ${beta_after}=    Get Click Count    ${BETA}
    Should Be Equal As Integers    ${beta_after}    ${beta_before}    msg=click should not have reached the un-raised window

Focus Raises The Background Window And Keyboard Input Lands There
    [Documentation]    Focus brings the element's window forward (focus alone is app-local, so the raise
    ...    is what makes it the desktop-active window); a following keystroke activates the focused
    ...    button, incrementing the counter of the now-foreground window.
    BM.Activate Window    ${ALPHA}
    ${before}=    Get Click Count    ${BETA}
    BM.Focus    ${BETA_BTN}
    Sleep    0.2s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${True}
    BM.Keyboard Type    ${None}    <Return>
    Sleep    0.3s
    ${after}=    Get Click Count    ${BETA}
    Should Be True    ${after} > ${before}    msg=keyboard activation did not register on the focused window

Take Screenshot Of A Raised Element
    [Documentation]    Take Screenshot raises the element's window first (default activate), so the
    ...    capture is of the target rather than an occluder. Works on both backends — X11 via the X
    ...    server, Wayland via our compositor's control-socket screenshot.
    BM.Activate Window    ${ALPHA}
    ${file}=    BM.Take Screenshot    ${BETA}    filename=auto-activate-beta.png
    Should Not Be Empty    ${file}


*** Keywords ***
Launch Both Instances
    [Documentation]    Launch the two instances and pin each by its launched ProcessId, so the roots
    ...    address exactly that copy of the program regardless of window stacking or title.
    ${ah}=    Launch Test App    PlatynUI Alpha    com.platynui.test.alpha
    ${bh}=    Launch Test App    PlatynUI Beta     com.platynui.test.beta
    ${aroot}=    App Window Root    ${ah}
    ${broot}=    App Window Root    ${bh}
    VAR    ${ALPHA_H}    ${ah}    scope=SUITE
    VAR    ${BETA_H}     ${bh}    scope=SUITE
    VAR    ${ALPHA}    ${aroot}    scope=SUITE
    VAR    ${BETA}     ${broot}    scope=SUITE
    VAR    ${BETA_BTN}    ${BETA}${BTN_CLICK_ME}    scope=SUITE

Terminate Both Instances
    Run Keyword And Ignore Error    Terminate App    ${ALPHA_H}
    Run Keyword And Ignore Error    Terminate App    ${BETA_H}

Stack Windows
    [Documentation]    Move both instances to the same position so they fully overlap — makes the
    ...    occlusion deterministic for the activate=${False} case regardless of WM placement.
    BM.Move Window    ${ALPHA}    ${140}    ${120}
    BM.Move Window    ${BETA}     ${140}    ${120}

Get Bounds
    [Arguments]    ${window}
    ${bounds}=    BM.Get Attribute    ${window}    Bounds
    RETURN    ${bounds}

Window Position Changed
    [Documentation]    Predicate for Wait Until Keyword Succeeds: pass once the window's top-left has
    ...    moved away from its original position — the asynchronously-applied effect of Move Window.
    [Arguments]    ${window}    ${origin}
    ${b}=    Get Bounds    ${window}
    Should Be True    $b.x != $origin.x or $b.y != $origin.y    msg=Move Window did not change the position

Window Size Is
    [Documentation]    Predicate for Wait Until Keyword Succeeds: pass once the window reports exactly the
    ...    target width and height — the asynchronously-applied effect of Resize Window.
    [Arguments]    ${window}    ${width}    ${height}
    ${b}=    Get Bounds    ${window}
    Should Be Equal As Numbers    ${b.width}     ${width}     msg=Resize Window did not set the width
    Should Be Equal As Numbers    ${b.height}    ${height}    msg=Resize Window did not set the height
