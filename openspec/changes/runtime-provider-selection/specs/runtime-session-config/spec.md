## MODIFIED Requirements

### Requirement: Configuration is grouped and keyed by component id

The `config` dictionary SHALL have two top-level buckets, `platform` and `providers`. Within each bucket, settings SHALL be keyed by the stable id of the component they configure (for example `platform.x11`, `platform.wayland`, `providers.atspi`), with the exception of **reserved non-id keys**: `platform.backend` selects the platform backend, and `providers.include` / `providers.exclude` select which providers run (see `provider-selection`). Each `PlatformFactory` and each `UiTreeProviderFactory` SHALL receive only the sub-dictionary under its own id; a component SHALL NOT read another component's settings, and reserved keys SHALL NOT be passed to any component as settings.

#### Scenario: A backend reads only its own sub-dictionary

- **WHEN** `config={'platform': {'x11': {'display': ':1'}}, 'providers': {'atspi': {'bus_address': 'unix:path=/run/user/1000/at-spi/bus_1'}}}` is resolved on X11
- **THEN** the X11 platform SHALL receive `{'display': ':1'}` and the AT-SPI provider SHALL receive `{'bus_address': 'unix:path=/run/user/1000/at-spi/bus_1'}`, and neither SHALL see the other's keys

#### Scenario: One dictionary is portable across operating systems

- **WHEN** a `config` carries `platform.x11`, `platform.wayland`, and `platform.windows` blocks and is resolved on an X11 host
- **THEN** only the `platform.x11` block SHALL be consulted and the `platform.wayland` and `platform.windows` blocks SHALL have no effect

#### Scenario: Reserved provider keys are not provider settings

- **WHEN** `config={'providers': {'include': ['atspi'], 'atspi': {'bus_address': '…'}}}` is resolved
- **THEN** `include` SHALL be consumed as a selection rule, no provider SHALL receive a settings dictionary for an id named `include`, and the AT-SPI provider SHALL receive only its own `bus_address` setting
