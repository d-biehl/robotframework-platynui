# Proposal

## Why

An attribute value is meant to be what the platform reported. `dev-docs/architecture.md` §5.7 builds on typed values that mirror the platform, and `description-attribute` says it outright: "the platform's accessible-description string, unmodified". No spec states it for every attribute, and four providers break it:

- **AT-SPI trims every string it reads, and turns a blank one into "no value".** `normalize_value` (`crates/provider-atspi/src/node.rs:774-777`) runs over:
  - names, ids and descriptions;
  - every native string property;
  - action names;
  - the application-node name, seeded on a separate path (`src/lib.rs:317-320`).

  The object-attribute maps drop keys that are blank after trimming. The helper came with the provider's first commit (58004c9) without a documented reason. Its evident purpose needs only an empty check, not a trim: D-Bus has no null string, so an unset property arrives as `""`.
- **UIA drops native string properties that are empty.** `read_uia_property` (`crates/provider-windows-uia/src/map.rs:885-887`) turns `""` into "absent", so `native:` does not show what UIA reported.
- **JAB trims value strings** before parsing them, and returns the trimmed string when a value is not numeric (`crates/provider-java-jab/src/interfaces.rs:255-265`, `node.rs:1128-1140`). Its own comment says "keep the raw string otherwise".
- **The Java agent drops empty strings on the wire** (`SwingElement.putIfPresent`, `java/agent/src/main/java/platynui/agent/SwingElement.java:708-712`), so its native attributes cannot show an empty value either.

The same element therefore answers `@Name`, `@Id`, `@Description` and its `native:` attributes differently depending on the provider, and a locator written against one provider's view can fail on another.

## What Changes

- **No provider rewrites a string value.** Every provider takes each string it reads from the platform exactly as reported: no trimming, no collapsing, no replaced characters.
- **Whether an empty string is a value depends on the attribute group:**
  - Common attributes: `Name` is always present and may be `""`. `Id` and `Description` are present only with a non-empty value. Both rules are unchanged.
  - Pattern attributes are present whenever the element implements the pattern, and `""` is a value. `control:Text` of a label, button or text field with empty text is `""`, and so is an empty `control:Value`.
  - Native attributes are passed through 1:1: `""` stays `""`. A property the platform does not report at all stays absent, as today.
- **AT-SPI:** an empty check without trimming replaces `normalize_value` everywhere:
  - `Name`, `Id` and `Description`;
  - the `Id` object-attribute fallback, which takes the first non-empty attribute unmodified;
  - every native string property and the action names;
  - the object-attribute keys;
  - the application-node name seed.
- **UIA:** native string properties that are `""` are listed with `""`.
- **JAB:** value strings are parsed as numbers without trimming. A non-numeric string stays unchanged, and no value stays no value. The Java API defines these values as `java.lang.Number` (`javax.accessibility.AccessibleValue`), and the bridge only transports them as `Number.toString()`.
- **Java agent:** the wire drops only `null`, not `""`, so native attributes pass an empty value through. `Id` and `Description` keep their non-empty rule on the provider side.
- **Core and platform details:**
  - One-line descriptions show a whitespace-only `Id` (`crates/core/src/ui/describe.rs:38`).
  - A whitespace-only monitor name counts as a name (`crates/platform-windows/src/desktop.rs:131`).
- **Not affected**, because none of them rewrites a reported value:
  - the XPath engine's casting rules, which follow XML Schema;
  - the mock's parsing of numeric lists;
  - log formatting.

**Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking. The release notes name each of them.

On AT-SPI:

- **`Name`, `Id`, `Description` and native string attributes keep their leading and trailing whitespace.** `@Name="Save"` no longer matches an element that reports `Save `, while `normalize-space(@Name)="Save"` does.
- **A whitespace-only value is a value.** A whitespace-only `Id` or `Description` is listed instead of dropped, and a whitespace-only `Name` is no longer empty.
- **Native string attributes that are `""` are listed with `""`** instead of `Null`. Object-attribute keys made of whitespace are kept.

On UIA:

- **Native string properties that are `""` appear** in `native:` listings, snapshots and the Inspector.

Through the Java Access Bridge:

- **A non-numeric value string is no longer trimmed.**

Through the Java agent:

- **Native attributes whose Swing value is `""` are listed with `""`**, for example `native:AccessibleName` and `native:ToolTipText`.

On every provider:

- **One-line descriptions**, as in action log lines, show a whitespace-only `Id`.

## Capabilities

### New Capabilities

- `attribute-values`: how every provider passes attribute values through. It covers that no string is rewritten, when an empty string is a value in each attribute group, and that native attributes are passed 1:1.

### Modified Capabilities

- `id-attribute`: the requirement *Id is the identifier the toolkit reports, taken by source* takes the identifier unmodified. AT-SPI's object-attribute fallback applies only when `Accessible.AccessibleId` is the empty string or unreadable, and it takes the first non-empty attribute unmodified.
- `description-attribute`: the requirement *Strict per-platform source mapping* takes the description unmodified, counts a whitespace-only description as a description, and gains scenarios for AT-SPI.

## Impact

- **Rust:**
  - `crates/provider-atspi`:
    - `src/node.rs`: the resolvers, `pick_attr_value`, `fetch_str`, the action names, `attributes_object` and `string_map_object`.
    - `src/lib.rs`: the application-name seed.
    - The existing `normalize_value`, `pick_attr_value` and `attributes_object` tests change with the rule.
  - `crates/provider-windows-uia/src/map.rs`: `read_uia_property` returns `""` for an empty string. `get_description` keeps its own non-empty filter.
  - `crates/provider-java-jab`: `numeric_or_string` (`src/interfaces.rs`) and the `control:Value` reader (`src/node.rs`).
  - `crates/core/src/ui/describe.rs` and `crates/platform-windows/src/desktop.rs`.
  - `crates/provider-java` needs no mapping change. It publishes what the agent sends, and its `Description` and `Id` keep their non-empty filters.
- **Java agent:**
  - `SwingElement.putIfPresent` drops only `null`.
  - The agent, `crates/java-agent` and `packages/provider-java` carry one version and move together.
- **Python / Robot Framework:** no keyword or API change.
- **Tests:**
  - unit tests on each provider's value decisions (AT-SPI, UIA, JAB) and on the core and platform helpers;
  - JUnit tests on the agent's payload.
  - The X11, compositor and Windows lanes run unchanged. `tests/acceptance/egui/query.robot` already checks `Name`, `Id` and `Description` of the Click Me button on all three.
- **Docs:** `dev-docs/architecture.md`:
  - §5.7 states the rule;
  - §5.5, §5.6 and §6.3 name the presence rules;
  - §7.3 gets a checklist item for providers.
- **Build:**
  - A native rebuild, because every provider is linked into `packages/native`.
  - A JAR rebuild, and `just install-provider-java` before any Python or Robot run.
- **Platforms:**
  - Linux AT-SPI (X11 and the compositor): names, ids, descriptions and native attributes.
  - Windows: UIA native attributes, JAB value strings, and the Java agent's native attributes.
  - The mock and macOS (stub) keep their behavior.
- **Order and coordination:**
  - This change lands before `name-is-accessible-name`. That change currently carries AT-SPI's untrimming of `Name`, `Id` and `Description`, its whitespace scenarios and "unmodified" clauses. Those move here, and `name-is-accessible-name` references `attribute-values` instead.
  - Both changes modify the same requirements of `id-attribute` and `description-attribute`. `name-is-accessible-name` bases its MODIFIED blocks on this change's result.
