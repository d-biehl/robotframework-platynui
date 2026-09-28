# Design

## Context

The motivation is in proposal.md (Why), and the contract is in `specs/atspi-application-level/spec.md`. This section records only the current code and the measurements that shape the approach. **Verified** marks what was read in the tree at `93cce5c` (2026-09-28) or measured on that day; the rest is marked as assumed.

**One interface check drives four decisions** in `crates/provider-atspi/src/node.rs`. **Verified.**

- **Namespace and role.**
  - `resolve_role` (:147-170) resolves lazily through `map_role_with_interfaces` (:996-1001), which answers `(App, "Application")` for any node with the Application interface and otherwise `map_role(role)`.
  - `map_role` itself also sends `Role::Application` to `App` (:937), independently of the interface.
- **Process attributes.** `process_attributes` (:253-256) reports them only when `is_application()` (:195-197), which is the same interface check.
- **Node identifier.** `id()` (:286-296) derives the identifier from the process ID under the same check.
- **Top-level windows.**
  - `parent_is_application` is set at construction from `parent.namespace() == Namespace::App` (:101).
  - It gates `is_window_surface` (:224), which exposes the window patterns and window-manager bounds, and `is_transient_popup` (:233).
  - `resolve_extents_via_parent_chain` stops at an `App` parent (:1498).

The `native:Application.*` properties are listed whenever the interface is present (:1094-1102). This is raw data and stays.

**Where nodes are built.** `crates/provider-atspi/src/lib.rs` and `node.rs`. **Verified.**

- Only two sites build a node for a registry application:
  - `get_nodes` builds each registry child and seeds its namespace and role through `map_role_with_interfaces` (`lib.rs:324-336`). It builds fresh nodes on every call.
  - The point hit-test builds the application from `application_for_pid`, which returns a registry child, with no parent and nothing seeded (`lib.rs:405-412`). That node resolves its role lazily.
- Every other site builds a node with a known parent: the popup search (`lib.rs:560`), `frame_for_window` (`:615`), `search_subtree` (`:666`) and `children()` (`node.rs:366`).

**Consumers of the application level outside the provider.** **Verified**, unless marked.

- `Runtime::top_level_window_for` falls back to an application node's children only when `namespace() == App && role() == "Application"` (`crates/runtime/src/runtime/window.rs:44-58`, the check at :50).
  - It serves `bring_to_front` and the other window actions (:71, :93).
  - The Python binding exposes it unchanged (`packages/native/src/runtime.rs:1079`).
  - Its tests live in the same file (:159 ff.), with the fixtures of `crates/runtime/src/runtime/test_fixtures.rs`.
- The X11 and the compositor window managers find a window's process ID by walking up from the node to the first ancestor with `control:ProcessId` (`crates/platform-linux-x11/src/window_manager.rs:377-392`, `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:415`). A window below an application node needs no process ID of its own; GTK and Qt windows have none today.
- The popup watcher records a popup under the owner it reaches through `parent()`, without looking at namespaces (`crates/provider-atspi/src/popups.rs:260-294`).
- **Assumed.** The high-level Python library picks a context class for a node by its role (`src/PlatynUI/core/context.py:5-16`, `ContextFactory` with `WeightCalculator`). How the weighting treats the prefix was not read.

**What toolkits register.** **Measured** on 2026-09-28 in `scripts/startxsession.sh --backend headless`, reading the registry's children and one level below with `busctl`:

| Toolkit | Root path | Root role | Root interfaces | Below the root |
|---|---|---|---|---|
| GTK 4.22 (`gtk4-demo`, `gnome-calculator`) | `/org/a11y/atspi/accessible/root` | application | Accessible, Application | `window` without Application |
| AccessKit (egui test app) | same | application | Accessible, Application | `frame` without Application |
| Qt 6.11 Widgets and Qt Quick (test apps) | same | application | Accessible, Application, Collection | `frame`/`dialog` without Application |
| Avalonia 12.1.2 | same | application | Accessible, Application | `frame` **with** Application |

- No root has a `Component` interface.
- The root's `Accessible.Parent` is null for GTK 4, AccessKit and Avalonia, and the registry object for Qt.
- PySide additionally registers empty GTK 3 roots named `python`, which `get_nodes` already skips because they have no children.
- Avalonia's backend registers its Application handler for every node whose automation peer has an `IRootProvider`, that is for every `TopLevel` (`AtSpiNode.BuildAndRegisterHandlers` in `src/Avalonia.FreeDesktop.AtSpi/AtSpiNode.cs`).

**What AT-SPI prescribes.** **Verified** against at-spi2-core `main`.

- `xml/Application.xml` and `xml/Socket.xml` require the interface on the application's root object.
- No role is prescribed. `ATSPI_ROLE_APPLICATION` is only described as "the toplevel accessible of an application" (`atspi/atspi-constants.h`).
- The root path is a convention that libatspi relies on (`ATSPI_DBUS_PATH_ROOT`), not a protocol rule.
- `atspi_accessible_get_application` finds an application by walking up to the desktop and never asks for the interface.

## Goals / Non-Goals

**Goals:**

- Whether a node is at the application level is decided once, where the provider already knows it, and does not depend on what the object reports.
- Namespace, role, process attributes, `Id` and the top-level-window decision all follow from that one fact.
- The classification is a pure function that unit tests cover without a bus.

**Non-Goals:**

- Detecting a toolkit that registers a window as its root, or wrapping such a root in a synthetic application node.
- Changing which native attributes a node lists.
- Any change to the other providers. The runtime change is neutral for them.

## Decisions

### D1: The application level is marked where the node is built

The two sites that build a node for a registry application mark it as application level (`lib.rs:324`, `:405`). Every other site builds an ordinary node.

