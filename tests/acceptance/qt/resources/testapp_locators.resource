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
${MAIN_PICK_BUTTON}    //(Button|PushButton)[@Name="main-pick-button"]
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

# Menu-bar "File" menu and one of its items. The MenuItem role sits in a
# provider-dependent namespace (``item:`` on AT-SPI, ``control:`` on UIA), so the
# locators use the ``*:`` namespace wildcard to stay cross-provider. Opening the
# menu shows a QMenu popup — on X11 a separate override-redirect window drawn
# outside the owning frame — used to exercise the hit-test's popup path.
${MENU_FILE}                //*:MenuItem[@Name="File"]
${MENU_FILE_NEW}            //*:MenuItem[@Name="menu-file-new"]

# Items of the right-click context menu (a transient QMenu popup). Unlike the
# menu-bar's File menu, whose items sit in the tree even while closed, these
# exist only while the popup is open — on AT-SPI the popup is exposed
# event-driven (see the atspi-event-driven-tree change), so the items are the
# probe for "an open popup is findable, a dismissed one is gone".
${CTX_MENU_COPY}            //*:MenuItem[@Name="ctx-copy"]
# The context menu's cascade: "ctx-more" opens a submenu (ctx-sub-alpha/-beta
# and the nested "ctx-deep" submenu holding ctx-deep-item). Every open cascade
# level is ANOTHER transient QMenu window, so these probe that grafted popups
# stay walkable in depth, not just the root menu.
${CTX_MENU_MORE}            //*:MenuItem[@Name="ctx-more"]
${CTX_MENU_SUB_ALPHA}       //*:MenuItem[@Name="ctx-sub-alpha"]
${CTX_MENU_DEEP}            //*:MenuItem[@Name="ctx-deep"]
${CTX_MENU_DEEP_ITEM}       //*:MenuItem[@Name="ctx-deep-item"]
