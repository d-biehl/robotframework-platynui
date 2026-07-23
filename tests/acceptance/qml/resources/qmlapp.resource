*** Settings ***
Documentation       Launch and tear down apps/test-app-qml instances from Robot Framework — the
...                 QML technology's launch configuration for the shared catalog suite. Each suite
...                 owns the instance(s) it needs; the top-level suite launches nothing.
...
...                 The interpreter and entrypoint are handed over via PLATYNUI_TEST_APP_QML_PYTHON
...                 / PLATYNUI_TEST_APP_QML_MAIN (prepared by scripts/platynui-robot-session.sh on
...                 Linux, the test-acceptance-windows recipe on Windows; set them yourself when
...                 launching outside those). Robot launches the interpreter DIRECTLY — never via
...                 ``uv run``, whose trampoline PID differs from the app's and would break the
...                 @ProcessId pinning.
...
...                 Window matching: a QQuickWindow's accessible name IS its title (verified UIA
...                 reality — QML ``Accessible`` cannot attach to a Window), so instances are pinned
...                 by ProcessId and their main window is matched via the launch title. That is the
...                 blueprint's launch-configuration fallback for window naming; all other catalog
...                 controls resolve by canonical @Name through the shared resource.

Library             Process
Library             PlatynUI.BareMetal    AS    BM

Resource            ../../resources/catalog.resource


*** Variables ***
# Honor PLATYNUI_TEST_APP_QML_PYTHON / _MAIN; default to the project venv + entrypoint. Paths must
# be ABSOLUTE and OS-native (a relative interpreter fails CreateProcess on Windows).
${QML_APP_PYTHON}    ${{ os.environ.get("PLATYNUI_TEST_APP_QML_PYTHON") or os.path.abspath(".venv/" + ("Scripts/python.exe" if os.name == "nt" else "bin/python")) }}
${QML_APP_MAIN}      ${{ os.environ.get("PLATYNUI_TEST_APP_QML_MAIN") or os.path.abspath("apps/test-app-qml/main.py") }}
# Windows only: the interpreter above is the BASE python; this redirects it into the venv (for
# PySide6) via __PYVENV_LAUNCHER__, sidestepping the uv trampoline's PID split. Empty on
# Linux/macOS (venv python is a symlink, no trampoline) — then it is not passed to the process.
${QML_APP_PYVENV_LAUNCHER}    %{PLATYNUI_TEST_APP_QML_PYVENV_LAUNCHER=}
${QML_WAIT_TIMEOUT}    15s
# Handle of the instance started by the suite-setup launchers below.
${QML_APP_HANDLE}    ${None}


*** Keywords ***
Launch QML Test App
    [Documentation]    Start one QML fixture instance (distinct app id + window title) by launching
    ...    its interpreter directly, and wait until its main window — pinned by the launched
    ...    ProcessId, matched by the launch title — is on the accessibility tree. Extra CLI args
    ...    (e.g. ``--open-modal``, ``--popup-mode native``) are forwarded. Returns the handle.
    [Arguments]    ${title}    ${app_id}    @{extra_args}
    # Robot only treats a LITERAL name=value as process config, so the env: argument is written out
    # per branch (see the Qt Widgets launcher for the same pattern).
    IF    $QML_APP_PYVENV_LAUNCHER
        ${handle}=    Start Process    ${QML_APP_PYTHON}    ${QML_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    env:__PYVENV_LAUNCHER__=${QML_APP_PYVENV_LAUNCHER}    stdout=${TEMPDIR}/qml-${app_id}.log    stderr=STDOUT
    ELSE
        ${handle}=    Start Process    ${QML_APP_PYTHON}    ${QML_APP_MAIN}    --app-id    ${app_id}    --title    ${title}
        ...    @{extra_args}    stdout=${TEMPDIR}/qml-${app_id}.log    stderr=STDOUT
    END
    ${root}=    QML App Root    ${handle}
    Wait Until Keyword Succeeds    ${QML_WAIT_TIMEOUT}    0.25s    QML Locator Is Present    ${root}//Window[@Name="${title}"]
    RETURN    ${handle}

QML App Root
    [Documentation]    App root locator for a launched instance, pinned by its ProcessId.
    [Arguments]    ${handle}
    ${pid}=    Get Process Id    ${handle}
    RETURN    /app:Application[@ProcessId=${pid}]

QML Locator Is Present
    [Documentation]    Assert a full locator resolves on the accessibility tree (predicate for
    ...    Wait Until Keyword Succeeds).
    [Arguments]    ${locator}
    ${node}=    BM.Query    ${locator}    only_first=${True}
    Should Not Be Equal    ${node}    ${None}    msg=not on the accessibility tree yet: ${locator}

Launch QML Catalog Instance
    [Documentation]    Suite Setup: launch the default catalog instance and expose its app root as
    ...    the SUITE variable ``${CATALOG_APP}`` (the anchor the shared catalog keywords use).
    [Arguments]    @{extra_args}
    ${handle}=    Launch QML Test App    PlatynUI QML TestApp    org.platynui.test.qml    @{extra_args}
    ${root}=    QML App Root    ${handle}
    VAR    ${CATALOG_APP}    ${root}    scope=SUITE
    VAR    ${QML_APP_HANDLE}    ${handle}    scope=SUITE

Launch QML Catalog Instance With Modal
    [Documentation]    Suite Setup for the modal suite: same as the catalog instance, plus
    ...    ``--open-modal`` so dialog-modal is present without interaction.
    ${handle}=    Launch QML Test App    PlatynUI QML TestApp (Modal)    org.platynui.test.qml.modal    --open-modal
    ${root}=    QML App Root    ${handle}
    VAR    ${CATALOG_APP}    ${root}    scope=SUITE
    VAR    ${QML_APP_HANDLE}    ${handle}    scope=SUITE

Launch QML Catalog Instance With Native Popups
    [Documentation]    Suite Setup for the popup-mode suite: menus/popups as native windows
    ...    (``--popup-mode native``, Qt >= 6.8 popupType).
    ${handle}=    Launch QML Test App    PlatynUI QML TestApp (Native Popups)    org.platynui.test.qml.native    --popup-mode    native
    ${root}=    QML App Root    ${handle}
    VAR    ${CATALOG_APP}    ${root}    scope=SUITE
    VAR    ${QML_APP_HANDLE}    ${handle}    scope=SUITE

Terminate QML Instance
    [Documentation]    Suite Teardown: terminate the instance started by the suite's launcher.
    Run Keyword And Ignore Error    Terminate Process    ${QML_APP_HANDLE}
