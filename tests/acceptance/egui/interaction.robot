*** Settings ***
Documentation       BareMetal pointer/keyboard/focus interaction coverage against
...                 the egui app, verified through observable UI changes on the
...                 accessibility tree. The app instance is shared across the run,
...                 so the click tests assert a delta, not an absolute count.
...
...                 The launcher scopes the whole suite to the app window with
...                 ``Set Root``, so the widget locators are relative
...                 (``.//*[@Id=...]``). Status-bar effects are checked with
...                 ``Get Attribute    ==``, which waits for the change instead
...                 of sleeping a fixed time.

Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance


*** Test Cases ***
Pointer Click Increments The Click Counter
    ${before}=    Get Click Count
    BM.Pointer Click    .//*[@Id="btn-click-me"]
    BM.Get Attribute    .//*[@Id="status-clicks"]    Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=click did not increment the counter

Selecting A Radio Button Updates The Status Bar
    BM.Pointer Click    .//*[@Id="radio-option-c"]
    BM.Get Attribute    .//*[@Id="status-radio"]    Name    ==    Radio: Option C
    ...    msg=radio selection not reflected in status bar

Focus Plus Keyboard Activates The Focused Button
    ${before}=    Get Click Count
    BM.Focus    .//*[@Id="btn-click-me"]
    BM.Keyboard Type    ${None}    <Return>
    BM.Get Attribute    .//*[@Id="status-clicks"]    Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=keyboard activation did not register a click

Highlight Does Not Error
    [Documentation]    Highlight draws its overlay around the window (the current root) without
    ...    raising; Take Screenshot is covered on both backends by auto_activate.robot.
    ${win}=    BM.Query    .    only_first=${True}
    BM.Highlight    ${win}    duration=0.5

A Selector Stops Matching An Element That No Longer Fits It
    [Documentation]    The status label keeps its identity across a click — stable ``@Id``, and the
    ...    element stays alive and valid — but its ``@Name`` carries the count, so a selector keyed on
    ...    the old name must match nothing afterwards. This is what a live app can show and the static
    ...    mock tree cannot: an element that is still perfectly valid, yet no longer the answer to the
    ...    selector that once found it. Reusing it would act on the wrong element.
    ${before}=    Get Click Count
    VAR    ${old_name}    Clicks: ${before}
    ${captured}=    BM.Query    .//*[@Name="${old_name}"]    only_first=${True}
    BM.Get Attribute    .//*[@Name="${old_name}"]    Id    ==    status-clicks
    BM.Pointer Click    .//*[@Id="btn-click-me"]
    BM.Get Attribute    .//*[@Id="status-clicks"]    Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=precondition: the click must have renamed the label
    Should Be True    ${{ $captured.is_valid() }}
    ...    msg=precondition: the element must still be alive — otherwise this proves nothing
    Run Keyword And Expect Error    *${old_name}*within timeout of 2.0 seconds*
    ...    BM.Get Attribute    .//*[@Name="${old_name}"]    Id    query_overrides={'timeout': 2}

A Selector Follows The Element That Now Fits It
    [Documentation]    The other direction of the same property: after the rename, the selector for
    ...    the *new* name resolves the label again — the selector follows the tree, the tree does not
    ...    follow the selector.
    ${before}=    Get Click Count
    BM.Pointer Click    .//*[@Id="btn-click-me"]
    VAR    ${new_name}    Clicks: ${{ $before + 1 }}
    BM.Get Attribute    .//*[@Name="${new_name}"]    Id    ==    status-clicks
