# PlatynUI Swing test app (Java 8)

A Java Swing test/fixture application for the **Java Access Bridge (JAB) provider**
work on Windows (OpenSpec change `add-jab-provider`), mirroring the role of
[`apps/test-app-qt`](../test-app-qt) for the native-widget tier in
[`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5. Swing
implements no UIA provider on Windows, so this app is only reachable through JAB —
which is exactly what the provider needs to exercise.

It is **plain Java 8**, not a Cargo crate (so it is `exclude`d from the workspace in
the root `Cargo.toml`, like the Qt app) and deliberately has **no Maven/Gradle** —
`javac` driven by `just` recipes is the whole build. A JDK 8+ on `PATH` is the only
prerequisite. The sources are platform-neutral: the same app will later serve a
Linux acceptance lane through `java-atk-wrapper` against the existing AT-SPI2
provider.

## Build & run

```sh
just build-test-app-swing      # javac -encoding UTF-8 -source 8 -target 8 → build/classes
just run-test-app-swing        # launches with JAB enabled for this process only (Windows)

# Options mirror the Qt/egui apps:
just run-test-app-swing --title "My Swing Window"
just run-test-app-swing --auto-close 10       # self-close for CI
just run-test-app-swing --dialogs 1           # reserved for stage 4 (accepted no-op)
just run-test-app-swing --open-modal          # reserved for stage 4 (accepted no-op)
```

### Accessibility enablement (Windows)

The run recipe passes

```
-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge
```

which activates the Java Access Bridge **for this process only** — no `jabswitch`,
no persistent machine or user configuration, nothing written to
`%USERPROFILE%\.accessibility.properties`. Only JDK-own code is loaded into the JVM
(the same mechanism screen readers use), which is the project's security stance:
the automation side never instruments the target app.

Manual fallback for foreign apps you don't launch yourself: `jabswitch -enable`
(per-user, persistent; ships with the JDK).

On Linux (future acceptance lane) the equivalent is the `java-atk-wrapper`
(distro package, e.g. `libatk-wrapper-java`), enabled via
`-Djavax.accessibility.assistive_technologies=org.GNOME.Accessibility.AtkWrapper`.

## Accessible names — the locator contract

JAB has **no AutomationId equivalent** and `Component#setName` is not exposed
out-of-process, so every interactive control carries an explicit, unique
`accessibleName`. These names are the locator anchors for all tests:

| Control | Accessible name |
|---|---|
| Frame | tracks the window title (default "PlatynUI Swing TestApp") |
| Menu bar / File / Exit / Help / About | `main-menubar`, `menu-file`, `menu-file-exit`, `menu-help`, `menu-help-about` |
| Stage 1 panel / button / text field | `stage1-panel`, `stage1-button`, `stage1-textfield` |
| Status label (click observable) | `stage1-status-clicks-<n>` — starts at `clicks-0`; each click of `stage1-button` sets text and name suffix to `clicks-<n>` |
| Stage 2 panel | `stage2-panel` |
| Checkbox / radios | `stage2-checkbox`, `stage2-radio-a`, `stage2-radio-b` |
| Combo / slider / spinner / progress | `stage2-combo`, `stage2-slider`, `stage2-spinner`, `stage2-progress` |
| Table panel / scroll pane / table | `table-panel`, `table-scroll`, `main-table` |
| Table cells | content `r<row>c<col>` (4×3 grid, row 2 preselected) — **not** name-addressable: the JDK bridge aliases all JTable cells to the shared renderer, so cell names are volatile; locate cells by row-major child position |

## Growth rules

The app grows panel by panel (the table panel carries the JAB
interface-attribute work; tabs/tree may follow; dialogs/popups —
`--dialogs`/`--open-modal` are already reserved; dynamic content later). Two
rules keep selectors stable across growth:

1. **Existing accessible names never change.** New stages add names, they never
   rename or reuse existing ones.
2. New stages live in their own titled panel appended below the existing ones, so
   the tree position of existing controls stays put.
