*** Settings ***
Documentation     Page-object locators for the Swing test app (apps/test-app-swing). JAB exposes no
...               AutomationId equivalent, so every control carries an explicit unique accessibleName —
...               the locators below key on ``@Name`` plus the normalized PlatynUI role (JAB
...               ``push button`` → ``Button``, ``text`` → ``Text``, ``spinbox`` → ``SpinButton``; see
...               ``crates/provider-jab/src/map.rs``).
...
...               Anchoring: the app root (``${APP}`` = ``/app:Application[@ProcessId=...]``) is pinned to
...               a launched instance by its ProcessId and set at SUITE scope by the launcher (see
...               testapp.resource). The fragments below are app-relative — prepend ``${APP}``. The main
...               window's ``@Name`` is its (per-suite unique) title, so window locators are built by the
...               ``Window Locator`` keyword instead of a fixed fragment.

*** Variables ***
# Set at SUITE scope by the launcher (testapp.resource). Declared here as placeholders so locators
# resolve statically for the linter: ${APP} = app root, ${WINDOW} = app-relative window locator,
# ${SWING_TITLE} = the launched window title.
${APP}                  ${None}
${WINDOW}               ${None}
${SWING_TITLE}          ${None}

# App-relative control locators (normalized role + @Name) — prepend ${APP}.
${MAIN_MENUBAR}         //MenuBar[@Name="main-menubar"]
${MENU_FILE}            //Menu[@Name="menu-file"]
${MENU_FILE_EXIT}       //MenuItem[@Name="menu-file-exit"]
${MENU_HELP}            //Menu[@Name="menu-help"]
${STAGE1_BUTTON}        //Button[@Name="stage1-button"]
${STAGE1_TEXTFIELD}     //Text[@Name="stage1-textfield"]
# The status label's @Name tracks the click counter: stage1-status-clicks-<n>.
${STAGE1_STATUS_0}      //Label[@Name="stage1-status-clicks-0"]
${STAGE1_STATUS_1}      //Label[@Name="stage1-status-clicks-1"]
${STAGE1_STATUS_2}      //Label[@Name="stage1-status-clicks-2"]
${STAGE2_CHECKBOX}      //CheckBox[@Name="stage2-checkbox"]
${STAGE2_RADIO_A}       //RadioButton[@Name="stage2-radio-a"]
${STAGE2_RADIO_B}       //RadioButton[@Name="stage2-radio-b"]
${STAGE2_COMBO}         //ComboBox[@Name="stage2-combo"]
${STAGE2_SLIDER}        //Slider[@Name="stage2-slider"]
${STAGE2_SPINNER}       //SpinButton[@Name="stage2-spinner"]
${STAGE2_PROGRESS}      //ProgressBar[@Name="stage2-progress"]
