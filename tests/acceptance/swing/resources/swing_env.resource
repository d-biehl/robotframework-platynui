*** Settings ***
Documentation     Where the Swing fixture lives and what it needs to run — shared by the
...               bridge-facing and the agent-facing suites, because none of it depends on which
...               backend a suite wants.
...
...               The ``PlatynUI.BareMetal`` import is deliberately NOT here. It is the one thing
...               the two lanes disagree about — the in-JVM agent switched off for the suites that
...               are *about* the Access Bridge, on for the suites that are about the agent — and a
...               resource importing one configuration would force it on the other.
Library           OperatingSystem
Library           Process


*** Variables ***
${SWING_JAVA}           ${{ os.environ.get("PLATYNUI_TEST_APP_SWING_JAVA") or "java" }}
${SWING_CLASSES}        ${{ os.environ.get("PLATYNUI_TEST_APP_SWING_CLASSES") or os.path.abspath("apps/test-app-swing/build/classes/java/main") }}
${SWING_ENABLE_FLAG}    -Djavax.accessibility.assistive_technologies\=com.sun.java.accessibility.AccessBridge
${APP_WAIT_TIMEOUT}     20


*** Keywords ***
Require Swing Prerequisites
    [Documentation]    Fail the suite (with an actionable message) when the fixture cannot be launched:
    ...    no usable java launcher, or the fixture classes have not been built. These are hard
    ...    failures, not skips — the `real-windows` lane profile selected this suite, so a missing
    ...    fixture is a defect of the lane setup (`just test-acceptance-windows` builds it as a hard
    ...    prerequisite). Platform scoping happens via the suite's `platform:windows` tag, not here.
    ${java_ok}=    Run Keyword And Return Status    Run Process    ${SWING_JAVA}    -version
    Should Be True    ${java_ok}
    ...    msg=no usable Java launcher (${SWING_JAVA}) — run `just build-test-app-swing` (sets up the provisioned runtime) or put a `java` 8+ on PATH
    Should Be True    ${{ os.path.isdir($SWING_CLASSES) }}
    ...    msg=Swing fixture not built at ${SWING_CLASSES} — run `just build-test-app-swing`

Start Swing Fixture Process
    [Documentation]    Start one fixture instance with a unique window title and return the Process
    ...    handle. Waiting for its window is the caller's job, because what it waits *for* differs
    ...    per backend (``@Technology`` is "JAB" or "JavaAgent").
    [Arguments]    ${title}    @{extra_args}
    ${handle}=    Start Process    ${SWING_JAVA}    ${SWING_ENABLE_FLAG}    -cp    ${SWING_CLASSES}
    ...    platynui.testapp.Main    --title    ${title}    @{extra_args}
    ...    stdout=${TEMPDIR}/swing-${title}.log    stderr=STDOUT
    RETURN    ${handle}
