# Proposal

## Why

`control:Name` is meant to be the accessible name. `dev-docs/architecture.md` §6.3 says so ("Accessible name; may be empty"), and UIA (`CurrentName`) and the Java Access Bridge (`AccessibleContextInfo.name`) follow it. Two providers do not.

**The Java agent** substitutes four other values in `Element::display_name()` (`crates/provider-java/src/agent/element.rs:240-252`):

- **A component's `Component.getName()`** wins over its accessible name. In a Swing dialog whose components carry developer names, every component is then named by its developer id. That id is already the node's `control:Id` (spec `id-attribute`), so it appears twice and the label the user sees is gone from `@Name`.
- **A window's title** wins over an accessible name set explicitly on the window.
- **A table cell's model value** (`String.valueOf(getValueAt(...))`) wins over what the cell displays. For a formatted number, a date or a boolean column, `@Name` is not the text on screen.
- **A column header's `TableColumn` header value** wins over the header's accessible name.

**AT-SPI** fills an empty `Accessible.Name` from the object attributes `accessible-name`, `name`, `label` or `title` (`crates/provider-atspi/src/node.rs:805-814`). This heuristic dates from the provider's first implementation (58004c9), and no spec covers it.

The description has a similar gap:

- The Java Access Bridge reports a description but publishes it only as `native:Description`.
- The `description-attribute` spec names no Java source at all, although the agent already publishes `control:Description`.

That values are taken as the platform reports them, without trimming, is not this change's concern. `attribute-values-as-reported` establishes it for every attribute and lands first. This change builds on it.

No spec pins what `Name` is, so nothing caught this. There are two more consequences:

- The same Swing application answers `@Name` and `@Description` differently depending on whether the bridge or the agent serves it.
- The agent reads text only from an `AccessibleExtendedText`. Where Swing provides a plain `AccessibleText`, as it does for HTML text in labels, buttons and renderers, the bridge reports the text and the agent does not.

## What Changes

- **`Name` is the accessible name the toolkit reports, and nothing else.** With no accessible name, `Name` is empty. No other value stands in for it: not the developer id, an object attribute, the window title, a model value or a class name. The value is taken as reported (capability `attribute-values`). Application nodes keep their own rule (spec `application-process-attributes`).
  - UIA: `CurrentName`, unchanged.
  - AT-SPI: `Accessible.Name`. The object-attribute fallback goes. The attributes stay readable as `native:Accessible.Attributes`.
  - JAB: the bridge's accessible name, unchanged.
  - Java agent: Swing's own `getAccessibleName()`, for every element kind.
- **JAB publishes `control:Description`** from the bridge's accessible description, present only when it is not empty, and its node answers the description accessor with it. The spec names both Java sources: the bridge's description, and Swing's `getAccessibleDescription()` through the agent.
- **The Java agent names every element by Swing's accessible name.**
  - Components: the accessible name. `Component.getName()` stays `control:Id` and is no longer the name.
  - Windows: the accessible name. Swing already falls back to the title inside `AccessibleJFrame`/`AccessibleJDialog`. The title stays readable as `native:WindowTitle`.
  - Table cells, column headers and list entries: the accessible name of Swing's per-item wrapper (`AccessibleJTableCell`, `AccessibleJTableHeaderEntry`, `AccessibleJListChild`). Each wrapper configures the renderer for exactly that item on every read, so in-process the name is the displayed text.
- **The agent reads an element's text from every `AccessibleText` Swing provides, plain or extended**, the way the JDK's own bridge does. Today it reads only `AccessibleExtendedText`.
  - Swing's labels and buttons, and the renderers built on them, provide a plain `AccessibleText` only for HTML text. Plain-text labels, buttons, check boxes, radio buttons and menu items, and the table cells, column headers and list entries their renderers draw, have no text interface and carry no `control:Text`. The agent invents none: it does not read a component's or a renderer's own text.
  - Cells, column headers and list entries report the `AccessibleText` of Swing's per-item wrapper when it provides one, as for an HTML renderer. The existing `TextContent` pattern carries it, so no new pattern is needed.
- **The table model value stays readable** as a typed `native:TableCell.ModelValue`: an integer, a number or a boolean, otherwise a string.
- **`native:ComponentName` carries only a component name.** The overloaded wire field `name` is split. Today a cell reports its model value as `native:ComponentName`.
- **The Swing fixture gains** a control whose component name differs from its accessible name and which carries an accessible description, and a small table whose displayed text differs from its model values.

**Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking. The release notes name each of them.

Through the Java agent:

- **`@Name` of a component with a developer name is its accessible name.** A locator `[@Name="okButton"]` that matched the component name no longer matches. `[@Id="okButton"]` does.
- **A component with a component name but no accessible name has an empty `@Name`.** Examples are Swing's own `null.contentPane` and `Spinner.nextButton`. Their `@Id` is unchanged.
- **A window whose accessible name differs from its title is named by the accessible name.** The title stays readable as `native:WindowTitle`.
- **`@Name` of a table cell or column header is its displayed text.** It is no longer the model or header value. The two differ for formatted, non-string and custom-rendered columns. The model value moves to `native:TableCell.ModelValue`.
- **Elements whose text Swing exposes as HTML gain `control:Text`**: labels, buttons and menu items with HTML text, and cells, column headers and list entries whose renderer draws HTML. The value is what Swing's `AccessibleText` reports, which starts with the line break of the HTML document. Plain-text elements stay without `control:Text`, as through the bridge.
- **`native:ComponentName` disappears from cells, column headers and list entries.**

