*** Settings ***
Documentation     Launch and tear down the PlatynUI Inspector (itself an AccessKit app) for the
...               inspector acceptance suites, which read and drive the Inspector's OWN a11y tree.
...
...               Every launch is hermetic: the Inspector must not load the user's real settings
...               file (a reconfigured picker activation combination or toolbar style would break
...               the suites), so each suite gets its own settings path under ``${OUTPUT DIR}``
...               via ``PLATYNUI_INSPECTOR_SETTINGS_PATH``. The launcher pins the Inspector window
...               by the launched ProcessId and exposes it as ``${INSP_WIN}``; the suites keep
...               absolute per-window prefixes (no suite-wide Set Root) because they always talk
...               to two windows at once — the Inspector and the inspected test app.
Library           OperatingSystem
Library           Process
Library           PlatynUI.BareMetal    AS    BM


*** Variables ***
${INSPECTOR_BIN}    ${{ os.environ.get("PLATYNUI_INSPECTOR_BIN") or os.path.abspath("target/debug/platynui-inspector-rs" + (".exe" if os.name == "nt" else "")) }}
# Handle + window root of the launched Inspector — set at SUITE scope by Launch Inspector.
${INSP_HANDLE}      ${None}
${INSP_WIN}         ${None}


*** Keywords ***
Launch Inspector
    [Documentation]    Start the Inspector with a hermetic per-suite settings file, pin its window
    ...    by the launched ProcessId as ``${INSP_WIN}`` (SUITE scope), and wait until the window is
    ...    on the accessibility tree.
    [Arguments]    ${settings_name}
    VAR    ${settings}    ${OUTPUT DIR}${/}${settings_name}
    Remove File    ${settings}
    ${insp}=    Start Process    ${INSPECTOR_BIN}
    ...    stdout=${TEMPDIR}/${settings_name}.log    stderr=STDOUT
    ...    env:RUST_LOG=platynui_inspector=trace,platynui=debug
    ...    env:PLATYNUI_INSPECTOR_SETTINGS_PATH=${settings}
    VAR    ${INSP_HANDLE}    ${insp}    scope=SUITE
    ${insp_pid}=    Get Process Id    ${insp}
    VAR    ${INSP_WIN}    /app:Application[@ProcessId=${insp_pid}]/(Frame|Window)    scope=SUITE
    BM.Wait Until Exists    ${INSP_WIN}    query_overrides={'timeout': 15}

Terminate Inspector
    [Documentation]    Best-effort teardown of the launched Inspector (tolerates a process that
    ...    already exited, e.g. after a close test).
    Run Keyword And Ignore Error    Terminate Process    ${INSP_HANDLE}

Lay Windows Out Side By Side
    [Documentation]    Place the given test-app window on the left and the Inspector to its right
    ...    so the Inspector never covers the app's widgets (a hit-test over the Inspector's own
    ...    window resolves nothing — it is meant to inspect *other* windows). The Inspector x is
    ...    derived from the app's actual bounds, so it adapts to whatever size the toolkit grants.
    [Arguments]    ${app_window}
    BM.Move And Resize Window    ${app_window}    20    20    640    480
    ${app}=    BM.Get Attribute    ${app_window}    Bounds
    VAR    ${insp_x}    ${{ int($app.x + $app.width + 20) }}
    BM.Move Window    ${INSP_WIN}    ${insp_x}    20
    Sleep    0.5s
