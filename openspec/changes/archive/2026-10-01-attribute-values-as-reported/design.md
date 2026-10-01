# Design

## Context

See `proposal.md` for the motivation and `specs/` for the behavior. This section records the code paths each provider takes today (verified against the code) and the platform facts the approach rests on.

**AT-SPI** (`crates/provider-atspi`):

- `normalize_value` (`src/node.rs:774-777`) trims, and turns a blank result into none. It is used by:
  - `resolve_name` (`:805-814`), `resolve_description` (`:820-827`) and `resolve_id` (`:829-838`);
  - `pick_attr_value` (`:779-788`), the object-attribute fallback of `Id` and of `Name`;
  - `fetch_str` (`:2126-2132`), the reader of every native string property: `Name` (`:2183`), `Description` (`:2184`), `AccessibleId` (`:2190`), `HelpText`, `Locale`, the role names, the `Application.*` properties and the image description (`:2312`);
  - the action machine names (`:2224-2230`);
  - the application-node name seed (`src/lib.rs:317-320`). It reaches `seed_application` (`src/node.rs:181-194`), which stores it into the node's name cache directly, so it never passes `resolve_name`.
- `attributes_object` (`:840-849`) and `string_map_object` (`:851-860`) drop keys that are blank after trimming. They keep values as they are.
- `resolve_text` (`:1541-1552`, `:1825-1829`) is already verbatim.
- The helper came with the provider's first commit (58004c9, 19 uses) with no doc comment and no commit text. D-Bus has no null string, so an unset property arrives as `""`. The empty check is the part the provider needs. The trim is not.

**UIA** (`crates/provider-windows-uia/src/map.rs`):

- No trimming anywhere.
- `variant_to_ui_value` turns an empty BSTR into none (`:885-887`). An empty BSTR inside an array stays (`:760-763`).
- `read_uia_property` (`:277`) feeds:
  - `get_description` (`:144-149`), which filters `""` itself;
  - two boolean helpers (`:336`, `:350`);
  - the native collection (`:666`, `:688`) and the named native lookup (`:724`).

  So only the native attributes lose their empty strings today.

**Java Access Bridge** (`crates/provider-java-jab`):

- `value_string` (`src/client.rs:376-389`) returns none when the bridge call fails, and the buffer's string otherwise.
- `numeric_or_string` (`src/interfaces.rs:255-265`) and the `control:Value` reader (`src/node.rs:1128-1140`) trim before parsing, and return the trimmed string when it is not a number. Their comment says "keep the raw string otherwise".
- On the Java side, `AccessibleValue` defines current, minimum and maximum as `java.lang.Number`, null when unset (JDK `javax/accessibility/AccessibleValue.java`). The bridge sends `value.toString()` and nothing for null (`jdk.accessibility/.../AccessBridge.java:2773-2855`). The C API is a 256-character buffer (`include/win32/bridge/AccessBridgeCalls.h:633-635`).
- Everything else is taken raw already: the name buffer, and `native:Description` through `static_attr`.

**Java agent:**

- `SwingElement.putIfPresent` (`java/agent/src/main/java/platynui/agent/SwingElement.java:708-712`) drops `null` and `""`. It is used for:
  - the component name (`:104`);
  - accessible names (`:106`, `:202`, `:250`, `:342`, `:356`) and descriptions (`:107`, `:203`);
  - the tool tip (`:136`) and the window title (`:167`).
- `explicitNameOf` (`:410-420`) maps an explicitly set empty name to null.
- The provider publishes what it receives (`crates/provider-java/src/agent/node.rs:511-549`). It filters `""` for `Description` (`:231-233`, `:303-305`) and for the `Id` (`element.rs:265-270`).

**Core and platform:**

- `describe` leaves out a whitespace-only id (`crates/core/src/ui/describe.rs:38`).
- The Windows desktop drops a whitespace-only monitor name (`crates/platform-windows/src/desktop.rs:131`). `trim_wstr` (`:118-121`) only cuts at the NUL terminator.

**Not value rewriting** (verified):

- the XPath engine's casts, which follow XML Schema whitespace rules (`crates/xpath/src/engine/evaluator/casting.rs`, `functions/constructors.rs`);
- the mock's numeric list parsing (`crates/provider-mock/src/tree.rs:372`, `:384`);
- the runtime's handling of object values: it serializes them to JSON and creates no attribute per key (`crates/runtime/src/xpath.rs:1135-1137`). A key made of whitespace, or an empty key, therefore needs no special handling.

## Goals / Non-Goals

**Goals:**

- Every provider takes every string exactly as its platform reports it.
- The empty string is a value, except for `Id` and `Description`, which are absent when empty. `Name` stays always present.
- Native attributes, including their empty strings, are passed through one to one.
- JAB's accessible values stay numbers and are parsed from the unmodified string.

**Non-Goals:**

- Which attributes exist, and where a name or description comes from. That is `name-is-accessible-name`: AT-SPI's object-attribute `Name` fallback, the Java agent's name sources, and JAB's `control:Description`. The fallback stays in place here and only stops trimming.
- The XPath engine's casting rules and the mock's parsing of numeric lists.
- How the Inspector or the CLI display values.

## Decisions

