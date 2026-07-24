*** Settings ***
Documentation       Functional proof that correct dialog bounds make pointer input land where it should.
...
...                 Clicking a dialog's "Click Me" button reports ``child-dialog-1-button`` through the
...                 main window's always-visible last-action label. Clicking it via PlatynUI drives the
...                 pointer to the button's activation point (derived from its bounds); the report only
...                 happens if that point is really inside the dialog. Before the bounds fix, the dialog
...                 resolved to the main window's coordinates, so the click would have missed the button
...                 — this is the real-world payoff of the fix.

Resource            resources/testapp.resource

Suite Setup         Launch Qt Test App    PlatynUI Test App (Qt Single)    org.platynui.test.qt.single
...                     --dialogs    1
Suite Teardown      Terminate Qt Instance

Test Tags           real


*** Test Cases ***
Clicking A Child Dialog Button Lands Inside The Dialog
    [Documentation]    ``last-action-child-dialog-1-button`` only appears if the click actually reached
    ...    the button. Uses a single-dialog instance so the target is not occluded by sibling dialogs
    ...    on backends that stack windows at the origin (Wayland).
    BM.Wait Until Exists    .//*[@Name="last-action-none"]
    BM.Pointer Click    .//*[@Name="child-dialog-1-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-child-dialog-1-button"]
