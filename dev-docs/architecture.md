# PlatynUI Architecture

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document explains how the PlatynUI runtime is put together and, more importantly, *why* it is put together that way. It describes the system as it is implemented today and is the authoritative reference for its design and conventions. Where it touches platform specifics it stays at the level of intent; the code remains the final word on exact details.

## 1. Introduction & Goals

PlatynUI is a cross-platform UI automation library. Its job is to give you one consistent view of the native UI on whatever machine you are automating, even though every operating system exposes its controls through a different accessibility technology — UIA on Windows, AT-SPI2 on Linux, the macOS Accessibility (AX) API, and so on. The core takes those very different sources and presents them as a single, normalized tree of UI elements. You then locate elements in that tree with XPath, and you act on them through **patterns** — small, named capabilities such as "this can be focused" or "this can be activated." The point is that your automation code never has to care which platform produced an element.

### Design Philosophy

A handful of principles run through the whole system. Each one is a deliberate trade-off, and knowing them up front makes the rest of this document much easier to read.

- **Composition over inheritance.** Capabilities are described by patterns — composable traits a node either has or doesn't — rather than by a deep class hierarchy. An element is defined by what it can *do*, assembled from independent pieces, not by where it sits in a type tree.
- **Streaming and lazy evaluation.** Nothing is built before it is needed. Children, attribute values, and pattern probes are all computed on demand, so opening a large UI is cheap until you actually walk into the part you care about.
- **A platform-agnostic core.** The `platynui-core` crate defines all the traits — the shared vocabulary of the system — while the platform and provider crates implement those traits independently. The core knows nothing about UIA or AT-SPI; it only knows the abstractions.
- **Build-time platform selection.** Which platform and provider crates get linked in is decided at compile time via `cfg(target_os)` attributes, not chosen at runtime. (The one planned exception is the Linux mediation crate — see §3.)
- **A single desktop-coordinate system.** Every position — `Bounds`, `ActivationPoint`, window geometry — is expressed in absolute desktop coordinates, so callers never have to reason about per-window or per-monitor offsets. Any DPI or scaling adjustment is handled on the provider side, before the numbers reach you.

## 2. Crate Landscape

PlatynUI is split into many small crates rather than one large one, and the split is deliberate. Each crate owns one clear responsibility, and the boundaries between them are where the platform-specific code is contained — so the parts that have to differ per operating system stay isolated from the parts that should look the same everywhere. The tree below is the whole map: the Rust workspace (`crates/`, `apps/`) and the Python packaging layer (`packages/`).

```
crates/
├─ core                      # Common traits/types — platynui-core
├─ xpath                     # XPath evaluator and parser — platynui-xpath
├─ runtime                   # Runtime, provider registry, XPath pipeline — platynui-runtime
├─ link                      # Linking helper macros — platynui-link
├─ platform-windows          # Windows devices — platynui-platform-windows
├─ provider-windows-uia      # UIA provider — platynui-provider-windows-uia
├─ platform-linux-x11        # Linux/X11 devices — platynui-platform-linux-x11
├─ platform-linux-wayland    # Linux/Wayland devices — platynui-platform-linux-wayland
├─ platform-linux            # Linux session mediator — platynui-platform-linux
├─ provider-atspi            # AT-SPI2 provider — platynui-provider-atspi
├─ platform-macos            # macOS devices (stub) — platynui-platform-macos
├─ provider-macos-ax         # macOS AX provider (stub) — platynui-provider-macos-ax
├─ platform-mock             # Mock devices for testing — platynui-platform-mock
├─ provider-mock             # Mock UI tree provider — platynui-provider-mock
├─ cli                       # CLI tool — platynui-cli
├─ xkb-util                  # Linux xkbcommon key mapping — platynui-xkb-util
└─ playground                # Development sandbox — playground

apps/
├─ inspector                 # GUI inspector (egui) — platynui-inspector
├─ wayland-compositor        # Wayland compositor for testing — platynui-wayland-compositor
├─ wayland-compositor-ctl    # CLI control for compositor — platynui-wayland-compositor-ctl
├─ test-app-egui             # egui test app (accessibility) — platynui-test-app-egui
└─ eis-test-client           # EIS/libei protocol validator — platynui-eis-test-client

packages/
├─ native                    # Python bindings (PyO3/maturin) — platynui_native
├─ cli                       # Python wheel for CLI binary
└─ inspector                 # Python wheel for Inspector binary
```

Two distinctions in that tree are worth holding onto, because nearly everything else builds on them. First, **`core`** defines the shared vocabulary — the traits and types that describe a UI element and what you can do with it — while **`runtime`** is the engine that puts those abstractions to work, tying providers, platforms, and the XPath pipeline together. Second, the per-operating-system crates come in two flavours, and the difference is fundamental: a **provider** reads the UI tree (it answers "what elements are on screen?"), and a **platform** drives the input devices (it answers "move the mouse here, type this key"). Reading and acting are kept apart all the way down. The mock crates (`provider-mock`, `platform-mock`) mirror this same split so tests can exercise either side without a real desktop.

### Naming Conventions

The names follow a small set of rules, so that once you know the pattern you can predict a crate's name from what it does:

- Workspace crates (under `crates/`, `apps/`) carry the `platynui-` prefix on their **package** name, declared in each crate's `Cargo.toml` (`[package].name`). The **directory** deliberately leaves the prefix off rather than repeating it — the prefix lives at the package level, so the folder only needs the short name (e.g., `crates/runtime` is the package `platynui-runtime`, `crates/core` is `platynui-core`).
- FFI and packaging crates that live outside the Cargo workspace (e.g., `packages/native`) follow the conventions of the ecosystem they ship into instead, so they are exempt from the prefix rule.
- Platform crates are named for their target: `crates/platform-<target>` (package `platynui-platform-<target>`).
- Provider crates are named for the accessibility technology they speak: `crates/provider-<technology>` (package `platynui-provider-<technology>`).

### Dependency Graph

The crate list says *what* exists; this graph says *who depends on whom*. Read it top to bottom: the three front ends — the Robot Framework library, the CLI, and the Inspector — all reach the system through the runtime, never around it. The runtime in turn rests on the `core` traits and fans out to the providers, platforms, and the XPath engine. Because every front end funnels through the same runtime and the same core model, they all see the desktop the same way.

```
┌────────────────────────┐
│  Robot Framework Lib   │
│    (src/PlatynUI)      │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐  ┌──────────┐  ┌─────────────┐
│  Python Bindings       │  │   CLI    │  │  Inspector  │
│  (packages/native,     │  │(crates/  │  │   (apps/    │
│   PyO3/maturin)        │  │  cli)    │  │  inspector) │
└───────────┬────────────┘  └────┬─────┘  └──────┬──────┘
            │                    │               │
            └────────────┬───────┘───────────────┘
                         │
                         ▼
                ┌───────────┐           ┌───────────────┐
                │  Runtime  │◄──────────│  Core traits  │
                │           │           │  & UI model   │
                └─────┬─────┘           └──────┬────────┘
                      │                        │
                ┌─────┴──────────┬─────────────┼──────┐
                │                │             │      │
                ▼                ▼             ▼      ▼
          ┌───────────┐  ┌─────────────┐  ┌──────────────┐
          │ Providers │  │  Platforms  │  │ XPath Engine │
          │ (UiTree)  │  │  (Devices)  │  │              │
          └───────────┘  └─────────────┘  └──────────────┘
```

One subtlety the arrows don't show: the Python wheels `packages/cli` and `packages/inspector` are not really part of this dependency chain. They are packaging wrappers that bundle the already-built Rust binaries so they can be installed with `pip` — they add no Rust dependencies of their own.

## 3. Registration & Extension Model

