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

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Qt Instance With Modal
Suite Teardown      Terminate Default Qt Instance

Test Tags           real


*** Variables ***
${SIZE_TOL}         ${8}
${EXTENTS_TOL}      ${4}


*** Test Cases ***
Modal Dialog Is Exposed
    [Documentation]    Sanity: the modal dialog opened at startup resolves on the tree.
    Node Is Present    ${APP}${DIALOG_MODAL}

Modal Dialog Bounds Are Its Own Not The Main Window's
    [Documentation]    The modal dialog must report its own ~340x200 client rect, smaller than and
    ...    distinct from the main window — the same guarantee as the modeless dialogs.
    ${main}=    Get Bounds    ${APP}${MAIN_WINDOW}
    ${b}=    Get Bounds    ${APP}${DIALOG_MODAL}
    Should Be True    $b.width < $main.width and $b.height < $main.height
    ...    msg=modal dialog reports the main window size (${main.width}x${main.height})
    Should Be True    abs($b.width - 340) <= ${SIZE_TOL} and abs($b.height - 200) <= ${SIZE_TOL}
    ...    msg=modal dialog client size ${b.width}x${b.height} does not match the designed ~340x200

Modal Dialog Bounds Match The AT-SPI Screen Extents
    [Documentation]    Correctness cross-check from an independent source (toolkit extents vs
    ...    window-manager bounds), as in the modeless bounds suite.
    Bounds Agree With Screen Extents    ${APP}${DIALOG_MODAL}    ${EXTENTS_TOL}    modal @Bounds vs AT-SPI extents
