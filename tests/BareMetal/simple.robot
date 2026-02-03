*** Settings ***
Library     PlatynUI.BareMetal    AS    BM
#Library    JsonRpcRemote  tcp://localhost  PlatynUI.BareMetal    AS    BM


*** Test Cases ***
first
    BM.Pointer Move To    ${None}    x=1    y=2    overrides={"acceleration_profile": "EASE_IN" }

second
    Take Screenshot

third
    Pointer Move To    ${None}    x=0    y=0
    Pointer Move To    ${None}    x=1920    y=1080
    Pointer Move To    ${None}    x=0    y=1080
    Pointer Move To    ${None}    x=1920    y=0
    Pointer Click    ${None}    x=500    y=500    button=RIGHT
    Highlight    rect={"x": 100, "y": 10, "width": 299, "height": 299}
    #Sleep    2s
    Highlight    rect={"x": 0, "y": 0, "width": 1920, "height": 1080}
    #wSleep    2s
    Highlight    .
    Sleep    2s

call node
    ${node}=    BM.Query  .
    BM.Highlight    ${node}    duration=2s
    Sleep    2s