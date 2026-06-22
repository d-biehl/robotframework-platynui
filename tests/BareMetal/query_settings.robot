*** Settings ***
Documentation     Mock-backed checks for the query settings on three levels — the library-import
...               default, the scope-based Set Query Settings keyword (LOCAL/TEST/SUITE, like Set
...               Root) and the per-keyword query_overrides argument — plus partial-field inheritance,
...               reset, restore, no-leak across the shared descriptor cache, ignore_exceptions, and
...               that @assertable Get Attribute still binds its operator. The wait is observed through
...               the timeout value reported in the ElementNotFoundError raised for a missing node;
...               the import sets a small 0.2 s default so the timeout-driven checks stay fast.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${MISSING}        //control:Button[@Name="NoSuchButton"]
${MISSING_ROOT}   //control:Window[@Name="NoSuchWindow"]


*** Test Cases ***
Import Default Timeout Applies
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Get Attribute    ${MISSING}    Name

Per Call Query Overrides Beat The Default
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Get Attribute    ${MISSING}    Name    query_overrides={'timeout': 0.6}

Scope Settings Beat The Default
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name

Per Call Beats Scope
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.7 seconds*
    ...    Get Attribute    ${MISSING}    Name    query_overrides={'timeout': 0.7}

Local Scope Does Not Reach Called Keywords
    Set Query Settings    {'timeout': 0.5}
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Resolve Missing In A Keyword

Local Scope Does Not Leak Into The Next Test
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Get Attribute    ${MISSING}    Name

Test Scope Reaches Called Keywords
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Resolve Missing In A Keyword

Test Scope Does Not Leak Into The Next Test
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Get Attribute    ${MISSING}    Name

Partial Override Inherits The Other Fields
    [Documentation]    A field-by-field override must keep its sibling fields. Setting only
    ...    retry_interval on top of a timeout-only override must leave the timeout intact (0.5),
    ...    proving Robot Framework passes a partial dict rather than coercing it into a fully
    ...    defaulted QuerySettings.
    Set Query Settings    {'timeout': 0.5}    scope=SUITE
    Set Query Settings    {'retry_interval': 0.05}    scope=SUITE
    ${effective}=    Set Query Settings    ${None}    scope=SUITE
    Should Be Equal As Numbers    ${effective.timeout}    0.5
    Should Be Equal As Numbers    ${effective.retry_interval}    0.05

Reset With None Falls Back To The Default
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name
    Set Query Settings    ${None}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Get Attribute    ${MISSING}    Name

Returned Settings Restore Exactly
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    ${previous}=    Set Query Settings    {'timeout': 0.9}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.9 seconds*    Get Attribute    ${MISSING}    Name
    Set Query Settings    ${previous}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name

No Leak Across The Shared Descriptor Cache
    [Documentation]    A per-call query_overrides must not stick to the cached descriptor for that
    ...    query string. After an overridden call, a plain call on the same selector must use the
    ...    scope/default timeout, not the previous override.
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Get Attribute    ${MISSING}    Name    query_overrides={'timeout': 0.2}
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name

Ignore Exceptions Keeps Retrying On Errors
    [Documentation]    A malformed selector raises while evaluating. By default the error propagates
    ...    immediately; with ignore_exceptions the lookup swallows it and keeps retrying until the
    ...    inherited timeout, then reports ElementNotFound — proving both the flag and that the
    ...    timeout-only default is inherited (0.2 s) rather than reset.
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Get Attribute    //control:Window[broken    Name    query_overrides={'ignore_exceptions': True}

Suite Scope Set In One Test
    Set Query Settings    {'timeout': 0.5}    scope=SUITE
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name

Suite Scope Still Active In The Next Test
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name
    [Teardown]    Set Query Settings    ${None}    scope=SUITE

Get Attribute Still Binds Its Assertion Operator
    Get Attribute    ${OPS}    Name    ==    Operations Console
    Run Keyword And Expect Error    *    Get Attribute    ${OPS}    Name    ==    Wrong Name

Get Attribute Assertion Works Alongside Query Overrides
    Get Attribute    ${OPS}    Name    ==    Operations Console    query_overrides={'timeout': 1}

Wider Scope Partial Does Not Inherit A Narrower Active Scope
    [Documentation]    A partial override at a wider scope must inherit the *enclosing* scope's siblings,
    ...    never those of a narrower scope that happens to be active. With a TEST override of
    ...    {timeout: 0.5, retry_interval: 0.5} active, a SUITE {'timeout': 0.9} must store
    ...    retry_interval=0.1 (the import default), not 0.5 from the active TEST.
    Set Query Settings    {'timeout': 0.5, 'retry_interval': 0.5}    scope=TEST
    Set Query Settings    {'timeout': 0.9}    scope=SUITE
    ${suite}=    Set Query Settings    ${None}    scope=SUITE
    Should Be Equal As Numbers    ${suite.timeout}    0.9
    Should Be Equal As Numbers    ${suite.retry_interval}    0.1

Per Call Override Does Not Reach The Root Resolution
    [Documentation]    A Set Root root re-resolves on every lookup. A per-call query_overrides tunes
    ...    only the keyword's own target, so when the (absolute, missing) root is what fails, the error
    ...    must report the default 0.2 s, not the 0.7 s passed to the target.
    Set Root    ${MISSING_ROOT}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Get Attribute    ${MISSING}    Name    query_overrides={'timeout': 0.7}

Scope Settings Reach The Root Resolution
    [Documentation]    Scope-level settings apply to every lookup, including the root re-resolution.
    ...    With a missing root, the failure must report the scope's 0.5 s.
    Set Root    ${MISSING_ROOT}    scope=TEST
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Get Attribute    ${MISSING}    Name

Per Call Override Reaches Each Keyword Shape
    [Documentation]    The mechanism is mostly exercised through Get Attribute; verify the per-call
    ...    override also reaches the other distinct wiring shapes — pointer (resolved via helpers),
    ...    focus and window (direct node), and keyboard (guarded) — by failing each on a missing node
    ...    with the overridden timeout in the message. Each fails at resolution, before any real device
    ...    action, so no display is needed.
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Pointer Click    ${MISSING}    query_overrides={'timeout': 0.6}
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Focus    ${MISSING}    query_overrides={'timeout': 0.6}
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Maximize Window    ${MISSING}    query_overrides={'timeout': 0.6}
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Keyboard Type    ${MISSING}    hello    query_overrides={'timeout': 0.6}


*** Keywords ***
Resolve Missing In A Keyword
    Get Attribute    ${MISSING}    Name
