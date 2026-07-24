*** Settings ***
Documentation     Launch and tear down platynui-test-app-egui instances from Robot Framework, so each
...               suite owns the instance(s) it needs and starts from a known state — the top-level
...               suite launches nothing. The binary is compiled by scripts/platynui-robot-session.sh
...               and handed over via the PLATYNUI_TEST_APP_BIN environment variable; set it yourself
...               when launching the app outside that script (e.g. the repl).
...
...               ``Launch Default Instance`` pins the launched instance's window as the suite-wide
...               query root (``Set Root``, SUITE scope) and exposes it as ``${WINDOW}``, so
...               single-instance suites address widgets relatively (``.//*[@Id=...]``) while suites
...               with several windows or library instances (auto_activate, the inspector suites,
...               coexisting_runtimes) prefix the absolute per-window root. Every interactive widget
...               carries a stable AccessKit author id, surfaced as ``@Id`` (language-independent,
...               stable across label/layout changes) — the locator contract of this fixture.
Library           Process
Library           PlatynUI.BareMetal    AS    BM


*** Variables ***
# Honor PLATYNUI_TEST_APP_BIN; otherwise default to the debug build as an
# ABSOLUTE, OS-native path: on Windows CreateProcess will not launch a relative,
# forward-slash exe path even when it exists, and it needs the .exe suffix.
# os.path.abspath resolves against the project root (robotcode's CWD).
${TEST_APP_BIN}        ${{ os.environ.get("PLATYNUI_TEST_APP_BIN") or os.path.abspath("target/debug/platynui-test-app-egui" + (".exe" if os.name == "nt" else "")) }}
${APP_WAIT_TIMEOUT}    15
# Window root of the instance started by Launch Default Instance, plus its handle.
${WINDOW}              ${None}
${TEST_APP_HANDLE}     ${None}


*** Keywords ***
Launch Test App
    [Documentation]    Start one egui instance (distinct app id + window title) and wait until its
    ...    window — pinned by the launched ProcessId — is on the accessibility tree. Returns the
    ...    Process handle; get its window root with ``App Window Root``. Terminate it with Terminate App.
    [Arguments]    ${title}    ${app_id}
    ${handle}=    Start Process    ${TEST_APP_BIN}    --app-id    ${app_id}    --title    ${title}
    ...    stdout=${TEMPDIR}/egui-${app_id}.log    stderr=STDOUT
    ${window}=    App Window Root    ${handle}
    BM.Wait Until Exists    ${window}    query_overrides={'timeout': ${APP_WAIT_TIMEOUT}}
    RETURN    ${handle}

App Window Root
    [Documentation]    Window root locator for a launched instance, pinned by its ProcessId:
    ...    ``/app:Application[@ProcessId=<pid>]/(Frame|Window)``. The role union is cross-platform —
    ...    Frame on AT-SPI (Linux), Window on UIA (Windows).
    [Arguments]    ${handle}
    ${pid}=    Get Process Id    ${handle}
    RETURN    /app:Application[@ProcessId=${pid}]/(Frame|Window)

Launch Default Instance
    [Documentation]    Suite Setup for the single-instance suites: launch the default egui instance,
    ...    pin its window as the suite-wide query root (``Set Root``) and as ``${WINDOW}``, and
    ...    remember the handle for Terminate Default Instance.
    ${handle}=    Launch Test App    PlatynUI Test App    com.platynui.test
    ${root}=    App Window Root    ${handle}
    BM.Set Root    ${root}    scope=SUITE
    VAR    ${WINDOW}    ${root}    scope=SUITE
    VAR    ${TEST_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Instance.
    Run Keyword And Ignore Error    Terminate App    ${TEST_APP_HANDLE}

Terminate App
    [Documentation]    Terminate a single instance by the handle returned from Launch Test App.
    [Arguments]    ${handle}
    Terminate Process    ${handle}

Get Click Count
    [Documentation]    Read the integer N from the "Clicks: N" status label of the given window
    ...    (default: the current query root's instance). ``Get Attribute`` waits/retries for the
    ...    label, then N is scraped from ``@Name`` — the dynamic count stays in the label text.
    [Arguments]    ${window}=.
    ${name}=    BM.Get Attribute    ${window}//*[@Id="status-clicks"]    Name
    RETURN    ${{ int($name.split(":")[1]) }}
