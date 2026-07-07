## ADDED Requirements

### Requirement: Empty configuration reproduces environment-derived behavior

A runtime built with no `config`, or with an empty `config`, SHALL behave exactly as a runtime built today: the active platform SHALL be auto-detected from the environment and each provider SHALL resolve its connection from the environment (for AT-SPI, discovering the accessibility bus as it does now). No configuration key SHALL be required for any currently-working scenario.

#### Scenario: No config argument

- **WHEN** a `Runtime` is constructed with no `config` argument
- **THEN** the platform SHALL be auto-detected from the environment and the runtime SHALL be functionally identical to one built before this change

#### Scenario: Empty config dictionary

- **WHEN** a `Runtime` is constructed with `config={}` (or `config={'platform': {}, 'providers': {}}`)
- **THEN** resolution SHALL be identical to passing no `config` at all

### Requirement: Configuration is grouped and keyed by component id

The `config` dictionary SHALL have two top-level buckets, `platform` and `providers`. Within each bucket, settings SHALL be keyed by the stable id of the component they configure (for example `platform.x11`, `platform.wayland`, `providers.atspi`). Each `PlatformFactory` and each `UiTreeProviderFactory` SHALL receive only the sub-dictionary under its own id; a component SHALL NOT read another component's settings.

#### Scenario: A backend reads only its own sub-dictionary

- **WHEN** `config={'platform': {'x11': {'display': ':1'}}, 'providers': {'atspi': {'bus_address': 'unix:path=/run/user/1000/at-spi/bus_1'}}}` is resolved on X11
- **THEN** the X11 platform SHALL receive `{'display': ':1'}` and the AT-SPI provider SHALL receive `{'bus_address': 'unix:path=/run/user/1000/at-spi/bus_1'}`, and neither SHALL see the other's keys

#### Scenario: One dictionary is portable across operating systems

- **WHEN** a `config` carries `platform.x11`, `platform.wayland`, and `platform.windows` blocks and is resolved on an X11 host
- **THEN** only the `platform.x11` block SHALL be consulted and the `platform.wayland` and `platform.windows` blocks SHALL have no effect

### Requirement: Configuration values override the environment

For any setting a component understands, the value from `config` SHALL take precedence over the corresponding environment variable, which SHALL in turn take precedence over auto-detection. A setting absent from `config` SHALL fall back to the environment; a setting present in neither SHALL fall back to the component's auto-detection.

#### Scenario: Explicit X11 display overrides the environment (real provider)

- **WHEN** `DISPLAY` in the environment names one display but `config` sets `platform.x11.display` to a different, valid display
- **THEN** the runtime SHALL connect to the display named in `config`, not the one in the environment

#### Scenario: Explicit AT-SPI bus address overrides discovery (real provider)

- **WHEN** `config` sets `providers.atspi.bus_address` to a specific bus
- **THEN** the AT-SPI provider SHALL connect to that bus instead of discovering one from the environment

### Requirement: An explicit platform backend can be selected

When `config.platform.backend` names a registered platform id, that backend SHALL be used regardless of environment auto-detection. When `config.platform.backend` is absent, the active backend SHALL be auto-detected from the environment as it is today. When it names a backend that cannot serve the current environment, construction SHALL fail with a clear error naming the requested backend.

#### Scenario: Forced backend selection

- **WHEN** `config={'platform': {'backend': 'x11', 'x11': {'display': ':1'}}}` on a host where X11 is available
- **THEN** the X11 backend SHALL be selected even if other session types are also detectable

#### Scenario: Requested backend cannot serve the environment

- **WHEN** `config.platform.backend` names a backend whose `can_serve` returns false for the current environment
- **THEN** construction SHALL fail with an error that names the requested backend, rather than silently falling back

### Requirement: Unclaimed configuration keys are tolerated

A top-level bucket other than `platform`/`providers`, a component id under a bucket that no registered component claims, or a setting key a component does not recognize, SHALL be ignored rather than cause an error, and SHALL be recorded at debug log level. This keeps portable and forward-compatible dictionaries usable; the accepted cost is that a mistyped key fails silent.

#### Scenario: Foreign-OS block on the wrong platform

- **WHEN** a `config` resolved on X11 contains a `platform.windows` block
- **THEN** the block SHALL be ignored, construction SHALL succeed, and a debug-level log SHALL record that `platform.windows` was not claimed

#### Scenario: Misspelled setting key

- **WHEN** `config={'platform': {'x11': {'dispaly': ':1'}}}` is resolved (note the typo)
- **THEN** the unknown `dispaly` key SHALL be ignored with a debug log and the X11 display SHALL fall back to the environment

### Requirement: Session configuration is fixed at construction

The `config` dictionary SHALL be consumed once, at runtime construction, and SHALL be immutable for the runtime's lifetime. There SHALL be no keyword or API to re-bind a live runtime to a different display or accessibility bus; changing the session SHALL require building a new runtime. This is distinct from the behavioral profiles (`query_settings`, `pointer_profile`, `keyboard_profile`), which remain scoped and per-call overridable.

#### Scenario: No mid-life re-binding

- **WHEN** a runtime has been constructed with a given session `config`
- **THEN** there SHALL be no supported way to change its bound display or accessibility bus without constructing a new runtime
