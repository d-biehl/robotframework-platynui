*** Settings ***
Documentation       Interaction proof through the JAB provider: pointer clicks land on JAB-reported
...                 bounds (click-counter observable), real keyboard input round-trips through the
...                 accessibility text API, and toggle/selection state is visible — all read back on the
...                 same runtime, because the provider reads JAB state live per access. Swing applies
...                 input on its event-dispatch thread, so the assertions poll (``Wait Until Keyword
...                 Succeeds``) rather than assuming the change is instant.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Interaction
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Clicking The Stage 1 Button Twice Ends At Clicks-2
    [Documentation]    The status label's text and accessible name track the click counter
    ...    (clicks-1, clicks-2, …), so a rename to clicks-2 proves two clicks really landed on the
    ...    button's activation point — the bounds/DPI payoff.
    Node Is Present    ${APP}${STAGE1_STATUS_0}
    BM.Pointer Click    ${APP}${STAGE1_BUTTON}
    Wait Until Keyword Succeeds    5s    0.25s    Node Is Present    ${APP}${STAGE1_STATUS_1}
    BM.Pointer Click    ${APP}${STAGE1_BUTTON}
    Wait Until Keyword Succeeds    5s    0.25s    Node Is Present    ${APP}${STAGE1_STATUS_2}

Typed Text Is Readable Through The Accessibility Text API
    [Documentation]    Focus via the JAB ``requestFocus`` pattern, typing through the real OS keyboard,
    ...    then ``@Text`` read back through chunked ``getAccessibleTextRange`` — never the accessible
    ...    name.
    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    Text    ==    ${EMPTY}
    BM.Keyboard Type    ${APP}${STAGE1_TEXTFIELD}    hello
    Wait Until Keyword Succeeds    5s    0.25s
    ...    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    Text    ==    hello
    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    IsFocused    ==    ${True}

Toggling The Checkbox Flips ToggleState
    [Documentation]    ``@ToggleState`` derives from the JAB ``checked`` state: Off → click → On.
    BM.Get Attribute    ${APP}${STAGE2_CHECKBOX}    ToggleState    ==    Off
    BM.Pointer Click    ${APP}${STAGE2_CHECKBOX}
    Wait Until Keyword Succeeds    5s    0.25s
    ...    BM.Get Attribute    ${APP}${STAGE2_CHECKBOX}    ToggleState    ==    On

Radio Selection Is Visible As ToggleState
    [Documentation]    Swing radio buttons carry the JAB ``checked`` state, surfaced as ``@ToggleState``:
    ...    radio-a starts On; clicking radio-b moves the selection within the button group.
    BM.Get Attribute    ${APP}${STAGE2_RADIO_A}    ToggleState    ==    On
    BM.Get Attribute    ${APP}${STAGE2_RADIO_B}    ToggleState    ==    Off
    BM.Pointer Click    ${APP}${STAGE2_RADIO_B}
    Wait Until Keyword Succeeds    5s    0.25s
    ...    BM.Get Attribute    ${APP}${STAGE2_RADIO_B}    ToggleState    ==    On
    BM.Get Attribute    ${APP}${STAGE2_RADIO_A}    ToggleState    ==    Off

Slider And Progress Bar Expose Their Numeric Values
    [Documentation]    StatefulValue surface: ``@Value``/``@MinValue``/``@MaxValue`` parse the JAB value
    ...    interface numerically (fixture defaults: slider 50 in 0..100, progress 30).
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    Value    ==    ${50}
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    MinValue    ==    ${0}
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    MaxValue    ==    ${100}
    BM.Get Attribute    ${APP}${STAGE2_PROGRESS}    Value    ==    ${30}
