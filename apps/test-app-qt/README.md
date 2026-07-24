# PlatynUI Qt test app (PySide6)

A Qt test/fixture application mirroring the role of the egui test app
([`apps/test-app-egui`](../test-app-egui)) for the *native-widget tier* in
[`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5. Unlike
egui's flat AccessKit tree, it exposes real native Qt controls.

It is **Python**, not a Cargo crate (so it is `exclude`d from the workspace in
the root `Cargo.toml`) — that way we get Qt from the PySide6 wheels without
compiling Qt. PySide6 is a **development dependency** of the main project (in
`[dependency-groups] dev`), so a normal `uv sync` installs it. There is no
separate venv — the app runs on the project's own interpreter.

## Current purpose: reproduce Qt dialog window-bounds bugs

There is **no `QMdiArea`**. The app simply creates several `QDialog` instances
**parented to the main window** (`QDialog(main_window)`) and shows them at known
positions inside it — the pattern the affected real app uses. Three modeless
child dialogs open at startup (`--dialogs N` limits how many); `--open-modal`
additionally opens a modal dialog. A **ground-truth geometry readout** in the
status bar shows the geometry Qt itself reports for each window, so you can
compare it against PlatynUI's `@Bounds`.

Each window's `geometry()` is its **client rect in global screen coordinates**
(excludes WM decorations) — the same rect PlatynUI derives on X11 from
`translate_coordinates(client origin)` + `get_geometry(size)`.

The point of interest: a parented `QDialog` is a top-level native window, but
where Qt's AT-SPI bridge hangs it in the accessible tree (under the main window
vs. directly under the Application) decides whether PlatynUI resolves its bounds
via the window manager or by summing parent-relative offsets up the tree. The
Inspector shows which happens.

> **X11 only.** Qt only knows a window's absolute position under X11. Under
> Wayland a client cannot know its global position; the readout marks those
> values as unavailable and the PlatynUI compositor is the source of truth.

## Run

```sh
# On the project venv (PySide6 is a dev dependency, installed by `uv sync`):
uv run python apps/test-app-qt/main.py

# Standalone (no project sync): PEP 723 inline metadata auto-installs PySide6:
uv run apps/test-app-qt/main.py

# Options mirror the egui app:
uv run apps/test-app-qt/main.py --app-id com.platynui.test.qt --title "My Qt Window"
uv run apps/test-app-qt/main.py --auto-close 10          # self-close for CI
uv run apps/test-app-qt/main.py --log-level info         # error|warn|info|debug
uv run apps/test-app-qt/main.py --open-modal             # also open a modal dialog at startup
uv run apps/test-app-qt/main.py --dialogs 1              # open only N child dialogs (avoids overlap)
```

Each widget carries a stable identity via `accessibleName` (surfaced as `@Name`
on the AT-SPI / UIA tree — verified against the running app; `@Id` is a compound
object path and is not used for locating). `objectName` is set too, for Qt-side
tooling. The three child dialogs use **distinct sizes** so tests can assert they
are pairwise different. Activating a dialog's "Click Me" button or a File-menu
action reports its ident through the always-visible `last-action-<ident>` status
label (`last-action-none` until the first activation) — a name-based observable
that the activation really happened (for the dialog buttons: that the click
landed inside the dialog), while every control keeps its stable name.

## Acceptance lane

Wired into the real-provider lane under
[`tests/acceptance/qt`](../../tests/acceptance/qt) (profile `real`, tags `acceptance` + `real`):
`bounds.robot` (the regression + correctness checks), `interaction.robot` (click
lands in the dialog), and `modal.robot` (modal opened via `--open-modal`). The
project interpreter + entrypoint are handed over via `PLATYNUI_TEST_APP_QT_PYTHON`
/ `PLATYNUI_TEST_APP_QT_MAIN` by `scripts/platynui-robot-session.sh` (Linux) and
the `test-acceptance-windows` recipe (Windows); Robot Framework launches that
interpreter directly so the started PID is the app's PID (`@ProcessId` pinning).

`ruff` and strict `mypy` both cover the app in `just check`. The `mypy` recipe
checks `main.py` in its own invocation (the Python fixture apps are standalone
PEP 723 scripts that all share the module name `main`, which one mypy run
rejects as duplicates — see the note in `pyproject.toml`); PySide6's stubs come
from the dev group.
