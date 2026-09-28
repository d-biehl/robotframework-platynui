# Design

## Context

The motivation is in proposal.md (Why), and the contracts are in `specs/avalonia-test-app/spec.md` and `specs/acceptance-lane-selection/spec.md`. This section records only the facts that shape the approach. **Verified** marks what was read in the tree at `93cce5c` or measured on 2026-09-28; the rest is marked as assumed.

**Avalonia on Linux.** **Verified.**

- **The X11 backend.** Avalonia 12.1.2 with `UsePlatformDetect()` runs on X11, also under the headless Xvfb of `scripts/startxsession.sh`. It registers on the accessibility bus within seconds.
- **The native Wayland backend.**
  - It exists since 12.1.0 as the package `Avalonia.Wayland`. It is experimental and opt-in (`UseWayland()`, `UseWaylandWithFallback()`), per the Avalonia docs.
  - Its project references `Avalonia`, `Avalonia.Dialogs`, `Avalonia.FreeDesktop` and `NWayland`, and no accessibility code (`src/Avalonia.Wayland/Avalonia.Wayland.csproj`).
  - The AT-SPI server is started only by the X11 backend (`src/Avalonia.X11/X11AtSpiAccessibility.cs`, `X11Platform.cs`).
  - Measured with that backend in `scripts/startcompositor.sh --backend headless -- scripts/platynui-robot-session.sh`: the window runs natively on Wayland, and the registry lists no application.
- **Without either backend's display.** An Avalonia application without `Avalonia.Wayland` does not start in the compositor when no XWayland runs, because it has no X display to connect to.
- **Under XWayland.** Measured in the same session with `--xwayland`, with the unchanged application:
  - It starts and registers on the accessibility bus.
  - PlatynUI reads its application node with `@ProcessId`, the window's bounds from the compositor, and the tree below it.
  - `platynui-cli element-at-point` resolves an element inside the window.
- **Window names.** Avalonia names a window after its title (`WindowAutomationPeer.GetNameCore() => Owner.Title`, `src/Avalonia.Controls/Automation/Peers/WindowAutomationPeer.cs:22`).
- **Popups.**
  - A popup is a `PopupRoot`, whose peer derives from `WindowBaseAutomationPeer` and reports `AutomationControlType.Window`. On UIA it is neither a control nor a content element (`PopupRootAutomationPeer.cs:18-19`).
  - Measured with an open menu on X11: the popup is an AT-SPI `frame` named `PopupRoot` directly under the application root. It reports `Accessible`, `Application` and `Component`.
  - It is not a client of the X11 window manager: `_NET_CLIENT_LIST` holds only the main window.
- **The Application interface.** Avalonia attaches it to every `TopLevel`. `atspi-application-level` makes PlatynUI classify such nodes by their role.

**Avalonia's roles on AT-SPI** come from `src/Avalonia.FreeDesktop.AtSpi/AtSpiNode.RoleMapping.cs` (**verified**). The PlatynUI names follow from `map_role` in `crates/provider-atspi/src/node.rs`. The UIA column is **assumed**: Avalonia's UIA peers report the same `AutomationControlType`, which the UIA provider maps to its control types. Task 2.5 verifies it.

| Avalonia control type | AT-SPI role → PlatynUI | UIA (assumed) |
|---|---|---|
| Window (main window, dialogs, `PopupRoot`) | `frame` → `Frame` | `Window` |
| Button, CheckBox, RadioButton | `push button`, `check box`, `radio button` | `Button`, `CheckBox`, `RadioButton` |
| Edit (TextBox, single- and multi-line) | `entry` → `Entry` | `Edit` |
| Text (TextBlock) | `label` → `Label` | `Text` |
| Group (GroupBox), Expander, Pane | `panel` → `Panel` | `Group`, `Expander`, `Pane` |
| ComboBoxItem, ListItem | `list item` → `item:ListItem` | `ListItem` |
| TreeItem, TabItem | `tree item`, `page tab` → `item:TreeItem`, `item:TabItem` | `TreeItem`, `TabItem` |
| Menu, MenuBar, MenuItem | `menu`, `menu bar`, `menu item` | `Menu`, `MenuBar`, `MenuItem` |

