*** Settings ***
Documentation       Real-lane proof that a construction-time ``config`` binds a BareMetal runtime to an
...                 explicit X11 display (spec ``runtime-session-config`` — "Configuration values
...                 override the environment"). ``CFG`` is pinned to the session's own display and
...                 AT-SPI bus through ``config`` and must drive the real tree exactly like the
...                 environment-derived default; ``BADX`` is pinned to an absent display and must fail,
...                 which proves ``platform.x11.display`` is actually consulted (an ignored config would
...                 fall back to the working environment ``DISPLAY`` and succeed).
...
...                 X11-only: forcing ``platform.backend=x11`` cannot serve the Wayland compositor lane,
...                 so the suite is tagged ``platform:x11`` and only the X11 lane profile selects it.
...                 The runtime is built lazily on first keyword use, so the forced-backend imports are
...                 harmless in lanes that never select the suite.

Resource            resources/testapp.resource
Library             PlatynUI.BareMetal    config={'platform': {'backend': 'x11', 'x11': {'display': '%{DISPLAY=}'}}, 'providers': {'atspi': {'bus_address': '%{AT_SPI_BUS_ADDRESS=}'}}}    AS    CFG
Library             PlatynUI.BareMetal    config={'platform': {'backend': 'x11', 'x11': {'display': ':987'}}}    AS    BADX

Suite Setup         Launch Default Instance
Suite Teardown      Run Keyword And Ignore Error    Terminate Default Instance

Test Tags           real    platform:x11


*** Test Cases ***
Config Bound Display Drives The Real Session
    [Documentation]    A runtime bound by ``config`` to the session's own display and AT-SPI bus resolves
    ...    the real tree and drives the display — the explicit-session path is functionally identical to
    ...    the environment-derived default.
    ${win}=    CFG.Query    ${WINDOW}    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=config-bound runtime did not resolve the window on its named display
    CFG.Highlight    ${win}    duration=1.0

A Wrong Config Display Fails Though The Environment Display Is Valid
    [Documentation]    Binding ``config`` to an absent display (``:987``) makes construction fail even
    ...    though the environment ``DISPLAY`` is the working session — proving the config display value
    ...    overrides the environment rather than being ignored.
    Run Keyword And Expect Error    *    BADX.Query    .    only_first=${True}
