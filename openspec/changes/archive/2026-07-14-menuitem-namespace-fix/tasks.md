## 1. Reclassify menu-entry roles (AT-SPI)

- [x] 1.1 In `crates/provider-atspi/src/node.rs` `map_role`, change `MenuItem`, `CheckMenuItem`, and `RadioMenuItem` from `Namespace::Item` to `Namespace::Control` (role name stays `MenuItem`).
- [x] 1.2 Change `TearoffMenuItem` from `Namespace::Item` to `Namespace::Control` (role name stays `TearoffMenuItem`).
- [x] 1.3 Confirm `Menu`, `MenuBar`, `PopupMenu` remain `Namespace::Control` (no change expected).

## 2. Update Rust unit tests

- [x] 2.1 Update the co-located `map_role_menu_item_is_item_namespace` / `map_role_check_menu_item_maps_to_menu_item` tests (and any radio/tearoff equivalents) to assert `Namespace::Control`; rename the `*_is_item_namespace` test(s) to reflect control.
- [x] 2.2 Add/keep a test asserting a genuine collection item (e.g. `ListItem`, `TreeItem`) still maps to `Namespace::Item`, so the reclassification stays scoped.
- [x] 2.3 Run `just test-crate platynui-provider-atspi` (or `just test`) and confirm green.

## 3. Check the Python test layer

- [x] 3.1 Review `tests/PlatynUI/test_menus.py` and `tests/PlatynUI/test_proxies.py`: these key on the role name `MenuItem` (not the XPath namespace), so they are expected to stay green. Update only if any assertion pins the `item` namespace for a menu entry.
- [x] 3.2 Grep the Python tests for `item:MenuItem`, `namespace='item'`, or equivalent menu-entry namespace assertions and fix any that encode the old classification.
- [x] 3.3 Run `just test-python` and confirm the menu/proxy tests pass against the freshly built native.

## 4. Update documentation and examples

- [x] 4.1 In `src/PlatynUI/BareMetal/__init__.py`, correct the namespace legend so the `item:` row no longer lists `MenuItem` (keep `ListItem`, `TabItem`, `TableCell`, ...).
- [x] 4.2 Grep the repo for `item:MenuItem` and other `MenuItem`-under-`item` references in docs (`dev-docs/`, `docs/`, READMEs) and correct them.

## 5. Robot Framework acceptance tests

- [x] 5.1 Update or remove the untracked `tests/acceptance/qt/menu_hit_test_scratch.robot` locators that use `//item:MenuItem` — switch to `//MenuItem`. *(File no longer exists — nothing to update.)*
- [x] 5.2 Review the egui menu locators (`tests/acceptance/egui/*.robot`, `resources/testapp_locators.resource`): menu-bar entries are `Button`; `wait.robot` already uses an unprefixed `(Button|MenuItem)` union, so no `item:` prefix needs removing. Confirm nothing still queries `item:MenuItem`. *(Confirmed; also switched the Qt locators from the `//*:MenuItem` wildcard workaround to plain `//MenuItem`.)*
- [x] 5.3 Add or extend a committed menu acceptance test that resolves an open menu entry via `//MenuItem` (control namespace) against a real provider — Qt is the natural home since egui/AccessKit does not surface popup menu items reliably (see `hit_test.robot` note). Keep it minimal (open menu → `//MenuItem` matches → entry activates). *(New `tests/acceptance/qt/menu.robot`; File-menu actions now rename to `<ident>-activated` on trigger so activation is observable by locator; includes the `//item:MenuItem` negative check.)*

## 6. Verify against a real provider

- [x] 6.1 Rebuild native so the RF layer observes the new namespace (`just test-python` builds it, or the appropriate native build recipe). *(Non-mock `just build-native` via the acceptance-lane recipe.)*
- [x] 6.2 Run the egui and Qt menu acceptance suites against a real AT-SPI provider; confirm `//MenuItem` (control namespace) resolves menu entries and menu open/activate flows still pass. *(Headless compositor lane 2026-07-14 23:06: 51/51 PASS incl. all egui/Qt menu, context-menu, and hit-test suites.)*
- [x] 6.3 Confirm `//item:MenuItem` no longer matches on AT-SPI (negative check per the spec scenario). *(`Node Is Absent //item:MenuItem[@Name="menu-file-new"]` step inside the passing `Acceptance.Qt.Menu` test.)*

## 7. Gate

- [x] 7.1 Run `just check` and `just pre-commit`; resolve any clippy/fmt/lint/ruff/mypy findings. *(Both green, no findings.)*
