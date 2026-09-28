# Spec Delta

## ADDED Requirements

### Requirement: Activation requests no focus change inside the window

Activating a window SHALL bring the element's top-level window to the front and make it the active window. This applies to Activate Window, Bring To Front, and the implicit activation before the pointer keywords, Highlight, Take Screenshot, Focus and the keyboard keywords. PlatynUI SHALL NOT, as part of the activation, request that the keyboard focus move to the element the activation was requested for, or to any other element inside the window. Where the focus goes inside the window is then up to the application and the platform. A keyword's own action may still move it: Focus, a keyboard keyword with a target, or a click that the application answers with focus.

#### Scenario: Highlight of another element leaves the focused text field focused

- **GIVEN** auto_activate on, and the text field `input-name` of an egui window focused with Focus
- **WHEN** Highlight is called on the button `btn-click-me` of the same window
- **THEN** after a short settle (this asserts that nothing happens) `input-name` SHALL still report `@IsFocused` True and `btn-click-me` SHALL report False
- **NOTE:** Real provider only, on every lane. egui keeps its focused widget when its window is activated. Fails on Windows before this change, where activation focuses the button.

#### Scenario: Take Screenshot of another element leaves the focused text field focused

- **GIVEN** auto_activate on, and `input-name` of an egui window focused
- **WHEN** Take Screenshot is called for `btn-click-me` with a relative file name
- **THEN** `input-name` SHALL still report `@IsFocused` True
- **NOTE:** Real provider only, on every lane. Fails on Windows before this change.

#### Scenario: Raising a background window keeps the application's own focus

- **GIVEN** two egui instances ALPHA and BETA, `input-name` of ALPHA focused, and then BETA activated with Activate Window
- **WHEN** Highlight is called on `btn-click-me` of ALPHA
- **THEN** ALPHA SHALL become active (awaited with Wait Until Query on `@IsActive`), and afterwards `input-name` of ALPHA SHALL report `@IsFocused` True
- **NOTE:** Real provider only, on every lane. An egui element reports focus only while its window is active, so the focus is read after the window became active. Fails on Windows before this change.

### Requirement: Bring To Front does not depend on the element accepting focus

Bring To Front SHALL activate the top-level window that contains the element, whether or not the element itself is enabled or can take the keyboard focus. When the element has no window that can be activated, Bring To Front SHALL keep failing with an error that names the element.

#### Scenario: Bring To Front on a disabled button raises its window

- **GIVEN** two egui instances ALPHA and BETA with BETA active, and `btn-conditional` of ALPHA disabled by clearing the checkbox `chk-enable-conditional` (awaited until `@IsEnabled` is False)
- **WHEN** Bring To Front is called on ALPHA's `btn-conditional`
- **THEN** the keyword SHALL succeed, and ALPHA SHALL become active (awaited with Wait Until Query)
- **NOTE:** Real provider only, on every lane. A teardown enables the button again. Fails on Windows before this change: UI Automation refuses to focus a disabled element, and Bring To Front reports that as an activation failure.

#### Scenario: A pointer action on a disabled element still raises its window

- **GIVEN** auto_activate on, BETA active, and `btn-conditional` of ALPHA disabled
- **WHEN** Pointer Move To is called on ALPHA's `btn-conditional`
- **THEN** ALPHA SHALL become active
- **NOTE:** Real provider only, on every lane. On Windows before this change the activation failure was swallowed at debug, and the window stayed behind.
