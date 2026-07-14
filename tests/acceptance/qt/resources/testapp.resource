*** Settings ***
Documentation     Launch and tear down platynui-test-app-qt instances from Robot Framework, so each
...               suite owns the instance(s) it needs and starts from a known state — the top-level
...               suite launches nothing.
...
...               The app is a standalone uv project. Its interpreter and entrypoint are handed over via
...               PLATYNUI_TEST_APP_QT_PYTHON / PLATYNUI_TEST_APP_QT_MAIN (prepared by
...               scripts/platynui-robot-session.sh on Linux, the test-acceptance-windows recipe on
...               Windows; set them yourself when launching outside those, e.g. the repl). Robot launches
...               the interpreter DIRECTLY — never via ``uv run``, which spawns Python as a child whose
...               PID differs from uv's and would break the @ProcessId pinning below.
...
...               Each instance is pinned by its launched ProcessId (``${APP}`` =
...               ``/app:Application[@ProcessId=...]``), so its window locators address exactly that copy
...               of the program regardless of window title or stacking. Re-exports the page-object
...               locators.
Library           Process
Library           PlatynUI.BareMetal    AS    BM

Resource          ${CURDIR}/testapp_locators.resource


*** Variables ***
# Honor PLATYNUI_TEST_APP_QT_PYTHON / _MAIN; otherwise default to the app's own venv + entrypoint. The path
# must be ABSOLUTE and OS-native — a relative interpreter fails CreateProcess with FileNotFoundError on
# Windows. os.path.abspath resolves against the project root (robotcode's CWD).
${QT_APP_PYTHON}       ${{ os.environ.get("PLATYNUI_TEST_APP_QT_PYTHON") or os.path.abspath("apps/test-app-qt/.venv/" + ("Scripts/python.exe" if os.name == "nt" else "bin/python")) }}
${QT_APP_MAIN}         ${{ os.environ.get("PLATYNUI_TEST_APP_QT_MAIN") or os.path.abspath("apps/test-app-qt/main.py") }}
# Windows only: the interpreter above is the BASE python; this redirects it into the venv (for PySide6) via
# __PYVENV_LAUNCHER__ so we sidestep the uv trampoline's PID split (see the test-acceptance-windows recipe).
# Empty on Linux/macOS (venv python is a symlink, no trampoline) — then it is not passed to the process.
${QT_APP_PYVENV_LAUNCHER}    %{PLATYNUI_TEST_APP_QT_PYVENV_LAUNCHER=}
${APP_WAIT_TIMEOUT}    15s
# Handle of the instance started by Launch Default Qt Instance — read by Terminate Default Qt Instance.
${QT_APP_HANDLE}       ${None}


*** Keywords ***
Launch Qt Test App
    [Documentation]    Start one Qt instance (distinct app id + window title) by launching its
    ...    interpreter directly, and wait until its main window — pinned by the launched ProcessId — is
    ...    on the accessibility tree. Extra CLI args (e.g. ``--open-modal``) are forwarded to the app.
    ...    Returns the Process handle; get its app root with ``App Root``.
    [Arguments]    ${title}    ${app_id}    @{extra_args}
    # On Windows ${QT_APP_PYTHON} is the BASE interpreter; __PYVENV_LAUNCHER__ redirects it into the venv so
    # PySide6 resolves — this sidesteps the uv trampoline's PID split (see the test-acceptance-windows recipe).
    # Empty on Linux/macOS (the venv python is a symlink, no trampoline) — then it must NOT be passed. Robot
    # only treats a LITERAL name=value as process config (not one built from a list/variable), so the env:
    # argument is written out directly per branch.
    IF    $QT_APP_PYVENV_LAUNCHER
        ${handle}=    Start Process    ${QT_APP_PYTHON}    ${QT_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    env:__PYVENV_LAUNCHER__=${QT_APP_PYVENV_LAUNCHER}    stdout=${TEMPDIR}/qt-${app_id}.log    stderr=STDOUT
    ELSE
        ${handle}=    Start Process    ${QT_APP_PYTHON}    ${QT_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    stdout=${TEMPDIR}/qt-${app_id}.log    stderr=STDOUT
    END
    ${root}=    App Root    ${handle}
    Wait Until Keyword Succeeds    ${APP_WAIT_TIMEOUT}    0.25s    Node Is Present    ${root}${MAIN_WINDOW}
    RETURN    ${handle}

App Root
    [Documentation]    App root locator for a launched instance, pinned by its ProcessId:
    ...    ``/app:Application[@ProcessId=<pid>]``. Prepend it to the window locators from the page object.
    [Arguments]    ${handle}
    ${pid}=    Get Process Id    ${handle}
    RETURN    /app:Application[@ProcessId=${pid}]

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

Get Bounds
    [Documentation]    Read a node's Bounds attribute (has .x/.y/.width/.height).
    [Arguments]    ${locator}
    ${bounds}=    BM.Get Attribute    ${locator}    Bounds
    RETURN    ${bounds}

Launch Default Qt Instance
    [Documentation]    Suite Setup helper: launch the default Qt instance, pin it by ProcessId, expose
    ...    its app root as the suite variable ``${APP}``, remember the handle, and wait for the child
    ...    dialogs (opened right after the main window) to appear.
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt)    org.platynui.test.qt
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=SUITE
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=SUITE
    FOR    ${dialog}    IN    ${DIALOG_1}    ${DIALOG_2}    ${DIALOG_3}
        Wait Until Keyword Succeeds    ${APP_WAIT_TIMEOUT}    0.25s    Node Is Present    ${APP}${dialog}
    END

Launch Single Dialog Qt Instance
    [Documentation]    Suite Setup helper for the interaction suite: launch an instance with exactly one
    ...    child dialog (``--dialogs 1``), pin it by ProcessId as ``${APP}``, and wait for the dialog.
    ...    A single dialog avoids sibling occlusion, so a pointer click reliably reaches its widgets on
    ...    every backend (on Wayland all windows stack at the origin).
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt Single)    org.platynui.test.qt.single    --dialogs    1
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=SUITE
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=SUITE
    Wait Until Keyword Succeeds    ${APP_WAIT_TIMEOUT}    0.25s    Node Is Present    ${APP}${DIALOG_1}

Launch Qt Instance With Modal
    [Documentation]    Suite Setup helper for the modal-bounds suite: launch an instance that also opens
    ...    a modal dialog at startup (``--open-modal``), pin it by ProcessId as ``${APP}``, and wait for
    ...    the modal dialog to appear. A dedicated instance keeps the modal's input grab out of the
    ...    other suites.
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt Modal)    org.platynui.test.qt.modal    --open-modal
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=SUITE
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=SUITE
    Wait Until Keyword Succeeds    ${APP_WAIT_TIMEOUT}    0.25s    Node Is Present    ${APP}${DIALOG_MODAL}

Terminate Default Qt Instance
    [Documentation]    Suite Teardown helper: terminate the instance started by Launch Default Qt Instance.
    Run Keyword And Ignore Error    Terminate Process    ${QT_APP_HANDLE}
