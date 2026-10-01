# Design

## Context

See `proposal.md` for the motivation and `specs/` for the behavior. This section records the current code paths and the Swing facts the approach rests on.

**Where `control:Name` comes from today** (verified against the code):

- `Element::display_name()` (`crates/provider-java/src/agent/element.rs:240-252`) is the only source. Both `UiNode::name()` (`node.rs:216-218`) and the listed attribute (`node.rs:289`) call it. It returns the window title, then the payload's `name`, then `accessibleName`.
- `Element::stable_id()` (`element.rs:265-270`) publishes the same `name` as `control:Id` for components and windows.
- `push_native` (`node.rs:532-535`) publishes `name` as `native:ComponentName` for every element kind.

**Where AT-SPI's `Name` comes from today** (verified, `crates/provider-atspi/src/node.rs`):

- `resolve_name` (`:805-814`) reads `Accessible.Name`. When that yields no name, it picks the first of the object attributes `accessible-name`, `name`, `label` and `title` (`pick_attr_value`, `:779-788`).
- It serves both `UiNode::name()` (`:333-335`) and the listed attribute (`:1617-1619`, `:1756`), for every node including application nodes.
- `resolve_id` (`:829-838`) uses `pick_attr_value` for its own, specified fallback (spec `id-attribute`).
- The object attributes are published as `native:Accessible.Attributes` (`:1175`, `attributes_object` `:840`).
- Trimming and the handling of empty strings are settled by `attribute-values-as-reported`, which lands first. This change touches only the fallback.

**The other providers** (verified):

- UIA returns `CurrentName` as is (`crates/provider-windows-uia/src/map.rs:133-135`).
- JAB:
  - It copies the bridge's name buffer (`crates/provider-java-jab/src/node.rs:429`).
  - It lists the bridge's description only as `native:Description` (`node.rs:503`), not in its `control:` attributes (`node.rs:427-450`).
  - It has no `description()` override, so the accessor answers none.
  - Its named lookup for `control:` attributes scans the enumeration (`node.rs:558-559`).
  - The bridge passes Swing's `getAccessibleDescription()` through unchanged (`AccessBridge.java:1426`).
- The Java agent passes Swing's strings through:
  - `accessibleDescription` becomes `control:Description` and the accessor when it is not empty (`crates/provider-java/src/agent/node.rs:231-233`, `:303-305`);
  - `componentName` becomes the `Id`.
- Swing derives the accessible description itself: an explicit value, else the client property, else the tool tip text, else the description of the labelling label (`JComponent.java:3931`).

**What the wire field `name` carries** (verified, `java/agent/src/main/java/platynui/agent/SwingElement.java`):

- components and windows: the explicitly set `Component.getName()` (`:104`, `explicitNameOf` `:410-420`);
- table cells: `String.valueOf(getValueAt(row, column))` (`:213`, `modelValueAt` `:376-386`);
- column headers: `TableColumn.getHeaderValue()` (`:340`);
- list entries and other accessibility-only children: the wrapper's accessible name (`:250`).

**Text today** (verified):

- `textOf` (`:506-530`) is called for components only (`:128`). Virtual children never carry `text`.
- For a non-text-component it reads the `AccessibleText` only when that is an `AccessibleExtendedText` (`:521`).
- `editableOf` (`:541-550`) counts any `AccessibleText` as text-bearing (`:545`). So a Swing label or button with HTML text gets `IsReadOnly` today but no `Text`. A plain-text one gets neither, because Swing gives it no `AccessibleText` (below).
- JAB publishes `Text` and `IsReadOnly` together for every context with the text interface (`crates/provider-java-jab/src/node.rs:447-450`).

**Swing** (verified in the JDK 21 and JDK 8 sources shipped with the provisioned Temurin runtimes):

- `AccessibleJTableCell.getCurrentAccessibleContext()` configures the cell renderer with that cell's value on every call (JDK 21 `JTable.java:8141`).
  - `getAccessibleName()` returns the renderer's accessible name first, then an explicit name, then the client property (`:8185`).
  - `getAccessibleText()` delegates to the renderer's context (`:8455`).
- `AccessibleJTableHeaderEntry` does the same.
  - `getAccessibleName()` finally falls back to `table.getColumnName(column)` (`JTableHeader.java:1020`).
  - `getAccessibleText()` has **no null check** on the renderer context (`:1165`).
