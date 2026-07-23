*** Settings ***
Documentation       Real-lane proof that two BareMetal runtimes coexist in one process against the same
...                 X11 session and share no platform state (spec ``per-runtime-platform-lifecycle`` —
...                 "Runtimes share no platform state"). ``A`` and ``B`` are both bound by ``config`` to
...                 the same display and AT-SPI bus; they differ only in ``auto_activate`` so Robot
...                 Framework instantiates two distinct library instances — hence two native runtimes,
...                 each owning its own ``Arc<X11Connection>`` and highlight controller. Both must
...                 resolve and drive the same live session independently.
...
...                 X11-only: forcing ``platform.backend=x11`` cannot serve the Wayland compositor lane,
...                 so the suite is tagged ``platform:x11`` and only the X11 lane profile selects it.
...                 The runtimes are built lazily on first keyword use, so the forced-backend imports are
...                 harmless in lanes that never select the suite.

Resource            resources/testapp.resource
Library             PlatynUI.BareMetal    config={'platform': {'backend': 'x11', 'x11': {'display': '%{DISPLAY=}'}}, 'providers': {'atspi': {'bus_address': '%{AT_SPI_BUS_ADDRESS=}'}}}    auto_activate=${True}    AS    A
Library             PlatynUI.BareMetal    config={'platform': {'backend': 'x11', 'x11': {'display': '%{DISPLAY=}'}}, 'providers': {'atspi': {'bus_address': '%{AT_SPI_BUS_ADDRESS=}'}}}    auto_activate=${False}    AS    B

Suite Setup         Launch Default Instance
Suite Teardown      Run Keyword And Ignore Error    Terminate Default Instance

Test Tags           real    platform:x11


*** Test Cases ***
Two Coexisting Runtimes Resolve The Same Window
    [Documentation]    Both runtimes, each with its own connection to the same session, independently
    ...    resolve the same live window — coexistence on one display, no shared connection required.
    ${wa}=    A.Query    ${WINDOW}    only_first=${True}
    ${wb}=    B.Query    ${WINDOW}    only_first=${True}
    Should Not Be Equal    ${wa}    ${None}    msg=runtime A did not resolve the window
    Should Not Be Equal    ${wb}    ${None}    msg=runtime B did not resolve the window

Each Coexisting Runtime Drives Its Own Highlight
    [Documentation]    Each runtime owns a per-instance X11 highlight controller, so both can draw the
    ...    overlay against the shared session without a global controller to collide on.
    A.Highlight    ${WINDOW}    duration=0.5
    B.Highlight    ${WINDOW}    duration=0.5

One Runtime Keeps Working While The Other Acts
    [Documentation]    An action through one runtime does not disturb the other: A clicks the button and
    ...    B, reading the same live tree through its own connection, observes the incremented count.
    ${before}=    B.Get Attribute    ${WINDOW}${STATUS_CLICKS}    Name
    A.Pointer Click    ${WINDOW}${BTN_CLICK_ME}
    Sleep    0.3s
    ${after}=    B.Get Attribute    ${WINDOW}${STATUS_CLICKS}    Name
    Should Be True    ${{ int($after.split(":")[1]) }} > ${{ int($before.split(":")[1]) }}
    ...    msg=B did not observe the click A performed on the shared session
