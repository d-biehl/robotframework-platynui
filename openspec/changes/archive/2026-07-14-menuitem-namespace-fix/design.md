## Context

Element namespaces (`control`, `item`, `app`, `native`) are defined once in `crates/core/src/ui/namespace.rs` and used as XPath element-name prefixes. `control` is the default (an unprefixed `//Button` means `//control:Button`); `item` is documented as "items belonging to container controls (ListItem, TreeItem, ...)".

The namespace of a node is produced by each provider:

- **AT-SPI** (`crates/provider-atspi/src/node.rs`, `map_role`) uses a static `Role -> (Namespace, name)` table. Menu entries are hard-coded into `Namespace::Item`: `MenuItem`, `CheckMenuItem`, `RadioMenuItem` (all named `MenuItem`) and `TearoffMenuItem`.
- **Windows UIA** (`crates/provider-windows-uia/src/node.rs`) derives the namespace dynamically: `IsControlElement -> control`, else `IsContentElement -> item`, else `control`. Menu items are control elements, so they already resolve to `control`.
- **macOS AX** has no role/namespace mapping yet.

The Python high-level layer already treats a menu entry as a control: `src/PlatynUI/ui/menus.py` declares `class MenuItem(Control)` and comments that `Menu` is intentionally *not* an `ItemContainer[MenuItem]`.

The result is an inconsistency: the same logical menu entry is `//item:MenuItem` on Linux but `//control:MenuItem` on Windows, and neither matches the definition of the `item` namespace.

## Goals / Non-Goals

**Goals:**
- Classify menu entries in the `control` namespace on AT-SPI, matching the `item`-namespace definition, the Windows provider, and the Python model.
- Make menu-entry locators portable across AT-SPI and Windows UIA.
- Correct documentation/examples that list `MenuItem` under `item:`.

**Non-Goals:**
- No change to Windows UIA or macOS AX providers (Windows is already correct; macOS has no mapping).
- No reclassification of genuine collection data-item roles (`ListItem`, `TreeItem`, `TableCell`, `TableRow`, `TabItem`) or of header roles (`ColumnHeader`, `RowHeader`, `TableColumnHeader`, `TableRowHeader`). Header reclassification is a separate discussion.
- No change to the Python `MenuItem` class hierarchy or the `MenuItemProxy` adapter proxy — `@pattern_proxy_for(role='MenuItem')` matches on role name, not namespace, and keeps working.

## Decisions

**Decision: Fix the AT-SPI static table rather than adding a core override.**
The wrong classification lives entirely in the AT-SPI `map_role` table. Change the four menu-entry entries from `Namespace::Item` to `Namespace::Control`. Windows needs no change because its content/control rule already yields `control`. Adding a cross-provider "menu roles are always control" shim in `crates/core` was considered and rejected: it would add indirection for a one-line-per-role table fix, and the core has no role table to hook into (namespace is a provider responsibility).

**Decision: Move all four menu-entry roles together.**
`CheckMenuItem` and `RadioMenuItem` already emit the role name `MenuItem`; leaving them in `item` while `MenuItem` moves to `control` would split identical role names across two namespaces. `TearoffMenuItem` is likewise a menu entry. All four move so the `control` namespace holds every menu entry uniformly.

**Decision: Treat this as a documented breaking change, no compatibility shim.**
The project is pre-1.0 (`0.13.0-dev`). A dual-namespace transition (matching both `item:` and `control:`) would entrench the very inconsistency being removed. Instead, flag BREAKING in the proposal and give the one-line migration (`//item:MenuItem` -> `//MenuItem`).

## Risks / Trade-offs

- **[Existing user locators using `//item:MenuItem` on Linux break.]** -> Documented BREAKING with a trivial migration (`//MenuItem`, or `//control:MenuItem`). Pre-1.0 status makes this acceptable; the repo's own suites use `//MenuItem` (e.g. egui `wait.robot`) so the new form is already the norm.
- **[Cross-provider drift could reappear if a future provider hard-codes menu entries into `item`.]** -> The `menu-item-namespace` spec pins the intended classification and its portability scenario documents the contract for new providers.
- **[Scope creep toward header roles.]** -> Explicitly deferred in Non-Goals; this change only touches the four menu-entry roles the user asked about.

## Migration Plan

1. Edit the four `map_role` entries and their unit tests; rebuild native.
2. Update the `PlatynUI.BareMetal` namespace legend and any docs/examples that show `item:MenuItem`.
3. Verify against a real AT-SPI provider via the egui/Qt menu acceptance suites (`//MenuItem` resolves; menus still open/activate).
4. Rollback is a straight revert of the table edit — no data or API surface migration involved.
