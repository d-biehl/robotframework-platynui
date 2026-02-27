*** Settings ***
Library    PlatynUI.BareMetal
...    pointer_profile=${POINTER_PROFILE}
...    keyboard_settings=${KEYBOARD_SETTINGS}


*** Variables ***
&{KEYBOARD_SETTINGS}
...    press_delay_ms=4
...    release_delay_ms=4
...    between_keys_delay_ms=0
...    chord_press_delay_ms=0
...    chord_release_delay_ms=0
...    after_sequence_delay_ms=0
...    after_text_delay_ms=0

&{POINTER_PROFILE}
#...    speed_factor=0.10
...    acceleration_profile=EASE_OUT
...    motion=JITTER
...    jitter_amplitude=100.0
#...    max_move_duration_ms=5000


*** Test Cases ***
# first
#    Pointer Click    //*[contains(@Name,'Konfiguration starten')]
#    Pointer Click    //*[contains(@Name,'Elegance Sedan')]
#    Pointer Click    //*[contains(@Name,'Sportmotor')]

second
    [Documentation]    This test case demonstrates how to use the Keyboard
    ...    Type keyword to send keystrokes to a specific application.
    ...    ${\n}hallo wie geht es dir
    # First, we click on the application to ensure it is in focus.
    Keyboard Type
    ...    app:Application[@Name="Notepad"]//Document
    ...    <CONTROL+A>Hallo${\n} ${TEST_DOCUMENTATION}
    # ...    overrides={"press_delay_ms": 0, "release_delay_ms": 0, "between_keys_delay_ms": 0, "chord_press_delay_ms": 0, "chord_release_delay_ms": 0, "after_sequence_delay_ms": 0, "after_text_delay_ms": 0}
    Keyboard Type    app:Application[@Name="Notepad"]//Document    äöüß§"%§$%/()=?`´^°+*~#'&@€N>,;:_-

third
    Pointer Move To    app:Application[@Name="Notepad"]//Document
    Pointer Click    app:Application[@Name="Notepad"]//Document
    Pointer Move To    x=0  y=0
    Pointer Click    x=2000  y=0
    Pointer Move To    x=0  y=2000
