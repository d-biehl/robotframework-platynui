*** Settings ***
Documentation       The modal dialog of the GTK 3 fixture (--open-modal): a ``Gtk.Dialog`` that is
...                 ``modal`` and transient for the main window, open without any interaction.

Resource            resources/gtk3app.resource

Suite Setup         Launch GTK3 Test App    PlatynUI GTK3 TestApp (Modal)    org.platynui.test.gtk3.modal    --open-modal
Suite Teardown      Terminate GTK3 Instance


*** Test Cases ***
Modal Dialog Is Present Without Interaction
    [Documentation]    The modal dialog and its contents are on the tree without any prior input.
    BM.Wait Until Exists    .//*[@Name="dialog-modal"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-label"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-button"]
