# Spec Delta

## MODIFIED Requirements

### Requirement: Unclaimed configuration keys are tolerated

A component id under a bucket that no registered component claims, or a setting key a component does not recognize, SHALL be ignored rather than cause an error. A component id that no component claims SHALL be recorded at debug log level; this keeps portable dictionaries usable, since a dictionary may carry every operating system's blocks. A setting key that a component does not recognize, under a component that is claimed and active in this runtime, SHALL be recorded as a warning naming the component and the key, because it is almost always a typo that would otherwise fall back to the default in silence.

A top-level key other than `platform` or `providers` SHALL be ignored and recorded as a warning naming the key and the accepted buckets. A bucket whose value is not a dict SHALL be ignored and recorded as a warning naming the bucket and the Python type of its value. No portable dictionary carries either, and whatever they hold is lost.

#### Scenario: Foreign-OS block on the wrong platform

- **WHEN** a `config` resolved on X11 contains a `platform.windows` block
- **THEN** the block SHALL be ignored, construction SHALL succeed, and a debug-level log SHALL record that `platform.windows` was not claimed

#### Scenario: Misspelled setting key

- **WHEN** `config={'platform': {'x11': {'dispaly': ':1'}}}` is resolved on X11 (note the typo)
- **THEN** the unknown `dispaly` key SHALL be ignored, construction SHALL succeed, the X11 display SHALL fall back to the environment, and a warning SHALL name `platform.x11` and `dispaly`
- **NOTE** Real X11 session only; the warning alone is unit-tested.

#### Scenario: Misspelled key of a component that is not active

- **WHEN** `config={'platform': {'windows': {'dispaly': ':1'}}}` is resolved on X11
- **THEN** the key SHALL be ignored and SHALL be recorded at debug level only, because `platform.windows` is not claimed in this runtime

#### Scenario: Misspelled bucket

- **WHEN** `config={'platfrom': {'x11': {'display': ':1'}}}` is resolved
- **THEN** construction SHALL succeed, the environment SHALL apply, and a warning SHALL name `platfrom` and the accepted buckets `platform` and `providers`
