*** Settings ***
Documentation     Mock-backed checks for Get Attribute Value that no other mock suite covers: the
...               typed value, a prefixed attribute, an assertion that holds, and the two failures
...               that come at once instead of after the timeout — an assertion that does not hold
...               and a missing attribute. Element-not-found and foreign-import failures are covered
...               by query_settings.robot and library_instance_isolation.robot.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]


*** Test Cases ***
Get Attribute Value Returns The Typed Value
    ${maximized}=    Get Attribute Value    ${OPS}    IsMaximized
    Should Be True    ${{ $maximized is False }}

Get Attribute Value Reads A Prefixed Attribute In Its Namespace
    ${pid}=    Get Attribute Value    ${OPS}    native:ProcessId
    Should Be Equal    ${pid}    ${4242}

Get Attribute Value Returns The Value When The Assertion Holds
    ${name}=    Get Attribute Value    ${OPS}    Name    ==    Operations Console
    Should Be Equal    ${name}    Operations Console

Get Attribute Value Fails At Once When The Assertion Does Not Hold
    [Documentation]    The value is checked once: even with a 30-second per-call timeout the check
    ...    fails within the test's few seconds instead of waiting for the value to change.
    [Timeout]    5 seconds
    Run Keyword And Expect Error    *'Operations Console' (str) should be 'Wrong Name' (str)
    ...    Get Attribute Value    ${OPS}    Name    ==    Wrong Name    query_overrides={'timeout': 30}

Get Attribute Value Fails At Once For A Missing Attribute
    [Documentation]    A missing attribute is an error at once, not something to wait for.
    [Timeout]    5 seconds
    Run Keyword And Expect Error    AttributeNotFoundError: *ToggleState*
    ...    Get Attribute Value    ${OPS}    ToggleState    query_overrides={'timeout': 30}
