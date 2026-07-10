*** Settings ***
Documentation     Page-object locators for the Qt test app (apps/test-app-qt). Locators address windows
...               by ROLE and stable ``@Name`` (never a bare ``*``), anchored to the launched
...               application. Every widget carries a stable identity set via both ``setObjectName`` and
...               ``setAccessibleName`` (see ``_tag`` in main.py); the accessible name is surfaced as
...               ``@Name`` on AT-SPI and UIA, which is why the locators below key on ``@Name``.
...
...               Anchoring: the app root (``${APP}`` = ``/app:Application[@ProcessId=...]``) is pinned
...               to a launched instance by its ProcessId and set at SUITE scope by the launcher (see
...               testapp.resource). The locators below are app-relative fragments — prepend ``${APP}``,
...               e.g. ``${APP}${MAIN_WINDOW}`` or ``${APP}${DIALOG_1}``.
...
...               Roles are given as unions so the same locators work across providers (AT-SPI: the main
...               window is a Frame and the child windows are Dialogs; UIA: both are Window). Confirm the
...               actual roles/attributes with the Inspector on first run and tighten if needed.
...
...               IMPORTANT: the child dialogs deliberately keep ``accessibleName`` ("child-dialog-N")
...               different from their ``windowTitle`` ("Dialog N"). That divergence is the regression
...               condition — it is what defeated the window manager's name-based disambiguation — so it
...               must not be "fixed" in the app.


*** Variables ***
# App root — set at SUITE scope by the launcher (testapp.resource). Declared here as a placeholder so
# locators resolve statically.
${APP}              ${None}

# App-relative window locators (role union + @Name) — prepend ${APP}.
${MAIN_WINDOW}      //(Frame|Window)[@Name="main-window"]
${DIALOG_1}         //(Dialog|Window|Frame)[@Name="child-dialog-1"]
${DIALOG_2}         //(Dialog|Window|Frame)[@Name="child-dialog-2"]
${DIALOG_3}         //(Dialog|Window|Frame)[@Name="child-dialog-3"]
# Opened only with --open-modal (modal-bounds suite).
${DIALOG_MODAL}     //(Dialog|Window|Frame)[@Name="dialog-modal"]

# A dialog's "Click Me" button. Clicking it renames its @Name to
# "<dialog>-button-clicked", proving the click landed inside that dialog. The
# interaction suite launches a single-dialog instance (--dialogs 1) so the target
# is not occluded by sibling dialogs (they all stack at the origin on Wayland).
${DIALOG_1_BUTTON}          //(Button|PushButton)[@Name="child-dialog-1-button"]
${DIALOG_1_BUTTON_CLICKED}  //(Button|PushButton)[@Name="child-dialog-1-button-clicked"]