PlatynUI is built to be extended without the runtime knowing — at compile time — which concrete platforms or providers exist. An extension makes itself known by *registering* itself, and the runtime *discovers* what has registered. The glue for this is the `inventory` crate, which lets a crate contribute entries to a global collection simply by being linked into the final binary. The practical upshot is that the runtime never names a concrete type like "the Windows UIA provider"; it asks "what pointer devices, what providers, what window managers have registered?" and works with whatever it finds. Adding support for a new technology is therefore a matter of writing a crate that registers the right things, not of editing the runtime.

### Registration Macros

A crate registers *factories*, and there are two kinds — each backed by its own `inventory` collection, so the runtime can enumerate providers independently of platforms:

- `register_provider!(&FACTORY)` — a `UiTreeProviderFactory`, which builds a UI-tree provider for one accessibility technology (UIA, AT-SPI2, mock).
- `register_platform_factory!(&FACTORY)` — a `PlatformFactory`, which builds a runtime's *platform bundle* for one display/input backend (X11, Wayland, Windows, mock): the pointer, keyboard, screenshot, highlight, window manager, and desktop-info devices it drives, all bound to one session.

Both kinds share the same shape — a factory the runtime discovers and *calls to create an owned instance per runtime* — so a platform backend and a provider are registered and instantiated the same way (this symmetry is deliberate; the platform layer used to be the exception and no longer is). The runtime never names a concrete backend: it asks which factories registered and selects one (platforms — see §4; providers — see §7.2). The individual input/output facilities (pointer, keyboard, …) are no longer registered one by one — a `PlatformFactory` assembles them into the bundle it returns.

### Linking Strategy

Because discovery happens at link time, *what gets linked* decides *what is available* — and the system uses that deliberately. OS-specific providers register themselves automatically the moment their crate is linked, so a normal build for a given operating system simply lights up the right providers. The mock providers (`platynui-provider-mock`, `platynui-platform-mock`) are the exception: they intentionally do **not** auto-register, so a test or tool never gets the mock by accident — you reach for it only through an explicit factory handle.

To make the common cases ergonomic, the helper crate `platynui-link` offers two macros. `platynui_link_providers!()` is feature-gated and links either the mock or the OS providers depending on how the build is configured; `platynui_link_os_providers!()` is the explicit "give me the real OS crates" form. Applications such as the CLI and the Python extension select platform and provider crates through `cfg(target_os = ...)`, while tests opt into the mock crates by linking them by hand — a small `use platynui_platform_mock as _;` / `use platynui_provider_mock as _;` that pulls the otherwise-unreferenced crates into the build so their mock factory handles become available.

### Linux Session Mediator (`platynui-platform-linux`)

Everywhere else, the platform is decided when you build: a Windows build talks to Windows, a macOS build talks to macOS. Linux breaks that assumption, because the display server — X11 or Wayland — is only known when the program actually runs. Linux is therefore the one place where platform selection is a **runtime** decision, and the crate `platynui-platform-linux` exists to absorb that complication so nothing downstream has to think about it.

**Architecture.** The mediator registers two `PlatformFactory`s — one for X11, one for Wayland — and nothing else. Behind it sit two real implementations, `platform-linux-x11` and `platform-linux-wayland`, which deliberately do **not** register themselves; each exposes a plain `create_*_bundle(config)` function that the mediator's factory calls. They stay self-contained X11/Wayland libraries with no opinion about how they get chosen; the mediator owns only the choice.

**Session detection** (`session.rs`). The mediator works out which session it is in once and remembers the answer for the lifetime of the process (cached behind a `Mutex<Option<SessionType>>`). The checks run in order of trustworthiness:

1. `$XDG_SESSION_TYPE` → `"wayland"` | `"x11"` (most authoritative — `XWayland` sets both `$DISPLAY` and `$WAYLAND_DISPLAY` but `$XDG_SESSION_TYPE=wayland`)
2. `$WAYLAND_DISPLAY` present → Wayland
3. `$DISPLAY` present → X11
4. None → `PlatformError::UnsupportedPlatform`

**Selection pattern.** Detection feeds selection. Each of the two factories answers `can_serve(config)` — true when the runtime's `config` names that backend explicitly (`platform.backend`), or, absent that, when session detection matches (X11 factory serves an X11 session, Wayland factory a Wayland one). The runtime asks each registered platform factory in turn and calls `create(config)` on the first that can serve, which builds that backend's per-runtime bundle by delegating to the sub-crate's `create_*_bundle`. There are no cached wrapper devices and no process-global resolution: the chosen backend's real devices go straight into the runtime's own bundle, and a second runtime makes the choice again from scratch.

*The two factories live in `crates/platform-linux`; the code is authoritative for the exact selection.*

**Consumers.** Everything downstream — the CLI, the Inspector, the Python bindings, the link crate — depends on `platynui-platform-linux` and never reaches past it to a sub-platform directly. That single dependency is what lets the rest of the toolkit treat Linux as "just another platform."

### Provider Modes

- **In-process** — Rust crate linked directly (UIA, AT-SPI2, Mock)
- **Out-of-process** — planned (see `dev-docs/planning.md` §3.4)

## 4. Runtime Context & Lifecycle

The **runtime** is the object that holds a live automation session together. It is what you create when you want to start looking at the UI, and it owns everything that has to be set up, kept alive, and torn down again as a unit: the platform-specific groundwork, the set of providers that read the accessibility trees (see §7), and the event plumbing that watches for changes. The rest of this section follows the runtime through its life — coming up, shutting down, and the special case of standing it up inside a test.

### Initialization Sequence

Creating a runtime happens in a fixed order, because each step depends on the one before it. The first step prepares the *platform*, the second prepares the *providers*, and the third connects them so changes can flow back to you.

1. **Build the platform bundle.** Constructing a runtime selects one `PlatformFactory` — the backend named by its `config` (`platform.backend`), or the first that can serve the detected session — and calls it to build a *platform bundle*: this runtime's own pointer, keyboard, screenshot, highlight, window manager, and desktop-info devices, all sharing that session's connection. The bundle is owned by the runtime, not shared; a second runtime builds its own. (Genuinely once-per-process host state still lives behind its own guard inside a factory — on Windows, declaring Per-Monitor-V2 DPI awareness — but the *devices and their connection* are per-runtime.) If no backend can serve, the runtime comes up without a platform (providers-only, e.g. a headless test); if a named backend cannot serve, construction fails.
2. **Bring up the providers.** The runtime instantiates its provider factories through the provider registry (see §7.2), each producing a provider that knows how to read one accessibility technology (see §7), and hands each the bundle's window manager so a provider's window nodes drive *this* runtime's session rather than a global.
3. **Wire up events.** Finally, the runtime subscribes its event listeners to the providers, so that changes observed in the underlying UI can be delivered back through the event pipeline (see §5.7, §7.2).

### Shutdown & Resource Cleanup

Shutting a runtime down unwinds those three steps in the reverse order, and it does so automatically — the runtime tears itself down when it goes out of scope, so you do not have to remember to do it by hand. Shutdown is idempotent, meaning a second teardown (an explicit one followed by the automatic one, say) is harmless.

The order matters as much on the way down as on the way up. The runtime first stops the event dispatcher, so no further change notifications arrive while things are being dismantled. It then shuts down each provider, giving it the chance to release everything it was holding on the host system — COM handles, D-Bus connections, and the like. Only then does it drop its own platform bundle: because the bundle's devices own that runtime's connection (an `Arc`) and its per-session threads (such as the highlight overlay, joined on drop), dropping it closes the connection and stops those threads — no shared lease to release, and nothing left running for another runtime to trip over. Releasing host resources cleanly in this phase is a firm obligation on every provider, not a courtesy.

### Test Injection

