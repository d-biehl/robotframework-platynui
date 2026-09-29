*** Settings ***
Documentation       Real-lane proof of auto_activate against two overlapping egui instances: acting on
...                 a backgrounded window brings it to the front first, so pointer/keyboard input
...                 lands there, and the per-call ``activate`` override gates that raise. Also covers
...                 the window-control keywords used to arrange the windows (Activate Window, Move
...                 Window, Resize Window) and Take Screenshot of a raised element, and proves that an
...                 element captured with Query, and a root pinned inside a window, still raise their
...                 window after the snapshot they were found in has been discarded. Runs on every lane:
...                 UI Automation on Windows, AT-SPI under the Wayland compositor and X11/Xephyr.
...
...                 The two instances are the SAME program, so they are pinned by ``@ProcessId`` (the
...                 robust way to tell copies apart, per the library's "Targeting a specific
...                 application"): Suite Setup captures each launched PID and builds the instance
...                 roots. No suite-wide Set Root here — with two live windows every locator says
...                 explicitly which instance it addresses (``${ALPHA}`` / ``${BETA}`` prefix).

Resource            resources/testapp.resource

Suite Setup         Launch Both Instances
Suite Teardown      Terminate Both Instances

Test Tags           real


*** Variables ***
# Per-instance roots + handles — assigned at suite scope by Launch Both Instances (Suite Setup).
# Declared here as placeholders so they resolve statically.
${ALPHA}        ${None}
${BETA}         ${None}
${ALPHA_H}      ${None}
${BETA_H}       ${None}


*** Test Cases ***
Activate Window Switches The Active Window Exclusively
    [Documentation]    The real window manager keeps a single foreground window: activating one makes
    ...    it active and drops the other, observable through @IsActive once the window manager has
    ...    applied the change.
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    BM.Wait Until Query    ${BETA}/@IsActive     ==    ${False}
    BM.Activate Window    ${BETA}
    BM.Wait Until Query    ${BETA}/@IsActive     ==    ${True}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${False}

Move And Resize Window Change The Window Bounds
    [Documentation]    The window-control keywords used to arrange the instances actually move and
    ...    resize a real window: resize sets the size exactly, move shifts the position. The compositor
    ...    applies both asynchronously — a move is composited server-side, but a resize is a full
    ...    configure/ack/commit round-trip with the client — so poll @Bounds until the change lands
    ...    rather than reading it once (a fixed settle sleep would be both flaky and needlessly slow).
    ${b0}=    BM.Get Attribute Value    ${ALPHA}    Bounds
    BM.Move Window    ${ALPHA}    ${260}    ${180}
    Wait Until Keyword Succeeds    3s    0.1s    Window Position Changed    ${ALPHA}    ${b0}
    BM.Resize Window    ${ALPHA}    ${640}    ${360}
    Wait Until Keyword Succeeds    3s    0.1s    Window Size Is    ${ALPHA}    ${640}    ${360}

Auto Activate Raises The Background Window For A Pointer Click
    [Documentation]    With auto_activate on (default), clicking an element in the backgrounded window
    ...    raises its window first, so the click lands there: @IsActive flips and the window's own
    ...    click counter increments.
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${False}
    ${before}=    Get Click Count    ${BETA}
    Move The Pointer Off The Button    ${ALPHA}
    BM.Pointer Click    ${BETA}//*[@Id="btn-click-me"]
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${True}
    BM.Wait Until Query    ${BETA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    assertion_message=click did not land on the raised window

Activate False Leaves The Target Behind So The Click Misses It
    [Documentation]    The negative case: with activate=${False} the background window is not raised, so
    ...    a click at the target's coordinates hits the occluding foreground window instead — the target
    ...    stays inactive and its counter does not move. Proves the raise is what makes the input land.
    ...    Asserting a NON-event needs a real settle pause: nothing observable would ever change.
    Stack Windows
    BM.Activate Window    ${ALPHA}
    ${beta_before}=    Get Click Count    ${BETA}
    BM.Pointer Click    ${BETA}//*[@Id="btn-click-me"]    activate=${False}
    Sleep    0.3s
    BM.Get Attribute Value    ${BETA}    IsActive    ==    ${False}
    ${beta_after}=    Get Click Count    ${BETA}
    Should Be Equal As Integers    ${beta_after}    ${beta_before}    msg=click should not have reached the un-raised window

A Captured Element Still Raises Its Window After The Snapshot Was Discarded
    [Documentation]    An element captured with Query keeps its window reachable after the snapshot it
    ...    was found in has been discarded. Every Query starts from a fresh snapshot, so by the time the
    ...    button is clicked nothing but the button itself holds its ancestors. With ALPHA covering
    ...    BETA, the click lands only if auto_activate still finds the button's window and raises it;
    ...    the unchanged bounds show that the captured element is still the same button, and the
    ...    final query that the button itself still reaches its window.
    Stack Windows
    ${before}=    Get Click Count    ${BETA}
    ${button}=    BM.Query    ${BETA}//*[@Id="btn-click-me"]    only_first=${True}
    ${bounds}=    BM.Get Attribute Value    ${button}    Bounds
    # This Query discards the snapshot the button was found in.
    ${cover}=    BM.Query    ${ALPHA}    only_first=${True}
    BM.Activate Window    ${cover}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    Move The Pointer Off The Button    ${ALPHA}
    BM.Pointer Click    ${button}
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${True}
    ...    assertion_message=the captured button's window was not raised
    BM.Wait Until Query    ${BETA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    assertion_message=the click did not land on the captured button
    BM.Get Attribute Value    ${button}    Bounds    ==    ${bounds}
    # Raising the window alone does not show the way up: UI Automation raises a window through any
    # element of it. The captured button itself has to reach its window.
    ${window}=    BM.Query    ancestor::*[self::Window or self::Frame]    root=${button}    only_first=${True}
    Should Not Be Equal    ${window}    ${None}    msg=the captured button no longer reaches its window

A Root Inside A Window Still Activates That Window
    [Documentation]    A root pinned inside a window — the button row, a container rather than the
    ...    window — is what holds the way up to that window once the snapshot the root was found in
    ...    has been discarded, and every Query discards it. With ALPHA covering BETA, the click on the
    ...    button below the root lands only if auto_activate still reaches the root's window and
    ...    raises it.
    Stack Windows
    ${before}=    Get Click Count    ${BETA}
    BM.Set Root    ${BETA}//*[@Id="btn-click-me"]/..    scope=LOCAL
    ${window}=    BM.Query    self::Frame | self::Window    only_first=${True}
    Should Be Equal    ${window}    ${None}    msg=the root must be a container inside the window, not the window
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    Move The Pointer Off The Button    ${ALPHA}
    BM.Pointer Click    .//*[@Id="btn-click-me"]
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${True}
    ...    assertion_message=the root's window was not raised
    BM.Wait Until Query    ${BETA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    assertion_message=the click did not land on the button below the root

Focus Raises The Background Window And Keyboard Input Lands There
    [Documentation]    Focus brings the element's window forward (focus alone is app-local, so the raise
    ...    is what makes it the desktop-active window); a following keystroke activates the focused
    ...    button, incrementing the counter of the now-foreground window.
    BM.Activate Window    ${ALPHA}
    ${before}=    Get Click Count    ${BETA}
    BM.Focus    ${BETA}//*[@Id="btn-click-me"]
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${True}
    BM.Keyboard Type    ${None}    <Return>
    BM.Wait Until Query    ${BETA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    assertion_message=keyboard activation did not register on the focused window

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

Terminate Both Instances
    Run Keyword And Ignore Error    Terminate App    ${ALPHA_H}
    Run Keyword And Ignore Error    Terminate App    ${BETA_H}

Stack Windows
    [Documentation]    Move both instances to the same position so they fully overlap — makes the
    ...    occlusion deterministic regardless of WM placement — and wait until both windows report
    ...    it: a window manager applies a move asynchronously, so geometry read right after Move
    ...    Window can still be the old one.
    BM.Move Window    ${ALPHA}    ${140}    ${120}
    BM.Move Window    ${BETA}     ${140}    ${120}
    BM.Wait Until Query    ${ALPHA}/@Bounds.X = ${BETA}/@Bounds.X and ${ALPHA}/@Bounds.Y = ${BETA}/@Bounds.Y

Move The Pointer Off The Button
    [Documentation]    Rest the pointer on a label of the covering window, without raising anything.
    ...    egui takes a press at the pointer position it last saw over its own window, so a click
    ...    on a window raised under a resting pointer is lost unless the pointer moves onto the
    ...    target after the raise. The click's own move does that only if the pointer is not
    ...    already there, and an earlier test may have left it exactly there.
    [Arguments]    ${window}
    BM.Pointer Move To    ${window}//*[@Id="status-clicks"]    activate=${False}

Window Position Changed
    [Documentation]    Predicate for Wait Until Keyword Succeeds: pass once the window's top-left has
    ...    moved away from its original position — the asynchronously-applied effect of Move Window.
    [Arguments]    ${window}    ${origin}
    ${b}=    BM.Get Attribute Value    ${window}    Bounds
    Should Be True    $b.x != $origin.x or $b.y != $origin.y    msg=Move Window did not change the position

Window Size Is
    [Documentation]    Predicate for Wait Until Keyword Succeeds: pass once the window reports exactly the
    ...    target width and height — the asynchronously-applied effect of Resize Window.
    [Arguments]    ${window}    ${width}    ${height}
    ${b}=    BM.Get Attribute Value    ${window}    Bounds
    Should Be Equal As Numbers    ${b.width}     ${width}     msg=Resize Window did not set the width
    Should Be Equal As Numbers    ${b.height}    ${height}    msg=Resize Window did not set the height
