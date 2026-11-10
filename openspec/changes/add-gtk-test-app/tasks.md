# Tasks

## 1. Environment

- [ ] 1.1 Add the Linux-only `PyGObject>=3.52; sys_platform == 'linux'` dev dependency to `pyproject.toml` and a `[[tool.mypy.overrides]]` entry for `gi`/`gi.*` with `ignore_missing_imports`. Refresh `uv.lock`. Verify `just bootstrap` succeeds on a machine with the build dependencies and that `uv run --no-sync python -c "import gi; gi.require_version('Gtk', '4.0')"` exits 0.
- [ ] 1.2 Add the build-dependency check to `just bootstrap` on Linux (design decision 2): `pkg-config --exists girepository-2.0 cairo`, failing with the apt names `libgirepository-2.0-dev libcairo2-dev pkg-config gcc`. Verify that `PKG_CONFIG_LIBDIR=/nonexistent just bootstrap` stops with that message before syncing.
- [ ] 1.3 Add `apps/test-app-gtk3` and `apps/test-app-gtk4` to the root `Cargo.toml` `exclude` list and its comment. Verify `cargo metadata --no-deps` does not list them.
- [ ] 1.4 Install `libgirepository-2.0-dev` and `libcairo2-dev` in every Linux job of `.github/workflows/ci.yml` that runs `just bootstrap` or `uv sync`. Verify the jobs pass on a pushed branch.
- [ ] 1.5 `CONTRIBUTING.md`: the new Linux prerequisites for the dev environment, the GTK runtime and typelibs for the acceptance lanes, and that distributions with GLib older than 2.80 cannot build the environment. Verify the documented apt line installs on Ubuntu 24.04.

## 2. Selection

- [ ] 2.1 In `robot.toml`, add `platform:linux` to `real-windows`' excludes and describe `platform:linux` and the `requires:<toolkit>-<version>` tags in the tag-dimension comment. Verify `uv run --no-sync robotcode --profile real-windows discover tests` lists the same tests as before.
- [ ] 2.2 `dev-docs/testing-strategy.md` §2.6: `platform:linux`, and `requires:` tags as selection (excluded where the version is missing; a selected test fails at setup). Verify the section names both tags and says CI currently excludes `requires:gtk-4.18`.

## 3. GTK 4 fixture

- [ ] 3.1 Write the suites first, following the `robot-test-style` skill.
  - `tests/acceptance/gtk4/__init__.robot` has the tags `acceptance real platform:linux`.
  - `resources/gtk4app.resource` modelled on `qmlapp.resource`:
    - launch by venv interpreter, pin by `@ProcessId`, wait by title;
    - fail with the fixture log when the process exits first;
    - the `Require GTK Version` setup for tests tagged `requires:gtk-*`.
  - `catalog.robot` with the blueprint's canonical test set. Its menu, submenu, context-menu, context-submenu and combo-box tests are tagged `requires:gtk-4.18`.
  - Tests from the `gtk-test-app` spec:
    - the main window found by `--title`;
    - `--bogus` failing fast;
    - the header bar reported as client-side decorations under the compositor (tag `platform:wayland`);
    - the dialog sizes;
    - `radio-first` inside `groupbox-basic`.
  - `modal.robot` for `--open-modal`.

  Verify `robotcode --profile real-x11 discover tests` lists them and that the X11 lane fails them because the fixture does not exist yet.
- [ ] 3.2 Export `PLATYNUI_TEST_APP_GTK4_PYTHON`/`_MAIN`, `GTK_A11Y=atspi`, and unset `NO_AT_BRIDGE` in `scripts/platynui-robot-session.sh`. Add `just run-test-app-gtk4` and the fixture's own `mypy` invocation. Verify `just run-test-app-gtk4 --help` prints the usage.
- [ ] 3.3 Write `apps/test-app-gtk4/main.py`'s frame:
  - the PEP 723 header;
  - the prerequisite imports with their messages (design decision 5);
  - the CLI (`--title`, `--auto-close`, `--open-modal`, `--app-id`, unknown argument → usage, exit 2);
  - the application window with its header bar at 900×700;
  - `button-basic`, `status-label` and the last-action label.

  Verify the CLI, window-title and click-counter tests pass on both lanes:

  ```
  just headless=true test-acceptance-x11 -X --suite '*.Gtk4.Catalog' --include <those tests>
  ```

  and the same with `test-acceptance-compositor`. Also run the fixture once by hand with `GI_TYPELIB_PATH` set to an empty directory and record the message.
