*** Settings ***
Documentation       Runtime ids of elements that UI Automation reports no runtime id for (spec
...                 *uia-runtime-id*).
...
...                 The tab headers of the WinForms test window reach UI Automation only through its MSAA
...                 proxy, as the MSAA child ids 1 to 5 of the tab control, and UI Automation gives them an
...                 empty runtime id. The provider derives their ids from the tab control's id, and a
...                 query that walks past them ends — with every match, or with "not found".

Library             Process
Library             PlatynUI.BareMetal    AS    BM

Suite Setup         Launch WinForms Test Window
Suite Teardown      Run Keyword And Ignore Error    Terminate Process    ${HANDLE}    kill=${True}


*** Variables ***
${SCRIPT}       ${CURDIR}${/}..${/}..${/}..${/}apps${/}winforms-test-window${/}winforms-test-window.ps1
${TITLE}        PlatynUI WinForms Runtime Ids
${HANDLE}       ${None}
${PID}          ${None}


*** Test Cases ***
A WinForms Tab Control Keeps The Id UI Automation Reports
    ${hwnd}=    BM.Get Attribute Value    .//*[@Id="tabMain"]    native:NativeWindowHandle
    BM.Get Attribute Value    .//*[@Id="tabMain"]    RuntimeId    ==    uia://app/${PID}/2a.${{ format(int($hwnd), 'x') }}

WinForms Tab Headers Get Distinct Ids From Their Child Ids
    ${tab_id}=    BM.Get Attribute Value    .//*[@Id="tabMain"]    RuntimeId
    @{headers}=    BM.Query    .//*[@Id="tabMain"]/TabItem
    Should Be Equal    ${{ [header.runtime_id for header in $headers] }}
    ...    ${{ [f'{$tab_id}/child:{n}' for n in range(1, 6)] }}

A Hit-Tested Tab Header Has The Id Of The Listed One
    BM.Activate Window    ./*[@Id="probeForm"]
    ${tab_id}=    BM.Get Attribute Value    .//*[@Id="tabMain"]    RuntimeId
    ${bounds}=    BM.Get Attribute Value    .//*[@Id="tabMain"]/TabItem[@Name="Properties"]    Bounds
    VAR    ${center}    ${{ $bounds.center() }}
    ${hit}=    BM.Get Element At Point    ${center.x}    ${center.y}
    Should Be Equal    ${hit.runtime_id}    ${tab_id}/child:3

A Fresh Snapshot Gives The Tab Headers The Same Ids
    @{first}=    BM.Query    .//*[@Id="tabMain"]/TabItem
    @{second}=    BM.Query    .//*[@Id="tabMain"]/TabItem
    Should Be Equal    ${{ [header.runtime_id for header in $second] }}
    ...    ${{ [header.runtime_id for header in $first] }}

Every Tab Header Is Found
    BM.Wait Until Query    count(.//*[@Id="tabMain"]/TabItem)    ==    ${5}
    BM.Wait Until Exists    .//*[@Id="tabMain"]/TabItem[@Name="History"]

A Missing Element Is Reported As Not Found
    VAR    ${start}    ${{ time.monotonic() }}
    Run Keyword And Expect Error    ElementNotFoundError: *
    ...    BM.Wait Until Exists    .//*[@Id="no-such-edit"]    query_overrides={'timeout': 3}
    Should Be True    time.monotonic() - $start < 10    msg=a query that matches nothing must end with its timeout


*** Keywords ***
Launch WinForms Test Window
    [Documentation]    Check that Windows PowerShell is there, start the helper window with it, wait
    ...    until UI Automation lists the window, and pin its application node as the suite's root.
    ...    The window belongs to the ``powershell.exe`` process itself, so its ProcessId pins it.
    Should Be True    shutil.which('powershell.exe')
    ...    msg=powershell.exe (Windows PowerShell 5.1, which runs the WinForms test window) is not on PATH
    ${handle}=    Start Process    powershell.exe    -NoProfile    -ExecutionPolicy    Bypass
    ...    -File    ${SCRIPT}    -Title    ${TITLE}    -AutoClose    600
    VAR    ${HANDLE}    ${handle}    scope=SUITE
    ${pid}=    Get Process Id    ${handle}
    VAR    ${PID}    ${pid}    scope=SUITE
    BM.Wait Until Exists    /app:Application[@ProcessId=${pid}]/*[@Name="${TITLE}"]
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
