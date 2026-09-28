# Proposal

## Why

PlatynUI's fixture technology matrix plans an Avalonia row (`test-app-blueprint` Purpose, `dev-docs/testing-strategy.md` §5). Avalonia is the most widely used cross-platform .NET desktop toolkit.

- On Windows it exposes its automation peers through UIA.
- Since Avalonia 12 it has its own AT-SPI backend on Linux. That backend already showed a shape no other fixture has: it attaches the AT-SPI Application interface to every window. PlatynUI turned that into a second application node, which `atspi-application-level` fixes. Nothing in the lanes guards that fix against a real Avalonia tree.

An Avalonia fixture gives both:

- a blueprint-conforming catalog on a toolkit that PlatynUI's users automate;
- a permanent lane proof for a class of accessibility trees that the existing fixtures do not produce.

**On Linux, Avalonia is an X11 application, and that decides how the Wayland lane runs it.**

- Avalonia 12.1 has an experimental, opt-in native Wayland backend (`Avalonia.Wayland`), but that backend exposes no accessibility. Measured on 2026-09-28: a build with that backend runs natively in the PlatynUI compositor, and nothing registers on the accessibility bus.
- On a Wayland desktop, an Avalonia application runs through XWayland by default. That is how PlatynUI's users meet it.
- The Wayland lane's compositor starts no XWayland today, so an Avalonia application cannot even start there.
- Measured on 2026-09-28 in the compositor with XWayland:
  - The unchanged Avalonia application starts and registers on the accessibility bus. PlatynUI reads its tree, its process ID and its window bounds, and the point hit-test resolves elements inside it.
  - The existing fixtures keep running on Wayland. The Qt Widgets and Qt Quick fixtures connect only to the Wayland display. The egui fixture and the Inspector keep their windows on Wayland and additionally open a connection to the X display.
  - PlatynUI itself stays on its Wayland platform, because the session declares itself as Wayland.

So the Wayland lane gets XWayland, the way a Wayland desktop has it, and the fixture runs on all three lanes.

## What Changes

- **A new fixture `apps/test-app-avalonia`**, a C# Avalonia 12 application that conforms to the blueprint:
  - The core tier and the extended tier's slider, progress bar and tab control, built only from Avalonia's standard controls, under the canonical names and with the name-based observables. The extended tier's table would need the separate `DataGrid` package and is left out. The custom-controls chapter is not implemented.
  - The blueprint CLI: `--title` (default `PlatynUI Avalonia TestApp`), `--auto-close`, `--open-modal`, and a failure for an unknown argument.
  - An Avalonia window reports its title as its accessible name. The main window is therefore matched by its launch title, and dialogs carry their canonical names as titles, following the blueprint's window-naming fallback.
  - On Linux it runs on Avalonia's X11 backend and never on the native Wayland backend: directly on the X11 lane, through XWayland on the Wayland lane. On Windows it runs on Win32 and UIA.
- **A reproducible .NET build.**
  - The current .NET SDK (10, the LTS) is pinned by `global.json` to its feature band.
  - Package versions are pinned and locked.
  - `just build-test-app-avalonia` builds the fixture to a fixed path, and `just run-test-app-avalonia` starts it.
  - The lanes launch the built executable directly, so `@ProcessId` pins the fixture's own process.
  - The fixture stays out of the Cargo workspace.
- **The Wayland lane's session provides XWayland.**
  - The `real-wayland` profile starts its compositor with `--xwayland`.
  - Applications that support Wayland keep using it.
  - A session whose XWayland does not become usable fails the lane before any suite runs. It never runs the suites without X11. This relies on the startup contract that `fix-compositor-xwayland-startup-hang` introduces.
- **Lane wiring on all three lanes.**
  - `tests/acceptance/avalonia/` holds:
    - the self-contained `catalog.robot` with the blueprint's canonical test set;
    - `modal.robot` for the instance started with `--open-modal`, as the QML fixture has it;
    - a suite that proves the application level on a real Avalonia tree.
  - The suites carry no platform tag.
  - Both Linux sessions build the fixture through `just build-test-app-avalonia` and hand it over as `PLATYNUI_TEST_APP_AVALONIA_BIN`. The Windows lane builds it as a hard prerequisite.
  - A missing fixture, or a missing X display in a compositor session opened without `--xwayland`, fails the suite with the command that fixes it.