Through the Java Access Bridge:

- **Swing elements gain `control:Description` and a description accessor** (`${el.description}`) wherever the bridge reports a description. Until now it was readable only as `native:Description`.

On AT-SPI:

- **An element whose `Accessible.Name` is empty has an empty `@Name`.** It no longer takes its name from the object attribute `accessible-name`, `name`, `label` or `title`. This holds for application nodes too. The attributes stay readable as `native:Accessible.Attributes`.

## Capabilities

### New Capabilities

- `name-attribute`: what `Name` is on UIA, AT-SPI, the Java Access Bridge and the Java agent, and where each takes it from. It may be empty and is never substituted.

### Modified Capabilities

These build on the text `attribute-values-as-reported` gives the same requirements.

- `id-attribute`: the scenario *A table cell has no Id although the agent reports a name for it* of the requirement *Id is the identifier the toolkit reports, taken by source* no longer claims that the cell's name is its model value.
- `description-attribute`: the requirement *Strict per-platform source mapping* names the sources of the Java Access Bridge and the Java agent, including the description Swing derives from a tool tip, and gains scenarios for both Java backends.
- `textcontent-pattern`: the requirement *TextContent exposes an element's text as a read-only Text attribute* names the Java agent's source: a text component's document, otherwise the `AccessibleText` Swing provides, which for table cells, column headers and list entries is the item's. Swing's plain-text labels, buttons and renderers provide none.
- `swing-test-app`: a new requirement for the fixture additions: a control with differing component and accessible names and an accessible description, and a table whose displayed text differs from its model values.

## Impact

- **Java agent** (`java/agent`, `SwingElement.java`):
  - The payload splits `name` into `componentName` (components and windows only) and `modelValue` (table cells only, typed).
  - It reads `text` from any `AccessibleText`, including a plain one. Swing's labels and buttons (`AccessibleJLabel`, `AccessibleAbstractButton`) and the renderers built on them return a plain one for HTML text only. It reports that text for components and, new, for cells, column headers and list entries.
  - The agent, `crates/java-agent` and `packages/provider-java` carry one version and move together. The handshake still compares versions for exact equality.
- **Rust:**
  - `crates/provider-java/src/agent/element.rs`: the payload fields, `display_name()` (the accessible name only), `stable_id()` (reads `componentName`) and the unit tests.
  - `crates/provider-java/src/agent/node.rs`: `push_native` publishes `ComponentName` from `componentName` only, and `TableCell.ModelValue`. `control:Text` is already published wherever the payload carries `text`.
  - `crates/provider-atspi/src/node.rs`: `resolve_name` loses its object-attribute step. `pick_attr_value` stays for the `Id` fallback.
  - `crates/provider-java-jab/src/node.rs`: `control:Description` from the bridge's description when it is not empty, and a `description()` accessor that returns it. The named lookup follows the enumeration, so the two agree.
  - UIA needs no code change.
- **Python / Robot Framework:** no keyword or API change. `description` starts working for bridge-served elements. `Cell.text` and `Item.text` work for agent-served cells only where the renderer draws HTML; a plain-text cell has no text interface, through the agent as through the bridge.
- **Tests:**
  - JUnit tests for the payload.
  - Rust unit tests for the Java mapping, for JAB's description, and for AT-SPI's name decision without the fallback.
  - The provider's live fixture checks (`crates/provider-java/tests/live_fixture.rs`), for both Java backends.
  - The transport check helper `named` (`crates/java-agent/tests/live_fixture.rs:625`), which reads `name`.
  - A Robot acceptance suite for the Swing agent on the Windows lane.
  - The documentation of `tests/acceptance/swing/agent_table.robot`.
  - The X11 and compositor lanes, whose egui suite already checks `@Name`, `@Id` and `@Description` of a named button.
- **Docs:**
  - `architecture.md` §5.6 and §6.4: the sources of `Name` and `Description` for AT-SPI and both Java backends, with Java rows in the §5.6 table.
  - The `TablePanel` Javadoc and the fixture README, which say "names are model values".
  - The Rust and Java doc comments on the payload and on `resolve_name`.
- **Build:**
  - A native rebuild, because all three providers are linked into `packages/native`.
  - A JAR rebuild, and `just install-provider-java` before any Python or Robot run.
- **Platforms:**
  - Linux AT-SPI (X11 and the compositor) changes for elements with an empty `Accessible.Name`.
  - Windows changes for windows served by the Java agent, and for `Description` through the bridge. The agent backend is platform-neutral.
  - UIA keeps its behavior.
- **Order and coordination:**
  - This change lands after `attribute-values-as-reported`. It relies on that change for values taken as reported (no trimming, native attributes one to one), and its `id-attribute` and `description-attribute` blocks start from that change's text.
  - `java-agent-tree-items` (open, not applied) takes tree-node names from the model (its design decision 1 and its `java-provider` delta). Under `name-attribute`, a tree node's `Name` is its accessible name, and its text comes through `TextContent`. That change has to adopt this before it is applied.
  - A later JavaFX or SWT adapter extends `name-attribute` and `description-attribute` with its toolkit's sources.
