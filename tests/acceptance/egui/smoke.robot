*** Settings ***
Documentation       Real-world acceptance smoke against the egui test app,
...                 launched inside an isolated session by
...                 ``scripts/startxsession.sh`` / ``scripts/startcompositor.sh``
...                 (Xephyr/X11 or the PlatynUI Wayland compositor). Uses the
...                 low-level ``PlatynUI.BareMetal`` library on purpose — this
...                 drives the real AT-SPI tree end-to-end.

Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance


*** Test Cases ***
Desktop Root Is Reachable
    [Documentation]    Proves the session -> native runtime -> BareMetal path is alive. The suite
    ...    root is the app window, so reset to the desktop for this test (LOCAL, self-clearing).
    BM.Set Root    ${None}
    ${root}=    BM.Query    .    only_first=${True}
    Should Not Be Equal    ${root}    ${None}    msg=No desktop root resolved — session/runtime not reachable
    BM.Highlight    ${root}    duration=1.0

Egui Window Is Exposed By Title
    [Documentation]    The egui app forwards its window title to the AccessKit root node, so the
    ...    window — the suite's query root — is discoverable by name on the accessibility tree.
    ${win}=    BM.Query    .    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=window 'PlatynUI Test App' not found on the accessibility tree
    BM.Get Attribute    .    Name    ==    PlatynUI Test App
    BM.Highlight    ${win}    duration=1.0

Window Carries The Common Attributes
    [Documentation]    The always-present common attributes (architecture §6.3): every ``control:``
    ...    node names the technology that surfaced it and the patterns it advertises. Both values are
    ...    platform-specific (``AT-SPI2`` here, ``UIAutomation`` on Windows), so this asserts presence
    ...    rather than a literal — the point is that no provider leaves them off.
    BM.Get Attribute    .    Technology    validate    len(value) > 0
    BM.Get Attribute    .    SupportedPatterns    validate    len(value) > 0

# Interaction coverage (click/radio/keyboard/focus, delta-verified) lives in
# interaction.robot; query/attribute/set-root coverage in query.robot. This
# suite stays a pure reachability smoke.
