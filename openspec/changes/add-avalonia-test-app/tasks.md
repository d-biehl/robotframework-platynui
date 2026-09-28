# Tasks

**Prerequisites:**

- **`atspi-application-level` lands first.** Without it the Avalonia window is `app:Application`, and every Linux suite of this change fails to find it.
- **`fix-compositor-xwayland-startup-hang` lands first.** Without it, an XWayland that fails to start leaves the Wayland lane hanging instead of failing it (design D5).
- **Windows work is verified on a real Windows machine.** CI has no Windows acceptance job, and Wine does not verify UIA.
- **Toolchain.** The .NET 10 SDK is needed for every task from 2.1 on, and `Xwayland` for every task that runs the Wayland lane. The Linux lanes do not run under WSL (design, Non-Goals).

## 1. XWayland in the Wayland lane

- [ ] 1.1 Guard the existing Wayland lane before and after enabling XWayland.
  - First run the lane twice on the same native build, into separate output directories:
    - as it is: `PLATYNUI_BACKEND=headless uv run robotcode --profile real-wayland run -d <dir-a>`;
    - with XWayland: `scripts/startcompositor.sh --xwayland --backend headless -- scripts/platynui-robot-session.sh uv run --no-sync robotcode --profile real-wayland run -d <dir-b>`.
  - Then change the `real-wayland` wrapper in `robot.toml` to include `--xwayland` (`:69-70`, design D5).
  - Add a comment above that wrapper. It says why the session starts XWayland, and that an interactive session serves `real-wayland` runs with X11 only when it was opened with `scripts/startcompositor.sh --xwayland`.
  - Update the wrapper line quoted in the header of `scripts/platynui-robot-session.sh` (:10).

  Verify:
  - `robotcode results diff` between the two runs reports no status change;
  - `just headless=true test-acceptance-compositor` is green with the new wrapper;
  - its session output shows a `DISPLAY` next to `WAYLAND_DISPLAY`.

  This was measured once while planning (design, Context); the rerun guards against changes in between.
- [ ] 1.2 Check that the Wayland-capable fixtures and PlatynUI stay on Wayland in that session.
  - Inside `scripts/startcompositor.sh --xwayland --backend headless -- scripts/platynui-robot-session.sh <script>`, start the egui, Qt Widgets and Qt Quick fixtures.
  - Map each process's display connections: its socket inodes from `/proc/<pid>/fd`, matched against the server side in `ss -xa`.

  Verify:
  - each fixture holds its Wayland connection. Qt Widgets and Qt Quick hold none to the X display (spec: *Wayland-capable fixtures stay on Wayland*);
  - `platynui-cli info` reports the Wayland platform (*PlatynUI stays on its Wayland platform*).
- [ ] 1.3 Check the failure path. Put a fake `Xwayland` that exits with `1` first on `PATH`, as the tests of `fix-compositor-xwayland-startup-hang` do, and run `just headless=true test-acceptance-compositor`.

  Verify:
  - the run ends non-zero;
  - the output contains the compositor's XWayland startup error;
  - no suite ran (spec: *A failing XWayland fails the lane*).

## 2. Fixture skeleton, session build and early checks

