*** Settings ***
Documentation     Mock-backed checks for Pointer Scroll. The mock pointer device tracks the cursor
...               position (move_to) but does not expose its scroll log to Python, so the emitted
...               wheel delta is asserted in the pytest (test_baremetal_pointer_scroll.py); here we
...               cover the wiring the mock *can* observe: it resolves and waits for its target like
...               the other pointer keywords (a missing selector fails with the same timeout-bounded
...               error, honoring query_overrides), it moves the pointer over a target before
...               scrolling, and with no target it scrolls at the current position without moving.
...               The import sets a small 0.2 s default so the timeout checks stay fast.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${MISSING}        //control:Button[@Name="NoSuchButton"]


*** Test Cases ***
Missing Target Fails Like The Other Pointer Keywords
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*    Pointer Scroll    ${MISSING}

Missing Target Honors Per Call Query Overrides
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Pointer Scroll    ${MISSING}    query_overrides={'timeout': 0.6}

Scrolling Over An Element Moves The Pointer Onto It First
    [Documentation]    Learn the element's resolved point with a direct move, step the pointer well
    ...    away, then scroll over the element: the pointer must end up back on the element's point,
    ...    proving the move-over-target happened before the scroll.
    Pointer Move To    ${OPS}
    ${target}=    Get Pointer Position
    Pointer Move To    x=${{ $target.x + 137 }}    y=${{ $target.y + 89 }}
    Pointer Scroll    ${OPS}
    Get Pointer Position    ==    ${target}    msg=scroll did not move the pointer over the target first

Scrolling Without A Target Stays At The Current Position
    [Documentation]    With descriptor ${None} and no coordinates there is no point to resolve, so the
    ...    pointer must not move — the wheel turns wherever the cursor already is.
    Pointer Move To    x=${42}    y=${17}
    ${before}=    Get Pointer Position
    Pointer Scroll    ${None}
    Get Pointer Position    ==    ${before}    msg=scroll at the current position should not move the pointer

Scrolling At Explicit Coordinates Moves There First
    [Documentation]    Absolute x/y (no element) resolve to a screen point, so the pointer moves there
    ...    before the wheel turns.
    Pointer Move To    x=${5}    y=${5}
    Pointer Scroll    x=${321}    y=${123}    direction=RIGHT    ticks=${2}
    ${pos}=    Get Pointer Position
    Should Be Equal As Numbers    ${pos.x}    321
    Should Be Equal As Numbers    ${pos.y}    123
