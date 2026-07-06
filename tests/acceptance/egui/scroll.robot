*** Settings ***
Documentation       Acceptance coverage for Pointer Scroll against the real egui app: scrolling the
...                 dedicated scroll box actually moves its content, in the requested direction, and
...                 the same number of notches back restores it. The box lives in its own side panel
...                 (it never moves), and reports its current scroll offset in two `@Id`-tagged
...                 labels (``scrollbox-offset-x`` / ``-y``) whose text is the bare integer offset, so
...                 the assertions read ``number(@Name)`` and wait out egui's smooth-scroll animation
...                 with ``Wait Until Query``. We assert direction + reversibility, never exact pixels
...                 (egui maps wheel notches to points device-dependently); the restore over-scrolls
...                 so the offset clamps cleanly back to 0.
...
...                 Suite Setup scopes the suite to the app window with ``Set Root`` (so locators are
...                 relative) and captures a fixed point inside the box to scroll over.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Open Scroll Window
Suite Teardown      Terminate Default Instance


*** Variables ***
# Offset readouts, as numbers, evaluated relative to the window root (Set Root scope).
${OFFSET_X}         number(.//(Label|Text)[@Id="scrollbox-offset-x"]/@Name)
${OFFSET_Y}         number(.//(Label|Text)[@Id="scrollbox-offset-y"]/@Name)
# A fixed point inside the box, set at SUITE scope by Open Scroll Window. Placeholders so the
# usages resolve statically; the real values are computed at runtime (like ${WINDOW}).
${BOX_X}            ${0}
${BOX_Y}            ${0}


*** Test Cases ***
Scroll Down Then Up Moves And Restores The Vertical Offset
    Reset The Box
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=DOWN    ticks=${3}
    BM.Wait Until Query    ${OFFSET_Y}    >    ${0}
    ...    msg=scrolling down did not increase the vertical offset
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=UP    ticks=${10}
    BM.Wait Until Query    ${OFFSET_Y}    ==    ${0}
    ...    msg=scrolling up did not restore the vertical offset

Scroll Right Then Left Moves And Restores The Horizontal Offset
    Reset The Box
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=RIGHT    ticks=${3}
    BM.Wait Until Query    ${OFFSET_X}    >    ${0}
    ...    msg=scrolling right did not increase the horizontal offset
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=LEFT    ticks=${10}
    BM.Wait Until Query    ${OFFSET_X}    ==    ${0}
    ...    msg=scrolling left did not restore the horizontal offset

Scroll Over An Element Scrolls The Box
    [Documentation]    Descriptor targeting (not coordinates): Pointer Scroll over an element moves the
    ...    pointer onto it, then scrolls — the real-platform counterpart to the mock move-to-target
    ...    check. A row that is visible at offset 0 (after the reset) resolves to a point inside the box.
    Reset The Box
    BM.Pointer Scroll    .//(Label|Text)[@Id="scrollbox-row-00"]    direction=DOWN    ticks=${3}
    BM.Wait Until Query    ${OFFSET_Y}    >    ${0}
    ...    msg=scrolling over an element did not scroll the box


*** Keywords ***
Open Scroll Window
    [Documentation]    Launch this suite's egui instance, scope all relative locators to it with Set
    ...    Root, and capture a fixed screen point inside the scroll box (the top row at offset 0) to
    ...    scroll over. The box is in its own panel, so this point stays inside it for the whole suite.
    Launch Default Instance
    BM.Set Root    ${WINDOW}    scope=SUITE
    # The top row is visible at the box's top-left at offset 0; move over it to learn a point inside.
    BM.Pointer Move To    .//(Label|Text)[@Id="scrollbox-row-00"]
    ${pt}=    BM.Get Pointer Position
    VAR    ${BOX_X}    ${pt.x}    scope=SUITE
    VAR    ${BOX_Y}    ${pt.y}    scope=SUITE

Reset The Box
    [Documentation]    Over-scroll up and left so both offsets clamp back to 0, making each test
    ...    independent of the order the suite runs in.
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=UP    ticks=${20}
    BM.Pointer Scroll    x=${BOX_X}    y=${BOX_Y}    direction=LEFT    ticks=${20}
    BM.Wait Until Query    ${OFFSET_Y}    ==    ${0}
    BM.Wait Until Query    ${OFFSET_X}    ==    ${0}
