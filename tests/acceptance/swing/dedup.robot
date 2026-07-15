*** Settings ***
Documentation       Window-claims cooperation between the JAB and UIA providers. The default runtime
...                 (claims honored) shows each Java window exactly once — as the JAB node; a second
...                 runtime configured with ``providers.windows-uia.honor_window_claims=false`` (the
...                 kill switch) sees the UIA shell again alongside the JAB node, distinguishable via
...                 ``@Technology``.

Library             PlatynUI.BareMetal    AS    BM
Library             PlatynUI.BareMetal
...                 config={'providers': {'windows-uia': {'honor_window_claims': False}}}
...                 AS    BMOFF
Resource            resources/testapp.resource

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
    ...    runtime's JAB rendezvous, hence the wait.
    Wait Until Keyword Succeeds    10s    0.5s    Both Representations Are Visible


*** Keywords ***
Both Representations Are Visible
    ${all}=    BMOFF.Query    /Window[@Name="${SWING_TITLE}"]
    Length Should Be    ${all}    2    msg=expected the JAB node plus the UIA shell with claims ignored
    ${jab}=    BMOFF.Query    /Window[@Name="${SWING_TITLE}" and @Technology="JAB"]
    Length Should Be    ${jab}    1    msg=exactly one of the two must be the JAB representation
