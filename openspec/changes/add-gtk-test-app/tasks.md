# Tasks

## 1. Environment

- [x] 1.1 Add the Linux-only `PyGObject>=3.52; sys_platform == 'linux'` dev dependency to `pyproject.toml` and a `[[tool.mypy.overrides]]` entry for `gi`/`gi.*` with `ignore_missing_imports`. Refresh `uv.lock`. Verify `just bootstrap` succeeds on a machine with the build dependencies and that `uv run --no-sync python -c "import gi; gi.require_version('Gtk', '4.0')"` exits 0.
- [x] 1.2 Add the build-dependency check to `just bootstrap` on Linux (design decision 2): `pkg-config --exists girepository-2.0 cairo`, failing with the apt names `libgirepository-2.0-dev libcairo2-dev pkg-config gcc`. Verify that `PKG_CONFIG_LIBDIR=/nonexistent just bootstrap` stops with that message before syncing.
- [x] 1.3 Add `apps/test-app-gtk3` and `apps/test-app-gtk4` to the root `Cargo.toml` `exclude` list and its comment. Verify `cargo metadata --no-deps` does not list them.
- [ ] 1.4 Install `libgirepository-2.0-dev` and `libcairo2-dev` in every Linux job of `.github/workflows/ci.yml` that runs `just bootstrap` or `uv sync`. Verify the jobs pass on a pushed branch.
- [x] 1.5 `CONTRIBUTING.md`: the new Linux prerequisites for the dev environment, the GTK runtime and typelibs for the acceptance lanes, and that distributions with GLib older than 2.80 cannot build the environment. Verify the documented apt line installs on Ubuntu 24.04.

## 2. Selection

- [x] 2.1 In `robot.toml`, add `platform:linux` to `real-windows`' excludes and describe `platform:linux` in the tag-dimension comment. Verify `uv run --no-sync robotcode --profile real-windows discover tests` lists the same tests as before.
- [x] 2.2 `dev-docs/testing-strategy.md` §2.6: `platform:linux`. Verify the section names the tag.

## 3. GTK 4 fixture

- [x] 3.1 Write the suites first, following the `robot-test-style` skill.
  - `tests/acceptance/gtk4/__init__.robot` has the tags `acceptance real platform:linux`.
  - `resources/gtk4app.resource` modelled on `qmlapp.resource`:
    - launch by venv interpreter, pin by `@ProcessId`, wait by title;
    - fail with the fixture log when the process exits first;
  - `catalog.robot` with the blueprint's canonical test set.
  - Tests from the `gtk-test-app` spec:
    - the main window found by `--title`;
    - `--bogus` failing fast;
    - the header bar reported as client-side decorations under the compositor (tag `platform:wayland`);
    - the dialog sizes;
    - `radio-first` inside `groupbox-basic`.
  - `modal.robot` for `--open-modal`.

  Verify `robotcode --profile real-x11 discover tests` lists them and that the X11 lane fails them because the fixture does not exist yet.
- [x] 3.2 Export `PLATYNUI_TEST_APP_GTK4_PYTHON`/`_MAIN`, `GTK_A11Y=atspi`, and unset `NO_AT_BRIDGE` in `scripts/platynui-robot-session.sh`. Add `just run-test-app-gtk4` and the fixture's own `mypy` invocation. Verify `just run-test-app-gtk4 --help` prints the usage.
- [x] 3.3 Write `apps/test-app-gtk4/main.py`'s frame:
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
- [x] 3.4 Add the remaining core-tier controls and the slider, progress bar and tab control per design decision 3, with the modeless dialog at 420×300 and the modal dialog. Before each test is relied on, confirm each surfaced `@Name` in the real tree with the Inspector or `Get Attribute Value`. Use the deprecated widget or a name-based observable only where the current widget does not expose what the test needs, and note it for the README. Verify every test of `catalog.robot` and `modal.robot` that opens no popover passes on both lanes.
- [x] 3.5 Add the menu bar, context menu and drop-down from `Gio.Menu`/`Gtk.StringList`, and name every menu and context-menu item by its label each time its popover opens (design decision 9). Verify on both lanes, on the host's GTK 4.24.1:
  - every popover opens, and each of its items is found by its canonical `@Name`, submenus included;
  - which pointer activations inside a popover miss with today's provider, recorded for `linux-element-geometry` 5.1.

  Commit the fixture and `catalog.robot` without the popover tests, which stay out of this change's commits while they fail. `linux-element-geometry` adds them in its task 5.1 and commits them with its task 5.4.
