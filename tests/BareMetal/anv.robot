*** Settings ***
Library     PlatynUI.BareMetal    AS    BM


*** Test Cases ***
# first
#    Pointer Click    //*[contains(@Name,'Konfiguration starten')]
#    Pointer Click    //*[contains(@Name,'Elegance Sedan')]
#    Pointer Click    //*[contains(@Name,'Sportmotor')]

second
    [Documentation]    This test case demonstrates how to use the Keyboard
    ...     Type keyword to send keystrokes to a specific application.
    ...    ${\n}hallo wie geht es dir
    # First, we click on the application to ensure it is in focus.
    Keyboard Type    app:Application[@Name="org.gnome.gedit"]//Text    <CONTROL+A>Hallo${\n} ${TEST_DOCUMENTATION}
    Keyboard Type    app:Application[@Name="org.gnome.gedit"]//Text    äöüß§"%§$%/()=?`´^°+*~#'&@€N>,;:_- 