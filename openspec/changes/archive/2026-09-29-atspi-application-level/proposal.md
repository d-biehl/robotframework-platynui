# Proposal

## Why

The AT-SPI provider decides which nodes are applications by asking one question: does the node report the `org.a11y.atspi.Application` interface? If it does, the node becomes `app:Application`, whatever its role (`map_role_with_interfaces`, `crates/provider-atspi/src/node.rs:996-1001`). The same interface check decides which node carries the process attributes and a process-derived `Id`. Through the parent's namespace, it also decides which nodes count as top-level windows.

AT-SPI states the rule in the other direction. The root object of an application must implement the interface:

- `xml/Application.xml`: "Interface that must be implemented by the root object of an application."
- `xml/Socket.xml`, `Embed`: "The application's root object, which it passes in @plug, must support the org.a11y.atspi.Application interface."

Nothing says that only the root does. libatspi finds an object's application by its position in the tree, walking up to the desktop (`atspi_accessible_get_application`), and never asks for the interface. The provider inverts the rule, and a toolkit that exposes the interface elsewhere breaks the tree.

Avalonia 12 does this. Its new AT-SPI backend adds the Application interface to every `TopLevel`. The window of an Avalonia application then appears in PlatynUI as a second, nested `app:Application` instead of `control:Frame`. This was observed on 2026-09-28 with Avalonia 12.1.2 in a headless X11 session:

- `//control:Frame` finds nothing, so neither a selector nor `platynui-cli window --list '//control:Frame'` reaches the window as a window.
- The window carries `@ProcessId` and the `app:*` process attributes.
- The window's own children, the panels `WindowChrome` and `Panel`, are treated as top-level windows. They expose every window pattern and `@IsActive`, and their bounds come from the window manager. Their bounds happen to be right here only because both panels fill the window.

The interface check also replaces the node's role with `Application`, so the role the toolkit reported is lost from the PlatynUI role. The other providers already treat the application as a level of the tree and not as a claim of the toolkit: UIA, JAB and the Java agent build their application nodes themselves.

## What Changes

- **The application level on AT-SPI is structural.** The applications the accessibility registry lists are the application-level nodes, in the `app` namespace. They are the root objects that applications registered through `Socket.Embed`. The Application interface no longer decides this.
- **An application-level node carries the role its object reports.** The role comes from the Accessible interface through the same role mapping as every other node, and is no longer replaced by `Application`. In every toolkit checked, the root's role is `application`, so its node stays `app:Application`: GTK 4, Qt 6 Widgets and Qt Quick, AccessKit (egui), and Avalonia 12. A root with another role would appear as, for example, `app:Frame`, and a root whose role cannot be read in time appears as `app:Unknown`.
- **Below the application level the Application interface classifies nothing.**
  - Namespace and role follow from the Accessible role alone.
  - Such a node carries no process attributes, and its `Id` is not derived from a process ID.
  - Its interfaces stay visible as native attributes (`native:Accessible.Interfaces`, `native:Application.*`).
- **A node below the application level with the role `application` becomes `control:Application`.** Today it is `app:Application`.
- **Top-level windows follow the level.** The children of an application-level node are its top-level windows, as today. For Avalonia this is now the frame, and no longer its panels.
- **The runtime recognises an application node by its `app` namespace alone** when it looks for an element's window to activate (`crates/runtime/src/runtime/window.rs:50`). The role may now differ from `Application`.
- **No synthetic application node.** If a toolkit registered a window directly as its root, the change would not wrap it in a synthetic application. No such toolkit is known.
- **Docs:** the AT-SPI node model in `dev-docs/platform-linux.md`.

**Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking. The release notes name each of them:

- In Avalonia 12 applications:
  - The window is `control:Frame` with the window patterns, so its window state can be read and changed.
  - The window's children lose the window patterns, `@IsActive` and the window-manager bounds.
  - `@ProcessId` and the `app:*` attributes disappear from the window.
- A node below the application level with the AT-SPI role `application` moves from `app:Application` to `control:Application`.
- An application root whose role is not `application` moves from `app:Application` to `app:<its role>`. No known toolkit does this.
- An application root whose role read times out is `app:Unknown` for that enumeration. Today it is `app:Application` if its interfaces could still be read.

## Capabilities

### New Capabilities

- `atspi-application-level`: which AT-SPI objects are application-level nodes, and the namespace and role they carry. It also states what the Application interface decides below that level (nothing, except native attributes) and how consumers recognise an application node.

### Modified Capabilities

None.

- `sidecar-deployment` states its process-ID rules for the AT-SPI application node. That node is still the registry's application, so those rules keep applying unchanged.
- `atspi-event-driven-tree` grafts popups under their owner without looking at namespaces, so it is unaffected.

## Impact

- **Rust:**
  - `crates/provider-atspi`:
    - `src/node.rs`: the classification replaces `map_role_with_interfaces`, and `map_role` maps `Role::Application` to `control`. The application-level marker replaces the interface check in `process_attributes` and `id`. Doc comments and unit tests follow.
    - `src/lib.rs`: the two sites that build application-level nodes, `get_nodes` and the point hit-test.
  - `crates/runtime`: `top_level_window_for` in `src/runtime/window.rs`, and its test.
- **Python / Robot Framework:** no keyword, argument or API change.
- **Tests:**
  - Provider unit tests for the classification and for the process attributes and `Id` below the application level.
  - A runtime unit test for the namespace-only recognition.
  - An egui acceptance test that guards the level structure on every real lane.
  - A manual check against an Avalonia 12 application. Avalonia is not a lane fixture.
- **Docs:** `dev-docs/platform-linux.md` §2.
- **Build:** a native rebuild, because the provider is linked into `packages/native`.
- **Platforms:**
  - Linux AT-SPI changes, on X11 and under the PlatynUI compositor.
  - The runtime change is provider-neutral. It changes nothing for UIA, JAB, the Java agent and the mock, whose application nodes always carry the role `Application`.
  - Windows and macOS are otherwise untouched.
- **Coordination:**
  - `application-process-attributes` moves AT-SPI's process reading into `crates/process`. This change decides which node gets the process attributes (`node.rs:253-256`). The overlap is one function; either order works, and the later change rebases.
  - `add-avalonia-test-app` follows as its own change and adds a blueprint-conforming Avalonia fixture. It depends on this change: without it the Avalonia window is `app:Application`, and the fixture's Linux suites cannot find it. Its suites, `application_level.robot` in particular, then prove this change on a real Avalonia tree in every Linux lane run: directly on the X11 lane, and through XWayland on the Wayland lane.