- [ ] 2.1 Create `apps/test-app-avalonia` with the parts of design D1 to D3.
  - The project file:
    - `AssemblyName` `platynui-test-app-avalonia` and `net10.0`;
    - `RestorePackagesWithLockFile`;
    - exact package pins, and no `Avalonia.Wayland`;
    - no trimming, AOT or single-file properties.
  - `global.json` (`10.0.100`, `latestFeature`) in the fixture's directory, the committed `packages.lock.json`, and an app-level `.gitignore` for `bin/` and `obj/`.
  - `Program.cs` with the CLI of the spec, parsed before Avalonia starts.
  - A main window with `button-basic`, `main-menubar`, `context-menu` and `combobox-basic`, the controls that open popups, and the `dialog-modeless` window. The rest follows in 4.1.
  - Add the directory to the root `Cargo.toml` `exclude` and to its comment (:3-6).
  - Add `just build-test-app-avalonia` and `just run-test-app-avalonia *ARGS` to the `justfile` (design D3):
    - the build changes into `apps/test-app-avalonia` first, following `build-test-app-swing` (:487-488);
    - it checks for a `dotnet` command first.

  Verify:
  - `just build-test-app-avalonia` produces `apps/test-app-avalonia/build/platynui-test-app-avalonia`;
  - `--bogus` prints the usage, opens no window and exits non-zero;
  - `--auto-close 5` exits with 0;
  - with a `PATH` that holds only links to `just`, `sh` and `bash`, the recipe fails naming the .NET 10 SDK (spec: *A missing dotnet command fails the build*);
  - with `global.json` temporarily set to `10.0.999` and `rollForward: disable`, the build fails naming `10.0.999` (*An SDK outside the pin fails the build*). Restore the file afterwards;
  - `cargo metadata --no-deps --format-version 1` lists no Avalonia member;
  - `git status` shows no `bin/`, `obj/` or `build/` content.
- [ ] 2.2 In `scripts/platynui-robot-session.sh`, build the fixture in both Linux sessions by running `just build-test-app-avalonia`, next to the egui build (:70-77), and export `PLATYNUI_TEST_APP_AVALONIA_BIN` (design D6). A failed build ends the session with an error naming the .NET 10 SDK and `just build-test-app-avalonia`.

  Verify:
  - both Linux sessions print the build and the exported path;
  - with a fake `dotnet` that exits with `1` first on `PATH`, both `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor` end non-zero with that message, and no suite ran (spec: *A failing fixture build ends a Linux session before the suites*).
