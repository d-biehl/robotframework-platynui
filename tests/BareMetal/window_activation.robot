*** Settings ***
Documentation     Mock-backed checks for what activation does to a window's minimized and maximized
...               state. Activate Window, Bring To Front and the implicit raise before Pointer Click
...               bring a minimized window back to the state it was minimized from and never
...               un-maximize; Restore Window stays the explicit way back to the normal state. The
...               mock keeps window state for the whole suite, so every test starts from its own
...               baseline (Restore Window, plus activating the other window where the test needs a
...               background window) and is order-independent. How each real backend meets the same
...               contract is covered by the acceptance lane.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.5}


*** Variables ***
${OC}             //control:Window[@Name="Operations Console"]
${DETAIL}         //control:Window[@Name="Detail View"]


*** Test Cases ***
Activating A Window Minimized From Normal Brings It Back Normal
    Restore Window     ${OC}
    Minimize Window    ${OC}
    Get Attribute      ${OC}    IsMinimized    ==    ${True}
    Activate Window    ${OC}
    Get Attribute      ${OC}    IsMinimized    ==    ${False}
    Get Attribute      ${OC}    IsMaximized    ==    ${False}
    Get Attribute      ${OC}    IsActive       ==    ${True}

Activating A Window Minimized From Maximized Brings It Back Maximized
    Restore Window      ${OC}
    Maximize Window     ${OC}
    Minimize Window     ${OC}
    Get Attribute       ${OC}    IsMinimized    ==    ${True}
    Activate Window     ${OC}
    Get Attribute       ${OC}    IsMinimized    ==    ${False}
    Get Attribute       ${OC}    IsMaximized    ==    ${True}
    Get Attribute       ${OC}    IsActive       ==    ${True}

Activating A Maximized Background Window Keeps It Maximized
    Restore Window     ${OC}
    Maximize Window    ${OC}
    Activate Window    ${DETAIL}
    ${before}=         Get Attribute    ${OC}    Bounds
    Activate Window    ${OC}
    Get Attribute      ${OC}    IsActive       ==    ${True}
    Get Attribute      ${OC}    IsMaximized    ==    ${True}
    Get Attribute      ${OC}    Bounds         ==    ${before}

Activating A Normal Window Keeps It Normal
    Restore Window     ${OC}
    Activate Window    ${DETAIL}
    ${before}=         Get Attribute    ${OC}    Bounds
    Activate Window    ${OC}
    Get Attribute      ${OC}    IsActive       ==    ${True}
    Get Attribute      ${OC}    IsMaximized    ==    ${False}
    Get Attribute      ${OC}    Bounds         ==    ${before}

Bring To Front On An Element Of A Maximized Window Keeps It Maximized
    Restore Window     ${OC}
    Maximize Window    ${OC}
    Activate Window    ${DETAIL}
    Bring To Front     ${OC}//*[@Name="OK"]
    Get Attribute      ${OC}    IsActive       ==    ${True}
    Get Attribute      ${OC}    IsMaximized    ==    ${True}

Bring To Front On An Element Of A Minimized Window Brings It Back
    Restore Window     ${OC}
    Minimize Window    ${OC}
    Bring To Front     ${OC}//*[@Name="OK"]
    Get Attribute      ${OC}    IsMinimized    ==    ${False}
    Get Attribute      ${OC}    IsActive       ==    ${True}

Pointer Click Into A Maximized Background Window Keeps It Maximized
    [Documentation]    auto_activate is on by default, so the click raises the window first — and
    ...    that raise must not un-maximize it.
    Restore Window     ${OC}
    Maximize Window    ${OC}
    Activate Window    ${DETAIL}
    Pointer Click      ${OC}//*[@Name="OK"]
    Get Attribute      ${OC}    IsActive       ==    ${True}
    Get Attribute      ${OC}    IsMaximized    ==    ${True}

Restore Window Un-Maximizes
    Restore Window     ${OC}
    Maximize Window    ${OC}
    Get Attribute      ${OC}    IsMaximized    ==    ${True}
    Restore Window     ${OC}
    Get Attribute      ${OC}    IsMaximized    ==    ${False}
    Get Attribute      ${OC}    IsMinimized    ==    ${False}

Restore Window Brings Back A Minimized Window
    Restore Window     ${OC}
    Minimize Window    ${OC}
    Get Attribute      ${OC}    IsMinimized    ==    ${True}
    Restore Window     ${OC}
    Get Attribute      ${OC}    IsMinimized    ==    ${False}
