*** Settings ***
Documentation       The X11 backend on a display that no window manager manages: windows are found
...                 among the X server's own top-level windows, activation raises and focuses a
...                 window itself, a window is active while it holds the input focus, closing asks
...                 the window through its delete protocol, and what only a window manager can do
...                 (minimize, maximize, restore) fails by name. Moving and resizing work, because
...                 the X server performs them.
...
...                 Runs only on the X11 lane without a window manager (``window-manager:none``,
...                 profile ``real-x11-bare``). Two instances of the same program are pinned by
...                 ``@ProcessId``; every locator names the instance it addresses (``${ALPHA}`` /
...                 ``${BETA}``).

Resource            resources/testapp.resource

Suite Setup         Launch Both Instances
Suite Teardown      Terminate Both Instances

Test Tags           platform:x11    window-manager:none


*** Variables ***
# Per-instance roots + handles — assigned at suite scope by Launch Both Instances (Suite Setup).
${ALPHA}        ${None}
${BETA}         ${None}
${ALPHA_H}      ${None}
${BETA_H}       ${None}


*** Test Cases ***
A Window Is Found Without A Window Manager
    [Documentation]    The window's Bounds come from the X server, which knows the window without a
    ...    window manager: after a move away from the screen origin they report the new position,
    ...    and no native warning says that a substitute geometry was used instead.
    VAR    ${handler}    ${{ logging.handlers.BufferingHandler(1000) }}
    VAR    ${logger}    ${{ logging.getLogger("platynui") }}
    Call Method    ${logger}    addHandler    ${handler}
    TRY
        BM.Move Window    ${ALPHA}    ${310}    ${210}
        BM.Wait Until Query    ${ALPHA}/@Bounds.X = 310 and ${ALPHA}/@Bounds.Y = 210
    FINALLY
        Call Method    ${logger}    removeHandler    ${handler}
    END
    VAR    ${warnings}    ${{ [r.getMessage() for r in $handler.buffer if r.levelno >= 30] }}
    Should Be Empty    ${warnings}    the window was not found through the X server: ${warnings}

A Background Window Comes To The Front
    [Documentation]    Activating the covered window raises it above the other and gives it the input
    ...    focus: it becomes the only active window, a point in the overlap belongs to it, and
    ...    keyboard input reaches it.
    BM.Move Window    ${ALPHA}    ${140}    ${120}
    BM.Move Window    ${BETA}     ${140}    ${120}
    BM.Wait Until Query    ${ALPHA}/@Bounds.X = ${BETA}/@Bounds.X and ${ALPHA}/@Bounds.Y = ${BETA}/@Bounds.Y
    BM.Activate Window    ${BETA}
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${True}
    BM.Activate Window    ${ALPHA}
    BM.Wait Until Query    ${ALPHA}/@IsActive    ==    ${True}
    BM.Wait Until Query    ${BETA}/@IsActive    ==    ${False}
    BM.Pointer Move To    ${ALPHA}//*[@Id="btn-click-me"]    activate=${False}
    ${p}=    BM.Get Pointer Position
    ${hit}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${hit}    ${None}    msg=hit-test resolved nothing in the overlap
    ${raised}=    BM.Query    ${ALPHA}    only_first=${True}
    VAR    ${chain}    ${{ [node.runtime_id for node in $hit.ancestors()] }}
    Should Contain    ${chain}    ${raised.runtime_id}    msg=the point in the overlap does not belong to the raised window
    ${before}=    Get Click Count    ${ALPHA}
    BM.Focus    ${ALPHA}//*[@Id="btn-click-me"]
    BM.Keyboard Type    ${None}    <Return>
    BM.Wait Until Query    ${ALPHA}//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}
    ...    assertion_message=keyboard input did not reach the raised window

Minimize Fails By Name
    [Documentation]    Minimizing is a window manager's operation: without one it fails, naming the
    ...    operation and the missing window manager, and the window stays as it was.
    Run Keyword And Expect Error    *minimize*window manager*    BM.Minimize Window    ${ALPHA}
    BM.Get Attribute Value    ${ALPHA}    IsMinimized    ==    ${False}

The State Of A Window Without A Window Manager Is Normal
    BM.Get Attribute Value    ${ALPHA}    IsMinimized    ==    ${False}
    BM.Get Attribute Value    ${ALPHA}    IsMaximized    ==    ${False}
    BM.Get Attribute Value    ${ALPHA}    IsTopmost      ==    ${False}

Move And Resize Work Without A Window Manager
    BM.Move Window    ${BETA}    ${200}    ${150}
    BM.Resize Window    ${BETA}    ${640}    ${480}
    BM.Wait Until Query
    ...    ${BETA}/@Bounds.X = 200 and ${BETA}/@Bounds.Y = 150 and ${BETA}/@Bounds.Width = 640 and ${BETA}/@Bounds.Height = 480

A Window That Supports The Delete Protocol Closes
    [Documentation]    Closing asks the window to close through its WM_DELETE_WINDOW protocol, which
    ...    egui lists; the window then leaves the tree.
    ${handle}=    Launch Test App    PlatynUI Gamma    com.platynui.test.gamma
    ${gamma}=    App Window Root    ${handle}
    TRY
        BM.Close Window    ${gamma}
        BM.Wait Until Gone    ${gamma}
    FINALLY
        Run Keyword And Ignore Error    Terminate App    ${handle}
    END


*** Keywords ***
Launch Both Instances
    [Documentation]    Launch the two instances and pin each by its launched ProcessId.
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