- `AccessibleJListChild` behaves likewise: its name comes from the renderer (`JList.java:3297`) and so does its text (`:3433`).
- `AccessibleJFrame` and `AccessibleJDialog` return an explicit accessible name, else the title (`JFrame.java:898`, `JDialog.java:1257`).
- `AccessibleJLabel` implements `AccessibleText` but not `AccessibleExtendedText` (`JLabel.java:1072-1073`), and so does `AccessibleAbstractButton` (`AbstractButton.java:2356-2358`). **Both return it from `getAccessibleText()` only for HTML text**: the method checks the `html` client property and answers null otherwise, in JDK 8 and JDK 21 alike.
  - The renderers of table cells, column headers and list entries are labels or check boxes, so a plain-text item has no `AccessibleText` either.
  - A headless probe on both JDKs confirms it. The label `clicks-0`, the button `Click me`, the cell `r0c0`, the header `col-0` and the list entry `Beta` report none. The HTML label `<html>Hi <b>there</b></html>` reports `\nHi there`, with the line break the HTML document starts with, and an HTML cell, header or list entry reports its text the same way through the item's wrapper.
  - The Boolean column's check box renderer reports neither a name nor a text.
- The JDK's own bridge reads a plain `AccessibleText` character by character with `getAtIndex(AccessibleText.CHARACTER, i)` (`jdk.accessibility/.../AccessBridge.java`, `getAccessibleTextRangeFromContext`).

**Wire format** (verified):

- `Json` writes `Boolean`, `Long` and `Integer` verbatim, and `Double`/`Float` through `Double.toString` (`Json.java:270-277`).
- It writes NaN and infinities as `null`.
- It has no branch for `Short` or `Byte`.

The agent, `crates/java-agent` and `packages/provider-java` carry one version, and the handshake compares it for exact equality.

## Goals / Non-Goals

**Goals:**

- `control:Name` of every agent-served element is Swing's accessible name, and nothing substitutes for it.
- AT-SPI's `control:Name` is `Accessible.Name` alone, without the object-attribute fallback.
- JAB publishes `control:Description` and answers the description accessor, like every other provider that has a description.
- Every value that stops being a name stays readable: `control:Id`, `native:ComponentName`, `native:WindowTitle`, `native:TableCell.ModelValue`.
- The agent reads text from any `AccessibleText`. Cells, column headers and list entries report it where Swing provides one.
- Each field on the wire means one thing.

**Non-Goals:**

- A common `Title` attribute. The title stays `native:WindowTitle`.
- Any change to UIA, and any change to JAB beyond `control:Description`. JAB's renderer-alias defect for cells stays a JAB limitation.
- Model values for column headers, list entries or tree nodes. Header values are labels, and the agent does not read list models.
- `IsReadOnly` for cells, headers and list entries. Their editability stays `native:TableCell.IsEditable`.
- A table or cell pattern. Positions stay `native:TableCell.*`.
- How values are passed through: no trimming, empty strings per attribute group, native attributes one to one. That is `attribute-values-as-reported`, which this change builds on.
- JavaFX and SWT adapters.

## Decisions

1. **`Name` is the payload's `accessibleName`, for every element kind.** `display_name()` loses the title and the `name` precedence and returns `accessibleName`, or the empty string.
   - *Alternative:* keep the title first for windows. Rejected: Swing already derives the accessible name from the title (`JFrame.java:898`), so the title only wins where an application deliberately set a different accessible name. That is exactly the case where the accessible name is the one to trust. The title stays readable.
   - *Alternative:* fall back to the component name when the accessible name is empty. Rejected: `@Name` would then mean the label on one element and the developer id on the next, and the id is already `control:Id`.

2. **Split the wire field `name` into `componentName` and `modelValue`, and drop `name`.**
   - `componentName` is sent for components and windows only, with the same `nameExplicitlySet` rule as today.
   - `modelValue` is sent for table cells only, typed (decision 5).
   - Column headers and list entries send neither. Their name comes from `accessibleName`.
   - *Why rename instead of narrowing `name`:* the field has meant four things. A new name makes every reader that still expects the old meaning fail visibly in review and tests instead of reading the wrong value. The transport check helper `named` (`crates/java-agent/tests/live_fixture.rs:625-627`) is one such reader.
   - Released pairs cannot mix, because of the exact-version handshake.
   - *Alternative:* keep `name` and add `modelValue`. Rejected: `name` would stay ambiguous for headers and list entries.

3. **Virtual items are named and read through Swing's per-item accessible wrapper**, in the same toolkit-thread call that already obtains it (`SwingElement.java:199-204`).
   - The wrapper is what screen readers and the bridge see. It honours an explicit name or a client property set on the item, and for headers the column-name fallback.
   - *Alternative:* call the renderer directly. Rejected: that would re-implement Swing's lookup and miss names set on the wrapper.
   - The Javadoc claim that the accessible view of a cell "is only correct while that renderer happens to be configured for it" (`SwingElement.java:186-192`) holds for the bridge's cached contexts. It does not hold for a read on the toolkit thread, because `getCurrentAccessibleContext()` configures the renderer on every call. The comment is corrected.

