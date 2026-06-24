*** Settings ***
Documentation     Page-object locators for the egui test app (apps/test-app-egui). Locators address
...               elements by their ROLE and stable ``@Id`` (never a bare ``*``), and are anchored to
...               the application — the library's recommended way to keep a selector from drifting into
...               the wrong program. Every interactive widget carries a stable AccessKit author id,
...               surfaced as ``@Id`` (language-independent, stable across label/layout changes).
...
...               Anchoring: ``${APP}`` matches the egui program by name (the binary,
...               ``platynui-test-app-egui``); ``${WINDOW}`` is its top-level ``Frame`` for the shared
...               default instance. The widget locators are window-relative fragments (role + ``@Id``):
...               prepend a window locator, e.g. ``${WINDOW}`` followed by ``${BTN_CLICK_ME}``. The
...               auto_activate suite runs two copies of the program, so it pins each instance by
...               ``@ProcessId`` instead (``app:Application[@ProcessId=...]//Frame``) and prepends the
...               same fragments.
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
# Application + default-instance window (smoke / query / interaction). The auto_activate suite
# launches its own Alpha/Beta copies and pins them by @ProcessId instead (see its Suite Setup).
${APP}                  /app:Application[@Name="platynui-test-app-egui"]
${WINDOW}               ${APP}/(Frame|Window)[@Name="PlatynUI Test App"]

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
