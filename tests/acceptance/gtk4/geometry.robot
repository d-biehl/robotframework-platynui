*** Settings ***
Documentation       Window and element geometry of the GTK 4 fixture: where the platform places its
...                 windows and what their elements report as ``Bounds``. GTK 4 reports no screen
...                 positions of its own, so every geometry here comes from the platform's view of
...                 the window.

Resource            resources/gtk4app.resource

Suite Setup         Launch GTK4 Test App
Suite Teardown      Terminate GTK4 Instance


*** Test Cases ***
Each Window Of Two Has Its Own Bounds
    [Documentation]    The main window and the modeless dialog belong to one process, and GTK 4
    ...    reports neither window's screen position, so the platform tells them apart by size.
    ...    The dialog opens at its default 420×300, which its small content does not enlarge;
    ...    the main window opens at 900×700 or larger, as its content needs.
    ${dialog}=    BM.Get Attribute Value    .//(Frame|Window)[@Name="dialog-modeless"]    Bounds
    Should Be True    $dialog.width == 420 and $dialog.height == 300
    ...    the dialog reports ${dialog}, not its own 420×300
    ${main}=    BM.Get Attribute Value    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]    Bounds
    Should Be True    $main.width >= 900 and $main.height >= 700
    ...    the main window reports ${main}, smaller than its own 900×700

The Main Window Reports Its Visible Frame
    [Documentation]    Under the compositor the main window draws its own shadow around its header
    ...    bar. Its Bounds are the visible frame the compositor reports for it, not its surface, and
    ...    its elements are placed from that frame.
    [Tags]    platform:wayland
    ${pid}=    Get Process Id    ${GTK4_APP_HANDLE}
    ${listing}=    Run Process    ${{ os.environ["PLATYNUI_COMPOSITOR_CTL_BIN"] }}    --json    list-windows
    Should Be Equal As Integers    ${listing.rc}    0    ${listing.stderr}
    VAR    ${window}    ${{ [w for w in json.loads($listing.stdout)["windows"] if w.get("pid") == int($pid) and w.get("title") == "PlatynUI GTK4 TestApp"][0] }}
    VAR    ${visible}    ${{ ($window["content_x"] + $window["geometry_x"], $window["content_y"] + $window["geometry_y"], $window["content_width"], $window["content_height"]) }}
    ${bounds}=    BM.Get Attribute Value    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]    Bounds
    Should Be Equal    ${{ (round($bounds.x), round($bounds.y), round($bounds.width), round($bounds.height)) }}    ${visible}

A Click On A Nested Radio Button Lands On It
    [Documentation]    With the window moved away from the screen origin, a click on the radio
    ...    button inside the group lands on it: selecting it reports ``last-action-radio-second``.
    BM.Move Window    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]    ${160}    ${120}
    BM.Wait Until Query
    ...    .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]/@Bounds.X >= 100 and .//(Frame|Window)[@Name="PlatynUI GTK4 TestApp"]/@Bounds.Y >= 100
    BM.Wait Until Gone    .//*[@Name="last-action-radio-second"]
    BM.Pointer Click    .//*[@Name="groupbox-basic"]//*[@Name="radio-second"]
    BM.Wait Until Exists    .//*[@Name="last-action-radio-second"]

A Point Inside A GTK 4 Submenu Resolves To Its Item
    [Documentation]    With the nested submenu open, the element at the center of a submenu item's
    ...    Bounds is that item or an element inside it: the reported bounds are where the item is
    ...    drawn, and the hit-test prefers the popup over the widgets beneath it.
    BM.Pointer Click    .//*[@Name="menu-edit"]
    BM.Pointer Click    .//*[@Name="menu-edit"]//*[@Name="menu-edit-more"]
    ${item}=    BM.Wait Until Exists    .//*[@Name="menu-edit"]//*[@Name="menu-edit-more"]//*[@Name="menu-edit-sub-one"]
    ${bounds}=    BM.Get Attribute Value    ${item}    Bounds
    ${hit}=    BM.Get Element At Point    ${{ $bounds.x + $bounds.width / 2 }}    ${{ $bounds.y + $bounds.height / 2 }}
    Should Not Be Equal    ${hit}    ${None}    msg=the hit-test resolved nothing at the item's center
    VAR    ${chain}    ${{ [$hit.runtime_id] + [node.runtime_id for node in $hit.ancestors()] }}
    Should Contain    ${chain}    ${item.runtime_id}    msg=the item's center resolved to ${hit}, outside the item
    [Teardown]    Run Keyword And Ignore Error    BM.Keyboard Type    ${None}    <Escape><Escape>