4. **One text read for every element, and only what Swing provides.**
   - A text component keeps `getText()`.
   - Otherwise the agent reads the element's `AccessibleText`: `getTextRange(0, count)` when it is an `AccessibleExtendedText`, else `getAtIndex(CHARACTER, i)` for each index, as the JDK's bridge does.
   - An element without an `AccessibleText` carries no `text`. For Swing's labels and buttons and the renderers built on them, that is every plain-text one; only HTML text has a text interface. The value is taken as reported, including the leading line break of the HTML document (capability `attribute-values`).
   - A read that throws counts as "no text interface", so `text` is absent. This covers the unguarded header path (`JTableHeader.java:1165`).
   - Components already pair this with `editable`/`IsReadOnly` through `editableOf` (`:541-550`). After the change `Text` and `IsReadOnly` appear together, as through JAB.
   - *Alternative:* read the displayed text where Swing provides no `AccessibleText`: `JLabel.getText()`, `AbstractButton.getText()`, or the text of the renderer Swing configures for a cell, header or list entry. Rejected by the maintainer: the agent reports what Swing provides and invents nothing, so plain-text elements keep no `Text`, as through the bridge.
   - *Alternative:* read plain `AccessibleText` only for virtual items. Rejected by the maintainer: a label in a cell would have `Text` while a free-standing label would not, and the agent would keep differing from JAB.

5. **The model value is typed once, on the agent side.**
   - `Byte`, `Short`, `Integer` and `Long` travel as a JSON integer (normalized to `Long`, since `Json` has no `Short`/`Byte` branch).
   - Finite `Float` and `Double` travel as a JSON number, and `Boolean` as a JSON boolean.
   - NaN, infinities and every other value travel as `String.valueOf(value)`.
   - A null value is omitted.
   - The provider maps a JSON integer to `UiValue::Integer`, another number to `UiValue::Number`, a boolean to `UiValue::Bool` and a string to `UiValue::String`.
   - The existing `json_scalar` (`node.rs:560-567`) turns every number into `Number`. The model value gets its own mapping so that `r2c0`, `42` and `42.0` stay distinguishable.
   - *Alternative:* always a string. Rejected by the maintainer: typed comparisons (`@native:TableCell.ModelValue > 1000`) are the point of keeping it.

6. **Provider mapping.**
   - `push_native` publishes `native:ComponentName` from `componentName` only, and adds `native:TableCell.ModelValue`.
   - `stable_id()` reads `componentName` and keeps its component/window gate as a second line of defence.
   - `control:Text` needs no provider change: it is already published wherever the payload carries `text` (`node.rs:345-347`).

7. **The fixture's coverage lives in a new `NamesPanel`**, not in `main-table`. A seventh column would break the 100×6 shape that JAB suites address by position (`row * 6 + column + 1`).
   - The `amount` column uses a renderer with a fixed pattern (`#,##0.00` with `Locale.ROOT` symbols), so it displays `1,234.50` on every machine.
   - The `active` column declares `Boolean.class`, so `JTable` uses its own check box renderer.
   - The button sets `setName("namesButton")`, an accessible name of `names-button` and an accessible description of `A button with a developer name`, with a visible label that differs from all three.

8. **AT-SPI's `Name` loses its object-attribute fallback.**
   - `resolve_name` reads `Accessible.Name` alone, taken as `attribute-values-as-reported` already makes it.
   - Its name decision, a pure function since that change, stops consulting the object attributes.
   - `pick_attr_value` stays for the `Id` fallback, which `id-attribute` specifies.
   - *Alternative:* keep `accessible-name` as a second source for `Name`. Rejected: it is not a standard AT-SPI attribute, and any fallback reintroduces a second source for one attribute.

9. **JAB publishes `control:Description`.**
   - `attributes()` lists `Description` from the bridge's description when it is not the empty string.
   - `description()` returns the same value.
   - The named lookup scans the enumeration (`node.rs:558-559`), so enumeration, lookup and accessor agree by construction.
   - `native:Description` stays.
   - *Alternative:* keep the description native only. Rejected by the maintainer: the same Swing application would carry `@Description` through the agent and not through the bridge.

