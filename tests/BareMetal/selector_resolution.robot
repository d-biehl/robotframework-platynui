*** Settings ***
Documentation     Mock-backed checks that a selector is resolved anew for every keyword rather than
...               reusing whatever an earlier call resolved. The mock tree makes the difference
...               visible: the Operations Console and the Detail View each hold exactly one
...               control:Text, named Status and Description respectively, so the same relative
...               selector must yield a different element under each root. A single unaliased import,
...               which also pins the documented variable names.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${DETAILS}        //control:Window[@Name="Detail View"]
${THE_TEXT}       .//control:Text


*** Test Cases ***
The Same Selector Follows A Changed Root
    Set Root    ${OPS}
    ${first}=    Get Attribute    ${THE_TEXT}    Name
    Should Be Equal    ${first}    Status
    Set Root    ${DETAILS}
    ${second}=    Get Attribute    ${THE_TEXT}    Name
    Should Be Equal    ${second}    Description    msg=the selector must re-resolve under the new root

The Same Selector Follows A Root That Narrows
    Set Root    ${OPS}
    ${outer}=    Get Attribute    ${THE_TEXT}    Name
    Should Be Equal    ${outer}    Status
    Set Root    .//control:Tree[@Name="Navigation"]
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Get Attribute    ${THE_TEXT}    Name

A Selector Resolved In A Keyword Does Not Leak Out Of It
    Set Root    ${OPS}
    Read The Text Inside The Detail View
    ${after}=    Get Attribute    ${THE_TEXT}    Name
    Should Be Equal    ${after}    Status    msg=the keyword's LOCAL root must not outlive it

The Unaliased Import Uses The Documented Variable Names
    Set Root    ${OPS}
    Set Query Settings    {'timeout': 0.3}
    Variable Should Exist    \${PLATYNUI_ROOT_DESCRIPTOR}
    Variable Should Exist    \${PLATYNUI_QUERY_SETTINGS}

A Captured Element Is Not Re-Resolved
    [Documentation]    A capture names one concrete element; only a selector re-resolves.
    Set Root    ${OPS}
    ${captured}=    Query    ${THE_TEXT}    only_first=${True}
    Set Root    ${DETAILS}
    ${name}=    Get Attribute    ${captured}    Name
    Should Be Equal    ${name}    Status    msg=the capture must stay on the element it named


*** Keywords ***
Read The Text Inside The Detail View
    Set Root    ${DETAILS}
    ${name}=    Get Attribute    ${THE_TEXT}    Name
    Should Be Equal    ${name}    Description
