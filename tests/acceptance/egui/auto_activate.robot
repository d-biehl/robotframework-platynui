*** Settings ***
Documentation       Real-lane proof of auto_activate against two overlapping egui instances: acting on
...                 a backgrounded window brings it to the front first, so pointer/keyboard input
...                 lands there, and the per-call ``activate`` override gates that raise. Also covers
...                 the window-control keywords used to arrange the windows (Activate Window, Move
...                 Window, Resize Window) and, on X11 only, Take Screenshot of a raised element.
...                 Runs under both the Wayland compositor and X11/Xephyr.

Resource            resources/testapp.resource

Suite Setup         Launch Both Instances
Suite Teardown      Terminate Both Instances

Test Tags           real


*** Variables ***
${ALPHA}        //*[@Name="PlatynUI Alpha"]
${BETA}         //*[@Name="PlatynUI Beta"]
${BETA_BTN}     //*[@Name="PlatynUI Beta"]//Button[@Name="Click Me"]


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
    ...    resize a real window: resize sets the size exactly, move shifts the position.
    ${b0}=    Get Bounds    ${ALPHA}
    BM.Move Window    ${ALPHA}    ${260}    ${180}
    Sleep    0.3s
    ${b1}=    Get Bounds    ${ALPHA}
    Should Be True    $b1.x != $b0.x or $b1.y != $b0.y    msg=Move Window did not change the position
    BM.Resize Window    ${ALPHA}    ${640}    ${360}
    Sleep    0.3s
    ${b2}=    Get Bounds    ${ALPHA}
    Should Be Equal As Numbers    ${b2.width}     640    msg=Resize Window did not set the width
    Should Be Equal As Numbers    ${b2.height}    360    msg=Resize Window did not set the height

Auto Activate Raises The Background Window For A Pointer Click
    [Documentation]    With auto_activate on (default), clicking an element in the backgrounded window
    ...    raises its window first, so the click lands there: @IsActive flips and the window's own
    ...    click counter increments.
    BM.Activate Window    ${ALPHA}
    BM.Get Attribute    ${BETA}    IsActive    ==    ${False}
    ${before}=    Click Count    ${BETA}
    BM.Pointer Click    ${BETA_BTN}
    Sleep    0.3s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${True}
    ${after}=    Click Count    ${BETA}
    Should Be Equal As Integers    ${after}    ${{ $before + 1 }}    msg=click did not land on the raised window

Activate False Leaves The Target Behind So The Click Misses It
    [Documentation]    The negative case: with activate=${False} the background window is not raised, so
    ...    a click at the target's coordinates hits the occluding foreground window instead — the target
    ...    stays inactive and its counter does not move. Proves the raise is what makes the input land.
    Stack Windows
    BM.Activate Window    ${ALPHA}
    ${beta_before}=    Click Count    ${BETA}
    BM.Pointer Click    ${BETA_BTN}    activate=${False}
    Sleep    0.3s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${False}
    ${beta_after}=    Click Count    ${BETA}
    Should Be Equal As Integers    ${beta_after}    ${beta_before}    msg=click should not have reached the un-raised window

Focus Raises The Background Window And Keyboard Input Lands There
    [Documentation]    Focus brings the element's window forward (focus alone is app-local, so the raise
    ...    is what makes it the desktop-active window); a following keystroke activates the focused
    ...    button, incrementing the counter of the now-foreground window.
    BM.Activate Window    ${ALPHA}
    ${before}=    Click Count    ${BETA}
    BM.Focus    ${BETA_BTN}
    Sleep    0.2s
    BM.Get Attribute    ${BETA}    IsActive    ==    ${True}
    BM.Keyboard Type    ${None}    <Return>
    Sleep    0.3s
    ${after}=    Click Count    ${BETA}
    Should Be True    ${after} > ${before}    msg=keyboard activation did not register on the focused window

Take Screenshot Of A Raised Element
    [Documentation]    X11 only — the Wayland compositor has no screenshot provider yet. Raising the
    ...    element's window first (default activate) means the capture is of the target, not an occluder.
    Skip If    '%{XDG_SESSION_TYPE=}' == 'wayland'    Take Screenshot is not implemented on the Wayland compositor
    BM.Activate Window    ${ALPHA}
    ${file}=    BM.Take Screenshot    ${BETA}    filename=auto-activate-beta.png
    Should Not Be Empty    ${file}


*** Keywords ***
Launch Both Instances
    ${h1}=    Launch Test App    PlatynUI Alpha    com.platynui.test.alpha
    ${h2}=    Launch Test App    PlatynUI Beta     com.platynui.test.beta
    Set Suite Variable    ${ALPHA_H}    ${h1}
    Set Suite Variable    ${BETA_H}    ${h2}

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

Click Count
    [Documentation]    Read the integer N from the "Clicks: N" status label of the given window.
    [Arguments]    ${window}
    ${label}=    BM.Query    ${window}//Label[starts-with(@Name, "Clicks:")]    only_first=${True}
    Should Not Be Equal    ${label}    ${None}    msg=Clicks label not found in ${window}
    RETURN    ${{ int($label.name.split(":")[1]) }}
