*** Settings ***
Documentation       Real-lane proof of the window activation contract and the window state attributes,
...                 against two egui instances. Activating a window — directly, through Bring To Front,
...                 or implicitly before a Pointer Click — brings a minimized window back to the state it
...                 was minimized from and never un-maximizes it; @IsMinimized, @IsMaximized and
...                 @IsTopmost report the window manager's view of the window. Runs under the Wayland
...                 compositor and X11/Xephyr.
...
...                 Window operations land asynchronously (the window manager or compositor applies
...                 them, then the client commits the new state), so a state change is awaited with
...                 Wait Until Query. A state that must NOT change is read only after the action's own
...                 effect has landed (the window became active, the click counted, the bounds settled),
...                 so a late un-maximize cannot slip past the check. Every test starts from both windows
...                 restored (Test Setup), so the tests are order-independent.

Resource            resources/testapp.resource

Suite Setup         Launch Both Instances
Suite Teardown      Terminate Both Instances
Test Setup          Restore Both Windows

Test Tags           real


*** Variables ***
# Per-instance roots + handles — assigned at suite scope by Launch Both Instances (Suite Setup).
# Declared here as placeholders so they resolve statically.
${ALPHA}        ${None}
${BETA}         ${None}
${ALPHA_H}      ${None}
${BETA_H}       ${None}


*** Test Cases ***
Maximized State Is Read Live From The Same Window
    [Documentation]    @IsMaximized follows the window manager on every read — the same captured node
    ...    reports False, then True after Maximize Window — and a maximized window is never also
    ...    reported as minimized.
    ${window}=    BM.Query    ${ALPHA}    only_first=${True}
    BM.Get Attribute    ${window}    IsMaximized    ==    ${False}
    BM.Maximize Window    ${window}
    BM.Wait Until Query    ./@IsMaximized    ==    ${True}    root=${window}
    BM.Get Attribute    ${window}    IsMinimized    ==    ${False}

Minimized State Is Reported And Activation Brings The Window Back
    [Documentation]    A minimized window reports @IsMinimized (and not @IsMaximized), stays resolvable
    ...    while it is minimized, and Activate Window brings it back as the active window.
    BM.Minimize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMinimized    ==    ${True}
    BM.Get Attribute    ${ALPHA}    IsMaximized    ==    ${False}
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMinimized    ==    ${False}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}

A Window Minimized From Maximized Comes Back Maximized
    BM.Maximize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMaximized    ==    ${True}
    BM.Minimize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMinimized    ==    ${True}
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    BM.Wait Until Query    ${ALPHA}/@IsMaximized    ==    ${True}
    BM.Get Attribute    ${ALPHA}    IsMinimized    ==    ${False}

Activating A Maximized Background Window Keeps Its Size
    BM.Maximize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMaximized    ==    ${True}
    ${before}=    Settled Bounds    ${ALPHA}
    BM.Activate Window    ${BETA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${False}
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    ${after}=    Settled Bounds    ${ALPHA}
    Should Be Equal    ${after}    ${before}    msg=activation changed the window's bounds
    BM.Get Attribute    ${ALPHA}    IsMaximized    ==    ${True}

Bring To Front On An Element Of A Minimized Window Brings It Back
    [Documentation]    The button is captured before minimizing: while a window is minimized, the
    ...    toolkit may stop exposing its contents, so a fresh query for the button could find nothing.
    ${button}=    BM.Query    ${ALPHA}//*[@Id="btn-click-me"]    only_first=${True}
    BM.Minimize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMinimized    ==    ${True}
    BM.Bring To Front    ${button}
    BM.Wait Until Query    ${ALPHA}/@IsMinimized    ==    ${False}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}

Pointer Click Into A Maximized Background Window Keeps It Maximized
    [Documentation]    auto_activate raises the background window before the click; the raise must
    ...    not un-maximize it, so the click lands on the button's current position and counts.
    BM.Maximize Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsMaximized    ==    ${True}
    BM.Activate Window    ${BETA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${False}
    ${before}=    Get Click Count    ${ALPHA}
    BM.Pointer Click    ${ALPHA}//*[@Id="btn-click-me"]
    BM.Wait Until Query    ${ALPHA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=click did not land on the raised window
    BM.Get Attribute    ${ALPHA}    IsActive    ==    ${True}
    BM.Get Attribute    ${ALPHA}    IsMaximized    ==    ${True}

Window State Attributes Exist Only On Windows
    @{attributes}=    BM.Query    ${ALPHA}//*[@Id="btn-click-me"]/(@IsMinimized|@IsMaximized|@IsTopmost)
    Should Be Empty    ${attributes}

The Compositor Reports No Window As Topmost
    [Tags]    platform:wayland
    BM.Get Attribute    ${ALPHA}    IsTopmost    ==    ${False}


*** Keywords ***
Launch Both Instances
    [Documentation]    Launch the two instances and pin each by its launched ProcessId, so the roots
    ...    address exactly that copy of the program regardless of window stacking or title.
    ${ah}=    Launch Test App    PlatynUI Gamma    com.platynui.test.gamma
    ${bh}=    Launch Test App    PlatynUI Delta    com.platynui.test.delta
    ${aroot}=    App Window Root    ${ah}
    ${broot}=    App Window Root    ${bh}
    VAR    ${ALPHA_H}    ${ah}    scope=SUITE
    VAR    ${BETA_H}     ${bh}    scope=SUITE
    VAR    ${ALPHA}    ${aroot}    scope=SUITE
    VAR    ${BETA}     ${broot}    scope=SUITE

Terminate Both Instances
    Run Keyword And Ignore Error    Terminate App    ${ALPHA_H}
    Run Keyword And Ignore Error    Terminate App    ${BETA_H}

Restore Both Windows
    [Documentation]    Baseline for every test: both windows back in the normal state, whatever the
    ...    previous test left behind.
    FOR    ${window}    IN    ${ALPHA}    ${BETA}
        BM.Restore Window    ${window}
        BM.Wait Until Query    ${window}/@IsMinimized    ==    ${False}
        BM.Wait Until Query    ${window}/@IsMaximized    ==    ${False}
    END

Settled Bounds
    [Documentation]    The window's bounds once they stop changing — the window manager applies a
    ...    maximize or a raise and the client then commits its new size, so a single read can still
    ...    see the geometry in between. Two reads a few frames apart must agree.
    [Arguments]    ${window}
    ${bounds}=    Wait Until Keyword Succeeds    5s    0.2s    Bounds Unchanged Over A Few Frames    ${window}
    RETURN    ${bounds}

Bounds Unchanged Over A Few Frames
    [Documentation]    Predicate for Settled Bounds: read @Bounds twice, 100 ms apart (several frames
    ...    at the compositor's refresh rate), and pass when both reads agree.
    [Arguments]    ${window}
    ${first}=    BM.Get Attribute    ${window}    Bounds
    Sleep    0.1s
    ${second}=    BM.Get Attribute    ${window}    Bounds
    Should Be Equal    ${first}    ${second}    msg=window bounds are still changing
    RETURN    ${second}
