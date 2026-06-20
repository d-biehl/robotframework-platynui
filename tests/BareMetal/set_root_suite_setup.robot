*** Settings ***
Documentation     Root provided via Suite Setup (scope=SUITE): it is effective in every test,
...               relative selectors drill into it, and a Set Root inside a keyword stays local to
...               that keyword. Uses the mock tree (Operations Console window: 4 item:ListItem, its
...               Navigation tree: 7 item:TreeItem).
Library           PlatynUI.BareMetal    use_mock=${True}
Suite Setup       Set Root    //control:Window[@Name="Operations Console"]    scope=SUITE


*** Test Cases ***
Suite Setup Root Is Effective In Every Test
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

Relative Set Root Drills Into The Suite Setup Root
    Set Root    .
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

Set Root Inside A Keyword Is Local To That Keyword
    Work Inside The Navigation Tree
    ${after}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${after}    4


*** Keywords ***
Work Inside The Navigation Tree
    Set Root    .//control:Tree[@Name="Navigation"]
    ${tree_items}=    Query    count(.//item:TreeItem)    only_first=${True}
    Should Be Equal As Integers    ${tree_items}    7
