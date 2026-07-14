*** Settings ***
Documentation       Real acceptance for menu-entry namespace classification (spec
...                 ``menu-item-namespace``). A menu entry is an interactive control — invokable like
...                 a button, possibly owning a submenu — not a data item of a collection, so every
...                 provider exposes it in the default ``control`` namespace: plain ``//MenuItem``
...                 must resolve it and ``//item:MenuItem`` must not, identically on AT-SPI and UIA.
...                 Drives the menu-bar's File menu of the Qt test app: open it, resolve an entry
...                 without a namespace prefix, activate it. Activation renames the action's @Name to
...                 ``menu-file-new-activated`` (the app's dialog-button pattern), so the activation
...                 is asserted purely by locator.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Test Teardown       Teardown Menu Test
Test Tags           real


*** Variables ***
# How long tree updates (popup surfacing, the activation rename) may take to become visible.
${MENU_SETTLE_TIMEOUT}      5s


*** Test Cases ***
An Open Menu Entry Resolves As A Control And Activates
    [Documentation]    Open the File menu and resolve one of its entries via ``//MenuItem`` — no
    ...    namespace prefix, i.e. the default ``control`` namespace. The same entry must NOT match
    ...    as ``//item:MenuItem`` (the negative half of the spec scenario). Clicking the entry must
    ...    activate it, observable as the action renaming itself to ``menu-file-new-activated``.
    Launch Main Window Only
    BM.Pointer Click    ${APP}${MENU_FILE}
    Wait Until Keyword Succeeds    ${MENU_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${MENU_FILE_NEW}
    Node Is Absent    ${APP}//item:MenuItem[@Name="menu-file-new"]
    BM.Pointer Click    ${APP}${MENU_FILE_NEW}
    Wait Until Keyword Succeeds    ${MENU_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${MENU_FILE_NEW_ACTIVATED}


*** Keywords ***
Launch Main Window Only
    [Documentation]    Launch a single top-level Qt window (``--dialogs 0``) and pin it as ``${APP}``.
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt Menu)    org.platynui.test.qt.menu    --dialogs    0
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=TEST
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=TEST

Teardown Menu Test
    [Documentation]    Best-effort menu dismissal (so no popup grab outlives a failed test), then
    ...    terminate the app instance.
    Run Keyword And Ignore Error    BM.Keyboard Type    ${None}    <Escape>
    Terminate Default Qt Instance
