*** Settings ***
Documentation       Bounds resolution for a MODAL dialog — a different window type / resolution path
...                 than the modeless child dialogs in bounds.robot.
...
...                 This suite uses a dedicated instance launched with --open-modal (so the modal's
...                 input grab does not affect the other suites). The modal dialog, like the modeless
...                 ones, has accessibleName ("dialog-modal") != windowTitle ("Modal Dialog"), so it
...                 exercises the same name-vs-title divergence that triggered the original bug.
...
...                 Note: PlatynUI's @IsModal currently reports False for this dialog even though it is
...                 application-modal in Qt — that looks like a separate provider/toolkit gap and is out
...                 of scope here, so this suite asserts bounds, not modality.

Resource            resources/testapp.resource

Suite Setup         Launch Qt Test App    PlatynUI Test App (Qt Modal)    org.platynui.test.qt.modal    --open-modal
Suite Teardown      Terminate Qt Instance

Test Tags           real


*** Variables ***
${SIZE_TOL}         ${8}


*** Test Cases ***
Modal Dialog Is Exposed
    [Documentation]    Sanity: the modal dialog opened at startup resolves on the tree.
    BM.Wait Until Exists    .//(Dialog|Window|Frame)[@Name="dialog-modal"]

Modal Dialog Bounds Are Its Own Not The Main Window's
    [Documentation]    The modal dialog must report its own ~340x200 client rect, smaller than and
    ...    distinct from the main window — the same guarantee as the modeless dialogs.
    ${main}=    BM.Get Attribute    .//(Frame|Window)[@Name="main-window"]    Bounds
    ${b}=    BM.Get Attribute    .//(Dialog|Window|Frame)[@Name="dialog-modal"]    Bounds
    Should Be True    $b.width < $main.width and $b.height < $main.height
    ...    msg=modal dialog reports the main window size (${main.width}x${main.height})
    Should Be True    abs($b.width - 340) <= ${SIZE_TOL} and abs($b.height - 200) <= ${SIZE_TOL}
    ...    msg=modal dialog client size ${b.width}x${b.height} does not match the designed ~340x200