In tests you usually do *not* want a runtime to go discover whatever providers and platform happen to be present on the machine running the suite — that would make tests depend on the host. Instead, the runtime offers seams where you hand it exactly the pieces you want: a chosen set of provider factories, plus a `config` selecting the `mock` platform backend so its factory supplies deterministic stand-ins for real input and screen hardware. This lets a test run the same runtime code against a controlled, predictable tree on any platform. (The mock backend is opt-in — it only serves when `config` names it — so a real runtime never selects it by accident.)

To keep tests from repeating that wiring, a central helper assembles a runtime from a given list of factories together with the mock platform, and a small family of shared `rstest` fixtures builds on it for the common shapes a test needs — a mock-device platform with no providers, a stub setup, and a focus-oriented setup — each delegating to that one helper.

*The injection entry points and the central helper live in `crates/runtime/src/test_support.rs`; the shared fixtures live in `crates/runtime/src/runtime/test_fixtures.rs`. The code is authoritative for the exact helper and fixture names; this section explains what they are for.*

## 5. Data Model

### 5.1 UiNode, UiAttribute, UiPattern Traits

Everything PlatynUI sees is a tree of **nodes**. A node is a single UI element — a window, a button, a list item, the desktop itself. The model keeps every node small and uniform on purpose, so that the same query and interaction code works no matter which platform produced the node.

A node answers three kinds of questions:

- **Who am I?** Its namespace (see §5.2), its role ("Window", "Button", "ListItem", …), its name, an optional developer-set stable id (see §5.5), and a runtime id that identifies it uniquely for its lifetime (see §5.4).
- **Who is around me?** Its parent and its children. Children are produced *lazily* — the tree is walked only on demand, never built up front. Opening a desktop with thousands of controls is therefore cheap until you actually descend into it. A node can also report cheaply whether it has any children at all (by default this just probes the child walk, but a provider can override it with a cheaper check), and it carries an optional hint for its position in document order.
- **What can you tell me, and what can you do?** Its **attributes** (typed values such as bounds, name, state — see §5.6) and its **patterns** (capabilities such as "focusable" or "activatable" — see §6). A node lists the patterns it supports and can hand back the implementation of a named pattern when asked.

Attributes are lazy too: a node lists which attributes it has — and lets you look one up by namespace and name — but the concrete value is computed only when something reads it, because reading a value can mean a round-trip to the operating system's accessibility API.

A node also manages its own freshness. It can report whether it is still **valid** (a cheap liveness check — by default a node assumes it is alive until told otherwise) and it can be **invalidated**, which tells it to discard whatever it has cached so the next access fetches fresh data from the native API. This is the hook the event pipeline (see §5.7) pulls on when the platform reports that something changed.

The other two model types are deliberately minimal. An **attribute** knows its namespace, its name (always PascalCase), and how to produce its value on demand. A **pattern** knows its own name and can be downcast to its concrete capability type, which is how interaction code recovers, say, the "activatable" behavior from a generic pattern handle.

Two supporting pieces round this out. **Navigation helpers** cover the common walks — reaching the parent, iterating ancestors, jumping to the enclosing top-level window, and finding the nearest ancestor that supports a given pattern — so call sites don't re-implement them. And the **pattern registry** is how a node carries its capabilities: it keeps them in insertion order and can register a pattern *lazily*, holding a small probe that runs once on first access and then caches its result, so an expensive platform probe is deferred until something actually needs that capability.

*The exact trait definitions live in `crates/core/src/ui/`. The code is the source of truth for signatures; this section explains what they are for.*

### 5.2 Namespaces

| Prefix | Scope | Description |
|--------|-------|-------------|
| `control` | Default | UI controls (Window, Button, TextBox, ...) |
| `item` | Container children | ListItem, TreeItem, TabItem, ... |
| `app` | Application/process | Application nodes |
| `native` | Technology-specific | Raw platform attributes |

Expressions without a prefix match only `control:` elements. Use `item:` or wildcards to widen scope.

### 5.3 Desktop Document Node

Every query starts somewhere, and in PlatynUI that somewhere is the **desktop document node** — the synthetic root of the tree, the element an XPath expression is evaluated against. Building it is the job of a `DesktopInfoProvider`, which is one of the devices in the runtime's platform bundle. The runtime asks its bundle's desktop-info device to describe the desktop and uses that to construct the root (falling back to a synthetic desktop when the runtime has no platform, as in a providers-only test).

If nothing registers one, the runtime still produces a usable root: it falls back to a generic desktop with placeholder values — bounds of 1920x1080, an empty monitor list, and the technology reported as "Fallback" — so queries have something to run against even on an unconfigured platform. When a real provider is present, the `DesktopInfo` it supplies is richer: the OS version, the list of monitors (each with its id, name, bounds, whether it is primary, and its scale factor), and the overall desktop bounds.

A provider exposes the same top-level controls through two complementary views:

1. **Flat view** — Top-level controls hang directly under the desktop in the `control:` namespace.
2. **Grouped view** — Those same controls also appear as children of `app:Application` nodes, which is what makes process-scoped queries such as `app:Application[@Name='Studio']//control:Window` possible.

### 5.4 RuntimeId Schema

Every node needs an identity that stays stable for as long as the node lives, and that identity is its **RuntimeId**. It takes the form `prefix:value`, where the prefix tells you which provider or technology minted it.

| Provider | Scheme | Example |
|----------|--------|---------|
| UIA | `uia://desktop/<hex>` or `uia://app/<pid>/<hex>` | `uia://desktop/2A0B3C` |
| AT-SPI2 | AT-SPI D-Bus object path | `atspi:///org/a11y/...` |
| Mock | `mock:<id>` | `mock:window-1` |
| Desktop | `platynui:Desktop` (reserved) | `platynui:Desktop` |

Providers generate deterministic IDs stable for the element's lifetime. The `platynui` prefix is reserved for the desktop node.

### 5.5 Developer Id (`control:Id`)

Visible labels change — they get renamed, translated, or reworded — so they make brittle selectors. The **developer id** exists to give you something steadier: an optional, developer-set stable identifier that is independent of the visible label and of language. When you want a selector that survives, prefer `@control:Id='...'` wherever it is available.

Each platform sources this id from its own native concept: on Windows it is the `AutomationId`, on Linux/AT-SPI2 the `accessible_id`, and on macOS/AX the `AXIdentifier`. The node only emits the attribute when the underlying value is actually set, so an empty developer id simply doesn't appear rather than showing up blank.

### 5.6 UiValue & Attribute Normalization

Attribute values are **typed** rather than stringly-typed: a `UiValue` is one of String, Bool, Integer, Number (an `f64`), or Null, plus the structured values Rect, Point, Size, Array, and Object. Carrying real types means the XPath and interaction layers can compare and compute on values without re-parsing them out of text.

For the structured values, the runtime makes the parts directly addressable. From a value like `Bounds` it auto-generates **alias attributes** for the components — `Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`, and so on — so a query can reach a single coordinate without unpacking the whole rectangle. These aliases are synthesized by the runtime and XPath layer; providers must not generate them themselves.

Naming is normalized so that queries look the same everywhere. All attribute names use **PascalCase**, and native role names are mapped onto PascalCase roles — for example UIA's `UIA_ButtonControlTypeId` becomes simply `Button` — while the untouched original role is still available as `native:Role` for when you need the platform's own term. To keep these names consistent across the codebase rather than scattered as string literals, `platynui-core::ui::attribute_names::<pattern>::*` provides the canonical string constants.

### 5.7 Event Capabilities & Invalidation

Providers differ in how much they can tell the runtime about change, and the runtime adapts to whatever a given provider is capable of. A `ProviderDescriptor` advertises this with an `event_capabilities` bitset, and the runtime picks its refresh strategy accordingly:

