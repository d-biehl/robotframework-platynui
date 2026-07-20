*** Settings ***
Documentation     Launch and tear down Swing fixture instances (apps/test-app-swing) from Robot
...               Framework, so each suite owns the instance(s) it needs — the top-level suite launches
...               nothing.
...
...               The fixture is plain Java 8 run straight from its class directory. The directory (and
...               optionally the launcher) come from PLATYNUI_TEST_APP_SWING_CLASSES /
...               PLATYNUI_TEST_APP_SWING_JAVA (prepared by the test-acceptance-windows recipe; set them
...               yourself when running outside it). Every launch passes
...               -Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge,
...               enabling the Access Bridge for that process only — no jabswitch, no machine state.
...
...               Each instance is pinned by its launched ProcessId (``${APP}`` =
...               ``/app:Application[@ProcessId=...]``) and gets a per-suite unique window title (the
...               frame's accessibleName tracks the title), so parallel instances never cross-talk.
Library           OperatingSystem
Library           Process
Library           PlatynUI.BareMetal    AS    BM

Resource          ${CURDIR}/testapp_locators.resource


*** Variables ***
${SWING_JAVA}           ${{ os.environ.get("PLATYNUI_TEST_APP_SWING_JAVA") or "java" }}
${SWING_CLASSES}        ${{ os.environ.get("PLATYNUI_TEST_APP_SWING_CLASSES") or os.path.abspath("apps/test-app-swing/build/classes") }}
${SWING_ENABLE_FLAG}    -Djavax.accessibility.assistive_technologies\=com.sun.java.accessibility.AccessBridge
${APP_WAIT_TIMEOUT}     20s
# Handle/title of the instance started by Launch Default Swing Instance.
${SWING_APP_HANDLE}     ${None}
${SWING_TITLE}          ${None}


*** Keywords ***
Require Swing Prerequisites
    [Documentation]    Skip the suite (with an actionable message) when the JAB lane cannot run here:
    ...    non-Windows platform, no java launcher, or the fixture classes have not been built.
    Skip If    ${{ os.name != "nt" }}    Swing/JAB acceptance is Windows-only (Linux Swing rides AT-SPI in a future lane)
    ${java_missing}=    Run Keyword And Return Status    Run Process    ${SWING_JAVA}    -version
    Skip If    not ${java_missing}    no usable Java launcher (${SWING_JAVA}) — install a JDK 8+ and put it on PATH
    Skip If    ${{ not os.path.isdir($SWING_CLASSES) }}
    ...    Swing fixture not built at ${SWING_CLASSES} — run `just build-test-app-swing`

Launch Swing Test App
    [Documentation]    Start one fixture instance with a unique window title and wait until its main
    ...    window — pinned by the launched ProcessId — is on the accessibility tree with
    ...    ``@Technology = "JAB"``. Extra CLI args are forwarded. Returns the Process handle.
    [Arguments]    ${title}    @{extra_args}
    ${handle}=    Start Process    ${SWING_JAVA}    ${SWING_ENABLE_FLAG}    -cp    ${SWING_CLASSES}
    ...    platynui.testapp.Main    --title    ${title}    @{extra_args}
    ...    stdout=${TEMPDIR}/swing-${title}.log    stderr=STDOUT
    ${root}=    App Root    ${handle}
    Wait Until Keyword Succeeds    ${APP_WAIT_TIMEOUT}    0.25s    Node Is Present    ${root}${{ '//Window[@Name="' + $title + '"]' }}
    RETURN    ${handle}

App Root
    [Documentation]    App root locator for a launched instance, pinned by its ProcessId:
    ...    ``/app:Application[@ProcessId=<pid>]``. Prepend it to the control locators.
    [Arguments]    ${handle}
    ${pid}=    Get Process Id    ${handle}
    RETURN    /app:Application[@ProcessId=${pid}]

Window Locator
    [Documentation]    App-relative locator of the fixture's main window (its @Name is the launch title).
    [Arguments]    ${title}
    RETURN    //Window[@Name="${title}"]

Node Is Present
    [Documentation]    Assert a node is currently resolvable on the accessibility tree (predicate for
    ...    Wait Until Keyword Succeeds).
    [Arguments]    ${locator}
    ${node}=    BM.Query    ${locator}    only_first=${True}
    Should Not Be Equal    ${node}    ${None}    msg=not on the accessibility tree yet: ${locator}

Node Is Absent
    [Documentation]    Assert a node does NOT currently resolve on the accessibility tree.
    [Arguments]    ${locator}
    ${node}=    BM.Query    ${locator}    only_first=${True}
    Should Be Equal    ${node}    ${None}    msg=node unexpectedly present: ${locator}

Launch Default Swing Instance
    [Documentation]    Suite Setup helper: check prerequisites, launch one fixture instance with a
    ...    suite-unique title, pin it by ProcessId, and expose ``${APP}`` (app root), ``${SWING_TITLE}``
    ...    (window title), ``${WINDOW}`` (app-relative window locator) at SUITE scope.
    [Arguments]    ${title}
    Require Swing Prerequisites
    ${handle}=    Launch Swing Test App    ${title}
    ${root}=    App Root    ${handle}
    ${window}=    Window Locator    ${title}
    VAR    ${APP}    ${root}    scope=SUITE
    VAR    ${WINDOW}    ${window}    scope=SUITE
    VAR    ${SWING_TITLE}    ${title}    scope=SUITE
    VAR    ${SWING_APP_HANDLE}    ${handle}    scope=SUITE

Terminate Default Swing Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Swing
    ...    Instance (ignoring errors for instances that already exited, e.g. after a close test).
    ...    ``kill=True`` because a JVM ignores the "graceful" CTRL_BREAK (it just dumps threads),
    ...    so the default path always burns the Process library's 30 s terminate timeout.
    Run Keyword And Ignore Error    Terminate Process    ${SWING_APP_HANDLE}    kill=${True}
