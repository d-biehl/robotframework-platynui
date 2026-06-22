*** Settings ***
Documentation     Launch and tear down platynui-test-app-egui instances from Robot Framework, so the
...               suites decide how many windows exist and clean them up. The binary is compiled by
...               scripts/platynui-robot-session.sh and handed over via %{PLATYNUI_TEST_APP_BIN}; set
...               that variable yourself when launching the app outside that script (e.g. the repl).
Library           Process
Library           PlatynUI.BareMetal    AS    BM


*** Variables ***
${TEST_APP_BIN}        %{PLATYNUI_TEST_APP_BIN}
${APP_WAIT_TIMEOUT}    15s


*** Keywords ***
Launch Test App
    [Documentation]    Start one egui instance with a distinct app id and window title, then wait until
    ...    its window is on the accessibility tree. Returns the Process handle; terminate it with
    ...    Terminate App, or all instances at once with Terminate Test Apps.
    [Arguments]    ${title}    ${app_id}
    ${handle}=    Start Process    ${TEST_APP_BIN}    --app-id    ${app_id}    --title    ${title}
    ...    stdout=${TEMPDIR}/egui-${app_id}.log    stderr=STDOUT
    Wait For App Window    ${title}
    RETURN    ${handle}

Wait For App Window
    [Documentation]    Poll the accessibility tree until a top-level element named ${title} appears.
    [Arguments]    ${title}    ${timeout}=${APP_WAIT_TIMEOUT}
    Wait Until Keyword Succeeds    ${timeout}    0.25s    App Window Is Present    ${title}

App Window Is Present
    [Arguments]    ${title}
    ${win}=    BM.Query    //*[@Name="${title}"]    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=window '${title}' is not on the accessibility tree yet

Terminate App
    [Documentation]    Terminate a single instance by the handle returned from Launch Test App.
    [Arguments]    ${handle}
    Terminate Process    ${handle}

Terminate Test Apps
    [Documentation]    Terminate every instance started through this resource — for the outermost
    ...    Suite Teardown.
    Terminate All Processes
