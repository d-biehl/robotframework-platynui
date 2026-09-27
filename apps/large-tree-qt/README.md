# PlatynUI large-tree app (PySide6)

A Qt window with a large, deterministic accessibility tree, and the script that
measures what a UI snapshot costs in memory against it.

This is a **measurement helper, not a fixture** of the test-app blueprint in
[`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5:
- it has no control catalog and no acceptance suite;
- it only holds many plain widgets under stable names, so that a measurement can
  build large snapshots against an application the repository owns, with the
  same tree on every run.

Like [`apps/test-app-qt`](../test-app-qt), it is Python, not a Cargo crate, and
runs on the project's interpreter (PySide6 is a development dependency).

## The app

```sh
uv run python apps/large-tree-qt/main.py --groups 50 --items 20
```

The window holds `--groups` group boxes, `group-<g>`, with `--items` widgets
each, `item-<g>-<i>`. The widgets cycle through a push button, a label and a
check box. Every widget carries its name as `accessibleName`, which Qt exposes
through UI Automation on Windows and AT-SPI on Linux. The label
`widget-count-<n>` names how many widgets the tree holds, groups included.

## The measurement

```sh
uv run python apps/large-tree-qt/measure_snapshot_memory.py [--groups 50] [--items 20] [--runs 100] [--rounds 3]
```

The script starts the app and waits until its widgets are on the tree. It finds
them through the application node of the process it launched, and scopes every
query to that node; nothing searches the whole desktop.

It then evaluates `.//*[@Name='x-not-there']` under that node. The search
matches nothing, so it walks the whole tree. It runs in two modes:
- **retained:** the snapshot is kept between evaluations;
- **discarded:** `clear_cache()` runs before each evaluation.

For each mode it prints, per round:
- how much the process's private memory grew per evaluation and per element;
- how long an evaluation took.

It also prints the time of `clear_cache()` after a full snapshot. `--json <file>`
writes the same report as JSON.

Private memory is read as follows:
- **Windows:** `PrivateUsage` from `GetProcessMemoryInfo`.
- **Linux:** anonymous and swapped memory from `/proc/self/smaps_rollup`.

Run it against a release build (`just release=true build-native`): a debug build
allocates differently.

**A flat result needs a positive control.** Run the same script with the
interpreter of an environment whose native extension still leaks, for example a
git worktree of an older commit with its own `uv sync` and build:

```sh
<worktree>/.venv/Scripts/python apps/large-tree-qt/measure_snapshot_memory.py
```

On that build the discarded mode must grow clearly. Only then does a flat result
on the current build mean that nothing leaks, rather than that the measurement
cannot see a leak.

**On Linux** the script belongs inside the lanes' session scripts, which bring up
AT-SPI and enable Qt's accessibility:

```sh
scripts/startxsession.sh -- scripts/platynui-robot-session.sh \
    uv run python apps/large-tree-qt/measure_snapshot_memory.py
```

The same works with `scripts/startcompositor.sh` for the PlatynUI compositor.

The script only reads: it gives no pointer or keyboard input. It ends the app
when it is done; the app also closes itself after `--lifetime` seconds.
