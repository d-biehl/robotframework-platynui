# Proposal

## Why

The fixture matrix has no GTK app, although GTK is the toolkit of most Linux desktop applications, and GTK 3 and GTK 4 are two different accessibility technologies:

- GTK 3 reaches AT-SPI through ATK and its bridge.
- GTK 4 implements AT-SPI itself.
- They differ in their tree shapes and in what their coordinate types mean. GTK 3 answers parent-relative extents with screen values. GTK 4 sends no screen position at all.

`linux-element-geometry` cannot prove its placement rules for either without a fixture: its GTK scenarios use these fixtures as their first tests. The semantic keyword layer will also need GTK proxies proven against real GTK trees.

## What Changes

- **Two new fixtures, `apps/test-app-gtk3` and `apps/test-app-gtk4`**, Python with PyGObject, each conforming to `test-app-blueprint`:
  - The core tier and the extended tier's slider, progress bar and tab control, built from the standard widgets of that GTK version. GTK 4 uses only widgets not deprecated in 4.10, unless a current widget provably does not expose the blueprint's names or states. Then the deprecated one is used, and the README documents why. The extended tier's table and the custom-controls chapter are not implemented.
  - The blueprint CLI: `--title` (defaults `PlatynUI GTK3 TestApp` and `PlatynUI GTK4 TestApp`), `--auto-close`, `--open-modal`, `--app-id`, and a failure for an unknown argument.
  - Window naming follows the blueprint's fallback. The main window is matched by its launch title, and dialogs carry their canonical names as titles.
  - What `linux-element-geometry` needs from them:
    - the main window draws a header bar as its title bar;
    - `groupbox-basic` holds the radio buttons;
    - `dialog-modeless` has a different size from the main window.
- **PyGObject in the project environment.**
  - `PyGObject` joins the dev dependency group with a Linux-only marker.
  - `uv sync` builds it from source, because PyPI ships no Linux wheels. Every Linux development machine and every Linux CI job that syncs the environment therefore needs its build dependencies: a C compiler, `pkg-config`, `libgirepository-2.0-dev` (GLib ≥ 2.80) and `libcairo2-dev`.
  - Running the fixtures also needs GTK 3, GTK 4.18 or later, and their introspection data (`gir1.2-gtk-3.0`, `gir1.2-gtk-4.0`).
  - Windows and macOS do not install it.
- **Lane wiring on both Linux lanes.**
  - `tests/acceptance/gtk3/` and `tests/acceptance/gtk4/` each hold:
    - the self-contained `catalog.robot` with the blueprint's canonical test set;
    - `modal.robot` for the instance started with `--open-modal`.
  - `scripts/platynui-robot-session.sh` hands the fixtures over as `PLATYNUI_TEST_APP_GTK3_PYTHON`/`_MAIN` and `PLATYNUI_TEST_APP_GTK4_PYTHON`/`_MAIN`. The suites launch the venv interpreter directly, so `@ProcessId` pins the fixture's own process.
  - A fixture that cannot start because PyGObject or a GTK typelib is missing fails the suite with the package that fixes it.
- **A Linux-only platform tag.** The GTK suites carry `platform:linux`, which the Windows lane excludes and both Linux lanes keep. The tag vocabulary had no way to say "both Linux lanes, not Windows".
- **GTK 4.18 or later.** The GTK 4 fixture supports no older release, so its suites need no version tag or version check. CI's runner image, Ubuntu 26.04, ships GTK 4.22.
- **Popover menu items named by the fixture.**
  - GTK 4.18 to 4.24.1 exposes popover menu items without accessible names (fixed in GTK commit fa085fe1 for 4.25, backported as 361abc89 for 4.24.2).
  - The GTK 4 fixture names every menu and context-menu item by its label each time its popover opens, so the catalog finds every item by its canonical name, as with every other fixture.
- **CI.**
  - The Linux jobs that sync the environment install the PyGObject build dependencies.
  - The acceptance job also installs the GTK runtime and typelibs.
  - The failure log dump includes the GTK fixtures' logs.
- **Docs:**
  - `dev-docs/testing-strategy.md` §5 (the matrix rows) and §2.6 (`platform:linux`);
  - `CONTRIBUTING.md` (prerequisites, recipes);
  - each fixture's README with its verified deviations;
  - `AGENTS.md` where it lists the fixtures.

Contributor-facing change: `just bootstrap` and every `uv sync` on Linux now need the PyGObject build dependencies listed above. PlatynUI's users are not affected.

## Capabilities

### New Capabilities

- `gtk-test-app`: the GTK 3 and GTK 4 fixtures. It covers:
  - the blueprint catalog, CLI and window naming on each;
  - the header bar and dialog geometry other changes rely on;
  - the PyGObject environment and its failure message;
  - the Linux lanes;
  - the supported GTK 4 versions and the names of popover menu items.

### Modified Capabilities

- `acceptance-lane-selection`:
  - *Platform requirements are declared as tags* gains `platform:linux`;
  - *Lane profiles select by excluding foreign platforms* makes `real-windows` exclude `platform:linux`.

## Impact

- **New:** `apps/test-app-gtk3/**` and `apps/test-app-gtk4/**` (`main.py`, README), and `tests/acceptance/gtk3/**` and `tests/acceptance/gtk4/**` (`__init__.robot`, a launcher resource, `catalog.robot`, `modal.robot`).
- **Modified:**
  - root `Cargo.toml`: `exclude` entries and the comment naming the non-crates;
  - `pyproject.toml`: the Linux-only `PyGObject` dev dependency, and a mypy override that treats `gi` as untyped;
  - `uv.lock`;
  - `justfile`: run recipes, and `mypy` checking each fixture in its own invocation like the Qt fixtures;
  - `scripts/platynui-robot-session.sh`;
  - `robot.toml`: the `real-windows` excludes, and the comment on the tag dimensions;
  - `.github/workflows/ci.yml`;
  - the docs listed above.
- **No Rust or Python library code changes, no native rebuild.**
- **Platforms:** Linux only, on X11 and under the PlatynUI compositor, where GTK 3 and GTK 4 run natively on Wayland. GTK on Windows and macOS is out of scope: GTK 3 has no accessibility there, and GTK 4 only through its AccessKit backend, which GTK builds without by default.
- **Order with `linux-element-geometry`:**
  - This change is applied first, and both fixtures and all their suites are built here.
  - The GTK 4 catalog's tests that open no popover pass on both lanes with today's provider, and so do the GTK 3 catalog's tests that do not use the pointer.
  - The GTK 3 catalog's pointer tests miss until `linux-element-geometry`'s provider rework, because GTK 3 elements are placed by summing positions GTK 3 reports as screen values. The GTK 4 popover tests miss for the same reason: GTK 4 reports popover positions measured from the widget that opened them. Both are committed with that rework, as its first failing tests.
  - This change is archived after them.
- **Coordination:**
  - `linux-element-geometry` also changes `acceptance-lane-selection` (its bare X11 profile and `window-manager:` tags). The change archived second merges both deltas.
  - `add-avalonia-test-app` changes the Wayland profile's session in the same spec, in a different requirement.
