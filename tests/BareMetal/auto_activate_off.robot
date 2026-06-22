*** Settings ***
Documentation     Mock-backed checks for an import with auto_activate=${False}: the action keywords
...               must NOT raise the target window by default, and a per-call activate=${True} must
...               override the import and raise it. Observed through the mock's exclusive @IsActive.
...               Counterpart to auto_activate.robot, which covers the auto_activate=True (default)
...               direction. A small query timeout keeps any missing-node waits fast.
Library           PlatynUI.BareMetal    use_mock=${True}    auto_activate=${False}    query_settings={'timeout': 0.2}


*** Variables ***
${OC}             //control:Window[@Name="Operations Console"]
${DETAIL}         //control:Window[@Name="Detail View"]
${DETAIL_TEXT}    //control:Window[@Name="Detail View"]//control:Text[@Name="Description"]


*** Test Cases ***
Import Auto Activate False Does Not Raise By Default
    [Documentation]    With auto_activate=${False} at import, a plain pointer click does not bring the
    ...    target window forward: the background window stays inactive, the foreground one stays active.
    Activate Window    ${OC}
    Pointer Click      ${DETAIL_TEXT}
    Get Attribute      ${DETAIL}    IsActive    ==    ${False}
    Get Attribute      ${OC}        IsActive    ==    ${True}

Per Call Activate True Overrides The Import Default
    [Documentation]    activate=${True} overrides the import's auto_activate=${False}: the window is
    ...    raised, and being exclusive the previously active one drops.
    Activate Window    ${OC}
    Pointer Click      ${DETAIL_TEXT}    activate=${True}
    Get Attribute      ${DETAIL}    IsActive    ==    ${True}
    Get Attribute      ${OC}        IsActive    ==    ${False}
