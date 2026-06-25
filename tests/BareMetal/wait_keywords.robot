*** Settings ***
Documentation     Mock-backed checks for the explicit wait keywords — Wait Until Exists, Wait Until
...               Gone and Wait Until Query. The import sets a small 0.2 s default so the
...               timeout-driven checks stay fast; waits are observed through the timeout value in
...               the raised error, exactly like query_settings.robot. The captured-node "gone
...               success" direction is covered in the egui acceptance lane, since the mock never
...               invalidates a captured node.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${MISSING}        //control:Button[@Name="NoSuchButton"]
${OPS_NAME}       //control:Window[@Name="Operations Console"]/@Name
${OPS_MAX}        //control:Window[@Name="Operations Console"]/@IsMaximized
${MISSING_CNT}    count(//control:Button[@Name="NoSuchButton"])


*** Test Cases ***
# --- Wait Until Exists ---------------------------------------------------------

Wait Until Exists Returns The Element
    ${el}=    Wait Until Exists    ${OPS}
    Should Be Equal    ${el.name}    Operations Console
    Should Be Equal    ${el.role}    Window

Wait Until Exists Times Out With A User Facing Message
    Run Keyword And Expect Error    *No element matched*within timeout of 0.2 seconds*
    ...    Wait Until Exists    ${MISSING}

Wait Until Exists Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Exists    ${MISSING}    query_overrides={'timeout': 0.6}

Wait Until Exists Honors Scope Settings
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Wait Until Exists    ${MISSING}

Wait Until Exists Rejects A Non Element Selector
    Run Keyword And Expect Error    *did not return a UiNode*    Wait Until Exists    count(//control:Window)

Wait Until Exists Does Not Leak Overrides Across The Shared Cache
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Exists    ${MISSING}    query_overrides={'timeout': 0.2}
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Wait Until Exists    ${MISSING}

# --- Wait Until Gone -----------------------------------------------------------

Wait Until Gone Returns Fast When Already Absent
    Wait Until Gone    ${MISSING}

Wait Until Gone Times Out While The Selector Persists
    Run Keyword And Expect Error    *still present*within timeout of 0.2 seconds*    Wait Until Gone    ${OPS}

Wait Until Gone Times Out For A Still Valid Captured Node
    ${el}=    Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *still valid*within timeout of 0.2 seconds*    Wait Until Gone    ${el}

Wait Until Gone Ignores A Stale Cached Descriptor Node
    Get Attribute    ${OPS}    Name
    Run Keyword And Expect Error    *still present*within timeout of 0.2 seconds*    Wait Until Gone    ${OPS}

Wait Until Gone Rejects A Value Selector
    Run Keyword And Expect Error    *Use Wait Until Query for value conditions*
    ...    Wait Until Gone    count(//control:Window)

Wait Until Gone Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Gone    ${OPS}    query_overrides={'timeout': 0.6}

Wait Until Gone With Ignore Exceptions Never Reports Gone
    Run Keyword And Expect Error    *still present*within timeout of 0.2 seconds*
    ...    Wait Until Gone    //control:Window[broken    query_overrides={'ignore_exceptions': True}

# --- Wait Until Query ----------------------------------------------------------

Wait Until Query Default Passes On A Truthy Value
    ${n}=    Wait Until Query    count(//control:Window)
    Should Be True    ${n} > 0

Wait Until Query Default Times Out On A Falsy Value
    Run Keyword And Expect Error    *did not become truthy*within timeout of 0.2 seconds*
    ...    Wait Until Query    ${MISSING_CNT}

Wait Until Query Default Times Out On A Falsy Attribute
    [Documentation]    A fresh mock window is not maximized, so @IsMaximized is False. The default
    ...    must test the attribute's value (falsy) — not the always-true wrapper — and time out.
    Run Keyword And Expect Error    *did not become truthy*within timeout of 0.2 seconds*
    ...    Wait Until Query    ${OPS_MAX}

Wait Until Query Default Returns The Raw Result
    ${r}=    Wait Until Query    ${OPS_NAME}
    Should Be Equal As Strings    ${r}    Operations Console

Wait Until Query Passes With A Comparison Operator
    Wait Until Query    ${OPS_NAME}    ==    Operations Console

Wait Until Query Surfaces The Assertion Diagnostic On Timeout
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${OPS_NAME}    ==    Wrong Name

Wait Until Query Expected Without Operator Does Not Raise ValueError
    ${r}=    Wait Until Query    count(//control:Window)    ${None}    ${5}
    Should Be True    ${r} > 0

Wait Until Query Times Out On An Order Operator That Stays False
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    >    ${0}

Wait Until Query Rejects The Then Operator
    Run Keyword And Expect Error    *Use 'validate'*    Wait Until Query    count(//control:Window)    then    value > 0

Wait Until Query Polls With The Validate Operator
    Wait Until Query    count(//control:Window)    validate    value > 0
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    validate    value > 0

Wait Until Query Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    >    ${0}    query_overrides={'timeout': 0.6}

Wait Until Query Evaluates Against A Root Node
    ${win}=    Query    ${OPS}    only_first=${True}
    ${r}=    Wait Until Query    count(.//item:ListItem)    >    ${0}    root=${win}
    Should Be True    ${r} > 0

Wait Until Query Matches Get Attribute For A Present Attribute
    Wait Until Query    ${OPS_NAME}    ==    Operations Console
    Get Attribute    ${OPS}    Name    ==    Operations Console

Wait Until Query With Ignore Exceptions Times Out On A Bad Expression
    Run Keyword And Expect Error    *did not become truthy*within timeout of 0.2 seconds*
    ...    Wait Until Query    count(//control:Window[broken    query_overrides={'ignore_exceptions': True}
