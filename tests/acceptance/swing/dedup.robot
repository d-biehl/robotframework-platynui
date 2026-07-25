*** Settings ***
Documentation       Window-claims cooperation between the JAB and UIA providers. The default runtime
...                 (claims honored) shows each Java window exactly once — as the JAB node; a second
...                 runtime configured with ``providers.windows-uia.honor_window_claims=false`` (the
...                 kill switch) sees the UIA shell again alongside the JAB node, distinguishable via
...                 ``@Technology``.

Resource            resources/testapp.resource

Library             PlatynUI.BareMetal
...                 config={'providers': {'windows-uia': {'honor_window_claims': False}}}
...                 AS    BMOFF

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Dedup
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Claims Suppress The UIA Shell In The Default Runtime
    ${jab}=    BM.Query    /Window[@Name="${SWING_TITLE}" and @Technology="JAB"]
    Length Should Be    ${jab}    1
    ${uia}=    BM.Query    /Window[@Name="${SWING_TITLE}" and @Technology="UIAutomation"]
    Length Should Be    ${uia}    0    msg=the UIA shell must be suppressed while the JAB provider claims the window

Kill Switch Restores The UIA Shell Alongside The JAB Node
    [Documentation]    With claims ignored, both representations appear. The JAB node is identified via
    ...    ``@Technology = "JAB"``; the UIA shell is the second node without that marker (the UIA
    ...    provider does not emit a Technology attribute today). The first query may race the second
    ...    runtime's JAB rendezvous, hence the waiting count queries.
    BMOFF.Wait Until Query    count(/Window[@Name="${SWING_TITLE}"])    ==    ${2}
    ...    msg=expected the JAB node plus the UIA shell with claims ignored    query_overrides={'timeout': 10}
    BMOFF.Wait Until Query    count(/Window[@Name="${SWING_TITLE}" and @Technology="JAB"])    ==    ${1}
    ...    msg=exactly one of the two must be the JAB representation

Each Import Keeps Its Own Query Root
    [Documentation]    The suite setup pins BM's root to the launched instance; BMOFF is a separate
    ...    import with its own runtime and window-claims setting, so it must not inherit that root —
    ...    which is why every locator above spells itself out absolutely.
    Variable Should Exist    \${PLATYNUI_ROOT_DESCRIPTOR_BM}
    Variable Should Not Exist    \${PLATYNUI_ROOT_DESCRIPTOR_BMOFF}
