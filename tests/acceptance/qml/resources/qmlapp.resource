*** Settings ***
Documentation       Launch and tear down apps/test-app-qml instances from Robot Framework — the
...                 QML technology's launch configuration for the shared catalog suite. Each suite
...                 launches the instance it needs in its Suite Setup; the top-level suite
...                 launches nothing.
...
...                 The interpreter and entrypoint are handed over via PLATYNUI_TEST_APP_QML_PYTHON
...                 / PLATYNUI_TEST_APP_QML_MAIN (prepared by scripts/platynui-robot-session.sh on
...                 Linux, the test-acceptance-windows recipe on Windows; set them yourself when
...                 launching outside those). Robot launches the interpreter DIRECTLY — never via
...                 ``uv run``, whose trampoline PID differs from the app's and would break the
...                 @ProcessId pinning.
...
...                 The launcher pins the instance as the suite-wide query root (``Set Root``,
...                 SUITE scope), so every locator in the suites and the shared catalog resource
...                 is relative (``.//``). Window matching: a QQuickWindow's accessible name IS
...                 its title (verified reality on UIA and AT-SPI — QML ``Accessible`` cannot
...                 attach to a Window), so the main window is matched via the launch title —
...                 the blueprint's launch-configuration fallback for window naming.

Library             Process
Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
# Honor PLATYNUI_TEST_APP_QML_PYTHON / _MAIN; default to the project venv + entrypoint. Paths must
# be ABSOLUTE and OS-native (a relative interpreter fails CreateProcess on Windows).
${QML_APP_PYTHON}    ${{ os.environ.get("PLATYNUI_TEST_APP_QML_PYTHON") or os.path.abspath(".venv/" + ("Scripts/python.exe" if os.name == "nt" else "bin/python")) }}
${QML_APP_MAIN}      ${{ os.environ.get("PLATYNUI_TEST_APP_QML_MAIN") or os.path.abspath("apps/test-app-qml/main.py") }}
# Windows only: the interpreter above is the BASE python; this redirects it into the venv (for
# PySide6) via __PYVENV_LAUNCHER__, sidestepping the uv trampoline's PID split. Empty on
# Linux/macOS (venv python is a symlink, no trampoline) — then it is not passed to the process.
${QML_APP_PYVENV_LAUNCHER}    %{PLATYNUI_TEST_APP_QML_PYVENV_LAUNCHER=}
${QML_WAIT_TIMEOUT}    15
# Handle of the instance started by Launch QML Test App (for the teardown).
${QML_APP_HANDLE}    ${None}


*** Keywords ***
Launch QML Test App
    [Documentation]    Suite Setup: start one QML fixture instance by launching its interpreter
    ...    directly, pin it as the suite-wide query root (by the launched ProcessId), and wait
    ...    until its main window — matched by the launch title, with the cross-bridge role union
    ...    (Window on UIA, Frame on AT-SPI) — is on the accessibility tree. Extra CLI args
    ...    (e.g. ``--open-modal``, ``--popup-mode native``) are forwarded.
    [Arguments]    ${title}=PlatynUI QML TestApp    ${app_id}=org.platynui.test.qml    @{extra_args}
    # Robot only treats a LITERAL name=value as process config, so the env: argument is written out
    # per branch (see the Qt Widgets launcher for the same pattern).
    IF    $QML_APP_PYVENV_LAUNCHER
        ${handle}=    Start Process    ${QML_APP_PYTHON}    ${QML_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    env:__PYVENV_LAUNCHER__=${QML_APP_PYVENV_LAUNCHER}    stdout=${TEMPDIR}/qml-${app_id}.log    stderr=STDOUT
    ELSE
        ${handle}=    Start Process    ${QML_APP_PYTHON}    ${QML_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    stdout=${TEMPDIR}/qml-${app_id}.log    stderr=STDOUT
    END
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
    BM.Wait Until Exists    .//(Frame|Window)[@Name="${title}"]    query_overrides={'timeout': ${QML_WAIT_TIMEOUT}}
    VAR    ${QML_APP_HANDLE}    ${handle}    scope=SUITE

Terminate QML Instance
    [Documentation]    Suite Teardown: terminate the instance started by Launch QML Test App.
    Run Keyword And Ignore Error    Terminate Process    ${QML_APP_HANDLE}
