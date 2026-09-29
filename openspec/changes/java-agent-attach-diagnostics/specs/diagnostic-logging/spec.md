## MODIFIED Requirements

### Requirement: Configuration mistakes are reported

The following configuration mistakes SHALL NOT be dropped silently. Components are named in bucket form, such as `providers.atspi` or `platform.x11`.

- A provider or platform setting of the wrong type SHALL be reported as a warning naming the component, the key, the expected type and the found type, and the default SHALL apply.
- A Java agent call timeout (`agent.call_timeout_ms` of `providers.java`) of zero or less SHALL be reported as a warning naming the component, the key and the value, and the default SHALL apply.
- A configuration value that the Python binding cannot pass on (a key that is not a string, or a value of an unsupported type) SHALL be reported as a warning naming its dotted path and its Python type.
- A pointer or keyboard profile value of the wrong type SHALL be rejected with an error naming the key and the expected type, as the profiles' other fields already are.
- A profile key that no field reads SHALL be reported as a warning naming the key.

A component checks its settings before it builds anything, so that its warnings are logged even when the build then fails.

#### Scenario: A string where a flag was expected

- **GIVEN** `config={'providers': {'atspi': {'surface_popups': 'False'}}}`
- **WHEN** the AT-SPI provider is built
- **THEN** a warning SHALL name `providers.atspi`, `surface_popups`, the expected boolean and the found string
- **AND** the default SHALL apply

#### Scenario: A string where a flag was expected, Java

- **GIVEN** `config={'providers': {'java': {'agent': {'enabled': 'False'}}}}`
- **WHEN** a runtime is created
- **THEN** a warning SHALL name `providers.java` and `agent.enabled`
- **NOTE** Windows only.

#### Scenario: A Java agent call timeout of zero

- **GIVEN** `config={'providers': {'java': {'agent': {'call_timeout_ms': 0}}}}`
- **WHEN** a runtime is created
- **THEN** a warning SHALL name `providers.java`, `agent.call_timeout_ms` and the value `0`
- **AND** the agent's default call timeout SHALL apply
- **NOTE** Windows only. A negative value is reported the same way; a positive value and an absent key are not reported.

#### Scenario: A wrong type that makes the build fail

- **GIVEN** `config={'platform': {'backend': 'x11', 'x11': {'display': 1}}}` and no reachable X server
- **WHEN** a runtime is created
- **THEN** a warning SHALL name `platform.x11` and `display`
- **AND** the construction SHALL fail as it would without the setting

#### Scenario: A value the binding cannot pass on

- **GIVEN** a setting whose value is a `pathlib.Path`
- **WHEN** a runtime is created from Python
- **THEN** a warning SHALL name the setting's dotted path and the type `Path`

#### Scenario: A profile value of the wrong type

- **WHEN** `pointer_profile={'after_click_delay_ms': '100ms'}` is given
- **THEN** the call SHALL fail with an error naming `after_click_delay_ms` and the expected number

#### Scenario: A misspelled profile key

- **WHEN** `pointer_profile={'speed_factr': 2}` is given
- **THEN** a warning SHALL name `speed_factr` as an unknown key
- **AND** the call SHALL proceed