1. **AT-SPI replaces `normalize_value` with two plain rules.**
   - A value is taken as the bus returned it.
   - For an attribute that is absent when empty (`Id`, `Description`), only the empty string means none.
   - The decisions for `Name`, `Id` and `Description` become pure functions over what the bus returned, so they are tested without a bus:
     - `Name` is the value, or `""` when the read failed; the object-attribute fallback stays until `name-is-accessible-name`, and it too takes the first non-empty value unmodified.
     - `Id` is `AccessibleId` unless that is `""` or unreadable, and then the first non-empty of `accessible-id`, `accessible_id` and `id`.
     - `Description` is the value unless that is `""`.
   - `fetch_str` returns `UiValue::String` for every successful read, `""` included, and null only for a failed read.
   - The action names, the application-name seed and both object-attribute maps take their strings as reported. The maps drop no key.
   - *Alternative:* keep treating a whitespace-only value as none. Rejected by the maintainer: whitespace is a value.

2. **UIA's variant conversion keeps an empty BSTR.** `variant_to_ui_value` returns `UiValue::String("")`.
   - `get_description` already filters `""` for `control:Description`, and the boolean helpers are unaffected.
   - The native collection and the named lookup list the empty value, and agree by construction because they use the same conversion.
   - *Alternative:* filter `""` again in the native collection. Rejected: native attributes are passed one to one.

3. **JAB parses the unmodified value string.**
   - A string that parses as `f64` becomes a number.
   - Any other non-empty string stays unchanged.
   - An empty string, like a failed call, means no value. A JDK `Number`'s `toString()` is never empty, so an empty buffer can only stand for the bridge's null. A custom `Number` subclass whose `toString()` is empty would read as no value; that is an accepted edge.
   - This applies to both readers, `numeric_or_string` and the `control:Value` reader, which duplicate the logic today and share one function afterwards.
   - *Alternative:* expose the raw transport string in `native:Value.*`. Rejected by the maintainer: the value is a `Number`, and the agent reports it as one.

4. **The Java agent drops only `null`.**
   - `putIfPresent` keeps `""`.
   - `explicitNameOf` returns an explicitly set empty name as `""`, and returns null only when no name was set.
   - The provider keeps its filters: `Id` and `Description` stay absent for `""`, and `Name` is `""`.
   - `native:AccessibleName`, `native:ToolTipText`, `native:ComponentName` and `native:WindowTitle` then carry `""` when Swing reports it.
   - The agent, `crates/java-agent` and `packages/provider-java` stay one version.

5. **The core description and the Windows monitor name use the empty check.** `describe` leaves out only an empty or absent id, and the monitor name only an empty device string. `push_escaped` already handles spaces.

6. **Test layers**, per `dev-docs/testing-strategy.md`:
   - AT-SPI unit tests on the three decisions, on the native string mapping (`""` and padded values), on `actions_value` with raw names, and on both object-attribute maps.
     - `normalize_value_*` go with the helper.
     - The `pick_attr_value` tests move to the new semantics.
     - `attributes_object_skips_empty_keys` becomes "keeps every key".
   - A UIA unit test on the variant conversion of an empty and a padded BSTR.
   - A JAB unit test on the value parsing: `50`, `1E+3`, `NaN`, `n/a `, `""` and none.
   - Unit tests for `describe` (a whitespace-only id) and the monitor-name decision.
   - JUnit tests on the agent's payload: an explicitly empty accessible name, component name and tool tip travel as `""`, while a null one is absent. A Rust mapping test shows `native:AccessibleName = ""` together with `Name = ""`.
   - The X11, compositor and Windows lanes run unchanged. `tests/acceptance/egui/query.robot` asserts `Name`, `Id` and `Description` of the Click Me button on all three, and the Qt, QML and Swing suites show whether any locator relied on trimmed values.

## Risks / Trade-offs

- [A suite or a user locator relies on a trimmed AT-SPI value] → `normalize-space(...)` matches either form. The release notes name the change, and the lanes show any suite that relied on it.
- [UIA native listings grow by every empty string property, such as `AcceleratorKey`, `AccessKey` and `ItemStatus`] → This is intended, because native is one to one. It costs no extra COM call, since those values are read already. Snapshots and the Inspector show more rows.
- [`[@native:…]` existence checks change on AT-SPI, because `""` is no longer null] → The release notes name it.
- [A stale dev JAR after the wire change] → In dev both sides report the same version. An old agent would still drop `""`, and only native `""` values would be missing. The tasks run `just install-provider-java` before any Python or Robot run.
- [`name-is-accessible-name` edits the same functions and requirements] → This change lands first, and that change's artifacts are rebased on it: its AT-SPI untrimming and whitespace scenarios move here, and its MODIFIED blocks start from this change's text.

## Migration Plan

- **Behavioral, not additive:**
  - AT-SPI values keep their whitespace;
  - empty native strings appear on UIA and AT-SPI;
  - JAB's non-numeric value strings are no longer trimmed;
  - the agent's native attributes carry `""`.

  No keyword or API signature changes.
- **Rebuilds:** the native module (`just build-native`), because every provider is linked into `packages/native`; the agent JAR (`just build-java-agent`), restaged with `just install-provider-java`.
- **Landing:** before `name-is-accessible-name`. The agent and the provider land together.
- **Rollback:** revert the change's commits, then rebuild the native module and the JAR. No state or configuration persists.
- **For users:** locators that compared against a trimmed AT-SPI value use `normalize-space(...)` or the exact reported value.