On the Wayland lane the fixture is an X11 client of XWayland, so the AT-SPI column applies there as well.

**XWayland in the Wayland lane.** **Verified.**

- **Starting it.** The compositor starts XWayland only with `--xwayland` (`scripts/startcompositor.sh:9`), which the `real-wayland` wrapper does not pass today (`robot.toml:66-70`).
- **WSL.** Under WSL, `startcompositor.sh` drops a requested XWayland without failing, because "XWayland does not work reliably" there.
  - It resets `XWAYLAND=0` (:108-112) after `--xwayland` set it (:55-57), and it passes the flag to the compositor only when it is still set (:143-145).
  - The session then starts without an X display.
  - The maintainer considers WSL generally unreliable for Wayland and X11 applications, so it is no host for these lanes (Non-Goals).
- **Runs inside an interactive session.** `startcompositor.sh` exports `ROBOTCODE_WRAPPER_APPLIED=1` for every session (:199). A `real-wayland` run started inside an interactive session therefore skips the profile's wrapper and uses that session as it is, as the existing spec requires ("A run inside an already established session does not nest"). An interactive session has XWayland only when it was opened with `--xwayland`.
- **When it fails to start.** A failing start leaves the compositor hanging without readiness until an outside timeout. `fix-compositor-xwayland-startup-hang` makes that a startup error with exit code `1`. It is not implemented yet, and it notes that no lane passes `--xwayland` so far.
- **PlatynUI's own platform choice.**
  - PlatynUI detects the session type from `XDG_SESSION_TYPE` first, then from `WAYLAND_DISPLAY`, then from `DISPLAY`. It names XWayland as the reason for that order (`crates/platform-linux/src/session.rs:19-45`).
  - `startcompositor.sh` exports `XDG_SESSION_TYPE=wayland` (:192).
  - With XWayland, PlatynUI therefore stays on its Wayland platform.
- **Where X11 windows appear.**
  - The compositor maps X11 windows into its space, override-redirect windows (menus, tooltips, dropdowns) included, at the position they request (`apps/wayland-compositor/src/xwayland.rs:217-229`).
  - `list_windows` reports every element of that space (`apps/wayland-compositor/src/control.rs:1234-1236`), and `list_popups` reports only `xdg_popup`s (:1238-1249).
  - PlatynUI's Wayland window manager resolves a node's window from `list_windows`, by process ID, title and size (`crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:70-92`). When the process has exactly one window, it takes that one without comparing anything (:377-379).
  - The X11 window manager resolves the window through `find_xid_for_pid` (`crates/platform-linux-x11/src/window_manager.rs:170-229`), which `resolve_window` calls (:681-697). The candidates are the process's windows in `_NET_CLIENT_LIST`:
    - with exactly one candidate it takes that one, without comparing anything (:196-199);
    - with several it tries a geometry match and a name match, and fails when neither fits (:219-226).
  - When the window manager cannot resolve a window, the AT-SPI provider falls back to the toolkit's own extents (`crates/provider-atspi/src/extents.rs:177-201`). On X11 those are correct screen coordinates.
- **Process IDs of X11 windows.** An X11 window's process ID is the one its client declares (capability `compositor-client-identity`). Avalonia declares one: the hit-test above resolved its window.
- **Effect on the existing fixtures.** Measured in the headless compositor session, once without and once with `--xwayland`, from the display connections each process holds:

  | Fixture | Without XWayland | With XWayland |
  |---|---|---|
  | Qt Widgets | Wayland | Wayland |
  | Qt Quick | Wayland | Wayland |
  | egui | Wayland | Wayland, plus a connection to the X display |
  | Inspector | Wayland | Wayland, plus a connection to the X display |

  - With XWayland, egui and the Inspector keep their Wayland connection unchanged. The additional X connection is presumably the clipboard library `arboard`, which egui-winit initializes next to its Wayland clipboard (**assumed**).
