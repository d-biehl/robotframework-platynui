*** Settings ***
Documentation     Mock-backed checks for the scope ladder ``Set Root`` and ``Set Query Settings``
...               accept: the same names Robot Framework's ``VAR`` syntax uses, with the same
...               meanings — ``LOCAL``, ``TEST``/``TASK``, ``SUITE``, ``SUITES``, ``GLOBAL``. What
...               crosses a suite boundary (``SUITES``, ``GLOBAL``) must be re-resolvable there,
...               because every suite builds its own library instance and runtime: a selector
...               qualifies, a captured element does not and is refused where it is set.
...
...               Counts come from the mock tree — the desktop holds 6 windows and 8 item:ListItem,
...               the Operations Console window holds 4 of those list items.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}

Test Teardown     Reset The Wide Scopes


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]


*** Test Cases ***
Task Is An Alias Of Test
    [Documentation]    Robot Framework accepts ``TASK`` wherever it accepts ``TEST``; so must we,
    ...    or an RPA suite has to spell its scopes differently from the rest of its code.
    Set Root    ${OPS}    scope=TASK
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

A Selector Root Can Be Set Globally
    Set Root    ${OPS}    scope=GLOBAL
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

Query Settings Can Be Set Globally
    Set Query Settings    {'timeout': 0.4}    scope=GLOBAL
    Run Keyword And Expect Error    *within timeout of 0.4 seconds*
    ...    Get Attribute    //control:Button[@Name="NoSuchButton"]    Name

A Captured Element Cannot Be Shared Across Suites
    [Documentation]    The element is this runtime's handle; a suite below could not find it again,
    ...    so the wide scopes refuse it instead of storing a root that fails on read elsewhere.
    ${window}=    Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *pins an element*    Set Root    ${window}    scope=SUITES
    Run Keyword And Expect Error    *pins an element*    Set Root    ${window}    scope=GLOBAL

A Captured Element Is Still Fine Within This Suite
    [Documentation]    The restriction is about crossing suites, not about pinning as such.
    ${window}=    Query    ${OPS}    only_first=${True}
    Set Root    ${window}    scope=SUITE
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

A Wider Scope Overwrites The Narrower One
    [Documentation]    Robot Framework's own behavior, and the reason the keyword returns the
    ...    previous root: a write at a wider scope lands in every active scope, so clearing the
    ...    global root does not bring back the suite-scoped one that was set before it.
    Set Root    ${OPS}    scope=SUITE
    Set Root    ${None}    scope=GLOBAL
    ${n}=    Query    count(.//control:Window)    only_first=${True}
    Should Be Equal As Integers    ${n}    6    msg=expected the desktop, i.e. the SUITE root is gone

A Rejected Root Leaves The Previous One Untouched
    ${window}=    Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *pins an element*    Set Root    ${window}    scope=GLOBAL
    ${n}=    Query    count(.//control:Window)    only_first=${True}
    Should Be Equal As Integers    ${n}    6    msg=expected the desktop, i.e. no root was stored


*** Keywords ***
Reset The Wide Scopes
    [Documentation]    ``GLOBAL`` outlives this suite by design, so it has to be cleared here — a
    ...    leftover root would silently rescope every suite that runs after this one.
    Set Root    ${None}    scope=GLOBAL
    Set Query Settings    ${None}    scope=GLOBAL
    Set Root    ${None}    scope=SUITE
