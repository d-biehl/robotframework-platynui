*** Settings ***
Documentation       A 32-bit process on 64-bit Windows reports its own architecture (spec
...                 *application-process-attributes*).
...
...                 The suite launches the Win32 test window, built for 32-bit x86, and reads the
...                 architecture of its application node. The answer must describe the process, not the
...                 64-bit machine the runtime runs on or the 64-bit build of the runtime.

Library             Process
Library             PlatynUI.BareMetal    AS    BM

Suite Setup         Launch Win32 Test Window
Suite Teardown      Run Keyword And Ignore Error    Terminate Process    ${HANDLE}    kill=${True}


*** Variables ***
${WINDOW_BIN}       ${{ os.environ.get("PLATYNUI_WIN32_TEST_WINDOW_X86", "") }}
${TITLE}            PlatynUI Win32 Process Attributes
${HANDLE}           ${None}


*** Test Cases ***
A 32-Bit Process Reports Its Own Architecture
    BM.Get Attribute Value    .    app:Architecture    ==    x86


*** Keywords ***
Launch Win32 Test Window
    [Documentation]    Check that the 32-bit window was built, start it, wait until UI Automation lists
    ...    its window, and pin its application node as the suite's root. The window is found without a
    ...    role: UI Automation may report a top-level STATIC window as ``Text`` rather than ``Window``.
    Should Be True    $WINDOW_BIN and os.path.isfile($WINDOW_BIN)
    ...    msg=the 32-bit Win32 test window is not built (PLATYNUI_WIN32_TEST_WINDOW_X86='${WINDOW_BIN}') — run `just build-win32-test-window-x86`, or the lane `just test-acceptance-windows`, which builds it
    ${handle}=    Start Process    ${WINDOW_BIN}    --title    ${TITLE}
    VAR    ${HANDLE}    ${handle}    scope=SUITE
    ${pid}=    Get Process Id    ${handle}
    BM.Wait Until Exists    /app:Application[@ProcessId=${pid}]/*[@Name="${TITLE}"]
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