- **The full Wayland lane with XWayland.** It was run on the same native build, once as today and once through `scripts/startcompositor.sh --xwayland --backend headless -- scripts/platynui-robot-session.sh uv run --no-sync robotcode --profile real-wayland run`.
  - Both runs passed all 84 tests, and `robotcode results diff` reports no differences.
  - The logs differ by one FAIL message. It comes from the first poll of the deliberately polled resize in `Egui.Auto Activate.Move And Resize Window Change The Window Bounds`, not from XWayland.

**The existing fixtures, lanes and CI.** **Verified.**

- Fixtures that are not crates are listed in the root `Cargo.toml` `exclude` (:2-13).
- Linux fixtures are built in `scripts/platynui-robot-session.sh`, the tail of both Linux lane wrappers, before any suite runs. The session script then exports their paths (:70-106).
- The Windows lane builds its fixtures in `test-acceptance-windows` as a hard prerequisite and exports their paths (`justfile:380-387`).
- Catalog suites pin the application node as the query root and wait for the window with a role union, e.g. `.//(Frame|Window)[@Name="${title}"]` (`tests/acceptance/qml/resources/qmlapp.resource:57-58`).
  - They address controls by `.//*[@Name="…"]`.
  - An unprefixed `*` matches any namespace, so `item:` nodes are included.
- The lanes select by excluding foreign `platform:` tags (`robot.toml:53-75`). A suite without a platform tag runs on every lane.
- **The CI acceptance job** (`.github/workflows/ci.yml:246-343`):
  - it runs a matrix `backend: [x11, compositor]`;
  - it installs neither .NET nor XWayland (:260-294);
  - it dumps `egui-*.log` and `qt-*.log` on failure (:323-335).
- There is no Windows acceptance job.
- `.gitignore` ignores `build/` everywhere, but not `bin/` or `obj/`.
- The local toolchain is .NET SDK 10.0.112.

## Goals / Non-Goals

**Goals:**

- A fixture that a contributor builds with one recipe and a pinned SDK, and that the lanes launch as its own process.
- The fixture on all three lanes, on the Wayland lane the way a Wayland desktop runs it.
- One catalog suite whose test bodies are the same on every lane.
- XWayland in the Wayland lane without changing how the existing fixtures run there.

**Non-Goals:**

- Avalonia's native Wayland backend.
- A mode in which the Wayland lane runs without XWayland when XWayland fails.
- WSL as a host for the Linux acceptance lanes. WSL is generally unreliable with Wayland and X11 applications, and `startcompositor.sh` keeps dropping XWayland there. `CONTRIBUTING.md` says that the lanes do not run under WSL.
- The custom-controls chapter and the extended tier's table.
- A Windows acceptance job in CI.
- Output scales other than 1 under XWayland. `verify-display-scaling` covers scaling.
- Fixing Avalonia's Application interface on its top-levels. That is upstream, and PlatynUI already copes with it.

## Decisions

### D1: A plain Avalonia project, no MVVM framework

`apps/test-app-avalonia` holds:

- one project, with `AssemblyName` `platynui-test-app-avalonia` and `net10.0`;
- `App.axaml`, a `MainWindow.axaml` with the catalog, and one window class per dialog;
- code-behind for the observables;
- a `Program.cs` that parses the CLI before Avalonia starts, so an unknown argument fails before any window opens.

Its only dependencies are `Avalonia`, `Avalonia.Desktop`, `Avalonia.Themes.Fluent` and `Avalonia.Fonts.Inter`. The embedded Inter font makes text rendering independent of the fonts installed on a headless runner. The project enables no trimming, AOT or single-file publishing, so the SDK injects no packages of its own into the dependency graph (see Risks).

An MVVM toolkit, the template's diagnostics package and `Avalonia.Wayland` are left out. The first two add a dependency without adding anything the catalog needs. `Avalonia.Wayland` would take the fixture off the accessibility bus on Wayland (Context).

### D2: Pinned SDK band, pinned and locked packages

- **The SDK pin.** `apps/test-app-avalonia/global.json` pins SDK `10.0.100` with `rollForward: latestFeature`, so any 10.0.x SDK builds the fixture.
  - The `dotnet` command looks for `global.json` from its working directory, not from the project path. The build therefore runs from the fixture's directory (D3); run from the repository root, it would never see the pin.
  - If a `dotnet` is present but none of its SDKs fits the pin, the SDK itself fails and names the requested version.
  - If no `dotnet` is present at all, the shell only says the command was not found. The recipe therefore checks for `dotnet` first and fails with its own message naming the .NET 10 SDK.
- **Package pins.** The Avalonia packages are pinned to an exact version, 12.1.2 or the then-current 12.1 patch.
- **The lock file.** `RestorePackagesWithLockFile` writes `packages.lock.json`, which is committed. CI restores in locked mode, so a changed dependency graph fails instead of drifting.
- **CI's SDK.** CI installs the SDK through `actions/setup-dotnet` with `global-json-file: apps/test-app-avalonia/global.json`, so CI and the pin agree.

.NET 10 is the current LTS release.

### D3: Build to `build/`, launch the executable

`just build-test-app-avalonia` changes into `apps/test-app-avalonia` and runs `dotnet build -c Release -o build` there, as `build-test-app-swing` changes into its project first (`justfile:487-488`).

- The `[unix]` recipe checks `command -v dotnet` first; the `[windows]` recipe uses `Set-Location` and checks `Get-Command dotnet`.
- The existing `build/` rule in `.gitignore` covers the output. An app-level `.gitignore` covers `bin/` and `obj/`, following `apps/inspector/.gitignore`.

The lanes start the apphost, `build/platynui-test-app-avalonia` or `.exe`, and never `dotnet run`. `dotnet run` starts the application as a child process, and `@ProcessId` would then name the wrong process. This is the same reason the Qt fixtures bypass uv's trampoline (`justfile:19-24`).

`just run-test-app-avalonia *ARGS` builds and starts it for manual use.

### D4: Avalonia's X11 backend on Linux, on both Linux lanes

The fixture uses `UsePlatformDetect()` and nothing else, so it runs on X11 on Linux:

- directly on the X11 lane;
- through the session's XWayland on the Wayland lane (D5).

Its suites carry no platform tag.

Alternatives considered:

- **The native Wayland backend.** It gives no accessibility at all.
- **Tagging the suites `platform:x11` and `platform:windows`.**
  - The lane selection cannot express that today: each lane excludes every test with a foreign platform tag, so such a suite would run nowhere. It would first need a changed exclude rule.
  - It would leave the fixture untested in the configuration users run on a Wayland desktop.

### D5: The Wayland lane always starts XWayland

The `real-wayland` wrapper becomes `["scripts/startcompositor.sh", "--xwayland", "--", "scripts/platynui-robot-session.sh"]`.

- **Comments.** A new comment above that wrapper (`robot.toml:69-70`) says why the session starts XWayland, and that an interactive session serves `real-wayland` runs with X11 only when it was opened with `--xwayland`. The header of `scripts/platynui-robot-session.sh` quotes the wrapper line verbatim (:10), so it changes with it.
- **The session provides it for every suite.** The session the wrapper establishes belongs to the lane run, not to a suite. A suite cannot request XWayland, and it may not probe for it to decide whether to skip (`acceptance-lane-selection`).
- **Runs inside an interactive session** reuse that session as it is (Context), so they have XWayland only when the session was opened with `--xwayland`. The Avalonia launcher fails with that hint when no X display is available (D7). That is a prerequisite failure, not a skip.
- **The existing fixtures are not affected** (Context: the connection measurement and the full lane run).
- **`fix-compositor-xwayland-startup-hang` is a hard prerequisite.** Without it, an XWayland that fails to start leaves the lane hanging until the CI timeout. With it, the compositor ends with `1` and names the cause, and the wrapper chain carries that exit code into the lane result. That is the fail-loud rule of `acceptance-lane-selection`.

Alternatives considered:

- **A separate lane profile `real-xwayland`.** It adds a session and a CI matrix entry for one fixture. A Wayland desktop with XWayland is what the Wayland lane is meant to model.
- **XWayland only in CI.** The local and the CI lane would differ.
- **Starting XWayland lazily, on the first X11 client.** The compositor does not offer that; it is a non-goal of `fix-compositor-xwayland-startup-hang`.

### D6: Both Linux sessions build the fixture

`platynui-robot-session.sh` builds the fixture before any suite runs, in both Linux sessions, like the egui fixture (:70-77). It then exports `PLATYNUI_TEST_APP_AVALONIA_BIN`.

- **One build definition.** It builds by running `just build-test-app-avalonia`, so the SDK pin, the working directory and the `dotnet` check of D2 and D3 apply there too. `just` is already a contributor prerequisite.
- **A failed build** ends the session with a message naming the .NET 10 SDK and `just build-test-app-avalonia`.

**Consequence:** the .NET 10 SDK is a prerequisite of every Linux lane run, also of a run narrowed to other suites. That matches how the session script already builds the egui fixture and the Inspector for every run. `dotnet build` is incremental, so later runs are fast.

`dev-docs/testing-strategy.md` §5 says today that "a lane needs only its own fixture built" (:270-271). That was already not the case for the Linux lanes, which build every Linux fixture. This change replaces the sentence with the actual model: each lane builds all of its fixtures before any suite runs.

On Windows, `test-acceptance-windows` runs `just build-test-app-avalonia` as a hard prerequisite next to the Swing build, and exports `PLATYNUI_TEST_APP_AVALONIA_BIN`.

Alternatives considered:

- **Building only when an Avalonia suite is selected.** The session script does not know the selection; it only runs the command appended to it.
- **Building in the suite setup.** A slow first build would race the suite's timeouts, which is why the session builds the egui fixture up front (:70-72).
- **Skipping the build when no SDK is present.** It would turn a missing prerequisite into a suite failure far from its cause.

### D7: The suites

- **`tests/acceptance/avalonia/__init__.robot`** carries `Test Tags acceptance real`, and no platform tag.
- **`resources/avaloniaapp.resource`** holds the launcher, following `qmlapp.resource`:
  - **The fixture path.** It resolves the path from `PLATYNUI_TEST_APP_AVALONIA_BIN`, falling back to `apps/test-app-avalonia/build/platynui-test-app-avalonia` (`.exe` on Windows). The other launchers do the same, e.g. `tests/acceptance/swing/resources/swing_env.resource:16`. A run inside an established session, which skips the session script, then still finds a fixture built by `just build-test-app-avalonia`.
  - **Prerequisite checks.** It checks its prerequisites first, following `Require Swing Prerequisites`, and fails, never skips:
    - when the resolved path names no file, naming `just build-test-app-avalonia`;
    - on Linux, when no X display is available, naming `scripts/startcompositor.sh --xwayland`.
  - **Starting the fixture.** It starts the executable with `--title` and optional extra arguments such as `--open-modal`, logging to `${TEMPDIR}/avalonia-<title>.log`.
  - **Query root.** It pins `BM.Set Root /app:Application[@ProcessId=${pid}]`, and waits for `.//(Frame|Window)[@Name="${title}"]`.
  - **Teardown.** It ends the process with `Terminate Process ... kill=${True}`, as the Swing resources do (`tests/acceptance/swing/resources/testapp.resource:73`). An apphost built as `WinExe` has no console, so on Windows it presumably never receives the graceful CTRL_BREAK, and each teardown would wait 30 s. That is assumed; task 7.4 checks it.
- **`catalog.robot`** replicates the canonical test set of `tests/acceptance/qml/catalog.robot`. Test bodies are written directly against BareMetal, and locators are name-only below the application node.
- **`modal.robot`** replicates `tests/acceptance/qml/modal.robot`:
  - its suite setup starts a separate instance with `--open-modal` and a title of its own;
  - it tests that `dialog-modal` with its label and button is present without interaction, and that a click on `dialog-modal-button` lands inside the dialog.

  The QML catalog covers the modal dialog the same way, in a suite of its own (`tests/acceptance/qml/catalog.robot:28-29`).
- **`application_level.robot`** holds the two scenarios of *One application level on a real Avalonia tree*, without `native:` attributes, so they are the same on every lane.

### D8: Locator rules across the bridges

The table in Context shows where the bridges differ: window role, text roles, namespaces of entries. The suites absorb that as follows:

- **By name.** `.//*[@Name="…"]` matches any role and namespace.
- **Windows.** The role union `(Frame|Window)` is used, and only for windows; dialogs are windows too.
- **Popups.** A popup is a separate top-level on every bridge, so the application node is the query root, not the main window.
- **Behavior differences.** Where a lane differs in behavior rather than in role (Context: a popup on UIA is neither a control nor a content element), the difference is verified on each affected lane first. It then goes into the README's deviation list, and the first matching rule applies:
  1. **Behavior that exists on exactly one lane** gets that lane's platform tag, as `tests/acceptance/qml/popups.robot` has `platform:windows`. A platform tag confines a test to exactly one lane, because each lane excludes every foreign platform tag (D4).
  2. **A limitation of the toolkit on every lane** gets the blueprint's documented skip.
  3. **A difference on some lanes but not all** keeps the test on every lane, asserting only the facts that hold on all of them. This is the blueprint's documented-deviation rule, as `tests/acceptance/qml/modal.robot` applies it.

  A PlatynUI defect gets none of these. The test fails until PlatynUI is fixed.

  The third rule covers the cases the Risks name, such as popup items missing only on UIA. Should a real case need a test that runs on exactly two lanes, the platform-tag vocabulary would be extended in its own change.

A per-platform locator variable is not an option, because the blueprint's shared contract is name-only.

### D9: Contributor documentation and CI

- **`CONTRIBUTING.md` §1:**
  - It names the .NET 10 SDK as a prerequisite for the acceptance lanes.
  - It names `Xwayland` as a prerequisite for the Wayland lane: the package `xwayland` on Debian and Ubuntu, `xorg-xwayland` on Arch.
  - It states that `just check`, `just test` and the mock lane need neither.
- **`CONTRIBUTING.md` §8 (acceptance lanes):**
  - An interactive compositor session serves `real-wayland` runs only when opened with `scripts/startcompositor.sh --xwayland`.
  - The Linux acceptance lanes do not run under WSL.
- **The `CONTRIBUTING.md` recipe table** gains `build-test-app-avalonia` and `run-test-app-avalonia`.
- **`AGENTS.md`** gets a ".NET fixture" bullet next to the Java workspace bullet (:17-18), naming `apps/test-app-avalonia` and its recipe.
- **`dev-docs/testing-strategy.md`:**
  - §5 moves Avalonia from "Planned rows" into the matrix: X11, Wayland through XWayland, and Windows, with the reason there is no native Wayland.
  - §5 replaces "a lane needs only its own fixture built" (:270-271) with the model of D6.
  - §2.6 states three things:
    - the Wayland lane's session provides XWayland, so X11-only fixtures run there;
    - a failing XWayland fails the lane;
    - an interactive session provides it only when opened with `--xwayland`.
- **The CI `acceptance-linux` job:**
  - adds `xwayland` to its apt packages;
  - runs `actions/setup-dotnet` with `global-json-file: apps/test-app-avalonia/global.json` and a NuGet cache keyed on `apps/test-app-avalonia/packages.lock.json`, for both matrix entries;
  - restores in locked mode;
  - its log dump adds `avalonia-*.log`.

## Risks / Trade-offs

