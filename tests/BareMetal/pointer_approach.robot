*** Settings ***
Documentation     Mock-backed checks for the paced approach at the end of a pointer move. The mock
...               pointer device records where the pointer goes but not when, so the spacing of the
...               approach steps is asserted in the runtime's unit tests; here we cover what the
...               library adds: the approach parameters are taken from ``pointer_profile`` at import
...               and from a keyword's ``overrides``, a click still lands on its target with the
...               approach on or off, and an invalid step count fails, naming the parameter, before
...               the pointer moves. The import sets a short approach, so every click here runs with
...               it.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}
...               pointer_profile={'approach_steps': 2, 'approach_interval_ms': 5}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]


*** Test Cases ***
A Click With The Imported Approach Lands On Its Target
    [Documentation]    Learn the element's resolved point with a move, step the pointer well away,
    ...    then click the element: the paced approach must still end exactly on that point.
    Pointer Move To    ${OPS}
    ${target}=    Get Pointer Position
    Pointer Move To    x=${{ $target.x + 137 }}    y=${{ $target.y + 89 }}
    Pointer Click    ${OPS}
    Get Pointer Position    ==    ${target}    assertion_message=the click did not land on its target

A Click With The Approach Turned Off Lands On Its Target
    Pointer Move To    ${OPS}
    ${target}=    Get Pointer Position
    Pointer Move To    x=${{ $target.x + 137 }}    y=${{ $target.y + 89 }}
    Pointer Click    ${OPS}    overrides={'approach_steps': 0}
    Get Pointer Position    ==    ${target}    assertion_message=the click did not land on its target

An Invalid Approach Step Count Names The Parameter And Leaves The Pointer Where It Is
    Pointer Move To    x=${42}    y=${17}
    ${before}=    Get Pointer Position
    Run Keyword And Expect Error    *approach_steps*
    ...    Pointer Click    ${OPS}    overrides={'approach_steps': -1}
    Get Pointer Position    ==    ${before}    assertion_message=the pointer moved although the overrides were rejected
