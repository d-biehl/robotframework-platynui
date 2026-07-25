*** Settings ***
Documentation     Same registered name, same import arguments as the parent suite — this child must
...               inherit the parent's suite-scoped root and resolve it on its own runtime.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Test Cases ***
The Parent Suite Root Is Effective Here
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4    msg=expected the Operations Console root inherited from the parent

The Inherited Root Resolves On This Suite's Own Runtime
    [Documentation]    Every node carries the id of the runtime that produced it. A node reached
    ...    through the inherited root must come from the same runtime as one this suite resolves
    ...    directly — otherwise the root is being resolved on the parent instance's runtime.
    ${via_root}=    Query    .//control:Text    only_first=${True}
    ${direct}=    Query    //control:Window[@Name="Detail View"]    only_first=${True}
    Should Be Equal As Integers    ${{ $via_root.owner_id }}    ${{ $direct.owner_id }}

The Parent Suite Query Settings Are Effective Here
    [Documentation]    This suite imports a 0.2 s default; the inherited suite-scoped 0.5 s wins.
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*
    ...    Get Attribute    //control:Button[@Name="NoSuchButton"]    Name
