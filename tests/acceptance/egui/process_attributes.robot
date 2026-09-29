*** Settings ***
Documentation       The process attributes of an application node (spec
...                 *application-process-attributes*).
...
...                 The suite launches an instance of its own, pinned by its ProcessId, and reads the
...                 ``app:`` process attributes of its application node: each in its one format, each
...                 describing the launched process. The expected values come from the platform the
...                 suite runs on, so it runs unchanged on every real lane: on Windows the node comes
...                 from UI Automation, on the Linux lanes from AT-SPI, which reports no architecture.
...
...                 The window title holds spaces on purpose: on Windows the launch passes it as one
...                 quoted argument, and the command line keeps those quotes.

Resource            resources/testapp.resource

Suite Setup         Launch Process Attributes Instance
Suite Teardown      Run Keyword And Ignore Error    Terminate App    ${HANDLE}


*** Variables ***
${TITLE}            PlatynUI Process Attributes
${ON_WINDOWS}       ${{ os.name == "nt" }}
# Set by the suite setup: the launched instance, its pid, and the UTC time just before the launch.
${HANDLE}           ${None}
${PID}              ${None}
${LAUNCHED}         ${None}


*** Test Cases ***
The Process ID Selects Exactly One Application
    ${applications}=    BM.Query    /app:Application[@ProcessId=${PID}]
    Length Should Be    ${applications}    1

The Process Name Is The Program's File Name
    [Documentation]    Without its directory, and on Windows without ``.exe``. On Windows the node
    ...    is named after its program, so ``@Name`` carries the same value.
    VAR    ${file_name}    ${{ os.path.basename(os.path.realpath($TEST_APP_BIN)) }}
    IF    $ON_WINDOWS
        VAR    ${expected}    ${{ os.path.splitext($file_name)[0] }}
        BM.Get Attribute Value    .    app:ProcessName    ==    ${expected}
        BM.Get Attribute Value    .    Name    ==    ${expected}
    ELSE
        BM.Get Attribute Value    .    app:ProcessName    ==    ${file_name}
    END

The Executable Path Names The Launched Binary
    [Documentation]    Compared normalized: the lane may hand the path over with ``/`` where Windows
    ...    reports ``\\``, and Linux reports the path with its symlinks resolved.
    ${path}=    BM.Get Attribute Value    .    app:ExecutablePath
    Should Be Equal    ${{ os.path.normcase(os.path.realpath($path)) }}
    ...    ${{ os.path.normcase(os.path.realpath($TEST_APP_BIN)) }}

The Command Line Holds The Launch
    [Documentation]    On Windows the command line is verbatim, so the title the launch passed as one
    ...    argument keeps its quotes.
    ${command_line}=    BM.Get Attribute Value    .    app:CommandLine
    Should Contain    ${command_line}    ${{ os.path.basename($TEST_APP_BIN) }}
    IF    $ON_WINDOWS
        Should Contain    ${command_line}    "${TITLE}"
    ELSE
        Should Contain    ${command_line}    ${TITLE}
    END

The Start Time Is UTC To The Second
    ${start}=    BM.Get Attribute Value    .    app:StartTime
    Should Match Regexp    ${start}    ^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$
    VAR    ${started}    ${{ datetime.datetime.strptime($start, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc) }}
    Should Be True    abs(($started - $LAUNCHED).total_seconds()) < 60
    ...    msg=the start time ${start} is not within a minute of the launch at ${LAUNCHED}

The User Name Is The Current Account
    [Documentation]    ``DOMAIN\\user`` on Windows, where a local account's domain is the computer
    ...    name, which ``%USERDOMAIN%`` then holds as well; Windows account names ignore case. The
    ...    login name of the effective user on Linux.
    IF    $ON_WINDOWS
        ${user}=    BM.Get Attribute Value    .    app:UserName
        Should Be Equal    ${user}    %{USERDOMAIN}\\%{USERNAME}    ignore_case=${True}
    ELSE
        BM.Get Attribute Value    .    app:UserName    ==    ${{ pwd.getpwuid(os.geteuid()).pw_name }}
    END

The Architecture Is The Process's Own Or Absent
    [Documentation]    The lane's egui build is an x64 program on Windows. Linux keeps no
    ...    architecture per process, so the attribute is absent there, while the other five are present.
    IF    $ON_WINDOWS
        BM.Get Attribute Value    .    app:Architecture    ==    x64
    ELSE
        ${application}=    BM.Query    .[@app:Architecture]    only_first=${True}
        Should Be Equal    ${application}    ${None}    msg=a Linux application carries no architecture
        FOR    ${attribute}    IN    ProcessName    ExecutablePath    CommandLine    UserName    StartTime
            ${application}=    BM.Query    .[@app:${attribute}]    only_first=${True}
            Should Not Be Equal    ${application}    ${None}    msg=app:${attribute} is missing
        END
    END

Get Attribute Value Reads The App Namespace
    ${attribute}=    BM.Query    ./@app:ProcessName    only_first=${True}
    Should Not Be Equal    ${attribute}    ${None}    msg=the XPath read found no app:ProcessName
    BM.Get Attribute Value    .    app:ProcessName    ==    ${attribute.value}


*** Keywords ***
Launch Process Attributes Instance
    [Documentation]    Launch an instance of its own and pin its application node as the suite's root.
    ...    ``Launch Default Instance`` pins the window instead, and records no launch time.
    VAR    ${LAUNCHED}    ${{ datetime.datetime.now(datetime.timezone.utc) }}    scope=SUITE
    ${handle}=    Launch Test App    ${TITLE}    com.platynui.test.processattributes
    VAR    ${HANDLE}    ${handle}    scope=SUITE
    ${pid}=    Get Process Id    ${handle}
    VAR    ${PID}    ${pid}    scope=SUITE
    BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE
