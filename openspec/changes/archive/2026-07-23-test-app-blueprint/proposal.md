## Why

PlatynUI's fixture apps grew scenario-driven (egui: provider smoke, Qt: dialog bounds, Swing: JAB) and each defines its own control set, naming and CLI surface ad hoc. The upcoming semantic keyword layer (`python-library-design.md` §5a: default proxies + shipped technology-specific proxies selected via the `@pattern_proxy_for` registry) needs the opposite: a **technology matrix** — one canonical fixture blueprint that every UI-toolkit test app implements, so the same catalog of pattern behaviors (select, toggle, expand, …) can be proven against Qt Widgets, QML, Swing, SWT, JavaFX and later WPF/Avalonia/Win32, and the differences the suite surfaces become the specification of the shipped per-technology proxies. Two fixture proposals (SWT, JavaFX) are still unimplemented and formable — settling the blueprint now is cheap; retrofitting six divergent fixtures later is not.

## What Changes

- A new capability spec **`test-app-blueprint`** that defines, for every current and future fixture app:
  - the **control catalog** in two tiers — a mandatory core (window, button, checkbox/radio, text field, label, list, tree, combo box, menu bar + context menu + submenu, modal & modeless dialogs) derived from the pattern families of the semantic layer, and an extended tier (table, slider/progress, tabs) fixtures adopt as needed;
  - the **canonical control names** (one shared naming scheme, so one locator set works against every technology);
  - the **observability discipline**: every catalog action has a name-/text-based observable effect (click counters, rename-on-activate) so tests never need screenshots;
  - the **CLI contract** (`--title`, `--auto-close`, `--open-modal`, usage + non-zero exit on unknown args; optional flags like `--app-id` where the platform supports them);
  - the **shared catalog acceptance-suite conventions**: catalog keywords live once in a shared Robot resource, each technology adds a thin `tests/acceptance/<tech>/catalog.robot` that sets its launch variables and declares documented skips for known technology limitations — no variable files, matching the existing `tests/acceptance/<tech>/` layout, robot.toml profiles and per-technology CI lanes;
  - an optional **custom-controls chapter** (hand-built controls with manually wired accessibility, plus a deliberately inaccessible negative case) technologies can implement to probe the lower bound of what default proxies can drive.
- `dev-docs/testing-strategy.md` §5 is rewritten from the current two-tier description (egui smoke / Qt native) to the technology-matrix framing, and a new §5.1 carries the blueprint itself as the developer-facing description (dev-docs are the authoritative convention docs; the capability spec is the acceptance-criteria form of the same contract — kept in sync when the blueprint evolves).
- **No fixture is retrofitted here.** The first full blueprint instance is the QML fixture (`add-qml-test-app`, separate change); the open `add-swt-test-app` / `add-javafx-test-app` proposals are aligned to reference the blueprint; existing fixtures (egui, Qt Widgets, Swing) follow via later per-app retrofit changes.

## Capabilities

### New Capabilities

- `test-app-blueprint`: the fixture blueprint — tiered control catalog with canonical names, observability discipline, CLI contract, shared catalog-suite conventions, and the optional custom-controls chapter that all technology fixture apps implement.

### Modified Capabilities

<!-- none — swing-test-app's requirements are unchanged by this change; the Swing fixture is retrofitted to the blueprint in a later change. -->

## Impact

- **New**: `openspec/specs/test-app-blueprint/**` (via this change's delta), the convention that future fixture changes (`add-qml-test-app`, `add-swt-test-app`, `add-javafx-test-app`, later WPF/Avalonia/Win32 apps and the egui/Qt/Swing retrofits) reference.
- **Modified**: `dev-docs/testing-strategy.md` (§5 technology-matrix rewrite + blueprint pointer).
- **No code, no native rebuild, no CI change** — a conventions/spec change only. The first executable artifacts (shared catalog resource, first catalog suite) land with `add-qml-test-app`. No BREAKING changes; existing fixtures stay valid until their retrofit changes.
- **Depends on**: nothing open. **Unblocks**: `add-qml-test-app` (first full instance), alignment of `add-swt-test-app` / `add-javafx-test-app`, later fixture retrofits, and ultimately the acceptance surface for the semantic keyword layer's shipped technology proxies.
