*** Settings ***
Documentation       Functional proof that correct dialog bounds make pointer input land where it should.
...
...                 A dialog's "Click Me" button renames its own @Name to "<dialog>-button-clicked" when
...                 clicked. Clicking it via PlatynUI drives the pointer to the button's activation point
...                 (derived from its bounds); the rename only happens if that point is really inside the
...                 dialog. Before the bounds fix, the dialog resolved to the main window's coordinates,
...                 so the click would have missed the button — this is the real-world payoff of the fix.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Single Dialog Qt Instance
Suite Teardown      Terminate Default Qt Instance

Test Tags           real


*** Test Cases ***
Clicking A Child Dialog Button Lands Inside The Dialog
    [Documentation]    The button starts as "child-dialog-1-button" and becomes
    ...    "child-dialog-1-button-clicked" only when the click actually reaches it. Uses a single-dialog
    ...    instance so the target is not occluded by sibling dialogs on backends that stack windows at
    ...    the origin (Wayland).
    Node Is Present    ${APP}${DIALOG_1_BUTTON}
    Node Is Absent     ${APP}${DIALOG_1_BUTTON_CLICKED}
    BM.Pointer Click    ${APP}${DIALOG_1_BUTTON}
    Wait Until Keyword Succeeds    5s    0.25s    Node Is Present    ${APP}${DIALOG_1_BUTTON_CLICKED}
