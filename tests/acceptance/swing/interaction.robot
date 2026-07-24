*** Settings ***
Documentation       Interaction proof through the JAB provider: pointer clicks land on JAB-reported
...                 bounds (click-counter observable), real keyboard input round-trips through the
...                 accessibility text API, and toggle/selection state is visible — all read back on the
...                 same runtime, because the provider reads JAB state live per access. Swing applies
...                 input on its event-dispatch thread, so the assertions use the self-waiting
...                 ``Wait Until Exists`` / ``Get Attribute    ==`` instead of assuming the change is
...                 instant.

Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Interaction
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Clicking The Stage 1 Button Twice Ends At Clicks-2
    [Documentation]    The status label's text and accessible name track the click counter
    ...    (clicks-1, clicks-2, …), so a rename to clicks-2 proves two clicks really landed on the
    ...    button's activation point — the bounds/DPI payoff.
    BM.Wait Until Exists    .//*[@Name="stage1-status-clicks-0"]
    BM.Pointer Click    .//*[@Name="stage1-button"]
    BM.Wait Until Exists    .//*[@Name="stage1-status-clicks-1"]
    BM.Pointer Click    .//*[@Name="stage1-button"]
    BM.Wait Until Exists    .//*[@Name="stage1-status-clicks-2"]

Typed Text Is Readable Through The Accessibility Text API
    [Documentation]    Focus via the JAB ``requestFocus`` pattern, typing through the real OS keyboard,
    ...    then ``@Text`` read back through chunked ``getAccessibleTextRange`` — never the accessible
    ...    name.
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    Text    ==    ${EMPTY}
    BM.Keyboard Type    .//*[@Name="stage1-textfield"]    hello
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    Text    ==    hello
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    IsFocused    ==    ${True}

Toggling The Checkbox Flips ToggleState
    [Documentation]    ``@ToggleState`` derives from the JAB ``checked`` state: Off → click → On.
    BM.Get Attribute    .//*[@Name="stage2-checkbox"]    ToggleState    ==    Off
    BM.Pointer Click    .//*[@Name="stage2-checkbox"]
    BM.Get Attribute    .//*[@Name="stage2-checkbox"]    ToggleState    ==    On

Radio Selection Is Visible As ToggleState
    [Documentation]    Swing radio buttons carry the JAB ``checked`` state, surfaced as ``@ToggleState``:
    ...    radio-a starts On; clicking radio-b moves the selection within the button group.
    BM.Get Attribute    .//*[@Name="stage2-radio-a"]    ToggleState    ==    On
    BM.Get Attribute    .//*[@Name="stage2-radio-b"]    ToggleState    ==    Off
    BM.Pointer Click    .//*[@Name="stage2-radio-b"]
    BM.Get Attribute    .//*[@Name="stage2-radio-b"]    ToggleState    ==    On
    BM.Get Attribute    .//*[@Name="stage2-radio-a"]    ToggleState    ==    Off

Slider And Progress Bar Expose Their Numeric Values
    [Documentation]    StatefulValue surface: ``@Value``/``@MinValue``/``@MaxValue`` parse the JAB value
    ...    interface numerically (fixture defaults: slider 50 in 0..100, progress 30).
    BM.Get Attribute    .//*[@Name="stage2-slider"]    Value    ==    ${50}
    BM.Get Attribute    .//*[@Name="stage2-slider"]    MinValue    ==    ${0}
    BM.Get Attribute    .//*[@Name="stage2-slider"]    MaxValue    ==    ${100}
    BM.Get Attribute    .//*[@Name="stage2-progress"]    Value    ==    ${30}
