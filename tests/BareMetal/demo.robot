*** Settings ***
Library     PlatynUI.BareMetal    AS    BM


*** Test Cases ***
first
    Enter Number    20268797234987234

second
    Enter Number    20268797234987234
    
*** Keywords ***
Activate Button
    [Arguments]    ${button}
    Pointer Click    app:Application[@Name='kalk']/Frame//Label[@Name='${button}']/parent::Button

 Enter Number
    [Arguments]    ${number}
    Set Root    app:Application[@Name='kalk']/Frame//item:TabItem
    FOR    ${c}    IN    @{{list($number)}}
        # Activate Button    ${c}
        Pointer Click    .//Label[@Name='${c}']
    END
