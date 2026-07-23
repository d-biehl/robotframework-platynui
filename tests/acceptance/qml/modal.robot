*** Settings ***
Documentation       The in-scene modal dialog face of the QML fixture (--open-modal): Qt Quick's
...                 Dialog is an overlay INSIDE the scene, not a native window. Verified UIA
...                 reality: it surfaces as a nested Window node named dialog-modal, but exposes
...                 NO modal state (no UIA window pattern) — per the blueprint's documented-
...                 deviation rule the asserted facts are presence without interaction and
...                 bounds-correct interactability (see apps/test-app-qml/README.md).

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Catalog Instance With Modal
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Modal Dialog Is Present Without Interaction
    Modal Dialog Should Be Present Without Interaction

Clicking The Modal Dialog Button Lands Inside The Dialog
    [Documentation]    The in-scene overlay's button renames on click — pointer coordinates derived
    ...    from the tree land correctly inside a scene-graph overlay, not just native windows.
    Clicking The Modal Dialog Button Should Land Inside The Dialog
