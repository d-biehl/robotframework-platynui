*** Settings ***
Documentation       Real acceptance for menu-entry namespace classification (spec
...                 ``menu-item-namespace``). A menu entry is an interactive control — invokable like
...                 a button, possibly owning a submenu — not a data item of a collection, so every
...                 provider exposes it in the default ``control`` namespace: plain ``//MenuItem``
...                 must resolve it and ``//item:MenuItem`` must not, identically on AT-SPI and UIA.
...                 Drives the menu-bar's File menu of the Qt test app: open it, resolve an entry
...                 without a namespace prefix, activate it. Activation reports the entry's ident
...                 through the main window's last-action label, so it is asserted purely by locator.

Resource            resources/testapp.resource

Test Teardown       Teardown Menu Test

Test Tags           real


*** Test Cases ***
An Open Menu Entry Resolves As A Control And Activates
    [Documentation]    Open the File menu and resolve one of its entries via ``//MenuItem`` — no
    ...    namespace prefix, i.e. the default ``control`` namespace. The same entry must NOT match
    ...    as ``//item:MenuItem`` (the negative half of the spec scenario). Clicking the entry must
    ...    activate it, observable as ``last-action-menu-file-new`` on the last-action label.
    Launch Qt Test App    PlatynUI Test App (Qt Menu)    org.platynui.test.qt.menu    --dialogs    0
    ...    scope=TEST
    BM.Pointer Click    .//MenuItem[@Name="File"]
    BM.Wait Until Exists    .//MenuItem[@Name="menu-file-new"]
    ${item}=    BM.Query    .//item:MenuItem[@Name="menu-file-new"]    only_first=${True}
    Should Be Equal    ${item}    ${None}    msg=menu entry must not be classified in the item: namespace
    BM.Pointer Click    .//MenuItem[@Name="menu-file-new"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-file-new"]


*** Keywords ***
Teardown Menu Test
    [Documentation]    Best-effort menu dismissal (so no popup grab outlives a failed test), then
    ...    terminate the app instance.
    Run Keyword And Ignore Error    BM.Keyboard Type    ${None}    <Escape>
    Terminate Qt Instance
