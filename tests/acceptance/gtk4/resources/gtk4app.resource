*** Settings ***
Documentation       Launch and tear down apps/test-app-gtk4 instances from Robot Framework — the
...                 GTK 4 technology's launch configuration for its catalog suites. Each suite
...                 launches the instance it needs in its Suite Setup; the top-level suite
...                 launches nothing.
...
...                 The interpreter and entrypoint are handed over via PLATYNUI_TEST_APP_GTK4_PYTHON
...                 / PLATYNUI_TEST_APP_GTK4_MAIN (prepared by scripts/platynui-robot-session.sh; set
...                 them yourself when launching outside a lane session). PyGObject is a Linux dev
...                 dependency of the project venv. Robot launches the interpreter DIRECTLY — never
...                 via ``uv run``, whose PID differs from the app's and would break the
...                 @ProcessId pinning.
...
...                 The launcher pins the instance as the suite-wide query root (``Set Root``, SUITE
...                 scope), so every locator in the suites is relative (``.//``). Window matching:
...                 a GTK 4 window's accessible name is its title, so the main window is matched by
...                 the launch title — the blueprint's launch-configuration fallback for window
...                 naming — and dialogs carry their canonical names as titles.

Library             OperatingSystem
Library             Process
Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
# Honor PLATYNUI_TEST_APP_GTK4_PYTHON / _MAIN; default to the project venv + entrypoint.
${GTK4_APP_PYTHON}      ${{ os.environ.get("PLATYNUI_TEST_APP_GTK4_PYTHON") or os.path.abspath(".venv/bin/python") }}
${GTK4_APP_MAIN}        ${{ os.environ.get("PLATYNUI_TEST_APP_GTK4_MAIN") or os.path.abspath("apps/test-app-gtk4/main.py") }}
${GTK4_WAIT_TIMEOUT}    15
# Handle of the instance started by Launch GTK4 Test App (for the teardown).
${GTK4_APP_HANDLE}      ${None}


*** Keywords ***
Launch GTK4 Test App
    [Documentation]    Suite Setup: start one GTK 4 fixture instance by launching its interpreter
    ...    directly, pin it as the suite-wide query root (by the launched ProcessId), and wait
    ...    until its main window — matched by the launch title — is on the accessibility tree.
    ...    A fixture that exits first (PyGObject or a GTK typelib missing, a crash) fails the
    ...    suite with its own output, which names what is missing. Extra CLI args (e.g.
    ...    ``--open-modal``) are forwarded.
    [Arguments]    ${title}=PlatynUI GTK4 TestApp    ${app_id}=org.platynui.test.gtk4    @{extra_args}
    VAR    ${log}    ${TEMPDIR}/gtk4-${app_id}.log
    ${handle}=    Start Process    ${GTK4_APP_PYTHON}    ${GTK4_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
    ...    @{extra_args}    stdout=${log}    stderr=STDOUT
    VAR    ${GTK4_APP_HANDLE}    ${handle}    scope=SUITE
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
    TRY
        BM.Wait Until Exists    .//(Frame|Window)[@Name="${title}"]    query_overrides={'timeout': ${GTK4_WAIT_TIMEOUT}}
    EXCEPT    AS    ${error}
        ${running}=    Is Process Running    ${handle}
        IF    not ${running}
            ${output}=    Get File    ${log}
            Fail    The GTK 4 test app exited before its window appeared:\n${output}
        END
        Fail    ${error}
    END

Terminate GTK4 Instance
    [Documentation]    Suite Teardown: terminate the instance started by Launch GTK4 Test App.
    Run Keyword And Ignore Error    Terminate Process    ${GTK4_APP_HANDLE}

Require GTK Version For Tagged Tests
    [Documentation]    Test Setup: a test tagged ``requires:gtk-<version>`` fails here when the
    ...    fixture runs on an older GTK 4, naming both versions and the tag to exclude. It never
    ...    skips — the installed GTK differs between machines on the same lane (see
    ...    dev-docs/testing-strategy.md §2.6).
    FOR    ${tag}    IN    @{TEST TAGS}
        IF    $tag.startswith("requires:gtk-")
            Require GTK Version    ${{ $tag.removeprefix("requires:gtk-") }}
        END
    END

Require GTK Version
    [Documentation]    Fail unless the fixture's GTK is at least ``required`` (``major.minor``).
    ...    ``installed`` defaults to the version the fixture's application node reports.
    [Arguments]    ${required}    ${installed}=${None}
    IF    $installed is None
        ${installed}=    BM.Get Attribute Value    .    native:Application.Version
    END
    IF    ${{ tuple(int(p) for p in $installed.split(".")[:2]) < tuple(int(p) for p in $required.split(".")[:2]) }}
        Fail    GTK ${installed} is older than the required GTK ${required}. Run with --exclude requires:gtk-${required} on this machine.
    END
