*** Settings ***
Documentation       Launch and tear down apps/test-app-qt instances from Robot Framework, so each
...                 suite owns the instance(s) it needs and starts from a known state — the top-level
...                 suite launches nothing.
...
...                 The app is a standalone uv project. Its interpreter and entrypoint are handed over
...                 via PLATYNUI_TEST_APP_QT_PYTHON / PLATYNUI_TEST_APP_QT_MAIN (prepared by
...                 scripts/platynui-robot-session.sh on Linux, the test-acceptance-windows recipe on
...                 Windows; set them yourself when launching outside those, e.g. the repl). Robot
...                 launches the interpreter DIRECTLY — never via ``uv run``, whose trampoline PID
...                 differs from the app's and would break the @ProcessId pinning.
...
...                 The launcher pins the instance as the query root (``Set Root`` on
...                 ``/app:Application[@ProcessId=...]``), so every locator in the suites is relative
...                 (``.//``) and addresses widgets by their stable accessible name — every widget
...                 carries one via ``setAccessibleName`` (see ``_tag`` in main.py), surfaced as
...                 ``@Name`` on AT-SPI and UIA alike. Window-level locators keep a role union
...                 (AT-SPI: Frame/Dialog, UIA: Window) so they resolve the window node itself.

Library             Process
Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
# Honor PLATYNUI_TEST_APP_QT_PYTHON / _MAIN; otherwise default to the app's own venv + entrypoint.
# Paths must be ABSOLUTE and OS-native (a relative interpreter fails CreateProcess on Windows);
# os.path.abspath resolves against the project root (robotcode's CWD).
${QT_APP_PYTHON}    ${{ os.environ.get("PLATYNUI_TEST_APP_QT_PYTHON") or os.path.abspath("apps/test-app-qt/.venv/" + ("Scripts/python.exe" if os.name == "nt" else "bin/python")) }}
${QT_APP_MAIN}      ${{ os.environ.get("PLATYNUI_TEST_APP_QT_MAIN") or os.path.abspath("apps/test-app-qt/main.py") }}
# Windows only: the interpreter above is the BASE python; this redirects it into the venv (for
# PySide6) via __PYVENV_LAUNCHER__, sidestepping the uv trampoline's PID split (see the
# test-acceptance-windows recipe). Empty on Linux/macOS (venv python is a symlink, no trampoline) —
# then it is not passed to the process.
${QT_APP_PYVENV_LAUNCHER}    %{PLATYNUI_TEST_APP_QT_PYVENV_LAUNCHER=}
${QT_WAIT_TIMEOUT}    15
# Handle of the instance started by Launch Qt Test App (for the teardown).
${QT_APP_HANDLE}    ${None}


*** Keywords ***
Launch Qt Test App
    [Documentation]    Start one Qt instance (distinct app id + window title) by launching its
    ...    interpreter directly, pin it as the query root (by the launched ProcessId), and wait
    ...    until its main window is on the accessibility tree. Extra CLI args (e.g. ``--dialogs 1``,
    ...    ``--open-modal``) are forwarded. ``scope`` is the Set Root / handle lifetime — SUITE for
    ...    a Suite Setup, TEST for per-test instances.
    [Arguments]    ${title}=PlatynUI Test App (Qt)    ${app_id}=org.platynui.test.qt    @{extra_args}
    ...    ${scope}=SUITE
    # Robot only treats a LITERAL name=value as process config, so the env: argument is written out
    # per branch.
    IF    $QT_APP_PYVENV_LAUNCHER
        ${handle}=    Start Process    ${QT_APP_PYTHON}    ${QT_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    env:__PYVENV_LAUNCHER__=${QT_APP_PYVENV_LAUNCHER}    stdout=${TEMPDIR}/qt-${app_id}.log    stderr=STDOUT
    ELSE
        ${handle}=    Start Process    ${QT_APP_PYTHON}    ${QT_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    stdout=${TEMPDIR}/qt-${app_id}.log    stderr=STDOUT
    END
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=${scope}
    BM.Wait Until Exists    .//(Frame|Window)[@Name="main-window"]    query_overrides={'timeout': ${QT_WAIT_TIMEOUT}}
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=${scope}

Terminate Qt Instance
    [Documentation]    Teardown: terminate the instance started by Launch Qt Test App (ignoring
    ...    errors for instances that already exited).
    Run Keyword And Ignore Error    Terminate Process    ${QT_APP_HANDLE}
