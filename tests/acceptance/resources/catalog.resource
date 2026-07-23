*** Settings ***
Documentation       Technology-neutral catalog keywords for the fixture blueprint
...                 (dev-docs/testing-strategy.md §5.1) — the shared half of the catalog
...                 acceptance suite. Keywords address catalog controls by their canonical
...                 ``@Name`` ONLY (never by role: roles differ across bridges — Qt Quick
...                 exposes lists/trees as Group and tree rows as ListItem — while the
...                 canonical names are pairwise-unique app-wide by blueprint rule).
...
...                 A technology onboards by adding a thin ``tests/acceptance/<tech>/catalog.robot``
...                 that imports this resource, launches its fixture (pinned by ProcessId), sets the
...                 app root as the SUITE variable ``${CATALOG_APP}``, and declares the catalog
...                 tests as one-liners over the ``... Should ...`` keywords below. Technology
...                 limitations are declared as documented skips in that suite, never patched into
...                 this resource.
...
...                 Menu and combo items exist on the tree only while their popup is OPEN
...                 (verified Qt Quick reality; menu items also rename only observably on the
...                 next open), and collapsed tree children appear only after expansion — the
...                 keywords below encode those dynamics.

Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
# App root locator (e.g. /app:Application[@ProcessId=...]) — set at SUITE scope by the
# technology's launcher keyword. Declared here so the locators resolve statically.
${CATALOG_APP}          ${None}
${CATALOG_WAIT}         10s

# Core-tier controls that must be enumerable on an idle (untouched) fixture instance.
# Deliberately absent: menu/context-menu/combo ITEMS (popups exist only while open),
# collapsed tree children (appear after expansion), dialog-modal (own instance via
# --open-modal), and the main window (matched via launch configuration — on bridges
# that derive a window's name from its title, "main-window" is not a resolvable name).
@{CATALOG_CORE_IDLE}    button-basic    status-label-clicks-0    checkbox-basic
...                     groupbox-basic    radio-first    radio-second    textfield-basic
...                     textarea-basic    label-basic    text-basic    image-basic
...                     combobox-basic    list-basic    list-item-1    list-item-2
...                     list-item-3    list-item-4    list-item-5    tree-basic
...                     tree-node-a    tree-node-b    main-menubar    menu-file
...                     menu-edit    menu-help
...                     dialog-modeless    dialog-modeless-label    dialog-modeless-button


*** Keywords ***
Catalog Node Is Present
    [Documentation]    Assert the catalog control ``${name}`` is currently resolvable by its
    ...    canonical ``@Name`` (predicate for Wait Until Keyword Succeeds).
    [Arguments]    ${name}
    ${node}=    BM.Query    ${CATALOG_APP}//*[@Name="${name}"]    only_first=${True}
    Should Not Be Equal    ${node}    ${None}    msg=catalog control not on the accessibility tree: ${name}

Catalog Node Is Absent
    [Documentation]    Assert the catalog control ``${name}`` does NOT currently resolve.
    [Arguments]    ${name}
    ${node}=    BM.Query    ${CATALOG_APP}//*[@Name="${name}"]    only_first=${True}
    Should Be Equal    ${node}    ${None}    msg=catalog control unexpectedly present: ${name}

Wait Until Catalog Node Appears
    [Arguments]    ${name}
    Wait Until Keyword Succeeds    ${CATALOG_WAIT}    0.25s    Catalog Node Is Present    ${name}

Wait Until Catalog Node Disappears
    [Arguments]    ${name}
    Wait Until Keyword Succeeds    ${CATALOG_WAIT}    0.25s    Catalog Node Is Absent    ${name}

Click Catalog Control
    [Documentation]    Left-click the catalog control's activation point.
    [Arguments]    ${name}
    BM.Pointer Click    ${CATALOG_APP}//*[@Name="${name}"]

Dismiss Catalog Popup
    [Documentation]    Close an open menu/popup without activating anything (Escape).
    BM.Keyboard Type    ${None}    <Escape>

Core Catalog Controls Should Be Enumerable
    [Documentation]    Every idle-state core-tier control resolves under its canonical name.
    FOR    ${name}    IN    @{CATALOG_CORE_IDLE}
        Wait Until Catalog Node Appears    ${name}
    END

Clicking The Catalog Button Should Update The Click Counter
    [Documentation]    ``button-basic`` twice: the counter observable ends at clicks-2. Must run
    ...    before any other keyword clicks the button (the counter is cumulative per instance).
    Wait Until Catalog Node Appears    status-label-clicks-0
    Click Catalog Control    button-basic
    Wait Until Catalog Node Appears    status-label-clicks-1
    Click Catalog Control    button-basic
    Wait Until Catalog Node Appears    status-label-clicks-2

