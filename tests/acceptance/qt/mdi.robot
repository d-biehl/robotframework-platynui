*** Settings ***
Documentation       The Qt Widgets fixture's MDI scenario (``--mdi``): its central page sits in a
...                 ``QMdiSubWindow`` offset inside the main window. Qt measures window coordinates
...                 from the subwindow but screen coordinates from the real window, so an element in
...                 the subwindow is placed by its screen coordinates. The fixture predates the
...                 blueprint, so the scenario has identifiers of its own. No child dialogs are opened,
...                 so none covers the subwindow.

Resource            resources/testapp.resource

Suite Setup         Launch Qt Test App    PlatynUI Test App (Qt MDI)    org.platynui.test.qt.mdi    --mdi    --dialogs    0
Suite Teardown      Terminate Qt Instance


*** Test Cases ***
A Button In A Group In An MDI Subwindow Is Clicked By Locator
    [Documentation]    The button reports the click through the last-action label only when the
    ...    pointer really lands on it inside the subwindow.
    BM.Wait Until Gone    .//*[@Name="last-action-mdi-button"]
    BM.Pointer Click    .//*[@Name="mdi-subwindow"]//*[@Name="mdi-group"]//*[@Name="mdi-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-mdi-button"]