- **[Avalonia's popups resolve to the wrong window]** After `atspi-application-level`, a `PopupRoot` is a `control:Frame` directly under the application level. The transient-popup exception covers only the roles `PopupMenu`, `Menu` and `ToolTip` (`crates/provider-atspi/src/node.rs:227-234`). The popup would therefore be a top-level window whose bounds come from the window manager. The result depends on the lane and on how many windows the process has (Context):
  - **On the X11 lane**, the popup is not in `_NET_CLIENT_LIST`.
    - While the main window is the process's only managed window, the popup takes the main window's bounds, and every menu item's bounds move. That is the failure mode the provider documents for Qt popups (`node.rs:214-222`).
    - With `dialog-modeless` open as well, as in the finished catalog, the window lookup fails, and the provider falls back to AT-SPI's own extents, which are correct on X11.
  - **On the Wayland lane**, the compositor lists the popup's override-redirect window among its windows. The popup may then resolve to its own window by process ID and size, or a mismatch in title or size may resolve it to another window.

  → Task 2.4 verifies both Linux lanes, once `atspi-application-level` is in place, with the fixture's menu, context menu and combo box. It checks each both with the main window alone and with `dialog-modeless` open, because the catalog instance alone would hide the single-window case. A wrong result in either case is a provider defect, not a technology limitation. It gets its own provider change, with its own test for the single-window case. The catalog's popup tests fail until it lands; they are neither skipped nor scoped away (D8). It would affect every Avalonia application, not just the fixture.
- **[UIA roles and popup visibility are unverified]** `PopupRoot` is neither a control nor a content element on UIA, so its items might not appear in the control view. → Task 2.5 verifies this on a real Windows machine before the suite encodes locators, and decides which rule of D8 applies. Wine does not count.
- **[An SDK update breaks the locked restore]** CI installs the newest SDK the pin admits. An SDK update can change packages the SDK adds to the dependency graph itself, and a locked restore then fails with NU1004 without any change in the repository. Microsoft's guidance pairs lock files with `rollForward: disable` for that reason. → The fixture enables nothing that makes the SDK add packages (D1). If it happens anyway, the remedy is to regenerate `packages.lock.json` with the new SDK and commit it. `rollForward: disable` would prevent it, at the price of every contributor installing that exact SDK.
- **[An interactive session without XWayland]** A `real-wayland` run inside an interactive session opened without `--xwayland` has no X display (Context). → The Avalonia launcher fails with a message naming `scripts/startcompositor.sh --xwayland` (D7), and `CONTRIBUTING.md` and `testing-strategy.md` §2.6 say so (D9).
- **[XWayland does not start on a machine or runner]** → `fix-compositor-xwayland-startup-hang` is a hard prerequisite (D5), and CI installs XWayland (D9). A missing or broken XWayland then fails the Wayland lane at once, naming the cause.
- **[egui and the Inspector open an X connection under XWayland]** Their windows stay on Wayland, and the full lane passed unchanged (Context). The assumed clipboard connection could matter for a future clipboard test that crosses toolkits. → Such a test verifies its clipboard path on the Wayland lane when it is written.
- **[.NET for every acceptance lane]** A contributor who runs only the egui suites on Linux now needs the .NET 10 SDK too (D6). → `CONTRIBUTING.md` says so. `just check`, `just test` and the mock lane stay free of it.
- **[NuGet needs network]** The first build downloads the packages. → The lock file makes the download deterministic, and CI caches the packages.
- **[Avalonia moves quickly]** 12.x patch releases change the accessibility backend. → Exact package pins. An upgrade is a deliberate change that reruns every lane.
- **[The upstream Application interface is fixed]** Avalonia may later stop attaching the interface to top-levels. → Nothing here depends on it; `atspi-application-level` handles both shapes.

## Migration Plan

- **For PlatynUI's users the change is additive.** It needs no native rebuild.
- **For contributors,** every acceptance lane needs the .NET 10 SDK from now on, and the Wayland lane also needs `Xwayland`. `CONTRIBUTING.md` says so. A missing SDK ends the Linux session before the suites with that message. A missing XWayland fails the Wayland lane with the compositor's startup error.
- **The existing Wayland lane suites are unaffected by XWayland** (Context: measured).
- **Rollback** is a revert. Removing only `--xwayland` from the `real-wayland` wrapper would leave the Avalonia suites failing on that lane, so the fixture and the flag go together.