- [ ] 2.3 In the Wayland lane's session, start the skeleton through `PLATYNUI_TEST_APP_AVALONIA_BIN` and check that it runs through XWayland:
  - its process holds a connection to the X display and none to the Wayland display (1.2's method);
  - its application node and main window are found with `platynui-cli query`.

  This covers the second half of *The Wayland lane runs the fixture through XWayland*.
- [ ] 2.4 With `atspi-application-level` in place, check the popups on both Linux lanes (design, Risks). Do it inside `scripts/startxsession.sh --backend headless -- scripts/platynui-robot-session.sh <script>`, and again inside `scripts/startcompositor.sh --xwayland --backend headless -- scripts/platynui-robot-session.sh <script>`. Check each popup twice: once with the main window alone, and once with `dialog-modeless` open.
  - Open the menu, the context menu and the combo box through real pointer input (`platynui-cli pointer`).
  - Read the popup nodes with `platynui-cli query`:
    - their namespace, role and supported patterns;
    - `@Bounds` of an item, compared with its `native:Component.Extents.Screen` on X11, and with the compositor's window list on Wayland.
  - Click `menu-file-new` at its reported bounds.

  Record the result per lane and per case in the design's Risks. If a popup resolves to the wrong window and the item bounds are wrong in either case, that is a provider defect:
  - name the provider change it needs, with its own test for the single-window case;
  - mark the catalog's popup tests (menu, context menu, combo box) as depending on it. Do not skip or scope them away.
- [ ] 2.5 On a real Windows machine, build the skeleton and inspect it with the Inspector or `platynui-cli query`:
  - the main window's name is its title;
  - the roles of the skeleton's controls;
  - the menu, context menu and combo box items are reachable below the application node while their popup is open.

  Record every difference from the design's role table in the README's deviation list (4.3). For each behavior difference, decide which rule of design D8 applies before 3.2 encodes locators.

## 3. Suites first

- [ ] 3.1 Add `tests/acceptance/avalonia/__init__.robot` (`Test Tags acceptance real`, no platform tag) and `resources/avaloniaapp.resource` (design D7), following the `robot-test-style` skill. The launcher:
  - resolves the fixture from `PLATYNUI_TEST_APP_AVALONIA_BIN`, falling back to `apps/test-app-avalonia/build/platynui-test-app-avalonia` (`.exe` on Windows);
  - fails, never skips:
    - when that path names no file, naming `just build-test-app-avalonia`;
    - on Linux without an X display, naming `scripts/startcompositor.sh --xwayland`;
  - starts the executable with `--title` and optional extra arguments, logging to `${TEMPDIR}/avalonia-<title>.log`;
  - pins `/app:Application[@ProcessId=${pid}]` as the suite root, and waits for `.//(Frame|Window)[@Name="${title}"]`;
  - tears down with `Terminate Process ... kill=${True}`.

  Verify:
  - with the resolved path naming a missing file, the suite fails with that message (spec: *A missing fixture fails with guidance*);
  - a run inside an interactive session opened with `scripts/startcompositor.sh` without `--xwayland` fails with the X-display message (*A compositor session without XWayland fails with guidance*);
  - a run inside an interactive session opened with `--xwayland`, without the hand-over variable, finds the built fixture through the fallback (acceptance-lane-selection: *An interactive session opened with XWayland serves a lane run*).
- [ ] 3.2 Add `tests/acceptance/avalonia/catalog.robot`.
  - It replicates the canonical test set of `tests/acceptance/qml/catalog.robot`, with name-only locators below the application node (design D8).
  - Add the extended-tier tests for slider, progress bar and tabs, and a test that `table-basic` is absent.

  Verify: on the X11 lane (`just headless=true test-acceptance-x11 --suite '*.Avalonia.Catalog'`) it fails only on controls the skeleton does not have yet.
- [ ] 3.3 Add `tests/acceptance/avalonia/modal.robot`, replicating `tests/acceptance/qml/modal.robot` (design D7):
  - its suite setup starts its own instance with `--open-modal` and a title of its own;
  - it tests that `dialog-modal`, `dialog-modal-label` and `dialog-modal-button` exist without interaction;
  - it tests that a click on `dialog-modal-button` makes `last-action-dialog-modal-button` appear.

  Verify: it fails only because `dialog-modal` does not exist yet (spec: *A dialog is found by its canonical name*, *A click on the modal dialog's button lands inside the dialog*).
- [ ] 3.4 Add `tests/acceptance/avalonia/application_level.robot` with the two scenarios of *One application level on a real Avalonia tree*, without `native:` attributes.

  Verify: it passes against the skeleton on the X11 lane and on the Wayland lane (`--suite '*.Avalonia.ApplicationLevel'`).
- [ ] 3.5 Verify the selection: `robotcode -p real-x11 discover tests`, `-p real-wayland` and `-p real-windows` each list the Avalonia tests (spec: *Every lane selects the fixture's suites*).

## 4. Catalog

- [ ] 4.1 Complete the core tier in `apps/test-app-avalonia`, using only Avalonia's standard controls.
  - The controls, under their canonical names:
    - `status-label` with its `clicks-<n>` name, and the last-action label;
    - `checkbox-basic`, and a `GroupBox` `groupbox-basic` with `radio-first` and `radio-second`;
    - `textfield-basic`, `textarea-basic`, `label-basic`, `text-basic` and `image-basic`;
    - the combo box's items, `list-basic` with five items, and `tree-basic` with three levels;
    - the three menus with the `menu-edit-more` submenu, and the context menu with the `ctx-more` submenu.
  - The windows:
    - `dialog-modeless` and `dialog-modal`, as windows titled with their canonical names;
    - `--open-modal`.

  Verify: `catalog.robot` and `modal.robot` pass on the X11 lane and on the Wayland lane. The popup tests are the exception while 2.4 names an open provider dependency.
- [ ] 4.2 Add `slider-basic`, `progress-basic` and `tabs-basic` with `tab-one` and `tab-two`. Verify: the extended-tier tests of `catalog.robot` pass on both Linux lanes.
- [ ] 4.3 Write `apps/test-app-avalonia/README.md`, following `apps/test-app-qml/README.md`. It covers:
  - build and run, and the SDK pin;
  - the platform scope: X11, Wayland through XWayland, and Windows. It also says why the fixture does not use Avalonia's native Wayland backend, and that the lanes do not run under WSL;
  - the window-naming fallback;
  - the missing table, and that the custom-controls chapter is not implemented;
  - the deviations verified in 2.4 and 2.5, each with the rule of design D8 it falls under.

  Verify by reading it against the spec.

## 5. Windows lane and CI

- [ ] 5.1 In the `justfile`, make `test-acceptance-windows` run `just build-test-app-avalonia` as a hard prerequisite next to the Swing build, and export `PLATYNUI_TEST_APP_AVALONIA_BIN` with the others (:380-387).

  Verify on a Windows machine: `just test-acceptance-windows --profile real-windows run --suite '*.Avalonia.*'` passes.
- [ ] 5.2 In `.github/workflows/ci.yml` (`acceptance-linux`, :246-343):
  - add `xwayland` to the apt packages (:260-294);
  - add `actions/setup-dotnet` with `global-json-file: apps/test-app-avalonia/global.json` and NuGet caching keyed on `apps/test-app-avalonia/packages.lock.json`, for both matrix entries;
  - make the fixture's restore run in locked mode;
  - add `avalonia-*.log` to the failure log dump (:323-335).

  Verify: after the maintainer pushes, both matrix entries are green and run the Avalonia suites.

## 6. Documentation

- [ ] 6.1 `CONTRIBUTING.md`:
  - In §1 Prerequisites, add two entries:
    - the .NET 10 SDK, needed for the acceptance lanes;
    - `Xwayland`, needed for the Wayland lane (`xwayland` on Debian and Ubuntu, `xorg-xwayland` on Arch).

    State that `just check`, `just test` and the mock lane need neither.
  - In §8, the acceptance lanes, add two notes:
    - an interactive compositor session serves `real-wayland` runs only when opened with `scripts/startcompositor.sh --xwayland`;
    - the Linux acceptance lanes do not run under WSL.
  - In the recipe table, add `build-test-app-avalonia` and `run-test-app-avalonia`.

  Verify by reading.
- [ ] 6.2 `AGENTS.md`: add a ".NET fixture" bullet next to the Java workspace bullet (:17-18), naming `apps/test-app-avalonia` and `just build-test-app-avalonia`. Verify by reading.
- [ ] 6.3 `dev-docs/testing-strategy.md`:
  - In §5 (:255-267), move Avalonia from "Planned rows" into the matrix. The Avalonia row runs on X11, Wayland through XWayland, and Windows, and the text gives the reason it does not use the native Wayland backend.
  - In §5, replace "a lane needs only its own fixture built" (:270-271) with the model of design D6: each lane builds all of its fixtures before any suite runs.
  - In §2.6, state three things:
    - the Wayland lane's session provides XWayland, so X11-only fixtures run there;
    - a failing XWayland fails the lane;
    - an interactive session provides it only when opened with `--xwayland`.

  Verify by reading against `specs/acceptance-lane-selection/spec.md`.

## 7. Verification

- [ ] 7.1 Run `just check`, and `uv run --no-sync robotcode analyze code tests/acceptance/avalonia` without errors.
- [ ] 7.2 Run the full X11 lane, `just headless=true test-acceptance-x11`. It is green including the Avalonia suites; inspect with `robotcode results`.
- [ ] 7.3 Run the full Wayland lane, `just headless=true test-acceptance-compositor`. It is green including the Avalonia suites, and the fixture's window is served through XWayland (2.3's check).
- [ ] 7.4 On a Windows machine, run `just test-acceptance-windows`. It is green including the Avalonia suites.
  - Each Avalonia suite's teardown returns well within 30 s.
  - It leaves no `platynui-test-app-avalonia.exe` process behind (design D7).
- [ ] 7.5 Compare each lane's discovery with the state before this change: the only new tests are the Avalonia tests, on all three lanes.
