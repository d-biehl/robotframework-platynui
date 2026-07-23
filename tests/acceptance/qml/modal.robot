*** Settings ***
Documentation       The in-scene modal dialog face of the QML fixture (--open-modal): Qt Quick's
...                 Dialog is an overlay INSIDE the scene, not a native window. Verified reality:
...                 on UIA it surfaces as a nested Window node named dialog-modal without modal
...                 state (no window pattern); on AT-SPI as role dialog WITH state Modal, but only
...                 under provider-native attributes the suites do not assert — per the blueprint's
...                 documented-deviation rule the asserted facts are presence without interaction
...                 and bounds-correct interactability on both platforms
...                 (see apps/test-app-qml/README.md).

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Test App    PlatynUI QML TestApp (Modal)    org.platynui.test.qml.modal    --open-modal
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Modal Dialog Is Present Without Interaction
    [Documentation]    The modal dialog and its contents are on the tree without any prior input.
    BM.Wait Until Exists    .//*[@Name="dialog-modal"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-label"]
    BM.Wait Until Exists    .//*[@Name="dialog-modal-button"]

Clicking The Modal Dialog Button Lands Inside The Dialog
    [Documentation]    The in-scene overlay's button reports through the last-action label — pointer
    ...    coordinates derived from the tree land correctly inside a scene-graph overlay, not just
    ...    native windows.
    BM.Wait Until Gone    .//*[@Name="last-action-dialog-modal-button"]
    BM.Pointer Click    .//*[@Name="dialog-modal-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-dialog-modal-button"]
