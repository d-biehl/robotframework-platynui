*** Settings ***
Documentation     Fixture for ``tests/PlatynUI/test_keyword_logging_rf.py``: runs keywords whose log lines
...               and errors the pytest reads back from ``output.xml`` (spec: *diagnostic-logging*). It
...               lives outside ``tests/BareMetal`` because its assertions are in the pytest; run on its own,
...               several tests fail on purpose. The pytest sets ``NATIVE_LOG_LEVEL``, RF's ``--loglevel``,
...               and the secrets ``PLATYNUI_TEST_SECRET`` (types) and ``PLATYNUI_TEST_BAD_SECRET`` (fails).
Library           PlatynUI.BareMetal    use_mock=${True}    native_log_level=${NATIVE_LOG_LEVEL}


*** Variables ***
${NATIVE_LOG_LEVEL}       ${None}


*** Test Cases ***
A Click Is Traced
    Pointer Click    //Button[@Name="OK"]

Ten Clicks Add No Info Record
    Query    //Button[@Name="OK"]    only_first=${True}
    FOR    ${i}    IN RANGE    10
        Pointer Click    //Button[@Name="OK"]
    END

Typing Is Traced Without Its Text
    Keyboard Type    ${None}    Zq8Wx

A Secret Is Typed Without Being Shown
    VAR    ${secret: Secret}    %{PLATYNUI_TEST_SECRET}
    Keyboard Type    ${None}    ${secret}

An Unknown Key Name Says Why
    Keyboard Type    ${None}    Qz7<Kq9>w

An Unknown Key Name In A Secret Says Only Where
    VAR    ${secret: Secret}    %{PLATYNUI_TEST_BAD_SECRET}
    Keyboard Type    ${None}    ${secret}

A Single Backslash Before A Bracket Is Consumed By Robot Framework
    Keyboard Type    ${None}    pa\<ss>wd

A Doubled Backslash Types The Bracket
    Keyboard Type    ${None}    pa\\<ss>wd

An Assertion Without A Message
    Get Attribute Value    //Button[@Name="OK"]    Name    ==    Cancel

A Failed Activation Is Traced
    Pointer Click    /

A Vanished Root Is Named As The Cause
    Set Root    //Window[@Name="Does Not Exist"]
    Set Query Settings    {'timeout': 0.5}
    Wait Until Exists    .//Button[@Name="OK"]