| Capability | Behavior |
|------------|----------|
| `None` | Runtime polls/refreshes before every query |
| `ChangeHint` | Provider signals "something changed"; runtime invalidates parent and re-queries children |
| `Structure` | Structural events with parent/RuntimeId; runtime handles affected subtrees selectively |
| `StructureWithProperties` | Additionally includes property change events |

Above and beyond those levels, `TreeInvalidated` is the fallback for drastic changes — a provider restart, for instance — and triggers a full reload of the tree.

The mechanism that makes all of this work is invalidation at the node level: a provider must implement `UiNode::invalidate()` (see §5.1) so that a node discards its cached data — children, attributes, and patterns — and the next access fetches fresh data from the native API. The payoff is that event-capable providers only trigger targeted updates of the affected parts of the tree, while providers without events fall back to a full refresh before each query.

## 6. Pattern System

> **This section describes the _intended_ capability model, not only what is wired up today.** Some patterns are already implemented, others are planned, and some may still change shape. The authoritative list of patterns currently present in the Rust core is `pattern_names` in [`crates/core/src/ui/identifiers.rs`](../crates/core/src/ui/identifiers.rs); the attribute-name constants live in `crates/core/src/ui/attributes.rs`.

### 6.1 Design Principles

The pattern system is how PlatynUI answers the question *"what can this element do?"* — and it does so by composition rather than by classification. Instead of deciding that something *is* a "button" and then assuming a fixed bundle of behavior, a node simply carries the small set of capabilities it actually has. A real button advertises that it has text, that it can be activated, and that it offers a point you can target; a list item advertises a slightly different mix. Same machinery, no special cases.

Two kinds of capability make up the system, and the distinction between them is the heart of the design:

- A **ClientPattern** is a read-only contract: it promises that certain typed attributes are available to read. `TextContent` promises there is text to read, `Selectable` promises a selected/not-selected state, `ActivationTarget` promises a point you can aim at. A ClientPattern never *does* anything — it only guarantees information.
- A **RuntimePattern** adds an action you can perform. `Focusable` lets you focus the element, `Activatable` lets you activate it, and so on. These are the verbs of the system.

A node only ever claims a capability it can genuinely back up. The list of supported patterns must stay honest: a pattern appears on a node only when every attribute it requires is actually available *and* a concrete pattern instance exists to serve it. There is no "claims to be scrollable but isn't" — if the attributes aren't there, the pattern isn't there.

Finally, the system deliberately stops at *describing* and *acting*; it does not dictate *how* a client carries out an interaction. Once a node tells you it has an activation point, it is up to the client to decide whether to drive a mouse click, a keyboard activation, or a gesture to reach it. This is intentional: it preserves exactly the range of interaction a human being would have, rather than locking automation into one synthetic technique.

### 6.2 Runtime Pattern Traits

Window behavior could have been modeled as one large "window" capability with a dozen methods on it. PlatynUI deliberately does the opposite: each window action is its own small, independent capability. A window that can be minimized but, on its platform, cannot be moved simply carries the minimize capability and omits the move one — there is no half-implemented monolith to apologize for. Each of these is a separate pattern, registered and probed on its own, so a node's advertised capabilities always match what the underlying platform can really do.

The window capabilities are:

