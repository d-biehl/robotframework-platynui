# Tasks

## 1. Tests first

Each test in this group fails until group 2 lands.

- [x] 1.1 Add unit tests in `crates/provider-atspi/src/node.rs` on the value decisions, written as pure functions over what the bus returned:
  - `Name`:
    - `  Save  as  ` stays unchanged;
    - three spaces stay three spaces;
    - `""` with no naming object attribute gives `""`;
    - `""` with a `label` attribute ` Save ` gives ` Save `, because the fallback stays until `name-is-accessible-name`.
  - `Id`:
    - ` btn-save ` stays unchanged;
    - two spaces stay two spaces, even with an `id` attribute present;
    - an empty `AccessibleId` and an empty `accessible-id` fall back to `id` = ` btn-save `, unchanged;
    - all empty gives none.
  - `Description`: `  Closes the dialog  ` stays unchanged, three spaces are a description, and `""` gives none.
  - Native string mapping: ` Press F1 ` stays unchanged, `""` gives `""` rather than null, and a failed read gives null.
  - `actions_value` keeps an action name of ` click ` unchanged.
  - `attributes_object` and `string_map_object` keep a whitespace key, an empty key and an empty value.

  Also:
  - replace the `normalize_value_*` tests;
  - change `pick_attr_value_skips_empty_values` to "first non-empty, unmodified";
  - change `attributes_object_skips_empty_keys` to "keeps every key".

  Verify: `just test-crate platynui-provider-atspi` fails exactly on the new and changed expectations.
- [x] 1.2 Add a unit test in `crates/provider-atspi/src/lib.rs`, or next to the seed, showing that an application's name ` gedit ` is seeded unchanged. Verify: it fails before 2.1.
- [x] 1.3 Add a unit test in `crates/provider-windows-uia/src/map.rs` on the variant conversion: an empty BSTR gives `UiValue::String("")`, and a padded one stays padded. Verify: `just test-crate platynui-provider-windows-uia` fails on the empty case.
- [x] 1.4 Add a unit test in `crates/provider-java-jab/src/interfaces.rs` on the value parsing:
  - `50` gives the number 50;
  - `1E+3` gives 1000;
  - `NaN` gives a number;
  - `n/a ` stays the string `n/a `;
  - `""` and a failed call give no value.

  The `control:Value` reader shares the function. Verify: `just test-crate platynui-provider-java-jab` fails on `n/a `.
- [x] 1.5 Add unit tests for `crates/core/src/ui/describe.rs` (an id of two spaces appears after ` #`) and for the Windows monitor-name decision (a device string of one space is a name), with the decision factored out of `monitor_friendly_name`. Verify: `just test-crate platynui-core` and `just test-crate platynui-platform-windows` fail on them.
- [x] 1.6 Add JUnit tests (`java/agent/src/test/java/platynui/agent/`, headless) on the payload of `SwingElement.describe`:
  - a component whose accessible name, name (`setName("")`) and tool tip are explicitly `""` carries them as `""`;
  - a component with none of them set carries no such keys.

  Add a Rust test in `crates/provider-java/src/agent/node.rs`: a payload with `accessibleName: ""` lists `native:AccessibleName = ""` and `control:Name = ""`, and a payload with `accessibleDescription: ""` lists no `control:Description`. Verify: `just test-java-agent` fails on the JUnit tests, and the Rust test passes already, which pins the provider side.

## 2. Implementation

- [x] 2.1 AT-SPI (design decision 1):
  - make `resolve_name`, `resolve_id`, `resolve_description` and `pick_attr_value` take bus values unmodified, with only `""` meaning none where an attribute is absent when empty;
  - make `fetch_str` return every successful read as a string;
  - take the action names, the application-name seed (`src/lib.rs:317-320`) and both object-attribute maps as reported;
  - remove `normalize_value`.

  Verify: `just test-crate platynui-provider-atspi` passes.
- [x] 2.2 UIA: make `variant_to_ui_value` return `UiValue::String("")` for an empty BSTR, and confirm `get_description` still filters `""`. Verify: `just test-crate platynui-provider-windows-uia` passes.
- [x] 2.3 JAB: one shared value-string function parses the unmodified string. It keeps a non-numeric string, and treats `""` or a failed call as no value. `numeric_or_string` and the `control:Value` reader use it, and its comment matches the behavior. Verify: `just test-crate platynui-provider-java-jab` passes.
- [x] 2.4 Core and platform:
  - `describe` leaves out only an empty or absent id;
  - the monitor name is kept unless the device string is empty.

  Verify: the tests of 1.5 pass.
- [x] 2.5 Java agent: make `putIfPresent` drop only `null`, and make `explicitNameOf` return an explicitly set empty name as `""`. Verify: `just test-java-agent` passes.
- [x] 2.6 Rebuild and deliver in this order:
  1. `just build-java-agent`
  2. `just install-provider-java`
  3. `just build-native`

  Verify: `just test-java-agent-live` passes, and `uv run python -c "import platynui_native"` loads the rebuilt module.

## 3. Docs

- [x] 3.1 Update `dev-docs/architecture.md`:
  - §5.7: add the rule. No provider rewrites a string, the empty string is a value except for `Id` and `Description`, and native attributes are passed one to one.
  - §5.5 and §5.6: `Id` and `Description` are taken unmodified.
  - §6.3: the presence of the common attributes, with a pointer to §5.7.
  - §7.3: a checklist item "Pass attribute values through as reported (§5.7)".

  Verify: the four sections agree with `attribute-values`.
- [x] 3.2 Update the doc comments at the changed call sites: AT-SPI's resolvers and `fetch_str`, UIA's variant conversion, JAB's value parsing, and the agent's `putIfPresent`. Verify: no comment still promises trimming or blank-to-none.

## 4. Verification

- [x] 4.1 Run `just pre-commit`, then `just build-native`, because pre-commit leaves a mock build. Verify: all gates pass.
- [x] 4.2 Run the Windows acceptance lane:
  1. `just install-provider-java`
  2. `just test-acceptance-windows`, with `PYTHONIOENCODING=utf-8` set for the Robot run

  Verify: the UIA, JAB and agent checks and all suites pass, with no WARN or ERROR.
- [x] 4.3 Run `just test-acceptance-x11` and `just test-acceptance-compositor`, with `PYTHONIOENCODING=utf-8` set for the Robot runs. Verify:
  - `tests/acceptance/egui/query.robot` passes its `Name`, `Id` and `Description` checks;
  - the Qt, QML and egui suites pass;
  - the runs show no WARN or ERROR.
- [x] 4.4 Run `openspec validate attribute-values-as-reported --strict` and `openspec validate name-is-accessible-name --strict`. Verify: both pass, since the second change builds on this one's text.
