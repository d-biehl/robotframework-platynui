*** Settings ***
Documentation     Deterministic, mock-backed checks for Set Root's scope= argument and relative
...               (drilling) vs absolute roots. The mock "Operations Console" window contains 4
...               item:ListItem nodes; .//item:ListItem returns 4 scoped to it and 8 unscoped,
...               and "Detail View" contains none (0). Body counts are measured inline (so a LOCAL
...               root is visible); the "in a called keyword" counts go through Count List Items.
Library           PlatynUI.BareMetal    use_mock=${True}


*** Variables ***
${OPS}      //control:Window[@Name="Operations Console"]
${DETAIL}   //control:Window[@Name="Detail View"]


*** Test Cases ***
Local Scope Is Default And Does Not Reach Called Keywords
    Set Root    ${OPS}
    ${body}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${body}    4
    ${in_keyword}=    Count List Items
    Should Be Equal As Integers    ${in_keyword}    8

Local Scope Does Not Leak Into The Next Test
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    8

Test Scope Reaches Called Keywords
    Set Root    ${OPS}    scope=TEST
    ${body}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${body}    4
    ${in_keyword}=    Count List Items
    Should Be Equal As Integers    ${in_keyword}    4

Test Scope Does Not Leak Into The Next Test
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    8

Relative Set Root Drills Into The Current Root
    Set Root    ${OPS}
    Set Root    .
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

Relative Set Root Drills Through Multiple Levels
    [Documentation]    Relative roots chain recursively: Window -> Panel -> List, each drilling into
    ...    the previous (the captured parent chain). At the List level only its 4 items remain and
    ...    the window's buttons are out of scope (0), proving the chain narrowed at every step.
    Set Root    ${OPS}
    Set Root    .//control:Panel[@Name="Workspace"]
    Set Root    .//control:List[@Name="Task List"]
    ${items}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${items}    4
    ${buttons}=    Query    count(.//control:Button)    only_first=${True}
    Should Be Equal As Integers    ${buttons}    0

Relative Parent Axis Root Climbs Back Up
    [Documentation]    ``..`` is context-dependent and climbs to the parent: from the Task List back
    ...    up to the Workspace panel, where the window's two buttons are in scope again.
    Set Root    ${OPS}
    Set Root    .//control:List[@Name="Task List"]
    ${at_list}=    Query    count(.//control:Button)    only_first=${True}
    Should Be Equal As Integers    ${at_list}    0
    Set Root    ..
    ${at_panel}=    Query    count(.//control:Button)    only_first=${True}
    Should Be Equal As Integers    ${at_panel}    2

Reset With None Returns To The Desktop
    [Documentation]    ``Set Root ${None}`` clears the root; subsequent queries run against the
    ...    desktop again (8 across the dual flat/grouped views vs 4 scoped to one window).
    Set Root    ${OPS}
    ${scoped}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${scoped}    4
    Set Root    ${None}
    ${desktop}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${desktop}    8

Malformed Selector Fails At Set Root
    [Documentation]    Set Root parses the selector to classify it, so a malformed one raises an
    ...    InvalidSelectorError immediately instead of being stored and failing lazily later.
    Run Keyword And Expect Error    *selector*    Set Root    //control:Window[broken

Cached Node Does Not Bypass Parent Drilling
    [Documentation]    A relative selector resolved earlier as an action target caches a node on its
    ...    shared descriptor. Set Root must re-resolve against the captured parent, not reuse that
    ...    cached node: with a broken (non-node) parent the re-resolution raises, whereas a reused
    ...    cached node would wrongly succeed.
    Set Root    ${OPS}
    Get Attribute    .//control:List[@Name="Task List"]    Name      # resolves + caches the List node
    Set Root    count(//control:Window)    scope=SUITE
    Set Root    .//control:List[@Name="Task List"]
    Run Keyword And Expect Error    *did not return a UiNode*    Query    count(.//item:ListItem)    only_first=${True}
    [Teardown]    Set Root    ${None}    scope=SUITE

Absolute Set Root Ignores The Current Root
    Set Root    ${OPS}
    Set Root    ${DETAIL}
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    0

Restore Puts The Previous Root Back Unchanged
    Set Root    ${OPS}
    ${previous}=    Set Root    ${DETAIL}
    ${detail}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${detail}    0
    Set Root    ${previous}
    ${restored}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${restored}    4

Absolute Root Does Not Resolve Its Parent
    [Documentation]    An absolute child root is independent and must never resolve the parent. The
    ...    suite root is a non-node expression that would raise ResultTypeError if it were ever
    ...    resolved as a root, so reaching 4 proves the absolute child never touched it.
    Set Root    count(//control:Window)    scope=SUITE
    Set Root    ${OPS}
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    [Teardown]    Set Root    ${None}    scope=SUITE

Parenthesized Absolute Root Does Not Resolve Its Parent
    [Documentation]    ``(//...)[1]`` is absolute but does not start with ``/``; the XPath engine
    ...    classifies it correctly, so the non-node suite root is never resolved (a leading-``/``
    ...    string heuristic would have wrongly drilled into and resolved it).
    Set Root    count(//control:Window)    scope=SUITE
    Set Root    (//control:Window[@Name="Operations Console"])[1]
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    [Teardown]    Set Root    ${None}    scope=SUITE

Set Root Returns The Suite-Scope Value Not A Local Override
    [Documentation]    ``Set Root X scope=SUITE`` returns the root that was at *suite* scope, not the
    ...    currently effective one. A LOCAL override (Detail View, 0 items) shadows the suite root
    ...    (Operations Console, 4 items); the returned value must be the suite root — proven by
    ...    re-applying it and counting 4. Reading the effective view would return Detail View → 0.
    Set Root    ${OPS}    scope=SUITE
    Set Root    ${DETAIL}                                                 # LOCAL override → effective = Detail View (0)
    ${suite_before}=    Set Root    //control:Window[@Name="Settings"]    scope=SUITE
    Set Root    ${suite_before}                                          # re-apply the returned value as the root
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    [Teardown]    Set Root    ${None}    scope=SUITE

Suite Root Saved And Restored At Suite Scope Round-Trips
    [Documentation]    Capturing the suite root, replacing it at suite scope, then restoring the
    ...    captured value at suite scope puts the original suite root (Operations Console, 4) back.
    Set Root    ${OPS}    scope=SUITE
    ${saved}=    Set Root    ${DETAIL}    scope=SUITE                     # ${saved} = Operations Console
    ${replaced}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${replaced}    0                       # suite is now Detail View (0)
    Set Root    ${saved}    scope=SUITE                                   # restore the captured suite root
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    [Teardown]    Set Root    ${None}    scope=SUITE

Suite Scope Set In One Test
    Set Root    ${OPS}    scope=SUITE
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4

Suite Scope Still Active In The Next Test
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    [Teardown]    Set Root    ${None}    scope=SUITE


*** Keywords ***
Count List Items
    ${n}=    Query    count(.//item:ListItem)    only_first=${True}
    RETURN    ${n}
