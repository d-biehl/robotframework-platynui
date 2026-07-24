*** Settings ***
Documentation       Discovery smoke for the JAB provider: the fixture window appears exactly once under
...                 the desktop with ``@Technology = "JAB"``, is grouped under its ``app:Application``
...                 node, exposes the stage-1/2 controls with normalized roles, and a JVM started after
...                 the runtime already answered queries shows up on a later poll.

Library             Process
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
    [Documentation]    The suite root is ``app:Application[@ProcessId=<pid>]`` (process-scoped XPath),
    ...    and it groups the window.
    ${windows}=    BM.Query    .//Window[@Name="${SWING_TITLE}"]
    Length Should Be    ${windows}    1
    BM.Get Attribute    .//Window[@Name="${SWING_TITLE}"]    Technology    ==    JAB

Stage 1 Controls Are Enumerable With Normalized Roles
    [Documentation]    Menu bar, menus, button, text field, and status label resolve via their
    ...    accessible names and report the normalized PlatynUI role (JAB ``push button`` → Button,
    ...    ``text`` → Text, …; see ``crates/provider-jab/src/map.rs``).
    FOR    ${name}    ${role}    IN
    ...    main-menubar    MenuBar
    ...    menu-file    Menu
    ...    menu-file-exit    MenuItem
    ...    menu-help    Menu
    ...    stage1-button    Button
    ...    stage1-textfield    Text
    ...    stage1-status-clicks-0    Label
        BM.Get Attribute    .//*[@Name="${name}"]    Role    ==    ${role}
    END

Stage 2 Controls Are Enumerable With Normalized Roles
    [Documentation]    Checkbox, radio group, combo box, slider, spinner (JAB ``spinbox`` →
    ...    SpinButton), and progress bar resolve via their accessible names with normalized roles.
    FOR    ${name}    ${role}    IN
    ...    stage2-checkbox    CheckBox
    ...    stage2-radio-a    RadioButton
    ...    stage2-radio-b    RadioButton
    ...    stage2-combo    ComboBox
    ...    stage2-slider    Slider
    ...    stage2-spinner    SpinButton
    ...    stage2-progress    ProgressBar
        BM.Get Attribute    .//*[@Name="${name}"]    Role    ==    ${role}
    END

Combo Box Entries Are Promoted To List Items
    [Documentation]    Swing list entries report the JAB role ``label`` (toolkit quirk); the provider
    ...    promotes selectable labels under a ``list`` to ``item:ListItem`` so ``item:`` selectors work.
    BM.Wait Until Exists    .//item:ListItem[@Name="Alpha"]

A JVM Started After The First Query Appears On A Later Poll
    [Documentation]    Polling re-discovery: the suite's runtime has long answered queries; a second
    ...    fixture instance launched now must appear without recreating the runtime (Launch Swing
    ...    Test App itself waits for the new window on the tree). The latecomer is addressed
    ...    absolutely — the suite root stays pinned to the first instance.
    ${handle}=    Launch Swing Test App    PlatynUI Swing Latecomer
    ${pid}=    Get Process Id    ${handle}
    BM.Get Attribute    /app:Application[@ProcessId=${pid}]//Window[@Name="PlatynUI Swing Latecomer"]
    ...    Technology    ==    JAB
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}
