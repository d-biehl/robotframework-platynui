*** Settings ***
Documentation       Point-based hit-testing (``Get Element At Point``) against the Swing fixture —
...                 the live-picker path of the JAB provider. Picking the stage-1 button by its
...                 bounds-center point must resolve exactly one node: the button's JAB
...                 representation (the UIA provider abstains for the claimed Java window), carrying
...                 the same RuntimeId top-down XPath produces and a walkable ancestor chain up to
...                 the fixture's ``app:Application`` node.

Library             Collections
Library             Process
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Picker
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Picking The Stage-1 Button By Its Center Point Resolves The JAB Node
    [Documentation]    The hit-test returns a single node — the button itself, as its JAB node and
    ...    not the UIA shell of the claimed window (the single-appearance guarantee).
    ${picked}=    Pick At Center Of    ${APP}${STAGE1_BUTTON}
    Should Be Equal    ${picked.name}    stage1-button
    Should Be Equal    ${picked.role}    Button
    Should Be Equal    ${picked.attribute("Technology")}    JAB
    ...    msg=the claimed Java window must resolve to the JAB node, not the UIA shell

Picked Node Reveals To The Same Tree Node
    [Documentation]    RuntimeId identity with top-down traversal plus a walkable parent chain up to
    ...    ``app:Application`` — what the Inspector's reveal-and-select walks.
    ${located}=    BM.Query    ${APP}${STAGE1_BUTTON}    only_first=${True}
    Should Not Be Equal    ${located}    ${None}    msg=top-down XPath must locate the button
    ${picked}=    Pick At Center Of    ${APP}${STAGE1_BUTTON}
    Should Be Equal    ${picked.runtime_id}    ${located.runtime_id}
    ...    msg=the picked node must carry the RuntimeId top-down traversal produces
    ${pid}=    Get Process Id    ${SWING_APP_HANDLE}
    ${ancestor_ids}=    Evaluate    [a.runtime_id for a in $picked.ancestors()]
    Should Contain    ${ancestor_ids}    jab://app/${pid}
    ...    msg=the picked node's ancestor chain must reach the fixture's app:Application node


*** Keywords ***
Pick At Center Of
    [Documentation]    Bring the fixture window to the front (the hit-test resolves whatever is
    ...    top-most at the point), read the target's calibrated Bounds, and hit-test its center.
    [Arguments]    ${locator}
    BM.Activate Window    ${APP}${WINDOW}
    ${bounds}=    BM.Get Attribute    ${locator}    Bounds
    ${picked}=    BM.Get Element At Point    ${{ $bounds.center().x }}    ${{ $bounds.center().y }}
    Should Not Be Equal    ${picked}    ${None}    msg=nothing resolved at the center of ${locator}
    RETURN    ${picked}
