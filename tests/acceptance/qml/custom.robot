*** Settings ***
Documentation       The custom-controls chapter of the blueprint on the QML fixture: a self-drawn
...                 Rectangle+TapHandler control with manually wired Accessible properties must be
...                 drivable like a native button, and a drawn element WITHOUT any Accessible
...                 wiring must be honestly absent from the accessibility tree (the lower bound
...                 real hand-rolled QML controls exhibit; see apps/test-app-qml/README.md).

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Test App
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Custom Control Is Drivable Like A Native One
    [Documentation]    Two pointer activations on the hand-built custom-button drive its counter
    ...    observable to clicks-2 — manually wired accessibility is enough for name-based driving.
    BM.Wait Until Exists    .//*[@Name="custom-status-label-clicks-0"]
    BM.Pointer Click    .//*[@Name="custom-button"]
    BM.Wait Until Exists    .//*[@Name="custom-status-label-clicks-1"]
    BM.Pointer Click    .//*[@Name="custom-button"]
    BM.Wait Until Exists    .//*[@Name="custom-status-label-clicks-2"]

Unwired Drawn Element Is Absent From The Tree
    [Documentation]    The "Hidden" rectangle (id customHidden in Main.qml) has no Accessible
    ...    attachment: it must not resolve by any name — the expected lower-bound behavior the
    ...    chapter asserts as a feature, not a bug.
    ${node}=    BM.Query    .//*[@Name="custom-hidden"]    only_first=${True}
    Should Be Equal    ${node}    ${None}    msg=unwired element unexpectedly on the tree: custom-hidden
    ${node}=    BM.Query    .//*[@Name="Hidden"]    only_first=${True}
    Should Be Equal    ${node}    ${None}    msg=unwired element unexpectedly on the tree: Hidden
