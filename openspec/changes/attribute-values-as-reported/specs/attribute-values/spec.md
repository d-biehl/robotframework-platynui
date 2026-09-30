# Spec Delta

## Purpose

How every provider passes attribute values through. String values are taken exactly as the platform reports them, whether an empty string is a value depends on the attribute group, and native attributes are passed one to one.

## ADDED Requirements

### Requirement: Providers pass string values through unmodified

Every provider SHALL take each string value it reads from its platform exactly as reported. It SHALL NOT trim or collapse whitespace, at the edges or inside, and SHALL NOT replace, add or remove characters. A value that consists only of whitespace is a value.

This SHALL hold in every attribute namespace (`control`, `item`, `app`, `native`), for every provider, including providers added later. It covers strings inside structured values as well, such as the keys and values of an object attribute and the entries of an array.

Where a platform transports a typed value as a string, the provider SHALL parse the unmodified string into that type. The Java Access Bridge is such a case: it sends an accessible value, a `java.lang.Number`, as its `toString()`. A string that does not parse SHALL stay the unchanged string.

#### Scenario: An AT-SPI name keeps its whitespace

- **GIVEN** an AT-SPI element whose `Accessible.Name` is `  Save  as  `
- **WHEN** its `Name` is decided
- **THEN** it SHALL be `  Save  as  `, unchanged
- **NOTE** Verified on the provider's decision in isolation. Today AT-SPI trims it to `Save  as`.

#### Scenario: A whitespace-only AT-SPI name is a name

- **GIVEN** an AT-SPI element whose `Accessible.Name` consists of three spaces
- **WHEN** its `Name` is decided
- **THEN** it SHALL be the three spaces, not the empty string
- **NOTE** Verified on the provider's decision in isolation.

#### Scenario: An AT-SPI application's name is taken as reported

- **GIVEN** an AT-SPI application whose root object reports the `Accessible.Name` ` gedit `
- **WHEN** its application node is created during desktop enumeration and its `Name` is read
- **THEN** it SHALL be ` gedit `, unchanged
- **NOTE** Verified on the provider's decision in isolation. Application nodes seed their name on a path of their own, which trims today.

#### Scenario: AT-SPI native string properties are taken as reported

- **GIVEN** an AT-SPI element whose `Accessible.HelpText` is ` Press F1 ` and whose `Accessible.Locale` is `de_DE`
- **WHEN** its native attributes are mapped
- **THEN** `native:Accessible.HelpText` SHALL be ` Press F1 ` and `native:Accessible.Locale` SHALL be `de_DE`
- **NOTE** Verified on the provider's mapping in isolation.

#### Scenario: AT-SPI action names are taken as reported

- **GIVEN** an AT-SPI element whose first action's machine name is ` click `
- **WHEN** its `native:Action.Actions` is mapped
- **THEN** that action's name SHALL be ` click `
- **NOTE** Verified on the provider's mapping in isolation.

#### Scenario: AT-SPI object attributes keep every key and value

- **GIVEN** an AT-SPI element whose object attributes are `toolkit` with the value `Qt`, a key of one space with the value `x`, and `tag` with an empty value
- **WHEN** its `native:Accessible.Attributes` is mapped
- **THEN** it SHALL carry all three entries with their keys and values as reported
- **NOTE** Verified on the provider's mapping in isolation. Today the whitespace key is dropped.

#### Scenario: A JAB value string is parsed without trimming

- **GIVEN** a Java Access Bridge element whose current value string is `50`, and one whose current value string is `n/a `
- **WHEN** their `control:Value` and `native:Value.Current` are read
- **THEN** the first SHALL be the number `50`, and the second SHALL be the string `n/a `, unchanged
- **NOTE** Verified on the provider's conversion in isolation. Today the second is trimmed to `n/a`.

#### Scenario: A whitespace-only identifier shows in a one-line description

- **GIVEN** an element named `Save` whose `Id` consists of two spaces
- **WHEN** it is described in one line, as action log lines do
- **THEN** the description SHALL end with the quoted name followed by ` #` and the two spaces
- **NOTE** Verified on the description function in isolation. Today a whitespace-only id is left out.

#### Scenario: A whitespace-only monitor name is a name

- **GIVEN** a Windows display device whose device string consists of a space
- **WHEN** its monitor's name is decided
- **THEN** the name SHALL be that space, not absent
- **NOTE** Verified on the decision in isolation.

### Requirement: Whether an empty string is a value depends on the attribute group

The empty string SHALL be handled per attribute group:

- **Common attributes:** `Name` SHALL be present on every `control:` and `item:` node, and the empty string is a valid `Name`. `Id` and `Description` SHALL be present only when their value is not the empty string (specs `id-attribute`, `description-attribute`). A value of whitespace only is not empty.
- **Pattern attributes** SHALL NOT be omitted or set to null because their value is the empty string. When the element implements the pattern, `""` is the attribute's value.
- **Native attributes** SHALL be passed through one to one: an empty string stays `""`. A property the platform does not report at all, because it is unsupported or its read failed, stays absent or null as before.

#### Scenario: An empty text field keeps an empty Text

- **GIVEN** the Swing test application's empty text field `stage1-textfield`, served once through the Java agent and once through the Java Access Bridge
- **WHEN** its `control:Text` is read on each backend
- **THEN** it SHALL be present and `""` on both
- **NOTE** Real provider only; the Java live fixture on Windows. It holds today, and the scenario guards it.

#### Scenario: An element without an accessible name has an empty Name

- **GIVEN** an AT-SPI element whose `Accessible.Name` is the empty string, and whose object attributes carry none of `accessible-name`, `name`, `label` and `title`
- **WHEN** its attributes are decided
- **THEN** `control:Name` SHALL be listed with `""`
- **NOTE** Verified on the provider's decision in isolation.

#### Scenario: An empty identifier or description is absent

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` and `Accessible.Description` are both the empty string and whose object attributes carry no identifier
- **WHEN** its `Id` and `Description` are decided
- **THEN** neither SHALL be present
- **NOTE** Verified on the provider's decision in isolation.

#### Scenario: An empty UIA native property is listed

- **GIVEN** a UIA element whose `HelpText` property is the empty string
- **WHEN** its native attributes are enumerated and `HelpText` is looked up by name
- **THEN** `native:HelpText` SHALL be listed with `""`, and the lookup SHALL find it
- **NOTE** Verified on the property conversion in isolation. Today an empty BSTR is dropped.

#### Scenario: An empty AT-SPI native property is listed

- **GIVEN** an AT-SPI element whose `Accessible.Description` is the empty string
- **WHEN** its native attributes are mapped
- **THEN** `native:Accessible.Description` SHALL be `""`, not null, while `control:Description` stays absent
- **NOTE** Verified on the provider's mapping in isolation.

#### Scenario: The Java agent passes an empty native value through

- **GIVEN** a Swing component whose accessible name was set to the empty string
- **WHEN** it is described by the Java agent and mapped by the provider
- **THEN** the payload SHALL carry `""` as its accessible name, `native:AccessibleName` SHALL be `""`, and `control:Name` SHALL be `""`
- **NOTE** Verified on the agent's description and the provider's mapping in isolation. Today the agent drops the empty string.

#### Scenario: A JAB element without a value reports none

- **GIVEN** a Java Access Bridge element whose bridge call for the current value reports no value
- **WHEN** its `control:Value` and `native:Value.Current` are read
- **THEN** both SHALL be absent or null, as before
- **NOTE** Verified on the provider's conversion in isolation.
