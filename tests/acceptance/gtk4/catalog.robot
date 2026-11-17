*** Settings ***
Documentation       Blueprint catalog acceptance for the GTK 4 fixture (the canonical test set of
...                 dev-docs/testing-strategy.md §5.1), plus the window shape other suites rely on.
...                 The Suite Setup launches the fixture and pins it as the query root, so every
...                 locator below is relative and addresses catalog controls by their canonical
...                 @Name only.
...
...                 Menu, context-menu and drop-down items exist on the tree only while their
...                 popover is open, and collapsed tree children appear only after expansion.
...                 Activations report through the always-visible ``last-action-<ident>`` label.
...
...                 Test order matters for the click-counter test (the counter is cumulative per
...                 instance), so it runs before anything else clicks button-basic.

Resource            resources/gtk4app.resource

Suite Setup         Launch GTK4 Test App
Suite Teardown      Terminate GTK4 Instance


*** Variables ***
# Core-tier and extended-tier controls that must be enumerable on an idle (untouched) fixture
# instance. Deliberately absent: menu/context-menu/combo ITEMS (popovers exist only while open),
# collapsed tree children (appear after expansion), dialog-modal (own instance via --open-modal,
# see modal.robot), and the main window (matched by its launch title).
@{CATALOG_IDLE}     button-basic    status-label-clicks-0    last-action-none
...                 checkbox-basic
...                 groupbox-basic    radio-first    radio-second    textfield-basic
...                 textarea-basic    label-basic    text-basic    image-basic
...                 combobox-basic    list-basic    list-item-1    list-item-2
...                 list-item-3    list-item-4    list-item-5    tree-basic
...                 tree-node-a    tree-node-b    main-menubar    menu-file
...                 menu-edit    menu-help
...                 slider-basic    progress-basic    tabs-basic    tab-one    tab-two
...                 dialog-modeless    dialog-modeless-label    dialog-modeless-button


*** Test Cases ***
Core Catalog Controls Are Enumerable
    [Documentation]    Every idle-state catalog control resolves under its canonical @Name.
    FOR    ${name}    IN    @{CATALOG_IDLE}
        BM.Wait Until Exists    .//*[@Name="${name}"]
    END

Clicking The Catalog Button Updates The Click Counter
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-0"]
    BM.Pointer Click    .//*[@Name="button-basic"]
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-1"]
    BM.Pointer Click    .//*[@Name="button-basic"]
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-2"]

Expanding Tree Nodes Reveals Descendants
    [Documentation]    Double-click expands a collapsed row (the cross-toolkit expansion gesture),
    ...    revealing child and grandchild — proving all three catalog tree levels.
    BM.Wait Until Gone    .//*[@Name="tree-basic"]//*[@Name="tree-node-a-1"]
    BM.Pointer Multi Click    .//*[@Name="tree-basic"]//*[@Name="tree-node-a"]
    BM.Wait Until Exists    .//*[@Name="tree-basic"]//*[@Name="tree-node-a-1"]
    BM.Wait Until Gone    .//*[@Name="tree-basic"]//*[@Name="tree-node-a-1-i"]
    BM.Pointer Multi Click    .//*[@Name="tree-basic"]//*[@Name="tree-node-a-1"]
    BM.Wait Until Exists    .//*[@Name="tree-basic"]//*[@Name="tree-node-a-1-i"]

Clicking The Modeless Dialog Button Lands Inside The Dialog
    [Documentation]    ``last-action-dialog-modeless-button`` only appears if the pointer action
    ...    really landed inside the dialog — the bounds-correctness observable of the blueprint.
    BM.Wait Until Gone    .//*[@Name="last-action-dialog-modeless-button"]
    BM.Pointer Click    .//*[@Name="dialog-modeless"]//*[@Name="dialog-modeless-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-dialog-modeless-button"]

The Main Window Is Found By Its Launch Title
    ${windows}=    BM.Query    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]
    Length Should Be    ${windows}    1

The Radio Buttons Are Inside The Group
    BM.Wait Until Exists    .//*[@Name="groupbox-basic"]//*[@Name="radio-first"]
    BM.Wait Until Exists    .//*[@Name="groupbox-basic"]//*[@Name="radio-second"]

The Modeless Dialog Differs In Size From The Main Window
    [Documentation]    The X11 window lookup tells two windows of one process apart by size, so
    ...    the dialog must not share the main window's size.
    ${main}=    BM.Get Attribute Value    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]    Bounds
    ${dialog}=    BM.Get Attribute Value    .//*[@Name="dialog-modeless"]    Bounds
    Should Be True    abs($main.width - $dialog.width) > 64 or abs($main.height - $dialog.height) > 64

The Main Window Draws A Header Bar
    [Documentation]    Under the compositor the main window uses client-side decorations, and its
    ...    title is part of its own tree, in the header bar.
    [Tags]    platform:wayland
    ${pid}=    Get Process Id    ${GTK4_APP_HANDLE}
    ${listing}=    Run Process    ${{ os.environ["PLATYNUI_COMPOSITOR_CTL_BIN"] }}    --json    list-windows
    Should Be Equal As Integers    ${listing.rc}    0    ${listing.stderr}
    VAR    ${windows}    ${{ [w for w in json.loads($listing.stdout)["windows"] if w.get("pid") == int($pid) and w.get("title") == "PlatynUI GTK4 TestApp"] }}
    Length Should Be    ${windows}    1
    Should Be Equal    ${windows}[0][decoration_mode]    csd
    BM.Wait Until Exists    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]//*[@Name="PlatynUI GTK4 TestApp"]

An Unknown Argument Fails Fast
    ${result}=    Run Process    ${GTK4_APP_PYTHON}    ${GTK4_APP_MAIN}    --bogus    stderr=STDOUT    timeout=60s
    Should Not Be Equal As Integers    ${result.rc}    0
    Should Contain    ${result.stdout}    --bogus
    Should Contain    ${result.stdout}    usage