10. **Test layers**, per `dev-docs/testing-strategy.md`:
   - JUnit, headless (the precedent is `SwingTableRowsTest`, which needs no display):
     - the payload of a named button, of a plain-text label and button (no text) and of an HTML label (its text);
     - the cells of a formatted, a boolean, a null and a NaN column;
     - a column header, a plain list entry and an HTML list entry.
   - Rust unit tests: `display_name`, `stable_id` and `push_native` on recorded payloads, including the ModelValue typing. These are free functions and need no session.
   - The provider's live fixture (`#[ignore]`, Windows): the agent-served names button, layered pane, `names-table` and headers, and the stage-1 label and button. Through JAB, the names button is found by `@Name`, and its `Description` agrees across enumeration, lookup and accessor. The live test launches the fixture with a German default locale, which covers the locale scenario without a second launch.
   - An AT-SPI unit test on the name decision: an empty name with the object attributes `accessible-name`, `name`, `label` and `title` gives `""`. The `pick_attr_value` tests stay, because `Id` still uses it.
   - The X11 and compositor lanes: `tests/acceptance/egui/query.robot` already asserts `Name`, `Id` and `Description` of the Click Me button. The Qt, QML and egui suites show whether any locator relied on the fallback.
   - A Robot acceptance suite on the Windows lane (`tests/acceptance/swing/agent_names.robot`, BareMetal, `robot-test-style`). It covers the real-provider scenarios at the locator and attribute level. The companion window comes from a second fixture instance started through `Launch Swing Agent Test App … --app-context --companion-window`. The fixture's default Java 8 runtime needs no `--add-exports` for that mode.

## Risks / Trade-offs

- [A custom renderer without an accessible name gives its cells an empty `Name`] → This is what screen readers get too. `Text`, `native:TableCell.ModelValue` and the positional `native:TableCell.*` attributes still address the cell.
- [Reading the text configures the renderer once more per cell] → Name, description and states already configure it once each (`SwingElement.java:202-204`), so the text read adds one more of several. Measure a full walk of the fixture's 600 cells in the live test before and after.
- [A header renderer that is not `Accessible` makes `getAccessibleText()` throw] → Every text read is guarded, and a failure means "no text interface", logged at debug.
- [Locators on `@Name` with a component name or a model value stop matching] → The release notes name the replacements: `@Id` and `native:TableCell.ModelValue`, or the displayed text.
- [Plain-text cells keep no `Text`, so `Cell.text` keeps failing for them] → It matches what Swing provides and what the bridge reports. A cell is read by its `Name`, which is its displayed text, or by `native:TableCell.ModelValue`.
- [A stale dev JAR after the wire rename] → In dev both sides report the same version, so the handshake cannot notice. The provider would then read no `componentName` and silently lose `@Id`. The tasks run `just install-provider-java` before any Python or Robot run, and the Rust live tests use the freshly built JAR (`PLATYNUI_JAVA_AGENT_JAR`).
- [An AT-SPI toolkit that leaves `Accessible.Name` empty and labels an element only through an object attribute loses its `@Name`] → No toolkit in the fixture set is known to do so. The attributes stay in `native:Accessible.Attributes`, and the X11 and compositor lanes show whether a suite relied on the fallback.
- [JAB elements gain `Description`, so snapshots, `platynui-cli query` and the Inspector list more, and `[@Description]` matches more] → This is intended. It matches the agent for the same application, and the release notes list it.
- [`java-agent-tree-items` takes tree-node names from the model] → Coordination: that change adopts `name-attribute` before it is applied. See the proposal.

## Migration Plan

- **Behavioral, not additive:**
  - for windows served by the Java agent: the name source, `Text` where Swing provides HTML text, and changed `native:` attributes;
  - for AT-SPI elements with an empty `Accessible.Name`;
  - for JAB elements with a description.

  No keyword or API signature changes.
- **Rebuilds:**
  - the native module (`just build-native`), because all three providers are linked into `packages/native`;
  - the agent JAR (`just build-java-agent`), restaged with `just install-provider-java`;
  - the fixture (`just build-test-app-swing`).
- **Landing:** after `attribute-values-as-reported`. The agent, the provider and the fixture land together, since they are version-locked.
- **Rollback:** revert the change's commits, then rebuild the JAR and the native module. No state or configuration persists.
- **For users:**
  - `[@Name="<component name>"]` becomes `[@Id="<component name>"]`.
  - Cell locators on model values move to `native:TableCell.ModelValue` or to the displayed text.

## Open Questions

- How much the extra text read adds to a full table walk. The live test measures it. The approach stands either way, because Swing re-renders per accessible read by design.
  - **Measured** on 2026-10-01 by `live_agent_serves_table_cells_the_bridge_cannot` (Windows, the fixture on Java 8, one run each): a full walk of `main-table` — 701 nodes, the table with its 100 rows and 600 cells — took 87.5 ms before the change and 86.1 ms after it. The extra read does not show, because a plain-text cell's wrapper provides no `AccessibleText`: the read ends after one more renderer configuration.
