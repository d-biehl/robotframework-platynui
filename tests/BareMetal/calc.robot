*** Settings ***
Library     PlatynUI.BareMetal
Library     Collections
Library   Process


*** Test Cases ***
Calculate 1 + 2
    Enter Number         18972349237891231239872345

calculate 1 + 2 full
    Enter Number full    1893

*** Keywords ***
Enter Number
    [Arguments]    ${number}
    VAR    @{rects}
    Set Root    app:Application[@Name="ApplicationFrameHost"]/Window[@Name="Calculator" or @Name="Rechner"]
    FOR    ${c}    IN    @{{list($number)}}
        ${a}    Query    .//Button[@Id="num${c}Button"]    only_first=True
        ${r}    Get Attribute    ${a}    Bounds
        Append To List    ${rects}    ${r}

        # Pointer Click    ${a}    overrides={"motion": "DIRECT"}
        # Pointer Click    .//Button[@Id="num${c}Button"]    overrides={"motion": "DIRECT"}
    END
    Highlight    rect=${rects}


Enter Number full
    [Arguments]    ${number}
    VAR    @{rects}

    FOR    ${c}    IN    @{{list($number)}}
        ${a}    Query    app:Application[@Name="ApplicationFrameHost"]/Window[@Name="Calculator" or @Name="Rechner"]//Button[@Id="num${c}Button"]    only_first=True
        ${r}    Get Attribute    ${a}    Bounds
        Append To List    ${rects}    ${r}


        Pointer Click   app:Application[@Name="ApplicationFrameHost"]//Button[@Id="Close"]
        Process.Start Process  calc.exe
        Sleep    2s

        # Pointer Click    ${a}    overrides={"motion": "DIRECT"}
        # Pointer Click    .//Button[@Id="num${c}Button"]    overrides={"motion": "DIRECT"}
    END
    Highlight    rect=${rects}