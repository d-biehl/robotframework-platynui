*** Settings ***
Documentation     Same registered name as the parent suite, but different import arguments — a
...               different session binding hiding behind the same variable name. The inherited root
...               belongs to a differently configured instance and must be ignored rather than
...               resolved through the wrong runtime, so relative selectors fall back to the desktop.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}    auto_activate=${False}


*** Test Cases ***
The Inherited Root Of A Differently Configured Import Is Ignored
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    8    msg=expected the desktop, not the parent's Operations Console root

Relative Selectors Fall Back To The Desktop
    ${n}=    Query    count(.//control:Window)    only_first=${True}
    Should Be Equal As Integers    ${n}    6

The Inherited Query Settings Of A Differently Configured Import Are Ignored
    [Documentation]    The parent set 0.5 s at suite scope; this import must fall back to its own
    ...    0.2 s default rather than adopt a differently configured import's settings.
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Get Attribute    //control:Button[@Name="NoSuchButton"]    Name
