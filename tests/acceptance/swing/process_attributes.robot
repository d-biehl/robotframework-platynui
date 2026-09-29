*** Settings ***
Documentation       The process attributes of a Java application's node, from the in-JVM agent and
...                 from the Access Bridge (spec *application-process-attributes*).
...
...                 Process attributes describe the process as the platform knows it, whichever
...                 provider serves the application: the launcher that was started, its verbatim command
...                 line and the account it runs as, never what the JVM says about itself. The fixture is
...                 launched through ``javaw.exe``, under a title with spaces.
...
...                 Two imports read the same instance. ``BM``, from ``testapp_agent.resource``, leaves
...                 the agent on; ``BMJAB`` switches it off, so that the Access Bridge serves the
...                 instance. Each import pins the instance's application node of its own backend as its
...                 root, named by ``@Technology``.

Resource            resources/testapp_agent.resource

Library             PlatynUI.BareMetal
...                 config={'providers': {'java': {'agent': {'enabled': False}}}}
...                 AS    BMJAB

Suite Setup         Launch Process Attributes Instance
Suite Teardown      Run Keyword And Ignore Error    Terminate Process    ${HANDLE}    kill=${True}


*** Variables ***
${TITLE}                    Swing Process Attributes
@{PROCESS_ATTRIBUTES}       ProcessName    ExecutablePath    CommandLine    UserName    StartTime    Architecture
${START_TIME_FORMAT}        ^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$
# Set by the suite setup: the launcher and the launched instance.
${JAVAW}                    ${None}
${HANDLE}                   ${None}


*** Test Cases ***
The Agent-Served Node Reports The JVM Process Under App
    [Documentation]    The process is ``javaw``, not the main class, and its path is the launcher that
    ...    was started, not one derived from ``java.home``. Nothing of it stays under ``control``.
    BM.Get Attribute Value    .    app:ProcessName    ==    javaw
    ${path}=    BM.Get Attribute Value    .    app:ExecutablePath
    Should Be Equal    ${{ os.path.normcase(os.path.realpath($path)) }}
    ...    ${{ os.path.normcase(os.path.realpath($JAVAW)) }}
    ${start}=    BM.Get Attribute Value    .    app:StartTime
    Should Match Regexp    ${start}    ${START_TIME_FORMAT}
    ${user}=    BM.Get Attribute Value    .    app:UserName
    Should Be Equal    ${user}    %{USERDOMAIN}\\%{USERNAME}    ignore_case=${True}
    BM.Get Attribute Value    .    app:Architecture    ==    x64
    ${application}=    BM.Query    .[@ProcessName]    only_first=${True}
    Should Be Equal    ${application}    ${None}    msg=the agent-served node still reports ProcessName under control

A Self-Described User Name Does Not Replace The Account
    [Documentation]    A second instance tells its JVM that it runs as someone else. Its node still
    ...    names the account the process runs as. The JVM's startup line in the log proves that the
    ...    override was in effect.
    VAR    ${title}    ${TITLE} Other User
    ${handle}=    Launch Swing Agent Test App    ${title}    java=${JAVAW}
    ...    env:JAVA_TOOL_OPTIONS=-Duser.name=someone-else
    ${log}=    Get File    ${TEMPDIR}/swing-${title}.log
    Should Contain    ${log}    Picked up JAVA_TOOL_OPTIONS: -Duser.name=someone-else
    ${pid}=    Get Process Id    ${handle}
    ${user}=    BM.Get Attribute Value    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]    app:UserName
    Should Be Equal    ${user}    %{USERDOMAIN}\\%{USERNAME}    ignore_case=${True}
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}

The Access Bridge Node Reports The Process In The Platform's Form
    [Documentation]    The account with its domain, the verbatim command line with the title's quotes,
    ...    and a node named after its process.
    ${user}=    BMJAB.Get Attribute Value    .    app:UserName
    Should Be Equal    ${user}    %{USERDOMAIN}\\%{USERNAME}    ignore_case=${True}
    ${command_line}=    BMJAB.Get Attribute Value    .    app:CommandLine
    Should Contain    ${command_line}    "${TITLE}"
    ${start}=    BMJAB.Get Attribute Value    .    app:StartTime
    Should Match Regexp    ${start}    ${START_TIME_FORMAT}
    ${process_name}=    BMJAB.Get Attribute Value    .    app:ProcessName
    BMJAB.Get Attribute Value    .    Name    ==    ${process_name}

Both Providers Report The Same Process Identically
    VAR    @{compared}
    FOR    ${attribute}    IN    @{PROCESS_ATTRIBUTES}
        ${agent}=    BM.Query    ./@app:${attribute}    only_first=${True}
        ${jab}=    BMJAB.Query    ./@app:${attribute}    only_first=${True}
        IF    $agent is not None and $jab is not None
            Should Be Equal    ${agent.value}    ${jab.value}
            ...    msg=app:${attribute} differs between the agent and the Access Bridge
            VAR    @{compared}    @{compared}    ${attribute}
        END
    END
    Should Not Be Empty    ${compared}    msg=no process attribute is present on both nodes


*** Keywords ***
Launch Process Attributes Instance
    [Documentation]    Launch the instance through ``javaw.exe`` next to the suite's launcher, wait until
    ...    both backends serve it, and pin each backend's application node as its import's root.
    Require Swing Prerequisites
    VAR    ${javaw}    ${{ shutil.which("javaw", path=os.path.dirname($SWING_JAVA) or None) }}
    Should Not Be Equal    ${javaw}    ${None}    msg=no javaw.exe next to the launcher ${SWING_JAVA}
    VAR    ${JAVAW}    ${javaw}    scope=SUITE
    ${handle}=    Launch Swing Agent Test App    ${TITLE}    java=${javaw}
    VAR    ${HANDLE}    ${handle}    scope=SUITE
    ${pid}=    Get Process Id    ${handle}
    BM.Set Root    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]    scope=SUITE
    BMJAB.Wait Until Exists    /app:Application[@ProcessId=${pid}][@Technology="JAB"]//Window[@Name="${TITLE}"]
    ...    query_overrides={'timeout': ${APP_WAIT_TIMEOUT}}
    BMJAB.Set Root    /app:Application[@ProcessId=${pid}][@Technology="JAB"]    scope=SUITE
