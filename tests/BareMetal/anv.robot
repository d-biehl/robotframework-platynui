*** Settings ***
Library     PlatynUI.BareMetal    AS    BM


*** Test Cases ***
first
    Pointer Click    //*[contains(@Name,'Konfiguration starten')]
    Pointer Click    //*[contains(@Name,'Elegance Sedan')]
    Pointer Click    //*[contains(@Name,'Sportmotor')]