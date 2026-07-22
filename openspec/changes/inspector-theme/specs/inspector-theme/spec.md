# inspector-theme Delta

## ADDED Requirements

### Requirement: Theme preference is a persisted three-value setting with System as default

The Inspector SHALL provide a theme preference with the values System, Light, and Dark, editable in the Settings dialog and persisted with the Inspector's other settings. The default SHALL be System. Light and Dark SHALL force the respective egui theme regardless of the operating system's setting.

#### Scenario: Default preference is System

- **WHEN** the Inspector starts with no persisted settings file
- **THEN** the theme preference SHALL be System

#### Scenario: Forced preference survives a restart

- **WHEN** the user selects Light in Settings and restarts the Inspector with the same settings file
- **THEN** the Inspector SHALL render the light theme regardless of the system setting

#### Scenario: Settings file from an older version loads

- **WHEN** the Inspector loads a persisted settings file that predates the theme setting
- **THEN** the settings SHALL load successfully and the preference SHALL be System

### Requirement: System preference follows the operating system's color scheme

With the System preference active, the Inspector SHALL follow the operating system's light/dark color scheme: via the windowing system's reported theme where available (Windows, macOS), and on Linux via the XDG Desktop Portal's `org.freedesktop.appearance` / `color-scheme` setting. A reported change of the system scheme SHALL be reflected while the Inspector is running, without a restart.

#### Scenario: Light desktop yields a light Inspector

- **WHEN** the Inspector starts with the System preference on a desktop whose color scheme is light (portal reports prefer-light on Linux)
- **THEN** the Inspector SHALL render the light theme
- **NOTE** Verifiable only against a real desktop session with a settings portal, not the mock and not the PlatynUI compositor lane.

#### Scenario: Live switch is picked up

- **WHEN** the system color scheme changes from light to dark while the Inspector runs with the System preference
- **THEN** the Inspector SHALL switch to the dark theme without a restart
- **NOTE** Verifiable only against a real desktop session (portal `SettingChanged` on Linux, winit theme event on Windows/macOS).

#### Scenario: Portal reports no preference

- **WHEN** the portal answers `color-scheme = 0` (no preference)
- **THEN** the Inspector SHALL render the light theme
- **NOTE** "No preference" means the default appearance, which is light by freedesktop/GTK convention — GNOME and DMS report `0` (`'default'`) as their light mode and rarely set `prefer-light` (2). Verified live under niri/DankMaterialShell.

### Requirement: System resolves to dark when no signal is available

With the System preference active, the Inspector SHALL render the dark theme when no system color scheme can be determined — no settings portal on the session bus, a portal without the `color-scheme` key, or an unreadable answer. Detection failures SHALL be silent (no error surfaced to the user) and SHALL NOT delay startup rendering.

#### Scenario: No portal on the session bus

- **WHEN** the Inspector starts with the System preference in a session without a settings portal (e.g. the PlatynUI compositor test session)
- **THEN** the Inspector SHALL render the dark theme
- **NOTE** Exactly today's appearance — keeps the acceptance lanes' rendering deterministic.

### Requirement: An ephemeral override pins the theme for one run

The Inspector SHALL accept a theme override via a `--theme` command-line flag and a `PLATYNUI_INSPECTOR_THEME` environment variable, each taking `system`, `light`, or `dark`. The command line SHALL take precedence over the environment, and both over the persisted setting. The override SHALL apply for the running instance only and SHALL NOT be written to the settings file. An invalid environment value SHALL be ignored with a logged warning.

#### Scenario: CLI override forces the theme

- **WHEN** the Inspector is started with `--theme light` while the persisted preference is Dark
- **THEN** the Inspector SHALL render the light theme

#### Scenario: CLI wins over environment

- **WHEN** the Inspector is started with `--theme dark` and `PLATYNUI_INSPECTOR_THEME=light`
- **THEN** the Inspector SHALL render the dark theme

#### Scenario: Override is not persisted

- **WHEN** the Inspector runs with `--theme light`, the user changes unrelated settings, and the Inspector is closed gracefully
- **THEN** the settings file SHALL still contain the previously persisted theme preference, not the override

#### Scenario: Invalid environment value is ignored

- **WHEN** the Inspector starts with `PLATYNUI_INSPECTOR_THEME=blue`
- **THEN** the Inspector SHALL log a warning and behave as if no environment override were set
