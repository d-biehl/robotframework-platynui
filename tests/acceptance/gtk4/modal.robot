*** Settings ***
Documentation       The modal dialog of the GTK 4 fixture (--open-modal): a ``Gtk.Window`` with
...                 ``modal`` and ``transient-for`` the main window, open without any interaction.

Resource            resources/gtk4app.resource

Suite Setup         Launch GTK4 Test App    PlatynUI GTK4 TestApp (Modal)    org.platynui.test.gtk4.modal    --open-modal
Suite Teardown      Terminate GTK4 Instance


*** Test Cases ***
Modal Dialog Is Present Without Interaction
    [Documentation]    The modal dialog and its contents are on the tree without any prior input.
    BM.Wait Until Exists    .//*[@Name="dialog-modal"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-label"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-button"]

Clicking The Modal Dialog Button Lands Inside The Dialog
    [Documentation]    The modal dialog's button reports through the last-action label of the
    ...    main window — the pointer action landed inside the dialog, not the window below.
    BM.Wait Until Gone    .//*[@Name="last-action-dialog-modal-button"]
    BM.Pointer Click    .//*[@Name="dialog-modal-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-dialog-modal-button"]