- **Locators that hold on AT-SPI and UIA.**
  - The catalog addresses controls by name only, from the fixture's application node, with role unions only at window level, as the QML catalog does.
  - Where the bridges differ beyond that, the fixture README documents the verified deviation, and the affected test follows the first rule that fits:
    - behavior on exactly one lane is scoped to that lane by platform tag;
    - a toolkit limitation on every lane is the blueprint's documented skip;
    - a difference on some lanes keeps the test on every lane and narrows what it asserts.

    A PlatynUI defect is fixed, not scoped away.
- **CI.**
  - Both entries of the Linux acceptance job install the .NET SDK with a NuGet cache.
  - The Linux acceptance job installs XWayland.
  - The failure log dump includes the fixture's log.
  - There is no Windows acceptance job in CI; the Windows lane runs on a Windows machine.
- **Contributor documentation.**
  - `CONTRIBUTING.md`:
    - names the .NET 10 SDK as a prerequisite for the acceptance lanes;
    - names XWayland as a prerequisite for the Wayland lane;
    - says that an interactive compositor session serves Wayland-lane runs only when opened with `--xwayland`, and that the Linux lanes do not run under WSL;
    - lists the new recipes.
  - `AGENTS.md` lists the .NET fixture and its recipes.
  - `dev-docs/testing-strategy.md`:
    - moves Avalonia from the planned rows into the matrix, with its lanes;
    - says that the Wayland lane's session provides XWayland.
  - The `robot.toml` comment at the Wayland profile says why its session starts XWayland.

**Contributor-facing change:** running any acceptance lane now requires the .NET 10 SDK, and the Wayland lane also requires the `Xwayland` binary. `just check`, `just test` and the mock lane need neither. Users of PlatynUI are not affected.

## Capabilities

### New Capabilities

- `avalonia-test-app`: the Avalonia fixture. It covers:
  - its build and toolchain pins;
  - the blueprint catalog, CLI and window naming on Avalonia;
  - its lanes (X11, Wayland through XWayland, Windows) with the prerequisite check;
  - the locator rules that hold on both bridges;
  - the application-level proof on a real Avalonia tree.

### Modified Capabilities

- `acceptance-lane-selection`: the Wayland lane's session provides XWayland, keeps Wayland-capable applications on Wayland, and fails the lane when XWayland does not become usable.

## Impact

- **New:**
  - `apps/test-app-avalonia/**`: the C# project, `global.json`, the NuGet lock file, a README and an app-level `.gitignore`.
  - `tests/acceptance/avalonia/**`: `__init__.robot`, a launcher resource, `catalog.robot`, `modal.robot` and `application_level.robot`.
- **Modified:**
  - Root `Cargo.toml`: an `exclude` entry and the comment naming the non-crates.
  - `justfile`: the build and run recipes, and the Windows lane's prerequisite and hand-over.
  - `scripts/platynui-robot-session.sh`: the fixture build and hand-over, in both Linux sessions, and the wrapper line quoted in its header.
  - `robot.toml`: `--xwayland` in the `real-wayland` wrapper, and a comment on it.
  - `.github/workflows/ci.yml`: `setup-dotnet` and the `xwayland` package for the Linux acceptance job, and the log dump.
  - `CONTRIBUTING.md`, `AGENTS.md`, `dev-docs/testing-strategy.md` (§2.6 and §5).
- **No Rust or Python library code changes, no native rebuild.**
- **Toolchain:**
  - The .NET 10 SDK for every acceptance lane, and `Xwayland` for the Wayland lane.
  - The first build needs network access for NuGet.
- **Platforms:**
  - Linux on X11 through AT-SPI, Linux on Wayland through XWayland and AT-SPI, and Windows through UIA.
  - macOS has no acceptance lane.
- **Depends on:**
  - `atspi-application-level`. Without it the Avalonia window is `app:Application`, and every Linux suite fails to find it.
  - `fix-compositor-xwayland-startup-hang`. Without it, an XWayland that fails to start leaves the Wayland lane hanging until the CI timeout instead of failing it.

  This change lands after both.
- **Coordination:**
  - `java-provider-linux` plans to reach Swing through XWayland and can then use the Wayland lane's XWayland.
  - `fix-compositor-xwayland-startup-hang` names `java-provider-linux` as the first user of `--xwayland` in a lane. With this change the Wayland lane uses it first.
  - `fix-compositor-xwayland-startup-hang` keeps its real-XWayland tests ignored because CI had no XWayland. This change installs XWayland only in the acceptance job, which does not run those tests. Running them in CI would need XWayland in the job that runs `just test`, and an explicit run of the ignored tests. That is a separate follow-up and part of neither change.
  - The Linux acceptance lanes do not run under WSL. `startcompositor.sh` drops XWayland there, and WSL is generally unreliable with Wayland and X11 applications.
