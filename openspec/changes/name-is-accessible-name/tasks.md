# Tasks

Prerequisite: `attribute-values-as-reported` is applied. This change builds on its value handling and on its text for `id-attribute` and `description-attribute`.

## 1. Fixture

- [ ] 1.1 Add `NamesPanel` to `apps/test-app-swing` and wire it into `Main` after `TablePanel`, as `swing-test-app` *Name-source coverage* and design decision 7 describe:
  - a titled panel `names-panel`;
  - the button `names-button` (`setName("namesButton")`, accessible description `A button with a developer name`, visible label `Named`);
  - the read-only table `names-table`:
    - `amount` holds the `Double` `1234.5`, rendered with the fixed `#,##0.00`/`Locale.ROOT` format;
    - `active` holds the `Boolean` `true` in a column of class `Boolean.class`.

  Verify: `just build-test-app-swing` succeeds, and a manual `just run-test-app-swing` shows `1,234.50` and a checked check box.
- [ ] 1.2 Document the panel in the fixture README's control table and in the class Javadoc, keeping the "fixed accessible names" convention. Verify: the README names `names-panel`, `names-button`/`namesButton` with its description, and `names-table` with its two values.

## 2. Tests first

Each test in this group fails until groups 3 and 4 land.

- [ ] 2.1 Write `tests/acceptance/swing/agent_names.robot`, following the `robot-test-style` skill and the BareMetal keywords of `testapp_agent.resource`. It covers the real-provider scenarios of `name-attribute`, `description-attribute`, `textcontent-pattern` and `swing-test-app` for the agent:
  - the names button: `@Name`, `@Id`, `@Description` and `native:ComponentName`, and that `[@Name="namesButton"]` matches nothing;
  - the layered pane: `[@Id="null.layeredPane"][@Name=""]`;
  - the `names-table` cells: `Name`, `Text` and `native:TableCell.ModelValue` (number, boolean);
  - a `main-table` cell's `native:TableCell.ModelValue` (`r2c0`) and the absence of `native:ComponentName` on it;
  - the `amount` and `col-1` headers: `Name` and `Text`;
  - the stage-1 label and button: `Text`;
  - the main frame's `Name`, equal to its title;
  - a second instance started with `--app-context --companion-window`, whose companion window has `Name` = `companion-window` and `native:WindowTitle`.

  Verify: `robotcode analyze` reports no errors for the suite.
- [ ] 2.2 Add JUnit tests (`java/agent/src/test/java/platynui/agent/`, headless like `SwingTableRowsTest`) on the payload of `SwingElement.describe`:
  - a `JButton` with `setName`, an accessible name and an accessible description gives `componentName`, `accessibleName` and `accessibleDescription`, and no `name` key;
  - a component with only a tool tip gives that tool tip as `accessibleDescription`;
  - a `JLabel` and a `JButton` without extended text get `text` from their plain `AccessibleText`;
  - the cells of a formatted `Double` column give `accessibleName`, `text` and `modelValue` as a `Double`;
  - a `Boolean` column gives an empty name, `text` `""` and `modelValue` `true`;
  - `Short` and `Integer` columns give `modelValue` as a `Long`;
  - a null cell gives no `modelValue`, and a NaN cell gives `modelValue` `"NaN"`;
  - a column header gives `accessibleName` and `text`;
  - a `JList` entry gives `accessibleName` and `text` `Alpha`.

  Verify: `just test-java-agent` fails exactly on the new tests.
- [ ] 2.3 Add Rust unit tests in `crates/provider-java/src/agent/element.rs` and `node.rs` on recorded payloads with the new fields:
  - `display_name()` returns `accessibleName` for:
    - a component carrying `componentName`;
    - a window whose accessible name differs from its title, and an untitled one;
    - a cell carrying `modelValue`;
    - a header and a row, the row giving `""`.
  - `stable_id()` reads `componentName`, and returns none for a cell.
  - `push_native` lists `ComponentName` only with `componentName`, and maps `TableCell.ModelValue` to `Integer`, `Number`, `Bool` and `String` (`"NaN"`), or omits it when it is absent.
  - A padded `accessibleName` and `accessibleDescription` pass through unchanged.

  Replace the test `component_get_name_wins_over_the_accessible_name`. Verify: `just test-crate platynui-provider-java` fails exactly on the new expectations.
- [ ] 2.4 Extend the live checks in `crates/provider-java/tests/live_fixture.rs`:
  - agent:
    - launch the fixture with `-Duser.language=de -Duser.country=DE`;
    - assert the values of 2.1 for the names button, layered pane, `names-table` cells and headers and the stage-1 label and button;
    - assert that no cell lists `native:ComponentName`;
    - time a full walk of `main-table` and print it for 6.4.
  - JAB:
    - `@Name="names-button"` finds the names button;
    - its `Description` is `A button with a developer name` through enumeration, named lookup and the description accessor.

  Update `named` in `crates/java-agent/tests/live_fixture.rs:625-627` to read `accessibleName`. Verify: `just check` compiles both test targets.
- [ ] 2.5 Add a unit test in `crates/provider-atspi/src/node.rs` on the name decision, the pure function `attribute-values-as-reported` introduced: an empty `Accessible.Name`, with object attributes carrying `accessible-name`, `name`, `label` and `title`, gives `""`. Replace the expectation that pinned the fallback. Verify: `just test-crate platynui-provider-atspi` fails exactly on it.
- [ ] 2.6 Add a unit test in `crates/provider-java-jab` on the description decision: the bridge's empty string gives none, and `  Closes the dialog  ` is kept unchanged. Verify: `just test-crate platynui-provider-java-jab` fails on it.