- [x] 3.6 Write `apps/test-app-gtk4/README.md`: usage, the widget choices, every verified deviation, the supported GTK 4 versions, and the naming of popover menu items with the GTK commits that drop and restore the names, noting that real applications on GTK 4.18 to 4.24.1 expose their menu items unnamed. Add the GTK 4 row to `dev-docs/testing-strategy.md` §5 and the fixture to `AGENTS.md` where it lists fixtures. Verify that the README's usage commands run as written.
- [x] 3.7 Run the whole GTK 4 tree on both lanes: `just headless=true test-acceptance-x11 -X --suite '*.Gtk4'` and the same with `test-acceptance-compositor`. Verify with `robotcode results` that all committed tests pass.
- [x] 3.8 Once `linux-element-geometry` task 5.4 is done, run the whole GTK 4 tree again on both lanes, now with the popover tests. Verify with `robotcode results` that all tests pass.

## 4. GTK 3 fixture

- [x] 4.1 Write `tests/acceptance/gtk3/` first, mirroring group 3:
  - `__init__.robot`;
  - `resources/gtk3app.resource`;
  - `catalog.robot` with the canonical test set;
  - the `gtk-test-app` spec tests;
  - `modal.robot`.

  Verify that discovery lists them and that the lane fails them without the fixture.
- [x] 4.2 Export `PLATYNUI_TEST_APP_GTK3_PYTHON`/`_MAIN` and add `just run-test-app-gtk3` and its `mypy` invocation. Verify `just run-test-app-gtk3 --help`.
- [x] 4.3 Write `apps/test-app-gtk3/main.py` per design decisions 3–5, with every window's ATK name set to its title. Confirm each surfaced `@Name` in the real tree before relying on it. Verify on both lanes (`just headless=true test-acceptance-x11 --suite '*.Gtk3'` and the same with `test-acceptance-compositor`) that every test not using the pointer passes, and that the pointer tests fail by missing their target, not by a fixture error. The fixture is committed with `catalog.robot` without its pointer tests; those are added in `linux-element-geometry`'s commit for its task 5.4.
- [x] 4.4 Write `apps/test-app-gtk3/README.md` and add the GTK 3 row to `testing-strategy.md` §5. Verify that the README's usage commands run as written.
- [x] 4.5 Once `linux-element-geometry` task 5.4 is done, run the whole GTK 3 tree on both lanes: `just headless=true test-acceptance-x11 -X --suite '*.Gtk3'` and the same with `test-acceptance-compositor`. Verify with `robotcode results` that all tests pass.

## 5. CI

- [ ] 5.1 In the acceptance job of `ci.yml`:
  - install `libgtk-3-0 libgtk-4-1 gir1.2-gtk-3.0 gir1.2-gtk-4.0`;
  - print the GTK 3 and GTK 4 versions the fixtures load into the log, through PyGObject in the project venv
    (`pkg-config --modversion gtk4` would need `libgtk-4-dev`, which the job does not install);
  - add the GTK fixtures' logs to the failure dump.

  Verify on a pushed branch that the GTK suites pass in both matrix entries and that the log shows a GTK 4 of at least 4.18.
- [ ] 5.2 After `linux-element-geometry` task 5.4, verify in a CI run that the GTK 4 popover tests passed in both matrix entries (spec *CI runs a supported GTK 4*).

## 6. Integration (after 3.8 and 4.5)

- [x] 6.1 Run:
  - `just check`;
  - `uv run --no-sync robotcode --profile real-windows discover tests`, to confirm no GTK suite there;
  - both Linux lanes in full: `just headless=true test-acceptance-x11 -X` and `just headless=true test-acceptance-compositor -X`.

  Verify that all pass and that the GTK suites ran on both lanes.

## Workflow follow-up

- `linux-element-geometry` builds its GTK scenarios on these fixtures from its group 3 on.
- Archive after 3.8, 4.5, 5.2 and 6.1.
