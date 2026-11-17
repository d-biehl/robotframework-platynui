*** Settings ***
Documentation       Window and element geometry of the GTK 3 fixture: where the platform places its
...                 windows and what their elements report as ``Bounds``. GTK 3 answers the
...                 parent-relative coordinates AT-SPI asks for with screen coordinates, so an element
...                 placed by adding up its ancestors counts each ancestor's position again.

Resource            resources/gtk3app.resource

Suite Setup         Launch GTK3 Test App
Suite Teardown      Terminate GTK3 Instance


*** Test Cases ***
The Main Window Reports Its Visible Frame
    [Documentation]    Under the compositor the main window draws its own shadow around its header
    ...    bar. Its Bounds are the visible frame the compositor reports for it, not its surface.
    [Tags]    platform:wayland
    ${pid}=    Get Process Id    ${GTK3_APP_HANDLE}
    ${listing}=    Run Process    ${{ os.environ["PLATYNUI_COMPOSITOR_CTL_BIN"] }}    --json    list-windows
    Should Be Equal As Integers    ${listing.rc}    0    ${listing.stderr}
    VAR    ${window}    ${{ [w for w in json.loads($listing.stdout)["windows"] if w.get("pid") == int($pid) and w.get("title") == "PlatynUI GTK3 TestApp"][0] }}
    VAR    ${visible}    ${{ ($window["content_x"] + $window["geometry_x"], $window["content_y"] + $window["geometry_y"], $window["content_width"], $window["content_height"]) }}
    ${bounds}=    BM.Get Attribute Value    .//(Frame|Window)[@Name="PlatynUI GTK3 TestApp"]    Bounds
    Should Be Equal    ${{ (round($bounds.x), round($bounds.y), round($bounds.width), round($bounds.height)) }}    ${visible}

A Click On A Nested Radio Button Lands On It
    [Documentation]    With the window moved away from the screen origin, a click on the radio
    ...    button inside the group lands on it: selecting it reports ``last-action-radio-second``.
    BM.Move Window    .//(Frame|Window)[@Name="PlatynUI GTK3 TestApp"]    ${160}    ${120}
    BM.Wait Until Query
    ...    .//(Frame|Window)[@Name="PlatynUI GTK3 TestApp"]/@Bounds.X >= 100 and .//(Frame|Window)[@Name="PlatynUI GTK3 TestApp"]/@Bounds.Y >= 100
    BM.Wait Until Gone    .//*[@Name="last-action-radio-second"]
    BM.Pointer Click    .//*[@Name="groupbox-basic"]//*[@Name="radio-second"]
    BM.Wait Until Exists    .//*[@Name="last-action-radio-second"]
