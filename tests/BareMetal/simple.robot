*** Settings ***
Library    PlatynUI.BareMetal    AS        BM

*** Test Cases ***
first
    BM.Pointer Move To    ${None}   x=1  y=2  overrides={"acceleration_profile": "EASE_IN" }
