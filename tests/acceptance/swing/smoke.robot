*** Settings ***
Documentation       Discovery smoke for the JAB provider: the fixture window appears exactly once under
...                 the desktop with ``@Technology = "JAB"``, is grouped under its ``app:Application``
...                 node, exposes the stage-1/2 controls with normalized roles, and a JVM started after
...                 the runtime already answered queries shows up on a later poll.

Library             Process
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Smoke
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Fixture Window Appears Exactly Once Under The Desktop
    [Documentation]    The merged desktop tree shows the Java window once — the JAB representation; the
    ...    UIA shell is suppressed via the window-claims registry (single-appearance requirement).
    ${windows}=    BM.Query    /Window[@Name="${SWING_TITLE}"]
    Length Should Be    ${windows}    1    msg=expected exactly one desktop-level node for the Java window
    BM.Get Attribute    /Window[@Name="${SWING_TITLE}"]    Technology    ==    JAB

Window Is Grouped Under Its Application
    [Documentation]    ``app:Application[@ProcessId=<pid>]`` groups the window (process-scoped XPath).
    ${windows}=    BM.Query    ${APP}${WINDOW}
    Length Should Be    ${windows}    1
    BM.Get Attribute    ${APP}${WINDOW}    Technology    ==    JAB

Stage 1 Controls Are Enumerable With Normalized Roles
    [Documentation]    Menu bar, menus, button, text field, and status label resolve via their
    ...    accessible names and PlatynUI roles (JAB ``push button`` → Button, ``text`` → Text, …).
    FOR    ${locator}    IN    ${MAIN_MENUBAR}    ${MENU_FILE}    ${MENU_FILE_EXIT}    ${MENU_HELP}
    ...    ${STAGE1_BUTTON}    ${STAGE1_TEXTFIELD}    ${STAGE1_STATUS_0}
        Node Is Present    ${APP}${locator}
    END

Stage 2 Controls Are Enumerable With Normalized Roles
    [Documentation]    Checkbox, radio group, combo box, slider, spinner (JAB ``spinbox`` →
    ...    SpinButton), and progress bar resolve via their accessible names.
    FOR    ${locator}    IN    ${STAGE2_CHECKBOX}    ${STAGE2_RADIO_A}    ${STAGE2_RADIO_B}
    ...    ${STAGE2_COMBO}    ${STAGE2_SLIDER}    ${STAGE2_SPINNER}    ${STAGE2_PROGRESS}
        Node Is Present    ${APP}${locator}
    END

Combo Box Entries Are Promoted To List Items
    [Documentation]    Swing list entries report the JAB role ``label`` (toolkit quirk); the provider
    ...    promotes selectable labels under a ``list`` to ``item:ListItem`` so ``item:`` selectors work.
    Node Is Present    ${APP}//item:ListItem[@Name="Alpha"]

A JVM Started After The First Query Appears On A Later Poll
    [Documentation]    Polling re-discovery: the suite's runtime has long answered queries; a second
    ...    fixture instance launched now must appear without recreating the runtime.
    ${handle}=    Launch Swing Test App    PlatynUI Swing Latecomer
    ${late_root}=    App Root    ${handle}
    ${late_window}=    Window Locator    PlatynUI Swing Latecomer
    Node Is Present    ${late_root}${late_window}
    BM.Get Attribute    ${late_root}${late_window}    Technology    ==    JAB
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}
