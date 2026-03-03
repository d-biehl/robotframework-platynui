*** Settings ***
Library    PlatynUI.BareMetal 

*** Test Cases ***
first
    Set Root  app:Application[@Name='Calculator']

    Pointer Click    //*[contains(@Name,'Checked')]
    Highlight  
    