- **Focusable** — give the window keyboard focus.
- **Activatable** — bring the window to the foreground / make it the active window.
- **Minimizable**, **Maximizable**, **Restorable** — drive the window between its minimized, maximized, and normal states.
- **Closeable** — close the window.
- **Movable** — move the window to a position on screen.
- **Resizable** — change the window's size.
- **Responsive** — ask whether the window is currently accepting user input (the answer can be "yes", "no", or "not known", since some platforms can't tell you for certain).

Each action can fail — a platform may refuse a move, or the window may have gone away — and the caller is expected to handle that.

*The exact trait definitions live in `crates/core/src/ui/`. The code is the source of truth for signatures; this section explains what they are for.*

### 6.3 Pattern Catalog

#### ClientPatterns (Attribute Contracts)

| Pattern | Required Attributes | Optional Attributes |
|---------|-------------------|-------------------|
| **Element** | Bounds, IsVisible, IsEnabled | IsInView, Technology |
| **Desktop** | Bounds, OsVersion, Monitors | — |
| **TextContent** | Text | — |
| **TextEditable** | Text, IsReadOnly=false | MaxLength |
| **Clearable** | — | — |
| **TextSelection** | SelectedText, SelectionStart, SelectionEnd | — |
| **Selectable** | IsSelected | — |
| **SelectionProvider** | SelectedItems, CanSelectMultiple | — |
| **Toggleable** | ToggleState | — |
| **StatefulValue** | Value | MinValue, MaxValue |
| **Activatable** | — | — |
| **ActivationTarget** | ActivationPoint | ActivationArea |
| **Focusable** | IsFocused | — |
| **Scrollable** | ScrollHorizontalPercent, ScrollVerticalPercent, ScrollHorizontalViewSize, ScrollVerticalViewSize | HorizontallyScrollable, VerticallyScrollable |
| **Expandable** | IsExpanded | — |
| **ItemContainer** | — | — |
| **Activatable** (window) | — | IsTopmost |
| **Minimizable** | — | IsMinimized |
| **Maximizable** | — | IsMaximized |
| **Restorable** | — | — |
| **Closeable** | — | — |
| **Movable** | — | — |
| **Resizable** | — | — |
| **Responsive** | — | — |
| **DialogSurface** | — | DialogResult |
| **Application** | ProcessId | ProcessName, ExecutablePath, CommandLine |  <!-- Note: ProcessName is the executable stem; `control:Name` (display name) is inherited from the common attribute set. On Windows, `control:Name` falls back to ProcessName since UIA has no separate app display name. On AT-SPI2, `control:Name` = Accessible.Name (display name). See planning.md §8 (Open Design Questions, item 5). -->
| **Highlightable** | — | — |
| **Annotatable** | — | — |

#### Pattern Composition Examples

- **Button** = TextContent + Activatable + ActivationTarget
- **ListItem** = TextContent + Selectable + ActivationTarget
- **Window** = Focusable + Activatable + Minimizable + Maximizable + Restorable + Closeable + Movable + Resizable + Responsive + ActivationTarget
- **TextBox** = TextContent + TextEditable + Focusable + ActivationTarget

### 6.4 Platform Mapping Tables

Each capability is defined once, in platform-neutral terms, but it has to be satisfied differently on each accessibility technology. The tables below are the translation layer: they show, for every pattern, which concrete UIA property, AT-SPI2 call, or macOS AX attribute a platform provider reads or invokes to fulfil the contract. A dash means the platform has no direct equivalent (and the value is either computed, approximated, or simply unavailable). Read these tables when you implement or debug a provider — they are the contract a provider must honor.

#### Role Mapping (Selection)

| PlatynUI Role | UIA ControlType | AT-SPI2 Role | macOS AX Role |
|---------------|-----------------|--------------|---------------|
| Button | Button | PUSH_BUTTON | AXButton |
| CheckBox | CheckBox | CHECK_BOX | AXCheckBox |
| ComboBox | ComboBox | COMBO_BOX | AXComboBox |
| Edit / TextBox | Edit | ENTRY / TEXT | AXTextField / AXTextArea |
| List | List | LIST | AXList |
| ListItem | ListItem | LIST_ITEM | AXStaticText (child) |
| Menu | Menu | MENU | AXMenu |
| MenuItem | MenuItem | MENU_ITEM | AXMenuItem |
| Tab | Tab | PAGE_TAB_LIST | AXTabGroup |
| TabItem | TabItem | PAGE_TAB | AXRadioButton (tab) |
| Tree | Tree | TREE_TABLE / TREE | AXOutline |
| TreeItem | TreeItem | TABLE_ROW / TREE_ITEM | AXRow |
| Window | Window | FRAME / WINDOW | AXWindow |
| Dialog | Window (IsDialog) | DIALOG | AXSheet / AXDialog |

#### Pattern-per-Platform Attribute Mapping

**TextContent**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Text | TextPattern.DocumentRange.GetText → ValuePattern.Value | Text.GetText(0, -1) | AXValue / AXTitle |

`Text` is sourced only from a genuine text interface — never the accessible name — so a plain label or button (no text interface) exposes no `control:Text`; its label stays in `control:Name`. `TextContent` has no provider attribute of its own beyond `Text`: it is surfaced on the client side by attribute-only synthesis (the presence of `control:Text`), not advertised in a provider's supported-patterns list. Read-only is **not** a `TextContent` attribute — it is a client-side derivation (`TextContent ∧ ¬TextEditable`) that belongs with `TextEditable`.

**TextEditable** — extends TextContent with write access:

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Text (write) | ValuePattern.SetValue / TextPattern ranges | EditableText.SetTextContents / InsertText | AXValue (settable) |
| MaxLength | — | — | — |

**TextSelection**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| SelectedText | TextPattern.GetSelection → GetText | Text.GetSelection → GetText | AXSelectedText |
| SelectionStart | TextPattern.GetSelection range start | Text.GetSelection nStartOffset | AXSelectedTextRange.location |
| SelectionEnd | TextPattern.GetSelection range end | Text.GetSelection nEndOffset | AXSelectedTextRange.location + length |

**Selectable / SelectionProvider**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| IsSelected | SelectionItemPattern.IsSelected | State.SELECTED | AXSelected |
| SelectedItems | SelectionPattern.GetSelection | Selection.GetSelectedChildren | AXSelectedChildren |
| CanSelectMultiple | SelectionPattern.CanSelectMultiple | Selection.NSelectedChildren context | AXAllowsMultipleSelection |

**Toggleable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ToggleState | TogglePattern.ToggleState (On/Off/Indeterminate) | State.CHECKED / State.INDETERMINATE | AXValue (0/1/2) |

**StatefulValue**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Value | RangeValuePattern.Value / ValuePattern.Value | Value.CurrentValue | AXValue |
| MinValue | RangeValuePattern.Minimum | Value.MinimumValue | AXMinValue |
| MaxValue | RangeValuePattern.Maximum | Value.MaximumValue | AXMaxValue |

**ActivationTarget**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ActivationPoint | GetClickablePoint() or Bounds center | Component.GetExtents(Screen) center | AXPosition center |
| ActivationArea | BoundingRectangle | Component.GetExtents(Screen) | AXFrame |

**Focusable**

| Attribute / Action | UIA | AT-SPI2 | macOS AX |
|--------------------|-----|---------|----------|
| IsFocused | HasKeyboardFocus | State.FOCUSED | AXFocused |
| focus() | SetFocus() | grab_focus() | AXFocused = true |

**Scrollable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ScrollHorizontalPercent | ScrollPattern.HorizontalScrollPercent | — (compute from Value) | — |
| ScrollVerticalPercent | ScrollPattern.VerticalScrollPercent | — (compute from Value) | — |
| HorizontallyScrollable | ScrollPattern.HorizontallyScrollable | — | — |
| VerticallyScrollable | ScrollPattern.VerticallyScrollable | — | — |

**Expandable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| IsExpanded | ExpandCollapsePattern.ExpandCollapseState | State.EXPANDED / State.EXPANDABLE | AXExpanded |

**Window capability patterns** (Activatable / Minimizable / Maximizable / Restorable / Closeable / Movable / Resizable / Responsive)

| Pattern → Attribute / Action | UIA | AT-SPI2 | macOS AX |
|--------------------|-----|---------|----------|
| Activatable.activate() | WindowPattern.SetWindowVisualState(Normal) + SetFocus | EWMH _NET_ACTIVE_WINDOW | AXRaise + AXFocused |
| Minimizable.minimize() | WindowPattern.SetWindowVisualState(Minimized) | EWMH _NET_WM_STATE | AXMinimized = true |
| Maximizable.maximize() | WindowPattern.SetWindowVisualState(Maximized) | EWMH _NET_WM_STATE | AXFullScreen (approx) |
| Restorable.restore() | WindowPattern.SetWindowVisualState(Normal) | EWMH _NET_WM_STATE | AXMinimized = false |
| Closeable.close() | WindowPattern.Close() | EWMH _NET_CLOSE_WINDOW | AXClose action |
| Movable.move_to() | TransformPattern.Move(x, y) | EWMH _NET_MOVERESIZE_WINDOW | AXPosition = (x, y) |
| Resizable.resize() | TransformPattern.Resize(w, h) | EWMH _NET_MOVERESIZE_WINDOW | AXSize = (w, h) |
| Responsive.accepts_user_input() | IsEnabled && IsInView + WaitForInputIdle | State.SENSITIVE && State.SHOWING | AXEnabled |
| IsMinimized | WindowPattern.WindowVisualState == Minimized | State.ICONIFIED | AXMinimized |
| IsMaximized | WindowPattern.WindowVisualState == Maximized | EWMH _NET_WM_STATE check | AXFullScreen |
| IsTopmost | WindowPattern.IsTopmost | EWMH _NET_WM_STATE_ABOVE | AXMain hint |

**Application**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ProcessId | CurrentProcessId | D-Bus peer credentials | AXPid (via kAXPIDAttribute) |
| ProcessName | Executable filename (without .exe) | /proc/PID/comm or cmdline[0] | NSRunningApplication.localizedName |
| ExecutablePath | OpenProcess + QueryFullProcessImageName | /proc/PID/exe | NSRunningApplication.executableURL |
| CommandLine | QueryProcessCommandLine (NtQueryInformationProcess) | /proc/PID/cmdline | — |
| UserName | Process token → LookupAccountSid | /proc/PID/status Uid → getpwuid | — |
| StartTime | GetProcessTimes → ISO 8601 | /proc/PID/stat field 22 (ticks → ISO 8601) | — |
| Architecture | IsWow64Process2 / PE header | ELF e_machine from /proc/PID/exe | — |

## 7. Provider Infrastructure

### 7.1 Providers and their factories

A **provider** is the piece that actually knows how to read *one* accessibility technology — UIA on Windows, AT-SPI2 on Linux, the mock tree in tests. The runtime never talks to UIA or AT-SPI directly; it only ever talks to providers. That is exactly what keeps the rest of the system platform-agnostic.

Each provider carries a **descriptor** — a small record of who it is: an id, a display name, which technology it speaks (`TechnologyId`), whether it is a real "native" provider or an external one (`ProviderKind`), and which change events it can emit (its `event_capabilities`, see §5.7). A **factory** creates the provider on demand. Creation can fail — a technology may be unavailable on the running system — and the runtime is prepared for that; nothing extra is handed to the factory at creation time.

Once created, a provider has three jobs:

- **Hand over children.** Asked for the children of a parent node, it returns them lazily — a stream the runtime pulls from as it descends, rather than a fully built list. The runtime stitches these results together under the desktop node.
- **Announce changes.** A provider can subscribe the runtime to its change events through an event listener (the default does nothing) — this is what feeds the event pipeline in §7.2. Providers that cannot emit events simply don't, and the runtime re-reads the tree when needed.
- **Clean up.** On shutdown it releases whatever it holds — COM handles, D-Bus connections, and the like.

*The exact signatures live in `crates/core/src/provider/`. The code is authoritative for the types; this section is about responsibilities.*

### 7.2 Provider registry and event pipeline

Providers don't register themselves with the runtime by hand. The **provider registry** (in `crates/runtime`) discovers the factories automatically — they announce themselves through the `inventory` mechanism — then groups them by technology and creates instances as needed. This is why adding a new provider is mostly a matter of declaring it: the registry finds it.

Change events flow through a small pipeline. Each provider gets its own **event listener** that forwards what the provider reports straight to a central **dispatcher**, which fans those events out synchronously to everyone who has asked to hear them. The per-provider listener keeps no snapshot of the tree; it only relays. The one piece of extra work it does is on a `NodeUpdated` event: before forwarding, it tells the affected node to invalidate its cached data, so that the next read goes back to the platform rather than returning a stale value.

The events a provider can report are:

- `NodeAdded`
- `NodeUpdated`
- `NodeRemoved`
- `TreeInvalidated`

External consumers — the CLI, the Inspector — don't touch the dispatcher directly. They subscribe through the runtime (`Runtime::register_event_sink`), which is the public seam for "tell me when the UI changes".

### 7.3 Provider compliance checklist

These are the rules every provider is expected to follow. They are listed as a checklist on purpose — it is the reference an implementer works through when bringing a provider up.

All providers must:

- Register via inventory macros; productive implementations use `cfg(target_os = ...)`.
- Provide complete `ProviderDescriptor` with accurate `event_capabilities`.
- Use lazy evaluation for `UiNode` and `UiAttribute` implementations.
- Use correct namespaces (`control`, `item`, `app`, `native`).
- Deliver all coordinates in the desktop coordinate system (DPI-aware).
- Maintain stable `RuntimeId` values for element lifetimes (format: `prefix:value`).
- Emit `Id` only when non-empty.
- Set `parent` references correctly in children iterators.
- Keep `SupportedPatterns` consistent with available pattern instances.
- Normalize roles to PascalCase; preserve originals under `native:*`.
- Filter out own-process windows/overlays from the UI tree.
- Implement `shutdown()` for resource cleanup.
- Implement `invalidate()` to discard cached data.
- Provided attributes match ClientPattern requirements from the pattern catalog (§6.3) — use constants from `platynui_core::ui::attribute_names`. The core testkit (`platynui_core::ui::contract::testkit`) validates this automatically.
- Keyboard device: key naming case-insensitive lookup, `known_key_names()` stable-sortable, `UnsupportedKey` errors informative.
- `ActivationTarget` provides `ActivationPoint` (native API preferred, bounds center as fallback).

#### Platform-Specific Checklist: Windows (UIA)

- Bounds from `BoundingRectangle` in desktop coordinates (Per-Monitor-V2 DPI active).
- ActivationPoint via `GetClickablePoint()` or center of bounds as fallback.
- `control:Text` (TextContent) priority: `TextPattern.DocumentRange.GetText` → `ValuePattern.Value`; never the accessible name. Absent when the element supports neither pattern.
- Window capability patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable) via `WindowPattern`/`TransformPattern`; `ResponsivePattern::accepts_user_input()` via `IsEnabled && IsInView` + `WaitForInputIdle`.
- Application node: process metadata (`ProcessId`, `Name`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime`, `Architecture`).
- `SelectionItemPattern`/`SelectionPattern` sync verified.
- COM initialized (`CoInitializeEx` MTA) before any UIA call.
- `VirtualizedItemPattern::Realize()` attempted before child traversal.
- Native properties via `GetCurrentPropertyValueEx(id, true)` with sentinel filtering (`ReservedNotSupportedValue`, `ReservedMixedAttributeValue`).
- VARIANT type conversion: `VT_BOOL` → Bool, `VT_I*/VT_UI*` → Integer, `VT_R*/VT_DECIMAL` → Number, `BSTR` → String, `SAFEARRAY(1D)` → Array.

#### Platform-Specific Checklist: Linux (AT-SPI2 + X11)

- Coordinates from `Component.GetExtents(ATSPI_COORD_TYPE_SCREEN)`.
- TextContent/TextSelection via AT-SPI `Text` interface.
- SelectionProvider via AT-SPI `Selection` interface.
- Window capability actions (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable) delegate to EWMH WindowManager (not direct X11 calls from provider).
- Component-gated attributes (`Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsInView`, `IsFocused`) only present when Component interface available.
- Native attributes: `Native/<Interface>.<Property>` format for all AT-SPI interfaces.
- Virtual desktops: provider does NOT handle desktop switching — that is `WindowManager::ensure_window_accessible()` responsibility.

#### Platform-Specific Checklist: macOS (AX)

- Coordinates from `AXFrame` in Core Graphics desktop coordinate system.
- `TextEditable` via `AXEditable`/`AXEnabled`.
- `Activatable` via `AXPress` action.
- Window capability patterns via accessibility actions + `CGWindow`/`NSWorkspace`.
- `kAXRaiseAction` for window activation (implicitly switches Spaces).

#### Platform-Specific Checklist: Mock

- Reference data covers typical pattern combinations (Button, Window, TextBox, List, TreeItem).
- Desktop coordinates tested (multi-monitor arrangement: 2160x3840 left, 3840x2160 center primary, 1920x1080 right).
- Scripted mock allows negative scenarios (missing patterns, failing focus).
- Pointer/keyboard mock produce deterministic logging (`take_pointer_log`, `take_keyboard_log`, `reset_*` helpers).
- Text buffer operations: `append_text`, `replace_text`, `apply_keyboard_events`.

## 8. Platform Devices

A **platform device** is the part of PlatynUI that *acts on* the desktop rather than reading it. Where providers (§7) answer "what is on screen?", devices answer "make something happen on screen" — move the pointer, press keys, capture pixels, draw a highlight, manage a window. Each device is defined as a trait in `platynui-core`, and each platform supplies the implementation that knows how to drive its own operating system. The runtime layers convenience and policy on top, so that test code can say "click this" without caring whether it is talking to Windows, X11, or Wayland.

### 8.1 PointerDevice

The **PointerDevice** is the lowest layer of mouse control. It deals only in raw, primitive actions expressed in desktop coordinates (as `f64`): where the pointer is, moving it to a point, pressing and releasing a button, and scrolling. A platform may also report two human-interface facts the runtime needs for correct clicking — the system's double-click time and the size of the region within which two clicks still count as a double-click. The trait is deliberately small; everything richer is built above it.

On top of that primitive, the runtime builds a **motion engine** — the part that decides not just *where* the pointer ends up but *how* it travels there, so that automated movement can resemble a human hand when that matters. You can choose between several **motion modes**: `direct` (jump straight to the target), `linear` (move in a straight line over time), `bezier` (a curved path), `overshoot` (sail slightly past the target and settle back, the way a real hand does), and `jitter` (small natural wobble). Independently of the path, an **acceleration profile** shapes the speed along it — constant, slow→fast, fast→slow, or a smooth S-curve.

Timing is just as configurable as geometry. The engine inserts delays at the natural seams of an interaction: after a move settles, between a press and its release, before the next click begins, and between the clicks of a multi-click. It can also optionally **verify** that the cursor actually reached its target before continuing.

These knobs are bundled rather than passed loose. A **PointerProfile** packages the movement and timing parameters together as a reusable preset; `PointerProfile::named_default()` provides the one built-in preset. When a single call needs to differ from the profile, you do *not* register another named preset — instead you supply **PointerOverrides**, a per-call set of deltas built up through a small builder API. Overrides also carry an optional `origin`, which says what a coordinate is measured *from*: `PointOrigin::Desktop` (absolute desktop space), `PointOrigin::Bounds(Rect)` (relative to some element's rectangle), or `PointOrigin::Absolute(Point)` (relative to an explicit anchor point). This is how "click 5 pixels inside the top-left of this button" becomes a concrete desktop coordinate.

The runtime exposes the whole engine through a handful of methods — `pointer_move_to`, `pointer_click`, `pointer_multi_click`, `pointer_press`, `pointer_release`, `pointer_drag`, and `pointer_scroll` — each of which accepts an optional set of overrides.

*Exact device-trait signatures and the override types live in `crates/core` and the runtime; the code is authoritative for the types, this section is about responsibilities.*

### 8.2 KeyboardDevice

The **KeyboardDevice** is the keyboard counterpart of the pointer device, and it is just as deliberately minimal. A platform implementation does four things: it translates a human-readable key name or alias into the provider-specific code for that key (a step that can fail if the name is unknown), it sends a single key event (one press or one release), it optionally runs setup and teardown hooks around a burst of input — useful for things like switching the IME — and it can list the key names it understands. A single **KeyboardEvent** pairs a key code with its state, either Press or Release; that is the only thing a platform ever actually sends.

What turns those bare events into something usable is the runtime's **KeyboardSequence**, the central representation of "what to type". A sequence is parsed from a single human-friendly string that freely mixes literal text with shortcut notation — for example `"text<Ctrl+a><Ctrl+Delete>Hello"`. The parser understands backslash **escapes** so the reserved characters can appear as literals: `\\<`, `\\>`, and `\\` for the delimiters and the backslash itself, plus `\\xNN` and `\\uNNNN` for characters by code point. It also understands **multi-shortcuts** — a chord followed by another chord inside one bracket, as in `<Ctrl+K Ctrl+C>`. Parsing is lazy: the sequence yields key events as they are consumed rather than expanding everything up front, and an unrecognized key name surfaces as an unsupported-key error.

Because `+`, `-`, `<`, and `>` are meaningful inside shortcut notation, you refer to them by **symbol alias** when you mean the literal key: `PLUS` (+), `MINUS` (-), `LESS` or `LT` (<), and `GREATER` or `GT` (>).

Key **naming conventions** aim to be both predictable and platform-honest. Names follow each platform's official terminology but drop noisy prefixes — Windows's escape key is `ESCAPE`, not `VK_ESCAPE`. Keys that exist everywhere share one name across platforms (`Enter`, `Escape`, `Shift`), while genuinely platform-specific keys keep their established OS terms: `Command` and `Option` on macOS, `Windows` on Windows, `Super` or `Meta` on Linux.

The runtime offers three entry points over a sequence: `keyboard_type`, which does a press followed by a release for each step (ordinary typing); `keyboard_press`, which only presses (useful for holding modifiers down); and `keyboard_release`, which only releases. As with the pointer, timing lives in a profile. A **KeyboardProfile** holds the timing fields — `press_delay`, `release_delay`, `between_keys_delay`, `chord_press_delay`, `chord_release_delay`, `after_sequence_delay`, and `after_text_delay` — and **KeyboardOverrides** supplies per-call deltas against it.

*Exact trait and type definitions live in `platynui-core` and the runtime.*

### 8.3 ScreenshotProvider

The **ScreenshotProvider** captures pixels from the screen. A capture request can name a sub-region of the desktop; with no region, the whole desktop is captured. What comes back is a screenshot carrying its `width` and `height`, a `format`, and the raw pixel buffer. The format is one of two byte orderings — `Rgba8` or `Bgra8` — because different platforms hand back pixels in different channel orders, and the caller needs to know which it received. The device itself stops at raw pixels on purpose: turning them into PNG or JPEG is left to the caller, so the capture path stays cheap and encoding policy stays out of the platform layer.

*Exact request and screenshot types live in `platynui-core`.*

### 8.4 HighlightProvider

The **HighlightProvider** draws the visible markers that show a human (or a video of a test run) which elements automation is touching. It has two operations: draw a highlight, and clear it. A highlight request carries one or more desktop bounding boxes — so several elements can be outlined at once — and an optional duration. Two behaviors keep highlighting unobtrusive: there is only ever **one active highlight** at a time, so a new request replaces whatever overlay was showing rather than stacking on top of it; and when a duration is given, the provider **auto-clears** itself when that timer elapses, with no follow-up call required.

*Exact request type lives in `platynui-core`.*

### 8.5 WindowManager

The **WindowManager** is the device that operates on whole windows rather than on individual controls: activating a window, closing, minimizing, maximizing, restoring, moving, and resizing it, and answering where it is and whether it is currently active. Internally a window is referred to by an opaque **WindowId** — a single number that stands in for whatever the platform's real handle is (an HWND on Windows, an XID on X11, a Wayland surface id), so the rest of the system never has to know the platform's native window type.

The central idea is **resolving a window from a node**. Automation finds elements as `UiNode`s in the accessibility tree (§5), but window operations need a window handle, and the relationship between "this node" and "the window it lives in" is platform-specific. So the WindowManager takes a node and figures out which window it belongs to, with each platform extracting whatever it needs:

- **Windows**: reads `native:NativeWindowHandle` → HWND (+ PID-fallback via `EnumWindows`)
- **X11**: walks parent chain for `control:ProcessId`, matches via `_NET_CLIENT_LIST` + `_NET_WM_PID`; disambiguates multi-window PIDs by comparing `node.name()` against `_NET_WM_NAME`

**Virtual Desktop Switching** (planned, not yet implemented) — `ensure_window_accessible()` will be added to the `WindowManager` trait and called in `bring_to_front()` before `activate()`. See `dev-docs/planning.md` §3.2 for the design:
- **X11**: read `_NET_WM_DESKTOP`, switch via `_NET_CURRENT_DESKTOP` ClientMessage if different
- **Windows**: `IVirtualDesktopManager::MoveWindowToDesktop` to move window to current desktop
- **macOS**: no-op (`kAXRaiseAction` implicitly switches spaces)

The reason the WindowManager is its own device — separate from the providers that read the tree — shows up in the **layer model**. A provider such as `provider-atspi` describes *what* the windows are; a platform crate knows *how* to manipulate them through the windowing system. Keeping these apart is what lets one accessibility provider work unchanged across very different windowing backends:

```
provider-atspi / provider-windows-uia  ──uses──►  core::WindowManager (trait)
                                                            ▲          ▲
                                            platform-linux-x11   platform-windows
                                              (EWMH/NetWM)       (Win32 HWND)
                                      platform-linux-wayland (IPC/wlr backends)
```

This keeps `provider-atspi` free of `x11rb` dependencies and working identically on X11 and Wayland. Today only the PlatynUI IPC compositor backend is implemented; other Wayland compositors return `CapabilityUnavailable`.

Wayland complicates the picture because, unlike X11 with its long-established EWMH conventions, it has **no standardized protocol** that lets an external client switch desktops or workspaces. What exists today is a scatter of compositor-specific and draft protocols, each covering only part of the problem:

| Protocol / API | Compositor | Capabilities |
|---|---|---|
| `wlr-foreign-toplevel-management-unstable-v1` | wlroots (Sway, Hyprland) | Activate, minimize, maximize, close windows. **No desktop switch.** |
| `ext-foreign-toplevel-list-v1` | Draft standard | Read-only listing of toplevel windows only |
| `cosmic-toplevel-info-unstable-v1` + `cosmic-workspace-unstable-v1` | COSMIC/Pop!_OS | Workspace listing and activation |
| KDE D-Bus (`org.kde.KWin`) | KDE-specific | `setCurrentDesktop`, `windowToDesktop` |
| GNOME Shell D-Bus (`org.gnome.Shell`) | GNOME-specific | Extensions API, not official |

Currently irrelevant since PlatynUI uses X11/XWayland. Long-term, Wayland support would require either compositor-specific backends or waiting for a cross-compositor protocol. See §4.4 in planning.md for details.

*Exact trait signatures, including the full set of window operations, live in `crates/core`.*

## 9. XPath Engine

### 9.1 Architecture

PlatynUI lets you address elements with **XPath** — the same path language you may know from XML — so you can write `//Window/Button[@Name='OK']` instead of hand-walking the node tree. The engine that makes this work lives in `crates/xpath` (package `platynui-xpath`) and implements XPath 2.0. It is organized as four cooperating layers, each handing its result to the next:

1. **Parser** — reads the XPath text and turns it into a strongly-typed syntax tree. It uses a PEG grammar (via the `pest` crate), so the grammar itself is the specification of what is and isn't valid XPath here.
2. **Compiler** — rewrites that syntax tree into an optimized intermediate form. This is where the engine does its thinking ahead of time: it pre-computes literal values, merges adjacent filter predicates, and specializes the individual axis steps so that evaluation later does less work.
3. **Engine** — walks the optimized form against the tree, carrying two kinds of context as it goes. The *dynamic* context is what changes during evaluation (the current node, the position in a sequence); the *static* context is fixed up front (such as the namespace prefixes described in §9.4).
4. **Model** — the abstraction the engine evaluates *against*. Rather than requiring the whole tree to exist in memory, the model is a trait that any backend can implement to expose its own tree lazily.

That model trait is the key seam. It describes a node in the **XDM** (XQuery and XPath Data Model) sense — a typed, navigable tree node — without committing to how the tree is stored or where it comes from. An implementation answers what *kind* of node it is, its name, its typed and string values, and lets you walk to its parent, children, attributes, and namespaces. Crucially, the children, attributes, and namespaces are produced lazily, on demand, as the engine descends — so evaluating a query against a huge desktop tree never forces the whole tree into existence. The trait is also designed so each backend can return its own concrete iterator types rather than boxed ones, keeping traversal cheap.

*The exact trait definition lives in `crates/xpath/`. The code is the source of truth for signatures; this section explains what the layers are for.*

### 9.2 Streaming & Normalization

The engine evaluates in **streaming** mode: instead of computing a whole result set and then handing it back, it yields partial results as soon as they are known and applies predicates early, so a query can start producing matches before it has finished walking the tree.

XPath 2.0 requires that a path expression's results come back in **document order** and **without duplicates**. The engine doesn't bake that guarantee into every step; it makes it explicit as two separate operations in the intermediate form, and inserts them only where an axis can actually violate the guarantee:

- **`EnsureDistinct`** — removes duplicate nodes while preserving the order they arrived in. It is implemented as a cursor, so it stays fully streaming (no need to buffer everything first).
- **`EnsureOrder`** — enforces document order. It is written to do the least work that correctness allows: input that is already monotone passes straight through, simple local inversions are repaired in place, and only genuine disorder falls back to buffering and sorting.

Which of these an axis needs follows conservative, spec-compliant rules:

- Forward axes `child`, `self`, `attribute`, `namespace`: no normalization.
- Forward axes `descendant`, `descendant-or-self`, `following`, `following-sibling`: `EnsureDistinct`.
- Reverse axes `parent`, `ancestor*`, `preceding*`: `EnsureDistinct` plus `EnsureOrder`.
- Path steps and set operations are normalized before the next step runs.

There is one more trick that keeps normalization rare. Before certain axes (`descendant*`, `following*`), the engine minimizes the context set — it removes overlapping starting points at the source. Because the overlap that would have produced duplicates is gone before the axis runs, the after-the-fact normalization often isn't needed at all.

### 9.3 XDM Cache

Building the node model is not free — it can mean reading the live UI tree from the platform — so the engine can keep a cache of it between evaluations. The **`XdmCache`** holds one cached desktop snapshot (keyed by its runtime id). It is built to be shared: it is `Clone + Send + Sync`, so a single runtime-owned cache can be handed to several threads and they all see the same snapshot. You create it explicitly through `Runtime::create_cache()`; the runtime constructor does not make one for you. If you evaluate without a cache, the runtime simply rebuilds the desktop snapshot before every single XPath evaluation.

The cache stays correct through **lazy revalidation** rather than eager rebuilding. Before an evaluation, it checks whether its provider nodes are still valid and resets the per-node "children already validated" flags; any subtree that turns out to be stale is rebuilt transparently the next time something touches it, while still-valid parts are reused. For convenience there are cache-aware counterparts to the normal evaluation entry points (`evaluate_cached()`, `evaluate_iter_cached()`, `evaluate_single_cached()`).

### 9.4 Evaluation API

The central entry point is `evaluate`. You give it an XPath string, a set of options, and an *optional* context node to start from — and if you pass no context, evaluation starts at the desktop root. The options let you choose whether to use a cache or not, supply a node resolver so the engine can re-resolve a context node by its `RuntimeId` (see §5), and hand in a **cancel flag**: a shared boolean the engine checks at each XPath axis step, so a long-running query can be cooperatively interrupted from another thread. For language bindings there is also an owned-iterator form (`EvaluationStream`) that returns results without any borrowed references, plus a cancellable variant of it for interruptible streaming.

Whatever the query selects, each result comes back as an **`EvaluationItem`**, which is one of three kinds:

- a **Node**,
- an **Attribute** (its owner, its name, and its value), or
- a **Value** (a plain `UiValue`).

A few supporting pieces shape how expressions are understood:

- **Static prefixes.** The static context registers a fixed set of namespace prefixes you can use in queries: `control`, `item`, `app`, and `native`.
- **Context-dependence.** `is_context_dependent(xpath)` answers whether an expression reads the context node at its top level — a relative path, `.`, or a context function taking no arguments — and it recurses through `if`/`for`/`let`/sequence/filter/union to decide. The BareMetal `Set Root` keyword uses this to choose between drilling relative to a node and starting from an absolute root.
- **Typed values.** Every node must be able to report a typed value, returned as XDM-conformant atomics (`xs:boolean`, `xs:integer`, `xs:double`, `xs:string`). Complex structures such as Rect, Point, and Size stay as JSON-encoded strings, but their derived components (`Bounds.X`, and the like) come back as numeric atomics.

## 10. Platform Implementations

Up to this point the architecture has been deliberately platform-neutral: nodes, providers, patterns, and the runtime all describe *what* PlatynUI does without committing to *how* any one operating system delivers it. The platform implementations are where that abstraction finally meets reality — where a "focusable" pattern becomes a concrete UIA call on Windows or an AT-SPI2 request on Linux, and where pointer and keyboard input turn into actual device events. Because each platform brings its own accessibility technology, window manager, and input plumbing, those details are large enough to live in their own documents rather than crowd the core design here. Start with the file for the platform you are working on:

- **Windows** (UIA, Win32 devices, WindowManager): [`dev-docs/platform-windows.md`](platform-windows.md)
- **Linux** (X11 devices, AT-SPI2, EWMH WindowManager): [`dev-docs/platform-linux.md`](platform-linux.md)

## 11. Companion Documentation

This document covers the Rust core and the cross-platform model that everything else stands on. A handful of companion docs go deeper into the parts that live at the edges of the system — where PlatynUI meets Python, the command line, the Inspector UI, and the logging infrastructure. Reach for them when your work touches one of those areas.

- **Python Bindings** — how the Rust core is exposed to Python via PyO3, how types are mapped across the boundary, and how threading is handled: [`dev-docs/python-bindings.md`](python-bindings.md)
- **CLI** — the available commands and the snapshot model the CLI works against: [`dev-docs/cli.md`](cli.md)
- **Inspector** — the architecture of the Inspector's TreeView: [`dev-docs/inspector.md`](inspector.md)
- **Logging & Tracing** — how the system emits diagnostics: [`.github/instructions/tracing.instructions.md`](../.github/instructions/tracing.instructions.md)
