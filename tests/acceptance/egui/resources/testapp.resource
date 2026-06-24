*** Settings ***
Documentation     Launch and tear down platynui-test-app-egui instances from Robot Framework, so each
...               suite owns the instance(s) it needs and starts from a known state — the top-level
...               suite launches nothing. The binary is compiled by scripts/platynui-robot-session.sh
...               and handed over via the PLATYNUI_TEST_APP_BIN environment variable; set it yourself
...               when launching the app outside that script (e.g. the repl).
...
...               Each instance is pinned by its launched ProcessId, so its window root locator
...               (``\${WINDOW}``, or Alpha/Beta in auto_activate) addresses exactly that copy of the
...               program regardless of window title or stacking. Re-exports the page-object locators.
Library           Process
Library           PlatynUI.BareMetal    AS    BM

Resource          ${CURDIR}/testapp_locators.resource


*** Variables ***
${TEST_APP_BIN}        %{PLATYNUI_TEST_APP_BIN=target/debug/platynui-test-app-egui}
${APP_WAIT_TIMEOUT}    15s
# Handle of the instance started by Launch Default Instance — read by Terminate Default Instance.
${TEST_APP_HANDLE}     ${None}


*** Keywords ***
Launch Test App
    [Documentation]    Start one egui instance (distinct app id + window title) and wait until its
    ...    window — pinned by the launched ProcessId — is on the accessibility tree. Returns the
    ...    Process handle; get its window root with ``App Window Root``. Terminate it with Terminate App.
    [Arguments]    ${title}    ${app_id}
    ${handle}=    Start Process    ${TEST_APP_BIN}    --app-id    ${app_id}    --title    ${title}
    ...    stdout=${TEMPDIR}/egui-${app_id}.log    stderr=STDOUT
    Wait For App Window    ${handle}
    RETURN    ${handle}

App Window Root
    [Documentation]    Window root locator for a launched instance, pinned by its ProcessId:
    ...    ``/app:Application[@ProcessId=<pid>]/(Frame|Window)``. The role union is cross-platform —
    ...    Frame on AT-SPI (Linux), Window on UIA (Windows).
    [Arguments]    ${handle}
    ${pid}=    Get Process Id    ${handle}
    RETURN    /app:Application[@ProcessId=${pid}]/(Frame|Window)

Wait For App Window
    [Documentation]    Poll the accessibility tree until the launched instance's window is present.
    [Arguments]    ${handle}    ${timeout}=${APP_WAIT_TIMEOUT}
    ${window}=    App Window Root    ${handle}
    Wait Until Keyword Succeeds    ${timeout}    0.25s    App Window Is Present    ${window}

App Window Is Present
    [Arguments]    ${window}
    ${win}=    BM.Query    ${window}    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=window not on the accessibility tree yet: ${window}

Launch Default Instance
    [Documentation]    Suite Setup helper for the single-window suites: launch the default egui
    ...    instance, pin it by ProcessId, expose its window root as the suite variable ``\${WINDOW}``,
    ...    and remember the handle for Terminate Default Instance.
    ${handle}=    Launch Test App    PlatynUI Test App    com.platynui.test
    ${root}=    App Window Root    ${handle}
    VAR    ${WINDOW}    ${root}    scope=SUITE
    VAR    ${TEST_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Instance.
    Run Keyword And Ignore Error    Terminate App    ${TEST_APP_HANDLE}

Terminate App
    [Documentation]    Terminate a single instance by the handle returned from Launch Test App.
    [Arguments]    ${handle}
    Terminate Process    ${handle}