## 3. Java agent

- [ ] 3.1 Make `textOf` read any `AccessibleText` (design decision 4):
  - through `getTextRange(0, count)` for an `AccessibleExtendedText`;
  - otherwise through `getAtIndex(AccessibleText.CHARACTER, i)` for each index.

  A throwing read counts as no text interface. Verify: the label and button tests of 2.2 pass.
- [ ] 3.2 In `describeComponent`, report the explicit component name as `componentName` instead of `name`. Verify: the button and tool tip tests of 2.2 pass.
- [ ] 3.3 In `describeVirtual` and `describeColumnHeader`:
  - drop `name`;
  - report `text` from the item's wrapper for cells, headers and list entries;
  - report the typed `modelValue` for cells (design decision 5: integral → `Long`, finite floating → `Double`, `Boolean`, otherwise `String.valueOf`, null omitted).

  Correct the Javadoc claims about the renderer alias and about the model value as the name. Verify: `just test-java-agent` passes all tests.

## 4. Providers and delivery

- [ ] 4.1 In `element.rs`:
  - replace `name` with `component_name` (`componentName`) and `model_value` (`modelValue`, a JSON scalar);
  - make `display_name()` return `accessible_name` or `""`;
  - make `stable_id()` read `component_name` and keep its kind gate;
  - update the doc comments.

  Verify: the `element.rs` tests of 2.3 pass.
- [ ] 4.2 In `node.rs`, make `push_native` publish `ComponentName` from `component_name` only, and add `TableCell.ModelValue` with its own typed mapping, `i64` → `Integer` before `f64` → `Number`. Verify: `just test-crate platynui-provider-java` passes.
- [ ] 4.3 In `crates/provider-atspi/src/node.rs` (design decision 8), drop the object-attribute step from the name decision, keep `pick_attr_value` for `Id`, and update the doc comment of `resolve_name`. Verify: `just test-crate platynui-provider-atspi` passes.
- [ ] 4.4 In `crates/provider-java-jab/src/node.rs` (design decision 9):
  - list `control:Description` from the bridge's description when it is not empty;
  - add a `description()` accessor that returns the same value;
  - keep `native:Description`.

  Verify: `just test-crate platynui-provider-java-jab` passes.
- [ ] 4.5 Rebuild and deliver in this order:
  1. `just build-java-agent`
  2. `just install-provider-java`
  3. `just build-native`

  Verify: `just test-java-agent-live` passes, including the updated `named` helper, and `uv run python -c "import platynui_native"` loads the rebuilt module.

## 5. Docs

- [ ] 5.1 Update `dev-docs/architecture.md`:
  - §5.6: add rows for the Java Access Bridge (the bridge's description) and the Java agent (Swing's `getAccessibleDescription()`, which Swing derives from the tool tip when nothing else is set).
  - §6.4: after the Java `Id` sentence, state the `Name` sources:
    - AT-SPI: `Accessible.Name`, with no object-attribute fallback;
    - the Java Access Bridge: its accessible name;
    - the Java agent: Swing's `getAccessibleName()` for every element, with the title in `native:WindowTitle` and the cell model value in `native:TableCell.ModelValue`.

  Verify: both sections name every source and no fallback, consistent with §5.7 as `attribute-values-as-reported` left it.
- [ ] 5.2 Update the statements that call a cell's name its model value:
  - the `TablePanel` Javadoc;
  - the fixture README table row;
  - the `agent_table.robot` documentation;
  - the comments in `live_fixture.rs:900-947`.

  Verify: `rg -n "name is (the|its) model value|names are model values"` finds nothing outside `openspec/changes/archive`.
- [ ] 5.3 Align `java-agent-tree-items` with `name-attribute`. Its design decision 1 and its `java-provider` delta must take a tree node's `Name` from the node's accessible name and its text through `TextContent`, instead of names from the model. Verify: `openspec validate java-agent-tree-items --strict` passes.

## 6. Verification

- [ ] 6.1 Run `just pre-commit`, then `just build-native`, because pre-commit leaves a mock build. Verify: all gates pass.
- [ ] 6.2 Run the Windows acceptance lane:
  1. `just install-provider-java`
  2. `just test-acceptance-windows`, with `PYTHONIOENCODING=utf-8` set for the Robot run

  Verify:
  - the provider's live checks for the agent and JAB pass;
  - `agent_names.robot`, `agent_table.robot` and all other Swing suites pass;
  - the run shows no WARN or ERROR.
- [ ] 6.3 Run the Linux acceptance lanes `just test-acceptance-x11` and `just test-acceptance-compositor`, with `PYTHONIOENCODING=utf-8` set for the Robot runs. Verify:
  - `tests/acceptance/egui/query.robot` passes its `Name`, `Id` and `Description` checks;
  - the Qt, QML and egui suites pass;
  - the runs show no WARN or ERROR.
- [ ] 6.4 Record the full-walk timing from 2.4 in `design.md` (Open Questions). Run `openspec validate name-is-accessible-name --strict`. Verify: the validation passes and the timing is written down.
