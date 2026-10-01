# Proposal

## Why

The user documentation for Robot Framework users (tutorials, guides and examples, later published with Astro/Starlight from `docs/`) needs one application that every reader can install and that behaves the same on Windows and Linux. That application also has to show the everyday cases of desktop testing: windows and dialogs, forms, tables, menus, and work that takes time. The existing apps under `apps/` are test fixtures. Their canonical kebab-case names (`button-basic`, `list-item-3`) are a cross-toolkit test contract, not something a tutorial can teach from. Nothing today reads like a real application, so the docs would have to teach with made-up examples that nobody can run.

## What Changes

- **New product: the demo application "PlatynUI Café"**, a café point-of-sale app. It is built with Qt Quick/QML on PySide6 ≥ 6.9 and ships as the pure-Python package `platynui-demo` (`uv tool install platynui-demo`, command `platynui-demo`). It is a delivered product like `platynui-cli`, not a test fixture, and follows no fixture blueprint.
- **Screens that carry the docs:**
  - The main window is the register. It has a menu bar, a tab bar (Register / Orders / Stock), a category tree, product tiles, an order panel with totals, and a status bar that reports the last action.
  - Customizing a product and paying are in-scene dialogs.
  - The Kitchen Display, the Daily Report and the Settings are separate top-level windows.
  - Two tables:
    - **Orders** (history): select rows by column values; context menu with a refund confirmation.
    - **Stock**: editable cells, and a Reorder checkbox in every row.
  - Optional start screens: a splash screen and a PIN login on a custom keypad.
- **Designed-in teaching cases.** Each case exists on purpose and maps to one topic of the docs:
  - repeated labels that need a narrower scope ("Serve" on every kitchen ticket);
  - work that takes a fixed time (card terminal, brewing, report loading);
  - buttons that only become enabled when the form is valid;
  - values to read and calculate (totals, discount, change);
  - a secret PIN;
  - one tile that is deliberately not exposed to accessibility;
  - a second language (`--lang de`) that changes every visible text but no id;
  - a second instance (`--register 2`).
- **Stable, testable identity.** Every interactive element carries an `Accessible.id` that is the same in every language, surfacing as the common `@Id`, and its visible text as `@Name`. Seed data is deterministic, nothing is persisted, and all delays scale with `--delay-factor`.
- **The launch recipe the docs teach.** An acceptance suite starts the demo exactly as the docs will: `Start Process    platynui-demo`, then the PID read from the application node that owns the main window. On Windows this recipe has to work behind the launcher processes of `uv tool install`.
- **A QML accessibility spike comes first.** It decides the facts the design depends on:
  - whether `Accessible.id` reaches UIA;
  - what shape tables take on UIA and AT-SPI;
  - whether cell text is readable as `@Name` or `@Text`;
  - how custom-drawn tiles surface.

  The spec is adjusted to the findings before the app is built.

## Capabilities

### New Capabilities
- `demo-app`: the PlatynUI Café demo application. It covers distribution and CLI, the screens and flows, stable identity (ids, names, window titles), deterministic data and timing, localization, the table structures, and the launch recipe the documentation relies on.

### Modified Capabilities
- None. The demo is not a blueprint fixture, so `test-app-blueprint` and `qml-test-app` are unchanged.

## Impact

- **Code:**
  - New pure-Python package `packages/demo` (`platynui_demo`: Python models and timers, QML views, bundled assets). The Rust workspace `exclude` list gains it, the uv workspace (`packages/*`) picks it up, and `pyproject.toml` `[tool.mypy] files` adds it.
  - No Rust code, no native bindings, no native rebuild.
- **Dependencies:** `PySide6-Essentials>=6.9` as a runtime dependency of the demo package only. It is not a dependency of `robotframework-PlatynUI` or `platynui-native`.
- **Build and release:**
  - A `just` recipe to run the demo and one to build its wheel.
  - The wheel builds in the existing pure-Python CI wheel job.
  - Versions sync through `scripts/update-git-versions.py`, which already picks up every `packages/**/pyproject.toml`.
- **Tests:**
  - A new acceptance directory `tests/acceptance/demo` (tags `acceptance`, `real`, no platform tag) that pins the demo's contract on every lane: ids, names, window titles, table structure, flows and the launch recipe.
  - The Linux session script and the Windows recipe provision it.
  - The mock lane is untouched.
- **Platforms:**
  - Windows (UIA) and Linux X11 (AT-SPI2).
  - Linux Wayland under the PlatynUI compositor, where the acceptance lanes run.
  - Generic Wayland only as far as the platform support matrix allows.
  - macOS is not a target while its provider is a stub.
- **Dependencies on other work:**
  - The common `@ToggleState` and `@IsSelected` attributes are set today only by the Java providers. UIA and AT-SPI report those states only under `native:`. Until a separate provider change adds them, the demo's acceptance suite asserts checkbox and selection effects through its observables (status bar, button captions, totals), not through these attributes.
  - The user documentation site itself is a separate change and builds on this one.
- **BREAKING:** none. The change adds a package.
