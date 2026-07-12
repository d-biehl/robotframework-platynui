## Why

The `item` XPath namespace is defined as "items belonging to container controls (ListItem, TreeItem, ...)" — the data rows, cells, and entries a collection widget owns. `MenuItem` does not fit that definition: it is an interactive control (invokable like a button, may own a submenu), not a data item of a collection like a list row or a combo-box entry. Today the AT-SPI provider hard-codes menu entries into `Namespace::Item`, so on Linux they are queried as `//item:MenuItem`. This is wrong on three counts:

- It contradicts the namespace's own definition.
- It disagrees with the Windows UIA provider, which classifies by `IsControlElement`/`IsContentElement` and therefore already emits menu items in the `control` namespace — so `//item:MenuItem` (Linux) vs `//MenuItem` (Windows) makes the same logical locator non-portable.
- It disagrees with the Python high-level model, where `MenuItem` subclasses `Control` and `Menu` is deliberately *not* an `ItemContainer[MenuItem]`.

## What Changes

- **BREAKING**: The AT-SPI provider classifies menu-entry roles in the `control` namespace instead of `item`. Affected AT-SPI roles: `MenuItem`, `CheckMenuItem`, `RadioMenuItem`, `TearoffMenuItem`. After this change they are matched by `//MenuItem` / `//TearoffMenuItem` (default `control` namespace), and `//item:MenuItem` no longer matches on Linux.
- No change to the Windows UIA provider — it already places menu items in `control` via `IsControlElement`; this change brings AT-SPI into line with existing Windows behavior.
- The `Menu`, `MenuBar`, and `PopupMenu` container roles are unaffected (already `control`).
- Table/list/tree/tab data-item roles are unaffected and remain in `item` (`ListItem`, `TreeItem`, `TableCell`, `TableRow`, `TabItem`, header roles, ...). Reclassifying header roles is explicitly out of scope.
- Documentation and namespace-example text that lists `MenuItem` under `item:` is corrected (notably the `PlatynUI.BareMetal` namespace legend).

## Capabilities

### New Capabilities
- `menu-item-namespace`: Menu-entry roles (MenuItem and its check/radio/tearoff variants) are classified in the default `control` XPath namespace across providers, consistent with the definition of the `item` namespace (collection data items only) and with the Windows provider's control/content classification.

### Modified Capabilities
<!-- No existing spec defines role/namespace classification; this is a new capability. -->

## Impact

- **Rust**: `crates/provider-atspi/src/node.rs` — `map_role` entries for `MenuItem`, `CheckMenuItem`, `RadioMenuItem`, `TearoffMenuItem`, plus the co-located `map_role_*_is_item_namespace` unit tests. Requires a native rebuild (`platynui_native`) for the Python/RF layers to observe the new namespace.
- **Providers/platforms**: AT-SPI (Linux X11 + Wayland/AccessKit) behavior changes; Windows UIA and macOS AX unchanged. See the README platform-support table for backend status.
- **Python/RF**: No source change to `src/PlatynUI/ui/menus.py` (already `Control`-based). The `PlatynUI.BareMetal` namespace legend and any docs/examples referencing `item:MenuItem` are updated. Existing user locators using `//item:MenuItem` on Linux must migrate to `//MenuItem`.
- **Tests**: AT-SPI unit tests updated. Python unit tests (`tests/PlatynUI/test_menus.py`, `test_proxies.py`) key on the role name `MenuItem` rather than the namespace, so they are expected to stay green but are re-run and checked. A committed Qt menu acceptance test verifies `//MenuItem` (control namespace) resolves against a real provider; the untracked `tests/acceptance/qt/menu_hit_test_scratch.robot` scratch locators (`//item:MenuItem[...]`) reflect the old behavior and are updated or removed. egui menu-bar entries are `Button`s and the existing `(Button|MenuItem)` union locator needs no `item:` removal.
