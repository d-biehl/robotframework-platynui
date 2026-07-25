*** Settings ***
Documentation     Same registered name as the parent suite, same import arguments — and still no
...               inherited context: the parent's ``scope=SUITE`` root and settings stop at the
...               parent suite. This child starts at the desktop with its own import defaults, and
...               pins whatever it needs itself.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Test Cases ***
The Parent Suite Root Does Not Reach This Suite
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    8
    ...    msg=expected the desktop (8), not the parent's Operations Console root (4)

The Parent Suite Query Settings Do Not Reach This Suite
    [Documentation]    This import's own 0.2 s default applies; the parent's suite-scoped 0.5 s does
    ...    not travel any further than its own suite.
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Get Attribute    //control:Button[@Name="NoSuchButton"]    Name

This Suite Pins Its Own Context
    [Documentation]    The boundary is not a restriction on the suite itself: a root set here works
    ...    for this suite's tests exactly as the parent's does for the parent's.
    Set Root    //control:Window[@Name="Operations Console"]    scope=SUITE
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