- [ ] 3.4 Add the remaining core-tier controls and the slider, progress bar and tab control per design decision 3, with the modeless dialog at 420×300 and the modal dialog. Before each test is relied on, confirm each surfaced `@Name` in the real tree with the Inspector or `Get Attribute Value`. Use the deprecated widget or a name-based observable only where the current widget does not expose what the test needs, and note it for the README. Verify every test of `catalog.robot` and `modal.robot` not tagged `requires:gtk-4.18` passes on both lanes.
- [ ] 3.5 Add the menu bar, context menu and drop-down from `Gio.Menu`/`Gtk.StringList`. Verify the `requires:gtk-4.18` tests pass on both lanes on the host's GTK (≥ 4.18). Run one of them with a faked lower version in `Require GTK Version` and confirm that the failure message names both versions and the exclude flag.
- [ ] 3.6 Write `apps/test-app-gtk4/README.md`: usage, the widget choices, every verified deviation, and the popover version requirement. Add the GTK 4 row to `dev-docs/testing-strategy.md` §5 and the fixture to `AGENTS.md` where it lists fixtures. Verify that the README's usage commands run as written.
- [ ] 3.7 Run the whole GTK 4 tree on both lanes: `just headless=true test-acceptance-x11 -X --suite '*.Gtk4'` and the same with `test-acceptance-compositor`. Verify with `robotcode results` that all tests pass.

## 4. GTK 3 fixture (after `linux-element-geometry` groups 1–5)

- [ ] 4.1 Confirm that `linux-element-geometry` tasks 1.1–5.3 are done. Without them GTK 3's pointer tests miss (proposal, Depends on). Verify by its tasks.md.
- [ ] 4.2 Write `tests/acceptance/gtk3/` first, mirroring group 3:
  - `__init__.robot`;
  - `resources/gtk3app.resource`;
  - `catalog.robot` with the canonical test set and no version tag;
  - the `gtk-test-app` spec tests;
  - `modal.robot`.

  Verify that discovery lists them and that the lane fails them without the fixture.
- [ ] 4.3 Export `PLATYNUI_TEST_APP_GTK3_PYTHON`/`_MAIN` and add `just run-test-app-gtk3` and its `mypy` invocation. Verify `just run-test-app-gtk3 --help`.
- [ ] 4.4 Write `apps/test-app-gtk3/main.py` per design decisions 3–5, with every window's ATK name set to its title. Confirm each surfaced `@Name` in the real tree before relying on it. Verify the whole tree passes on both lanes: `just headless=true test-acceptance-x11 -X --suite '*.Gtk3'` and the same with `test-acceptance-compositor`.
- [ ] 4.5 Write `apps/test-app-gtk3/README.md` and add the GTK 3 row to `testing-strategy.md` §5. Verify that the README's usage commands run as written.

## 5. CI

- [ ] 5.1 In the acceptance job of `ci.yml`:
  - install `libgtk-3-0 libgtk-4-1 gir1.2-gtk-3.0 gir1.2-gtk-4.0`;
  - print `pkg-config --modversion gtk4` into the log;
  - pass `--exclude requires:gtk-4.18` to the lane recipe, with a comment saying to drop it once the runner's GTK 4 is 4.18 or later;
  - add the GTK fixtures' logs to the failure dump.

  Verify on a pushed branch that both matrix entries pass and that the results list the GTK suites without the tagged tests.
- [ ] 5.2 Run the acceptance job once without the exclusion. Verify that the tagged tests fail at setup with the message naming the runner's GTK 4 version (spec *A menu test on an older GTK 4 fails with guidance*), then restore the exclusion.

## 6. Integration

- [ ] 6.1 Run:
  - `just check`;
  - `uv run --no-sync robotcode --profile real-windows discover tests`, to confirm no GTK suite there;
  - both Linux lanes in full: `just headless=true test-acceptance-x11 -X` and `just headless=true test-acceptance-compositor -X`.

  Verify that all pass and that the GTK suites ran on both lanes.

## Workflow follow-up

- After group 4, `linux-element-geometry` can run its GTK group 7 against these fixtures.
