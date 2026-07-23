*** Settings ***
Documentation       The custom-controls chapter of the blueprint on the QML fixture: a self-drawn
...                 Rectangle+TapHandler control with manually wired Accessible properties must be
...                 drivable like a native button, and a drawn element WITHOUT any Accessible
...                 wiring must be honestly absent from the accessibility tree (the lower bound
...                 real hand-rolled QML controls exhibit; see apps/test-app-qml/README.md).

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Catalog Instance
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Custom Control Is Drivable Like A Native One
    [Documentation]    Two pointer activations on the hand-built custom-button drive its counter
    ...    observable to clicks-2 — manually wired accessibility is enough for name-based driving.
    Wait Until Catalog Node Appears    custom-status-label-clicks-0
    Click Catalog Control    custom-button
    Wait Until Catalog Node Appears    custom-status-label-clicks-1
    Click Catalog Control    custom-button
    Wait Until Catalog Node Appears    custom-status-label-clicks-2

Unwired Drawn Element Is Absent From The Tree
    [Documentation]    The "Hidden" rectangle (id customHidden in Main.qml) has no Accessible
    ...    attachment: it must not resolve by any name — the expected lower-bound behavior the
    ...    chapter asserts as a feature, not a bug.
    Catalog Node Is Absent    custom-hidden
    Catalog Node Is Absent    Hidden
