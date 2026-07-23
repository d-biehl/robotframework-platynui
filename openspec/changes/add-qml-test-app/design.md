## Context

`test-app-blueprint` defines the fixture contract (tiered catalog, canonical names, observables, CLI, shared-suite conventions) but ships no code; this change builds its first full instance. Qt Quick is technologically distinct from the existing Qt Widgets fixture: scene-graph rendering, accessibility from `Accessible` attached properties routed through the same Qt bridge (UIA on Windows, AT-SPI on Linux), and in-scene popups/dialogs by default. PySide6 is already a project dev dependency and the Qt Widgets fixture (`apps/test-app-qt`) established the Python-fixture mechanics: PEP 723 `main.py`, Cargo `exclude`, mypy/ruff coverage, `PLATYNUI_TEST_APP_QT_*` launch wiring on both lanes.

## Goals / Non-Goals

**Goals:**

- A blueprint-conforming QML fixture (core tier + custom-controls chapter) with verified `@Name` surfacing on UIA and AT-SPI.
- Both popup realities (in-scene default, native via Qt ≥ 6.8) and both dialog faces (native child `Window`, in-scene modal `Dialog`).
- The canonical catalog test set as reference implementation (self-contained suite), onboarded for QML on the Windows and Linux lanes.

**Non-Goals:**

- No extended-tier controls (table, slider/progress, tabs) — follow-up change, additive under the blueprint's no-rename rule.
- No retrofit of other fixtures and no changes to the existing Qt Widgets suites.
- No macOS lane; Wayland only inherits what the Qt Widgets lane already supports.
- No provider or library code changes — findings about bridge gaps become documented skips/deviations feeding the later proxy work, not fixes here.

## Decisions

1. **Python/PySide6, mirroring `apps/test-app-qt`'s mechanics.** Thin `main.py` (PEP 723 metadata, argparse, `QQmlApplicationEngine`), QML files beside it carrying the UI — the split real QML apps have. Same integration set as the Qt Widgets fixture: root `Cargo.toml` `exclude`, `[tool.mypy] files` entry, PySide6 from the existing dev group, no separate venv. *Alternative:* a Rust `qmetaobject`/CXX-Qt fixture — rejected; it would compile Qt into the workspace and contradict the established Python-fixture path.

2. **Separate app `apps/test-app-qml`, not a `--qml` mode of `test-app-qt`.** The accessible trees, popup mechanics, and future proxy routing differ fundamentally; one fixture per technology row keeps the matrix and the naming convention (`test-app-<tech>`) clean.

3. **Catalog from QtQuick.Controls, names via `Accessible.name`.** Controls (Button, CheckBox, RadioButton, TextField, ComboBox, ListView delegates, TreeView, MenuBar/Menu, Dialog) get explicit `Accessible.name` set to the canonical names — auto-derived names (button text) are not relied on, matching the blueprint's explicit-name discipline. Roles come from Controls' built-in accessibility; the custom chapter sets `Accessible.role` manually. Every encoded name/role is first read back from a real tree (Inspector / `Get Attribute`) per the testing strategy — Qt Quick's bridge output, not the QML source, is the contract.

4. **Popup modes via `--popup-mode {inscene,native}`, default in-scene.** In-scene is Qt Quick's default and the case the Widgets fixture cannot cover (popups as scene items — the hard bounds/hit-test case); `native` flips `popupType`/native menus (Qt ≥ 6.8, satisfied by the pinned PySide6) so the same catalog runs against native popup windows. One flag rather than doubled menus keeps names identical in both modes; the catalog suite runs against the default, a QML-specific suite re-runs menu coverage in native mode.

5. **Dialogs deliberately split across both faces.** `dialog-modeless` = child `Window` (native top-level, the analogue of the Widgets fixture's parented `QDialog`); `dialog-modal` = in-scene modal `Dialog` (Quick's overlay reality). This covers the two ways real QML apps build dialogs and probes exactly where the bridge hangs each one in the tree. Should modal state not surface, the blueprint's deviation rule (README + documented skip + fallback observable) applies — that finding is fixture output, not a blocker.

6. **Shared catalog resource lands here, shaped against two consumers.** The keywords are written technology-neutral (canonical names only, launch config injected by the thin per-tech suite) and validated by onboarding QML while keeping the wording usable for the aligned SWT/JavaFX proposals. Launch/session plumbing reuses the existing per-tech resource pattern (`tests/acceptance/qt/resources/testapp.resource` as model) with `PLATYNUI_TEST_APP_QML_PYTHON` / `PLATYNUI_TEST_APP_QML_MAIN` wired by `scripts/platynui-robot-session.sh` (Linux) and `test-acceptance-windows` (Windows) exactly like the Qt Widgets fixture.

## Risks / Trade-offs

- [Qt Quick's accessibility bridge is historically thinner than Widgets' — states or roles may be missing, differently named, or platform-divergent] → that discovery is the fixture's purpose; the blueprint's documented-skip/deviation mechanism turns gaps into tracked findings instead of red lanes. Encode only reality-verified values.
- [In-scene popups may be exposed event-driven on AT-SPI (like the Widgets fixture's transient `QMenu` case) and be invisible to top-down walks] → known risk, already tracked for Widgets (`atspi-event-driven-tree`); affected scenarios become documented skips on Linux rather than blocking the lane.
- [First blueprint contact may force spec adjustments] → intended feedback loop; both changes are open simultaneously and `test-app-blueprint` tasks include reconciling wording before archive.
- [Two lanes from day one doubles first-run verification effort] → accepted per decision; the platform-divergence data is the value of the matrix row.

## Migration Plan

Purely additive: new app directory, new suites, env wiring beside the existing Qt Widgets entries. Rollback = remove the directory, suites, and wiring lines. No existing suite or fixture changes behavior.

## Open Questions

- Whether `TreeView` (Quick) exposes expandable rows usably on both bridges or the tree control needs `TreeViewDelegate` tweaks — resolve during implementation against the real trees; worst case the tree rows' expand coverage starts as a documented skip on one platform.
- Whether the native popup mode is reliably available on the Linux lane's Qt platform plugins (xcb/wayland) or stays a Windows-first assertion with a documented skip on Linux — verify at implementation.
