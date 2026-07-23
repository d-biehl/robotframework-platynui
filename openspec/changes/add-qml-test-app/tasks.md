## 1. Fixture app

- [x] 1.1 Scaffold `apps/test-app-qml`: PEP 723 `main.py` (argparse: `--title`, `--auto-close`, `--open-modal`, `--app-id`, `--popup-mode`, `--log-level`; usage + non-zero exit on unknown args; `QQmlApplicationEngine` loading `main.qml`), README, root `Cargo.toml` `exclude` entry, `[tool.mypy] files` entry
- [x] 1.2 Core-tier catalog in QML under the canonical blueprint names via explicit `Accessible.name`: `button-basic` + `status-label` (`clicks-<n>`), `checkbox-basic`, `groupbox-basic` (+`radio-first`/`radio-second`), `textfield-basic`, `textarea-basic` (multi-line), `label-basic`, `text-basic`, `image-basic`, `combobox-basic` (+`combo-item-*`), `list-basic` (+`list-item-1..5`), `tree-basic` (three levels, `tree-node-*`), `main-menubar` with `menu-file`/`menu-edit` (+submenu `menu-edit-more`)/`menu-help` (+items with `<ident>-activated` rename), `context-menu` (+items, submenu `ctx-more`, `ctx-sub-*`) — groupbox/text/image/textarea/extra menus were added to the core tier via this change's blueprint delta
- [x] 1.3 Dialogs: `dialog-modeless` as child `Window`, `dialog-modal` as in-scene modal `Dialog` (opened by `--open-modal`), each with `<ident>-button`/`<ident>-label` and the `-clicked` rename observable
- [x] 1.4 Popup modes: default in-scene; `--popup-mode native` switches menus/popups to native windows via Qt ≥ 6.8 `popupType`/native menu APIs, names unchanged between modes
- [x] 1.5 Custom-controls chapter: `custom-button` (`Rectangle`+`MouseArea`, manual `Accessible` role/name/focus) with `custom-status-label` counter, plus one unwired drawn element documented in the README as expected-absent

## 2. Reality verification (before encoding tests)

- [x] 2.1 Windows/UIA: verify every canonical name, role, and observable surfaces as expected (Inspector / `Get Attribute` against the running fixture); record deviations in the README
- [x] 2.2 Linux/AT-SPI (X11): same verification; note in-scene popup exposure behavior (top-down visibility vs event-driven grafting) and modal-state surfacing; record deviations and derive the documented-skip list
- [x] 2.3 Feed any blueprint friction (names, observables, tier content) back into the open `test-app-blueprint` change before either change is archived

## 3. Shared catalog suite + QML onboarding

- [x] 3.1 `tests/acceptance/resources/` catalog resource: technology-neutral Given/When/Then keywords for the core-tier catalog, canonical-name locators only, launch/session injected by the consuming suite (model: `tests/acceptance/qt/resources/testapp.resource`)
- [x] 3.2 `tests/acceptance/qml/catalog.robot`: thin onboarding suite — launch config from `PLATYNUI_TEST_APP_QML_PYTHON`/`PLATYNUI_TEST_APP_QML_MAIN`, actionable skip when unset, documented skips for verified bridge limitations, tags per existing `real`/platform conventions
- [x] 3.3 QML-specific suites beside it: popup modes (in-scene drivable incl. submenu; native mode as verified per platform), both dialog faces (bounds-correct clicks), custom-controls chapter (drivable + expected-absent assertion)

## 4. Lane wiring

- [x] 4.1 `scripts/platynui-robot-session.sh`: export `PLATYNUI_TEST_APP_QML_*` beside the Qt Widgets entries
- [x] 4.2 `justfile`: `test-acceptance-windows` env wiring for the QML fixture; `run-test-app-qml *ARGS` convenience recipe
- [x] 4.3 Docs: fixture-app pointers (README table / AGENTS orientation if listed) mention the QML fixture as first blueprint instance

## 5. Verification

- [x] 5.1 `just check` green (ruff + strict mypy cover `main.py`); fixture starts standalone via `uv run apps/test-app-qml/main.py` and honors `--auto-close`
- [x] 5.2 Windows acceptance lane green including the QML suites (with the documented-skip list applied)
- [x] 5.3 Linux acceptance lane green including the QML suites; skips match the verified deviation list, none silent
- [x] 5.4 `openspec validate add-qml-test-app` passes; blueprint reconciliation (task 2.3) confirmed done

## 6. Last-action observable + suite simplification (post-review)

- [x] 6.1 Replace rename-on-activate with the always-visible `last-action-<ident>` label (fixture `Main.qml`, blueprint observables delta, qml spec delta, README) — no control changes its name on activation anymore
- [x] 6.2 Simplify the suites: launcher pins the instance via `Set Root` (SUITE scope), locators relative, hand-rolled wait/click wrappers replaced by BareMetal's `Wait Until Exists`/`Wait Until Gone`/built-in action waits, single-use keywords inlined into their suites
- [x] 6.3 Linux lanes (X11 + compositor) re-run green after the redesign
- [x] 6.4 Dissolve the shared keyword resource (`tests/acceptance/resources/`): catalog/popups test bodies are self-contained in their suites; blueprint spec + testing-strategy reframed to "canonical test set, not shared keyword code"
- [ ] 6.5 Windows lane re-run + spot-check of the last-action observable on UIA (redoes the 2.1/5.2 facts the redesign touches)