Activating A Menu Item Should Rename It
    [Documentation]    Open ``${menu}``, activate ``${item}``, then re-open the menu and assert the
    ...    ``<ident>-activated`` rename (items are only observable while their menu is open).
    ...    Leaves the menu closed.
    [Arguments]    ${menu}    ${item}
    Click Catalog Control    ${menu}
    Wait Until Catalog Node Appears    ${item}
    Click Catalog Control    ${item}
    Click Catalog Control    ${menu}
    Wait Until Catalog Node Appears    ${item}-activated
    Dismiss Catalog Popup

Activating A Menu Bar Submenu Item Should Rename It
    [Documentation]    Open ``${menu}`` in the menu bar, open its nested ``${submenu}``, activate
    ...    ``${item}``, then re-open both levels and assert the rename. Leaves the menu closed.
    [Arguments]    ${menu}    ${submenu}    ${item}
    Click Catalog Control    ${menu}
    Wait Until Catalog Node Appears    ${submenu}
    Click Catalog Control    ${submenu}
    Wait Until Catalog Node Appears    ${item}
    Click Catalog Control    ${item}
    Click Catalog Control    ${menu}
    Wait Until Catalog Node Appears    ${submenu}
    Click Catalog Control    ${submenu}
    Wait Until Catalog Node Appears    ${item}-activated
    Dismiss Catalog Popup

Open Catalog Context Menu
    [Documentation]    Right-click on ``label-basic`` (a guaranteed idle catalog control inside the
    ...    main window) to open the fixture's context menu.
    BM.Pointer Click    ${CATALOG_APP}//*[@Name="label-basic"]    button=right

Activating A Context Menu Item Should Rename It
    [Documentation]    Open the context menu, activate ``${item}``, re-open and assert the rename.
    [Arguments]    ${item}
    Open Catalog Context Menu
    Wait Until Catalog Node Appears    ${item}
    Click Catalog Control    ${item}
    Open Catalog Context Menu
    Wait Until Catalog Node Appears    ${item}-activated
    Dismiss Catalog Popup

Activating A Context Submenu Item Should Rename It
    [Documentation]    Open the context menu, hover/open the ``ctx-more`` submenu, activate
    ...    ``${item}``, then re-open both levels and assert the rename.
    [Arguments]    ${item}
    Open Catalog Context Menu
    Wait Until Catalog Node Appears    ctx-more
    Click Catalog Control    ctx-more
    Wait Until Catalog Node Appears    ${item}
    Click Catalog Control    ${item}
    Open Catalog Context Menu
    Wait Until Catalog Node Appears    ctx-more
    Click Catalog Control    ctx-more
    Wait Until Catalog Node Appears    ${item}-activated
    Dismiss Catalog Popup

Opening The Combo Box Should Reveal Its Items
    [Documentation]    Combo items exist only while the popup is open; selecting one closes it.
    Catalog Node Is Absent    combo-item-2
    Click Catalog Control    combobox-basic
    Wait Until Catalog Node Appears    combo-item-2
    Click Catalog Control    combo-item-2
    Wait Until Catalog Node Disappears    combo-item-2

Expanding Tree Nodes Should Reveal Descendants
    [Documentation]    Double-click expands a collapsed row (the cross-toolkit expansion gesture):
    ...    tree-node-a reveals tree-node-a-1, whose expansion reveals the grandchild — proving all
    ...    three catalog tree levels.
    Catalog Node Is Absent    tree-node-a-1
    BM.Pointer Multi Click    ${CATALOG_APP}//*[@Name="tree-node-a"]
    Wait Until Catalog Node Appears    tree-node-a-1
    Catalog Node Is Absent    tree-node-a-1-i
    BM.Pointer Multi Click    ${CATALOG_APP}//*[@Name="tree-node-a-1"]
    Wait Until Catalog Node Appears    tree-node-a-1-i

Clicking The Modeless Dialog Button Should Land Inside The Dialog
    [Documentation]    The rename to ``-clicked`` only happens if the pointer action really landed
    ...    inside the dialog — the bounds-correctness observable of the blueprint.
    Catalog Node Is Absent    dialog-modeless-button-clicked
    Click Catalog Control    dialog-modeless-button
    Wait Until Catalog Node Appears    dialog-modeless-button-clicked

Modal Dialog Should Be Present Without Interaction
    [Documentation]    On an instance started with ``--open-modal``: the modal dialog and its
    ...    contents are on the tree without any prior input. Modal STATE is asserted per
    ...    technology (documented deviation where the bridge does not surface it).
    Wait Until Catalog Node Appears    dialog-modal
    Wait Until Catalog Node Appears    dialog-modal-label
    Wait Until Catalog Node Appears    dialog-modal-button

Clicking The Modal Dialog Button Should Land Inside The Dialog
    Catalog Node Is Absent    dialog-modal-button-clicked
    Click Catalog Control    dialog-modal-button
    Wait Until Catalog Node Appears    dialog-modal-button-clicked