The marker is a property of `AtspiNode`, set through a dedicated constructor for application-level nodes. No call site passes a bare flag, and a new construction site cannot produce an application node by accident.

Alternatives considered:

- **The root path `/org/a11y/atspi/accessible/root`.** It matches every toolkit measured, and libatspi relies on it. But the protocol lets the application choose the path it passes to `Embed`, so the path is a convention that stands in for the fact the provider already has.
- **The interface and the role `application`.** It still trusts what the object reports. A root with an unexpected role would land in `control` directly under the desktop.
- **The topmost node with the interface.** This is a heuristic over the same claim, and a root without the interface would be lost.

### D2: One classification function of level and role

A single function takes the level and the Accessible role and returns the namespace and the role name:

- At the application level it answers `App` with the role name that `map_role` gives.
- Below it, it answers `map_role(role)` unchanged.

It replaces `map_role_with_interfaces`, and both `resolve_role` and the seeding in `get_nodes` call it. The interface set is no longer an input. The role comes from the object as the maintainer decided (spec: *An application-level node carries the role its object reports*). The unit tests cover the level-and-role table without a bus.

A role that could not be read reaches the function as `Role::Invalid` (`lib.rs` seeds it that way on a timeout) and becomes `app:Unknown` at the application level.

### D3: `map_role` no longer produces the `app` namespace

`map_role(Role::Application)` answers `(Control, "Application")`. After D1 and D2 the `app` namespace comes only from the level. Otherwise a deep node with the role `application` would again make its children look like top-level windows through `parent_is_application`.

### D4: Process attributes and `Id` follow the level

`process_attributes` and `id()` ask the level marker instead of `is_application()`, which goes away. The decision is expressed like `application_id` already is (`node.rs`, tested at :2342-2363): as a pure function of the level and the peer, so a unit test covers *Process attributes and Id follow the level, not the interface* without a bus. The native `Application.*` properties stay tied to the interface. They are raw data, and `native:Accessible.Interfaces` shows the interface anyway.

### D5: The top-level-window rule keeps its mechanism

`parent_is_application` stays derived from the parent's namespace at construction (`node.rs:101`). After D1 to D3, only application-level nodes are in `App`, so the check now means exactly "the parent is the application level". Reading a marker through `&Arc<dyn UiNode>` would need a downcast that buys nothing. The same holds for `resolve_extents_via_parent_chain` (:1498).

### D6: The runtime recognises the application node by namespace

`top_level_window_for` drops the role condition (`window.rs:50`). UIA, JAB, the Java agent and the mock always give their application nodes the role `Application`, so nothing changes for them. The doc comment of the function says "`app:` nodes" instead of "`app:Application` nodes".

### D7: No log for an application root with another role

An application-level node whose role is not `Application` shows that role in the Inspector, in `platynui-cli query` and in XPath. A log record would repeat what the tree already shows.

### D8: Avalonia is checked by hand here, and in the lane by the follow-up fixture

- This change does not add an Avalonia fixture. A fixture brings the .NET SDK, a NuGet restore and a blueprint catalog into the lanes. That is its own change, `add-avalonia-test-app`, which depends on this one and makes the lane proof permanent.
- The classification is covered by unit tests on the exact shape Avalonia produces: a node below the application level with role `frame` and the interface.
- An egui acceptance test guards the level structure on every real lane.
- A manual check against an Avalonia 12 application verifies the real tree once.

## Risks / Trade-offs

- **[A root with another role is `app:<Role>`]** A selector written as `app:Application` misses it. The Python library would presumably not wrap it as `Application` (assumed, see Context). → No measured toolkit does this, and the role is visible in the Inspector. The case is revisited with a concrete toolkit.
- **[A role read that times out makes the root `app:Unknown`]** A selector `app:Application[...]` misses that application for one enumeration. Today the interface could still classify it. → Timeouts already make results incomplete, and the first one warns, naming the application (`timeout.rs`). The next enumeration builds fresh nodes and reads the role again (`lib.rs:324`).
- **[A toolkit that registers a window as its root]** Its children would count as top-level windows. → Not known to exist. A non-goal of this change.
- **[An accessible embedded from another process through an AT-SPI socket]** Its plug implements the Application interface below the application level. It is now classified by its role and carries no process attributes. → Assumed from the protocol, not verified with a toolkit. The process of the application the tree belongs to is still reported on its application node.
- **[Avalonia's popups become window surfaces]** Measured on 2026-09-28 with an open menu on X11:
  - An Avalonia popup is an AT-SPI `frame` named `PopupRoot` directly under the application root, and it reports the Application interface.
  - It is not a client of the X11 window manager: `_NET_CLIENT_LIST` holds only the main window.

  Today it is a second `app:Application`. With this change it becomes a `control:Frame` directly under the application level. The transient-popup exception covers only the roles `PopupMenu`, `Menu` and `ToolTip` (`crates/provider-atspi/src/node.rs:227-234`), so the popup becomes a window surface whose bounds come from the window manager. That can move the bounds of its menu items, for example when the main window is the process's only managed window.
  → `add-avalonia-test-app` verifies this on both Linux lanes (its task 2.4) and names the provider change a wrong result needs. This change does not widen the popup exception on a guess.
- **[Overlap with `application-process-attributes`]** Both changes touch `process_attributes` (`node.rs:253-256`). → One function; the later change rebases.

## Migration Plan

- **The change is behavioral, not additive.** It changes the shape of the AT-SPI tree in the cases the proposal lists. For every measured toolkit except Avalonia, the tree stays the same.
- **It needs a native rebuild** (`just build-native`), because the provider is linked into `packages/native`.
- **There is no configuration and no stored data.** Rolling back means reverting the change.
