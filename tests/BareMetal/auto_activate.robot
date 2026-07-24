*** Settings ***
Documentation     Mock-backed checks for auto_activate and the per-call activate override on the
...               keyword family that raises the target's top-level window before acting (focus,
...               keyboard, take_screenshot, highlight; the pointer keywords already had it). Two
...               things are verified: the wiring (activate is accepted and the keywords run), and the
...               gating EFFECT, observed through the mock's @IsActive (window) and @IsFocused
...               (element) attributes. The mock activate() is exclusive — exactly one window is active
...               at a time — so each test sets its own baseline with Activate Window and is therefore
...               order-independent. The Highlight cases lock in the regression that a missing
...               descriptor is skipped, not raised, even with activation on. A small query timeout
...               keeps the missing-node waits fast.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OC}             //control:Window[@Name="Operations Console"]
${DETAIL}         //control:Window[@Name="Detail View"]
${OC_OK}          //control:Window[@Name="Operations Console"]//control:Button[@Name="OK"]
${DETAIL_TEXT}    //control:Window[@Name="Detail View"]//control:Text[@Name="Description"]
${MISSING}        //control:Button[@Name="NoSuchButton"]


*** Test Cases ***
Highlight Skips A Missing Descriptor
    [Documentation]    With activation on (default) a missing node must be skipped inside the
    ...    per-descriptor loop, not raised — the regression fixed by moving the activation call inside
    ...    the try/except. Reaching the end without error is the assertion.
    Highlight    ${MISSING}

Highlight Skips Only The Missing Ones In A List
    [Documentation]    A mix of found and missing descriptors highlights the found ones and skips the
    ...    rest, without raising.
    VAR    @{nodes}    ${OC_OK}    ${MISSING}    ${DETAIL_TEXT}
    Highlight    ${nodes}

Activate Is Accepted On The New Keywords
    [Documentation]    Smoke: the per-call activate parameter is wired on every newly covered keyword
    ...    and the keywords run end-to-end on the mock — with activate on and off, and alongside
    ...    query_overrides.
    Focus              ${OC_OK}    activate=${False}
    Focus              ${OC_OK}    activate=${True}    query_overrides={'timeout': 1}
    Keyboard Type      ${OC_OK}    hello               activate=${False}
    Keyboard Press     ${OC_OK}    <Ctrl>              activate=${False}
    Keyboard Release   ${OC_OK}    <Ctrl>              activate=${False}
    Take Screenshot    ${OC}       filename=EMBED      activate=${False}
    Highlight          ${OC_OK}    activate=${True}

Pointer Activation Raises The Target Window By Default
    [Documentation]    With auto_activate on (import default), acting on an element brings its
    ...    top-level window to the front. Baseline: activate the other window; a pointer click in the
    ...    background window then flips @IsActive and, because the mock is exclusive, deactivates the
    ...    previously active window.
    Activate Window    ${OC}
    Get Attribute      ${DETAIL}    IsActive    ==    ${False}
    Pointer Click      ${DETAIL_TEXT}
    Get Attribute      ${DETAIL}    IsActive    ==    ${True}
    Get Attribute      ${OC}        IsActive    ==    ${False}

Per Call Activate False Suppresses The Raise
    [Documentation]    activate=${False} overrides the import's auto_activate=True: the click acts
    ...    without raising, so the background window stays inactive and the foreground one stays active.
    Activate Window    ${OC}
    Pointer Click      ${DETAIL_TEXT}    activate=${False}
    Get Attribute      ${DETAIL}    IsActive    ==    ${False}
    Get Attribute      ${OC}        IsActive    ==    ${True}

Focus Always Focuses But Activate Gates The Window Raise
    [Documentation]    Focus sets element focus regardless of activate (focus is app-local), while the
    ...    window raise is gated. With activate=${False} the element gets @IsFocused but its window is
    ...    NOT raised; with activate=${True} the window is raised (and, being exclusive, the other one
    ...    drops). This is exactly why keyboard/focus need the raise: focus alone is not desktop-wide.
    Activate Window    ${DETAIL}
    Focus              ${OC_OK}    activate=${False}
    Get Attribute      ${OC_OK}     IsFocused    ==    ${True}
    Get Attribute      ${OC}        IsActive     ==    ${False}
    Get Attribute      ${DETAIL}    IsActive     ==    ${True}
    Focus              ${OC_OK}    activate=${True}
    Get Attribute      ${OC}        IsActive     ==    ${True}
    Get Attribute      ${DETAIL}    IsActive     ==    ${False}
    Get Attribute      ${OC_OK}     IsFocused    ==    ${True}
