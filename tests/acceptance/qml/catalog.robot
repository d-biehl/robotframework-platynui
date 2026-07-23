*** Settings ***
Documentation       Thin catalog onboarding suite for the QML fixture — supplies only the launch
...                 configuration (resources/qmlapp.resource) and declares the shared catalog tests
...                 (tests/acceptance/resources/catalog.resource). Test logic lives in the shared
...                 resource; QML-specific coverage lives in the sibling suites.
...
...                 Test order matters for the click-counter test (the counter is cumulative per
...                 instance), so it runs before anything else clicks button-basic.

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Catalog Instance
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Core Catalog Controls Are Enumerable
    [Documentation]    Every idle-state core-tier control resolves under its canonical @Name.
    Core Catalog Controls Should Be Enumerable

Clicking The Catalog Button Updates The Click Counter
    Clicking The Catalog Button Should Update The Click Counter

Activating A Menu Bar Item Renames It
    Activating A Menu Item Should Rename It    menu-file    menu-file-new

Activating A Menu Bar Submenu Item Renames It
    Activating A Menu Bar Submenu Item Should Rename It    menu-edit    menu-edit-more    menu-edit-sub-one

Activating A Context Menu Item Renames It
    Activating A Context Menu Item Should Rename It    ctx-copy

Activating A Context Submenu Item Renames It
    Activating A Context Submenu Item Should Rename It    ctx-sub-alpha

Opening The Combo Box Reveals Its Items
    Opening The Combo Box Should Reveal Its Items

Expanding Tree Nodes Reveals Descendants
    Expanding Tree Nodes Should Reveal Descendants

Clicking The Modeless Dialog Button Lands Inside The Dialog
    Clicking The Modeless Dialog Button Should Land Inside The Dialog
