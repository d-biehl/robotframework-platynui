*** Settings ***
Documentation     Page-object locators for the egui test app (apps/test-app-egui). Locators address
...               elements by their ROLE and stable ``@Id`` (never a bare ``*``), and are anchored to
...               the application — the library's recommended way to keep a selector from drifting into
...               the wrong program. Every interactive widget carries a stable AccessKit author id,
...               surfaced as ``@Id`` (language-independent, stable across label/layout changes).
...
...               Anchoring: the window root (``${WINDOW}``, or Alpha/Beta in auto_activate) is pinned
...               to a launched instance by its ProcessId — ``/app:Application[@ProcessId=...]/(Frame|
...               Window)`` — and set at SUITE scope by the launcher (see testapp.resource). The widget
...               locators below are window-relative fragments (role + ``@Id``): prepend a window root,
...               e.g. ``${WINDOW}`` followed by ``${BTN_CLICK_ME}``.
...
...               Roles on this app's AT-SPI tree (from the Inspector) — add a variable when a test
...               needs one: window=Frame; Button (btn-click-me/-reset/-conditional, the menu buttons,
...               collapse-advanced); RadioButton (radio-option-a/-b/-c); Label (status-clicks/-slider/
...               -radio); Link (link-platynui); Entry (input-name/-notes/-secret); CheckBox
...               (chk-accept-terms/-enable-conditional, toggle-dark-mode); ComboBox (combo-fruit);
...               Slider (slider-value/-progress); ProgressBar (progress-bar). Menu items
...               (menu-file-new ...) and combo items (combo-item-<fruit>) exist only while open.
Library           PlatynUI.BareMetal    AS    BM


*** Variables ***
# The window root is set at SUITE scope by the launcher (testapp.resource: Launch Default Instance,
# or the auto_activate Suite Setup) to /app:Application[@ProcessId=...]/(Frame|Window), pinned to the
# launched copy. Declared here as a placeholder so locators resolve statically.
${WINDOW}               ${None}

# Window-relative widget locators (role + @Id) — prepend a window locator (see the docs above).
${BTN_CLICK_ME}         //Button[@Id="btn-click-me"]
${BTN_RESET}            //Button[@Id="btn-reset"]
${BTN_CONDITIONAL}      //Button[@Id="btn-conditional"]
${MENU_FILE}            //Button[@Id="menu-file"]
${MENU_EDIT}            //Button[@Id="menu-edit"]
${MENU_HELP}            //Button[@Id="menu-help"]
${LINK_PLATYNUI}        //(Link|Hyperlink)[@Id="link-platynui"]
${RADIO_OPTION_C}       //RadioButton[@Id="radio-option-c"]
${STATUS_CLICKS}        //(Label|Text)[@Id="status-clicks"]
${STATUS_RADIO}         //(Label|Text)[@Id="status-radio"]


*** Keywords ***
Get Click Count
    [Documentation]    Read the integer N from the "Clicks: N" status label of the given window
    ...    (default: the shared instance), located by role + stable ``@Id``. ``Get Attribute``
    ...    waits/retries for the label, then N is scraped from ``@Name`` — the dynamic count stays in
    ...    the label text.
    [Arguments]    ${window}=${WINDOW}
    ${name}=    BM.Get Attribute    ${window}${STATUS_CLICKS}    Name
    RETURN    ${{ int($name.split(":")[1]) }}
