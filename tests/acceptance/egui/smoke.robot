*** Settings ***
Documentation       Real-world acceptance smoke against the egui test app,
...                 launched inside an isolated session by
...                 ``scripts/platynui-robot-session.sh`` (Wayland compositor or
...                 Xephyr/X11). Uses the low-level ``PlatynUI.BareMetal``
...                 library on purpose — the high-level PlatynUI keywords arrive
...                 with Phase 5. This drives the real AT-SPI tree end-to-end.

Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
${WINDOW}           //*[@Name="PlatynUI Test App"]


*** Test Cases ***
Desktop Root Is Reachable
    [Documentation]    Proves the session -> native runtime -> BareMetal path is alive.
    ${root}=    BM.Query    .    only_first=${True}
    Should Not Be Equal    ${root}    ${None}    msg=No desktop root resolved — session/runtime not reachable
    BM.Highlight    ${root}    duration=1.0

Egui Window Is Exposed By Title
    [Documentation]    The egui app forwards its window title to the AccessKit
    ...    root node, so the window is discoverable by name on the AT-SPI tree.
    ${win}=    BM.Query    ${WINDOW}    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=window 'PlatynUI Test App' not found by name on the AT-SPI tree
    BM.Highlight    ${win}    duration=2.0

Clicking A Button Updates The UI
    [Documentation]    Click 'Click Me' and verify the counter label changed — a
    ...    real perform + observe roundtrip through the accessibility tree.
    ${btn}=    BM.Query    ${WINDOW}//Button[@Name="Click Me"]    only_first=${True}
    Should Not Be Equal    ${btn}    ${None}    msg='Click Me' button not exposed
    BM.Pointer Click    ${btn}
    Sleep    0.3s
    ${label}=    BM.Query    ${WINDOW}//Label[starts-with(@Name, "Clicks:")]    only_first=${True}
    Should Not Be Equal    ${label}    ${None}    msg=Clicks counter label not found
    Should Be Equal    ${label.name}    Clicks: 1    msg=counter did not increment after click
