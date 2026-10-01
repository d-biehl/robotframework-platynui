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
├─ process                   # Process identity (pid + start time) and process attributes — platynui-process
├─ platform-windows          # Windows devices — platynui-platform-windows
├─ provider-windows-uia      # UIA provider — platynui-provider-windows-uia
├─ provider-java             # The single Java provider, routing to toolkit backends — platynui-provider-java
├─ provider-java-jab         # Java Access Bridge backend (Swing/AWT, Windows) — platynui-provider-java-jab
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
├─ win32-test-window         # Plain Win32 window for process-level tests (built 32-bit) — platynui-win32-test-window
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
3. **Wire up events.** Finally, the runtime subscribes its event listeners to the providers, so that changes observed in the underlying UI can be delivered back through the event pipeline (see §5.8, §7.2).

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

- **Who am I?** Its namespace (see §5.2), its role ("Window", "Button", "ListItem", …), its name, the identifier its toolkit reports when there is one (see §5.5), and a runtime id that identifies it uniquely for its lifetime (see §5.4).
- **Who is around me?** Its parent and its children. Children are produced *lazily* — the tree is walked only on demand, never built up front. Opening a desktop with thousands of controls is therefore cheap until you actually descend into it. A node can also report cheaply whether it has any children at all (by default this just probes the child walk, but a provider can override it with a cheaper check), and it carries an optional hint for its position in document order.
- **What can you tell me, and what can you do?** Its **attributes** (typed values such as bounds, name, state — see §5.7) and its **patterns** (capabilities such as "focusable" or "activatable" — see §6). A node announces the patterns it has, and hands out the implementation of a runtime pattern when asked (§6.3).

Attributes are lazy too: a node lists which attributes it has — and lets you look one up by namespace and name — but the concrete value is computed only when something reads it, because reading a value can mean a round-trip to the operating system's accessibility API.

A node also manages its own freshness. It can report whether it is still **valid** (a cheap liveness check — by default a node assumes it is alive until told otherwise) and it can be **invalidated**, which tells it to discard whatever it has cached so the next access fetches fresh data from the native API. This is the hook the event pipeline (see §5.8) pulls on when the platform reports that something changed.

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
| JAB (Swing/AWT) | `jab://<vmID>/0x<hwnd>[/<enum-index-path>]`, app view `jab://app/<pid>/…` | `jab://12345/0x2A0B3C/0/3/1` |
| Java agent | `agent/<pid>/<id>`, app view `agent/app/<pid>/<id>` | `agent/4711/42` |
| AT-SPI2 | AT-SPI D-Bus object path | `atspi:///org/a11y/...` |
| Mock | `mock:<id>` | `mock:window-1` |
| Desktop | `platynui:Desktop` (reserved) | `platynui:Desktop` |

Providers generate deterministic IDs stable for the element's lifetime. The `platynui` prefix is reserved for the desktop node.

**One id per view.** An id names an element at one place in the tree. A provider that shows an element in two views — in the desktop's list of top-level windows, and below its application's `app:Application` node — gives it one id per view, so the two copies are two nodes. That is what the `app` part of the UIA, JAB and Java agent schemes is for. The XPath engine removes duplicate nodes by identity (§9.2); if both copies shared one id, how many windows `//Window` returns would depend on how the query is written, because a search that reaches a window through both views keeps only one of them.

### 5.5 Id (`control:Id`)

Visible labels change — they get renamed, translated, or reworded — so they make brittle selectors. The **Id** exists to give you something steadier: the identifier an element's toolkit reports for automation, independent of the visible label and of language. When you want a selector that survives, prefer `@control:Id='...'` wherever it is available.

Each provider takes the Id from one source: on Windows the UIA `AutomationId`; on Linux/AT-SPI2 the `AccessibleId` property, or, when that is empty or unreadable, the first of the object attributes `accessible-id`, `accessible_id` and `id` that is not empty; through the Java agent, `Component.getName()` of components and windows, when it was set through `setName`; on macOS/AX the `AXIdentifier`. The Java Access Bridge reports none. Who set the value does not matter: an identifier the toolkit or framework generated — a Qt object path, a name Swing gives its own parts such as `Spinner.nextButton`, an AccessKit author id forwarded to UIA and AT-SPI — is the Id just like one the application set. A provider never puts another value in place of a missing identifier: not the name, the process ID or the runtime id. It takes the identifier unmodified, as every value (§5.7): an identifier of whitespace is an identifier, and only the empty string counts as none.

The Id is present only with a value. A node without one lists no `control:Id`, and its id accessor (`element.id` in Python) answers none rather than an empty string. The accessor and the attribute always carry the same value, and listing a node's attributes agrees with looking `Id` up by name. **Application nodes carry no Id** on any provider: an application is identified by its `@ProcessId`, so its one-line description ends with its quoted name, without a `#…` suffix. The rule is spec `id-attribute`; the contract testkit checks it with `verify_id`.

### 5.6 Accessible Description (`control:Description`)

The **description** is the accessibility API's longer, human-readable explanation of an element — distinct from its short accessible name. It is a common attribute (see §6.6), and like the Id it is **optional**: a node emits `control:Description` only when its platform value is non-empty, so absence is normal and expected rather than an error. The value is taken unmodified (§5.7), so a description of whitespace is a description.

The mapping is deliberately **strict** — `control:Description` maps 1:1 to the platform's genuine accessible-description property and nothing else:

| Platform | Source | Explicitly excluded |
|---|---|---|
| Windows UIA | `FullDescription` (`UIA_FullDescriptionPropertyId`) | `HelpText`, `LegacyIAccessible.Description` |
| Linux AT-SPI2 | `Accessible.Description` | `Accessible.HelpText` |
| Java Access Bridge (Swing/AWT, Windows) | the accessible description the bridge reports, which is Swing's `getAccessibleDescription()` | — |
| Java agent (Swing/AWT) | Swing's `getAccessibleDescription()`, which Swing derives from the tool tip text when nothing else is set | — |
| macOS AX | open (stub provider; `AXHelp` is help text, not a description) | — |

The Java rows add no fallback either: where Swing derives a description from a component's tool tip, that is the accessible description Swing reports, and the providers take it as such.

There is **no fallback** to help/tooltip-like properties: HelpText is a different concept (tooltip/help semantics), and returning it as a "description" would surface plausible-looking but semantically wrong values. The excluded properties remain reachable under the `native:` namespace for callers who genuinely want them; a dedicated help/tooltip attribute, if ever wanted, would be a separate concept. UIA `FullDescription` requires Windows 10 1703+ and app support, so on older targets or apps that never set it the attribute is simply absent — again, correct behavior, not an error.

The value parity with `name`/`id` extends up the stack: `UiNode::description()` is the first-class accessor (see §5.1), exposed through the native bindings and as a read-only `description` property on the Python `Adapter` and `Context`.

### 5.7 UiValue & Attribute Normalization

Attribute values are **typed** rather than stringly-typed: a `UiValue` is one of String, Bool, Integer, Number (an `f64`), or Null, plus the structured values Rect, Point, Size, Array, and Object. Carrying real types means the XPath and interaction layers can compare and compute on values without re-parsing them out of text.

Values themselves are **not normalized**: every provider passes a value through as its platform reported it. No provider trims, collapses or otherwise rewrites a string, in any namespace, and that includes the strings inside structured values (the keys and values of an object, the entries of an array); a value of whitespace is a value. Where a platform transports a typed value as a string, the provider parses the unmodified string, and a string that does not parse stays the string it was — the Java Access Bridge sends an accessible value, a `java.lang.Number`, as its `toString()`. Whether the empty string is a value depends on the attribute group:

- **Common attributes:** `Name` is always present and may be `""`; `Id` and `Description` are present only when they are not the empty string (§5.5, §5.6).
- **Pattern attributes** are present whenever the node implements the pattern, and `""` is a value: an empty text field has `control:Text` = `""`.
- **Native attributes** are passed one to one, so an empty string stays `""`. A property the platform does not report at all, because it is unsupported or its read failed, stays absent or null.

A locator that wants to ignore surrounding whitespace says so with `normalize-space(...)`. The rule is spec `attribute-values`.

For the structured values, the runtime makes the parts directly addressable. From a value like `Bounds` it auto-generates **alias attributes** for the components — `Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`, and so on — so a query can reach a single coordinate without unpacking the whole rectangle. These aliases are synthesized by the runtime and XPath layer; providers must not generate them themselves.

Naming is normalized so that queries look the same everywhere. All attribute names use **PascalCase**, and native role names are mapped onto PascalCase roles — for example UIA's `UIA_ButtonControlTypeId` becomes simply `Button` — while the untouched original role is still available as `native:Role` for when you need the platform's own term. To keep these names consistent across the codebase rather than scattered as string literals, `platynui-core::ui::attribute_names::<pattern>::*` provides the canonical string constants.

**Roles are normalized in spelling, not in meaning.** Each accessibility technology has its own vocabulary of roles, and within one technology the toolkit decides which role an element gets: a technology may know both a frame and a window role, and one toolkit reports its top-level windows as frames while another reports them as windows. Whether an element sits in `control:` or `item:` can differ in the same way (§5.2). A role is therefore not part of the cross-platform contract; the common attributes and the patterns are (§6). A selector that has to work across platforms and toolkits accepts the alternatives — `(Frame|Window)`, `*:MenuItem`, or `*` where the position in the tree already says enough — or selects by attributes such as `@Id` and `@Name`. §10 lists the roles each technology typically reports; the Inspector shows the role an element actually has.

### 5.8 Event Capabilities & Invalidation

Providers differ in how much they can tell the runtime about change, and the runtime adapts to whatever a given provider is capable of. A `ProviderDescriptor` advertises this with an `event_capabilities` bitset, and the runtime picks its refresh strategy accordingly:

| Capability | Behavior |
|------------|----------|
| `None` | Runtime polls/refreshes before every query |
| `ChangeHint` | Provider signals "something changed"; runtime invalidates parent and re-queries children |
| `Structure` | Structural events with parent/RuntimeId; runtime handles affected subtrees selectively |
| `StructureWithProperties` | Additionally includes property change events |

Above and beyond those levels, `TreeInvalidated` is the fallback for drastic changes — a provider restart, for instance — and triggers a full reload of the tree.

The mechanism that makes all of this work is invalidation at the node level: a provider must implement `UiNode::invalidate()` (see §5.1) so that a node discards its cached data — children, attributes, and patterns — and the next access fetches fresh data from the native API. The payoff is that event-capable providers only trigger targeted updates of the affected parts of the tree, while providers without events fall back to a full refresh before each query.

**Status: the runtime does not implement this table.** It never reads `event_capabilities`; only `platynui-cli watch` does, to warn when no active provider has an event source. The runtime reacts to one event only, as §7.2 describes: on `NodeUpdated` it invalidates that node. `NodeAdded`, `NodeRemoved` and `TreeInvalidated` are only forwarded to event sinks, and no event reaches an XPath cache. How current a query's view of the UI is follows the snapshot model of §9.3 instead.

## 6. Pattern System

> **Scope of this section.** It defines the pattern contract independently of any accessibility technology: what a pattern is, who provides what, and the rules for announcing patterns. The catalog in §6.6 is the contract we design towards, so it also lists patterns that are not implemented yet; the authoritative list of what exists in code is `pattern_names` in [`crates/core/src/ui/identifiers.rs`](../crates/core/src/ui/identifiers.rs), and the attribute names live in `crates/core/src/ui/attributes.rs`. How each provider implements the patterns is described per provider in §10.
>
> **Draft.** Points marked **Open question** are not decided yet; they are collected in §6.7.

### 6.1 What a pattern is

A pattern is a contract we impose on ourselves, the same on every platform. It says two things about an element: **what a user can do with it**, and **which attributes it carries** for that. A check box is something a user can switch on and off, and it tells you whether it is on; a slider is something a user can move, and it tells you its value and its range; a window can be minimized, and it tells you whether it is.

The point of the contract is that automation code is written once per pattern, not once per platform and toolkit. Every accessibility technology describes a check box in its own way — one offers a dedicated toggle interface, another a set of states, a third only a role plus a state flag. A provider translates its technology's description into the pattern's attributes, and from then on a keyword or a Python class that understands "Toggleable" works for every check box on every platform, without knowing where it came from.

Patterns compose. An element does not get a fixed class ("this is a button, so it can do X"); it carries the small set of patterns it really has. A list item can be selected and clicked; a text field can be focused, read and typed into; a top-level window can additionally be activated, minimized and moved. The role (§5) says what an element *is*; its patterns say what you can *do* with it and what you can *read* from it.

### 6.2 Providers read, the client acts

Providers give **read access** to the elements and their attributes. The only exception are the **runtime actions**: setting the keyboard focus and controlling windows — activate, minimize, maximize, restore, close, move, resize — plus the question whether a window currently accepts input. These are the actions a provider performs itself, through the platform.

Everything else a user does with an element — clicking it, switching it, selecting it, expanding it, typing into it — the client does the way a user does: with the pointer and the keyboard, through the platform devices (§8). No provider offers a programmatic shortcut such as "select this item" or "set this text", even where its accessibility technology has one. A programmatic shortcut takes a different path through the application than a user does, so a test that uses it can pass while the real interaction is broken. For text input this is spec `text-input-policy`.

The system therefore stops at *describing* and at the few runtime actions. Once an element announces that it can be toggled and where it can be clicked, it is up to the client whether it clicks it, presses the space bar, or uses a keyboard shortcut — exactly the range a human has.

### 6.3 Two kinds of pattern

- A **runtime pattern** is an action the provider performs: Focusable, Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable and Responsive (§6.6). A node that announces one hands out an *instance* of it on request, and the instance carries the action. The instance is what the announcement guarantees. Where the action changes a state the element has — focus, minimized, maximized — the pattern also carries that state as a required attribute, so a client can check the effect of the action.
- A **client pattern** promises attributes, and nothing else. The interaction that goes with it — clicking a toggle, selecting an item — is carried out by the client (§6.2). A client pattern never has an instance; it is a name in the announcement plus its attributes. Every client pattern has **at least one attribute a client can always evaluate**: a required attribute, or an optional one whose absence has a defined meaning (an absent `CanSelectMultiple` means single selection). That way the announcement always promises something a client can read.

Each pattern defines **required** and **optional** attributes. Required attributes are present whenever the pattern is announced. Optional attributes are present when the platform provides them, and absent otherwise; their absence is normal, not an error.

### 6.4 Announcing patterns

Every node lists the patterns it has in its **`SupportedPatterns`** attribute (and, in code, through `supported_patterns()`). The announcement has two audiences:

- **The user.** The Inspector and the query output show what can be done with an element, so a test author sees at a glance that a node is selectable, or that a window can be maximized.
- **The client library.** The Python side uses the announcement to bind an element to the right automation classes: an element that announces Toggleable gets the code that reads its toggle state and toggles it with a click (§6.5).

The rules:

1. **Announced means guaranteed.** A node that announces a client pattern carries all of the pattern's required attributes. A node that announces a runtime pattern hands out an instance of it. A client relies on this without checking.
2. **Announce what the element really has.** The decision comes from what the platform reports about the element — its role, its states, its interfaces, the patterns its accessibility API offers — and is made per element, never guessed from the element's name.
3. **Attributes without the announcement are allowed, but should be the exception.** Some technologies do not give enough information to decide whether an element implements a pattern completely. A technology may, for example, report whether an element with a text interface is editable without saying whether it is a text field or a label. A provider may then publish the attributes it can read without announcing the pattern. Such attributes guarantee nothing; they let a user write their own implementation for that element on the Python side, based on what is there.
4. **Pattern attributes live in the `control` namespace,** whatever the namespace of the node. A table row in `item:` still reports `control:IsSelected`, so `@IsSelected` finds it like any other element.
5. **The announcement does not change at runtime.** It describes what an element is and what it supports, not what is possible at this moment. A window that supports minimizing announces Minimizable for as long as it exists, even while the application blocks minimizing. Whether an action is possible right now is a state, and a pattern expresses it with an optional `Can…` attribute (`CanMinimize`, `CanClose`, …). An absent `Can…` attribute means the action is possible.
6. **Values are taken as the platform reports them** (§5.7). For a pattern attribute, the empty string is a value: an empty text field announces TextContent and reports `Text` = `""`.

In XPath, `@SupportedPatterns` is a single string that holds the list, so a query tests for a pattern with `contains(@SupportedPatterns, "org.platynui.patterns.Toggleable")`. The client library and Robot Framework keywords receive the list as a list.

### 6.5 How the client uses patterns

The provider's side of a pattern ends with the announcement and the attributes; the user's actions are not part of it (§6.2). The client library builds on that. For each pattern it has an implementation of the same name — `Selectable` for Selectable, `Toggleable` for Toggleable — which reads the pattern's attributes and adds the actions a user performs, carried out with pointer and keyboard: a click selects an item or switches a check box, focus, select-all and typing enter text. The client chooses the implementation by the announced patterns, together with the role where a role needs different handling, so the same code serves every platform. How these implementations are structured is the client library's design (`python-library-design.md`).

When an element publishes a pattern's attributes without announcing the pattern (§6.4, rule 3), no implementation is bound automatically. The user can register their own implementation for that element instead, written against the attributes that are there.

### 6.6 Pattern catalog

#### Common attributes

Every `control:` and `item:` node carries a small set of attributes, independent of its patterns. `Role`, `Name`, `RuntimeId`, `Technology` and `SupportedPatterns` are always present; `Id` and `Description` only when the platform reports a value. The constants live in `crates/core/src/ui/attributes.rs` (`common`).

| Attribute | Presence | Meaning |
|---|---|---|
| **Role** | always | The element's kind as a PascalCase name, e.g. `Button`; the original platform term stays in `native:Role` (§5.7). |
| **Name** | always | The accessible name and nothing else; no other value takes its place when it is empty (spec `name-attribute`). |
| **Id** | when not empty | The identifier the toolkit reports for automation (§5.5). Never on `app:` nodes. |
| **Description** | when not empty | Accessible description (§5.6), from one strict source per platform. |
| **RuntimeId** | always | Identity for the lifetime of the node (§5.4). |
| **Technology** | always | The accessibility technology that produced the node; each provider reports its own identifier (§10). |
| **SupportedPatterns** | always | The patterns the node announces (§6.4). |

#### Runtime patterns

| Pattern | The provider performs | Required | Optional |
|---|---|---|---|
| **Focusable** | give the element the keyboard focus | `IsFocused` | — |
| **Activatable** | bring a window to the foreground and make it the active one | — | — |
| **Minimizable** | minimize a window | `IsMinimized` | `CanMinimize` |
| **Maximizable** | maximize a window | `IsMaximized` | `CanMaximize` |
| **Restorable** | return a window to its normal size and position | — | — |
| **Closeable** | close a window | — | `CanClose` |
| **Movable** | move a window to a screen position | — | `CanMove` |
| **Resizable** | change a window's size | — | `CanResize` |
| **Responsive** | answer whether a window currently accepts input (yes, no, unknown) | — | — |

Each window action is its own pattern, so a window that can be minimized but not moved announces exactly that. A `Can…` attribute that is absent counts as true (§6.4, rule 5). Each action can still fail — the platform may refuse it, or the window may be gone — and the caller handles that.

#### Client patterns

| Pattern | The user can … | Required | Optional |
|---|---|---|---|
| **Element** | see it on screen and interact with it | `IsVisible`, `IsEnabled` | `Bounds`, `IsInView` |
| **ActivationTarget** | point at it and click it | `ActivationPoint` | `ActivationArea`, `ActivationHint` |
| **TextContent** | read its text | `Text` | — |
| **Editable** | change its value — type its text, adjust its number, pick another option — unless it is read-only right now | `IsReadOnly` | `MaxLength` |
| **Toggleable** | switch it on and off | `ToggleState` (`On`, `Off`, `Indeterminate`) | — |
| **Selectable** | select it within its container | `IsSelected` | — |
| **SelectionProvider** | select items inside it | — | `CanSelectMultiple` (absent: single selection), `IsSelectionRequired` |
| **Expandable** | expand and collapse it | `IsExpanded` | `CanExpand` (absent: true) |
| **StatefulValue** | set a value within a range | `Value` (number) | `MinValue`, `MaxValue` |
| **WindowState** | see the state of a window | `IsActive` | `IsTopmost`, `IsModal` |
| **TextSelection** | select part of its text | `SelectedText`, `SelectionStart`, `SelectionEnd` | — |
| **Scrollable** | scroll its content | `ScrollHorizontalPercent`, `ScrollVerticalPercent`, `ScrollHorizontalViewSize`, `ScrollVerticalViewSize` | `HorizontallyScrollable`, `VerticallyScrollable` |

Notes on the client patterns:

- **Element.** `Bounds` is optional because an element that is not on screen — scrolled out of a list, on a hidden tab — has no rectangle.
- **TextContent.** `Text` comes only from a genuine text interface of the platform, never from the accessible name (spec `textcontent-pattern`). A label or button whose platform offers no text interface has its caption in `Name` and does not announce TextContent.
- **Editable** adds the ability to change to the patterns that carry an element's value: TextContent (`Text`), StatefulValue (`Value`), Toggleable (`ToggleState`), or the chosen option of a combo box. Which value is meant follows from those patterns: a text field is TextContent and Editable, a spin box StatefulValue and Editable. The pattern stays announced while the element is read-only; `IsReadOnly` says whether the value can be changed right now (§6.4, rule 5). Text is always entered with the keyboard (spec `text-input-policy`, whose marker rule follows this definition). `MaxLength` applies to text.
- **Read-only is not disabled.** A read-only combo box can still take the focus, be opened and show its options; only its value cannot be changed. A disabled one (`IsEnabled` = false, Element) takes no focus and cannot be opened. The two states are independent attributes.
- **Toggleable.** A radio button is both Toggleable and Selectable: it is switched on by a click, and it is the selected option of its group.
- **SelectionProvider.** `IsSelectionRequired` says that at least one item must stay selected, so the selection cannot be cleared — as in a tab list, where one tab is always active. Not every platform reports it; when it is absent, nothing is known about it. The container carries no list of its selected items. Which items are selected is found through the items themselves — `./*[@IsSelected = true()]`, or any other criterion a query adds — which is what a list would be built from anyway.
- **Expandable** is announced by the kind of element — a tree item, a combo box, a menu item with a submenu, a collapsible group — and stays announced when there is nothing to expand at the moment (§6.4, rule 5). A tree item without children announces Expandable with `IsExpanded` = false and `CanExpand` = false; once it has children, `CanExpand` becomes true or is left out. An element that shows part of its children counts as expanded.

> **Open questions (§6.7):** Q6 Application and Desktop; Q7 table patterns; Q8 catalog entries without attributes.

#### Examples

What an element announces depends on its platform; these combinations are typical:

- **Check box:** Element, ActivationTarget, Focusable, Toggleable.
- **List item:** Element, ActivationTarget, Selectable.
- **Text field:** Element, ActivationTarget, Focusable, TextContent, Editable.
- **Spin box:** Element, ActivationTarget, Focusable, StatefulValue, Editable.
- **Slider:** Element, ActivationTarget, Focusable, StatefulValue.
- **Top-level window:** Element, WindowState, Focusable, Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable, Responsive.

### 6.7 Open questions (draft)

These points are not decided. Each answer lands in the sections above, and this list goes away.

- **Q6 — Application and Desktop.** Every process attribute is optional (spec `application-process-attributes`), so Application has no required attribute. Are Application and Desktop patterns, or kinds of node described in §5?
- **Q7 — Tables.** A Table pattern (`RowCount`, `ColumnCount`) and a TableItem pattern (`RowIndex`, `ColumnIndex`, `RowSpan`, `ColumnSpan`), plus the link between a cell and its column header. The providers already read these values under `native:` (§10). Do indexes count from 1, like XPath positions, or from 0, like the platforms?
- **Q8 — Catalog entries without attributes.** The previous catalog also listed Clearable (clearing is a keyboard action on an editable text), ItemContainer (client side), Highlightable (a device, §8.4), Annotatable (no cross-platform source) and DialogSurface (no platform reports a dialog result), plus the attributes `IsActivationEnabled` and `DefaultAccelerator`, which no provider publishes. They have no required attribute, so they are left out of §6.6. Drop them for good, or keep them as ideas?
- **Q9 — Changes this draft makes to the previous catalog.** `Bounds` moves from required to optional in Element (an element out of view has no rectangle); WindowState joins the catalog as a client pattern (until now only the Python side knew it); `IsFocused` moves from a client pattern Focusable to the runtime pattern Focusable.
- **Further candidates.** Not part of the catalog yet: `Orientation`, `SmallChange` and `LargeChange` for StatefulValue; `IsPassword` and `IsMultiLine` for editable text; a busy state; required and invalid form fields; links; the keyboard shortcut of a menu item.

## 7. Provider Infrastructure

### 7.1 Providers and their factories

A **provider** is the piece that actually knows how to read *one* accessibility technology — UIA on Windows, AT-SPI2 on Linux, the mock tree in tests. The runtime never talks to UIA or AT-SPI directly; it only ever talks to providers. That is exactly what keeps the rest of the system platform-agnostic.

Each provider carries a **descriptor** — a small record of who it is: an id, a display name, which technology it speaks (`TechnologyId`), whether it is a real "native" provider or an external one (`ProviderKind`), and which change events it can emit (its `event_capabilities`, see §5.8). A **factory** creates the provider on demand. Creation can fail — a technology may be unavailable on the running system — and the runtime is prepared for that; nothing extra is handed to the factory at creation time.

Once created, a provider has three jobs:

- **Hand over children.** Asked for the children of a parent node, it returns them lazily — a stream the runtime pulls from as it descends, rather than a fully built list. The runtime stitches these results together under the desktop node.
- **Announce changes.** A provider can subscribe the runtime to its change events through an event listener (the default does nothing) — this is what feeds the event pipeline in §7.2. Providers that cannot emit events simply don't; what the runtime then re-reads depends on how a query is evaluated (§9.3).
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
- Pass attribute values through as reported (§5.7): never trim or rewrite a string, list `""` for a pattern or native attribute that has it, and parse a string-transported number without trimming it first.
- Emit `Id` only when non-empty, and never on application nodes. `id()` answers none rather than `""`, and always the same value as `control:Id`; the listing and the lookup by name agree (§5.5, checked by the testkit's `verify_id`).
- Emit `Description` only when non-empty, sourced strictly from the platform's accessible-description property (no HelpText/tooltip fallback; see §5.6).
- Set `parent` references correctly in children iterators, and keep the parent of every node a child iterator lists alive with a strong reference from child to parent, so that a node the runtime hands out keeps its chain of ancestors while it is held (§9.3). Never make a node hold its children, and keep the runtime's desktop only as a `Weak`: a top-level node reaches the desktop only while the runtime lives. The strong link belongs in the child iterators, not in the constructors, which also serve `get_nodes` with the desktop as parent. A provider that owns its whole tree, such as the mock provider, meets this already. The testkit checks `verify_children_keep_parent` and `verify_subtree_released` test both halves of the rule.
- Announce patterns by the rules of §6.4: a node that announces a runtime pattern hands out its instance, a node that announces a client pattern carries the pattern's required attributes, a client pattern never has an instance, and pattern attributes live in `control:`.
- Normalize roles to PascalCase; preserve originals under `native:*`.
- Filter out own-process windows/overlays from the UI tree.
- Implement `shutdown()` for resource cleanup.
- Implement `invalidate()` to discard cached data.
- Name attributes and patterns through the constants in `crates/core/src/ui/attributes.rs` and `pattern_names`. The contract testkit (`platynui_core::ui::contract::testkit`) checks a node against the patterns a test names (`verify_node`).
- Keyboard device: key naming case-insensitive lookup, `known_key_names()` stable-sortable, `UnsupportedKey` errors informative.
- `ActivationTarget` provides `ActivationPoint` (native API preferred, bounds center as fallback).

#### Platform-Specific Checklist: Windows (UIA)

- Bounds from `BoundingRectangle` in desktop coordinates (Per-Monitor-V2 DPI active).
- ActivationPoint via `GetClickablePoint()` or center of bounds as fallback.
- `control:Text` (TextContent) priority: `TextPattern.DocumentRange.GetText` → `ValuePattern.Value`; never the accessible name. Absent when the element supports neither pattern.
- `control:Description` from `FullDescription` (`UIA_FullDescriptionPropertyId`) only — never `HelpText` or `LegacyIAccessible.Description`. Win10 1703+ property; absent when empty/unsupported. The hardcoded `attribute()` fast-path must agree with the `attributes()` iterator.
- Window capability patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable) via `WindowPattern`/`TransformPattern`; `ResponsivePattern::accepts_user_input()` via `IsEnabled && IsInView` + `WaitForInputIdle`.
- Application node: process attributes (`control:ProcessId` for a positive pid; `app:ProcessName`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime`, `Architecture`), each listed only when it was read for the node's recorded process; `control:Name` is the process name, and the node carries no `control:Id`.
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
- `control:Description` from `Accessible.Description` only — never `Accessible.HelpText` (which stays under `native:`). Emitted only when non-empty.
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
- Spec-driven `control:Description`: emitted (and returned from `description()`) when the mock tree spec sets a description, omitted otherwise. Deliberately partial — not authoritative for platform mapping semantics.
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
- **PlatynUI compositor (Wayland)**: matches the node's process id, window title and size against the windows the compositor reports over its control socket — the mapped ones and the minimized ones, so a minimized window can still be found and brought back

**Activation never changes a window's size.** Activating a window brings it to the foreground and gives it the keyboard focus. If the window is minimized, it comes back in the state it was minimized from: a window that was maximized before it was minimized comes back maximized. A window that is not minimized keeps whether it is maximized, and activation never moves or resizes it. Returning a window to its normal size is a separate operation, *restore*, and is never a side effect of activation.

This matters because activation is rarely an explicit step. The runtime's *bring to front* activates the top-level window of an element, and the Robot Framework library does that before every pointer and keyboard action while `auto_activate` is on. If activation un-maximized windows, simply clicking into a maximized application would resize it, and the click itself could land on geometry that was about to change. Each backend meets the guarantee with the platform's own "bring back from minimized" mechanism: Win32 restores an iconic window before raising it, X11 maps an iconified client window as ICCCM prescribes before requesting activation, and the PlatynUI compositor takes a minimized window out of its minimized list at its old position without touching its maximized state.

**Reading a window's state.** The WindowManager also answers how a window is currently shown. The answer names exactly one visual state — *normal*, *minimized* or *maximized* — plus whether the window is kept above other windows. The three visual states are exclusive on purpose: a window that was maximized and then minimized reports *minimized*, even though the platform may still remember that it will come back maximized. A window manager without a notion of "always on top" reports every window as not kept on top. A window manager that cannot read window state at all reports the query as unavailable rather than guessing.

Two kinds of consumers use this answer. Providers whose toolkit accessibility API does not carry window state — AT-SPI and the Java Access Bridge — derive the `IsMinimized`, `IsMaximized` and `IsTopmost` attributes of top-level windows from it, the same way they already derive `IsActive` from the window manager. Providers that do get the state from their toolkit, such as UIA (`WindowPattern`) or the in-JVM Java agent, keep using that. And backends use it internally, for example X11 activation, which only needs to bring a window back when it is actually minimized.

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
2. **Compiler** — rewrites that syntax tree into an optimized intermediate form. This is where the engine does its thinking ahead of time: it pre-computes literal values, tells predicates that count positions from those that do not, turns common `//` searches into a single descendant step, folds filter predicates into the step before them where that keeps their meaning, and specializes the individual axis steps so that evaluation later does less work (§9.2).
3. **Engine** — walks the optimized form against the tree, carrying two kinds of context as it goes. The *dynamic* context is what changes during evaluation (the current node, the position in a sequence); the *static* context is fixed up front (such as the namespace prefixes described in §9.4).
4. **Model** — the abstraction the engine evaluates *against*. Rather than requiring the whole tree to exist in memory, the model is a trait that any backend can implement to expose its own tree lazily.

That model trait is the key seam. It describes a node in the **XDM** (XQuery and XPath Data Model) sense — a typed, navigable tree node — without committing to how the tree is stored or where it comes from. An implementation answers what *kind* of node it is, its name, its typed and string values, and lets you walk to its parent, children, attributes, and namespaces. Crucially, the children, attributes, and namespaces are produced lazily, on demand, as the engine descends — so evaluating a query against a huge desktop tree never forces the whole tree into existence. The trait is also designed so each backend can return its own concrete iterator types rather than boxed ones, keeping traversal cheap.

*The exact trait definition lives in `crates/xpath/`. The code is the source of truth for signatures; this section explains what the layers are for.*

### 9.2 Document Order & Streaming

XPath 2.0 requires a path expression to return its nodes in **document order** — the order in which you meet them reading the tree from the top, each element before its children and its children before its next sibling — and **each node once**. The engine meets that requirement while it **streams**: it hands out each result as soon as it is known instead of computing the whole result first, so a query can deliver its first match long before it has walked the tree.

**Order by construction.** Most steps produce document order simply by the way they walk, so nothing has to be sorted afterwards:

- `self`, `attribute` and `child` steps keep the order of their input. When the context nodes of a child step lie inside one another — in `//Pane/Button`, a pane nested in another pane — the step merges their children as it goes: a button of the inner pane that lies before the outer pane's next button comes out first.
- `descendant`, `descendant-or-self` and `following` first drop the context nodes whose results another context node already covers — for `descendant`, a context node inside another one — and then walk from the rest.
- A reverse axis from a single context node, such as `ancestor::Window` from one button, walks nearest first; the engine reverses its output. `preceding` keeps only the last of its context nodes, because that node's preceding nodes include every other's.

Only where a step cannot guarantee the order does the engine **normalize**: it collects the step's output, removes duplicates and sorts it into document order. That happens for `//Button/..`, the parents of many buttons, for a path that starts from a variable or a sequence, such as `($dialog, $main)//Button`, for a path step that is an arbitrary expression, and for `union`, `intersect` and `except`. A step that normalizes has to read all of its input before it can hand out its first result, and it checks the cancel flag (§9.4) while it reads.

**Each node once, by identity.** Two nodes are the same when the model says they are equal — for the runtime, the same runtime id (§5.4) — not when they were reached the same way. Where a walk may meet one node twice, the engine removes the second occurrence as the results stream by. To look nodes up quickly, it asks each node for an identity hint, a number that equal nodes share, and compares only nodes with the same hint; a model that offers none is searched linearly.

**A total order.** To sort, the engine must be able to place any two nodes. A model can give its nodes document-order keys, and then the keys decide. Otherwise the engine derives a node's place from the lists of children on its path from the root, and reads each list at most once per sort. Comparing two nodes needs only the lists below their deepest shared ancestor. An attribute comes right after its element, before the element's children; attributes of one element are ordered by name, so their order is stable, and the engine never reads an element's attributes to place them. A node that its parent's list does not contain goes after the listed siblings, and nodes of different trees keep the order in which their trees first appeared. The order is therefore defined for every pair of nodes, and a sort cannot fail on a model whose answers do not fit together.

**Positions count per context node.** A positional predicate — a number such as `[2]`, or anything that reads `position()` or `last()` — counts within its own step, separately for every context node. `//Button[2]` is every button that is the second button child of its parent, while `(//Button)[2]` is the second button of the whole tree. `.//Pane/Button[last()]` is the last button of every pane. A predicate that cannot count positions, such as `[@Name='OK']`, selects the same nodes whether it runs per context node or over all of them, so the engine is free to run it over the merged stream and to fold it into the step itself.

**Rewrites into one descendant step.** Two common shapes become a single `descendant` step when the query is compiled, because that walk is cheaper and streams in document order on its own:

- `//Button[@Name='OK']` and `.//Button[@Name='OK']` become `descendant::Button[@Name='OK']`;
- the union idiom `.//(Frame|Window)[@Name='Main']` becomes `descendant::*[self::Frame or self::Window][@Name='Main']`.

The rewrite applies only when no predicate counts positions: `//Button[1]` is the first button child of every parent, and one descendant step would select only the first button of all. After the rewrite, a filter on the whole result, such as `(//Button)[@Name='OK']`, folds into the step as well.

**The first match is cheap.** A caller that needs only the first result, such as `Runtime::evaluate_single`, stops the stream after one item, so the first item has to come out before the engine has read the rest of the tree. The child steps see to that. In `//Pane/Button[1]`, the first button of the first pane may be handed out only once no pane still to come could hold a button that lies before it. So the step asks the search that finds the panes for "the next pane, if it lies before this button", and the search can answer without reading past the button. A first match therefore reads only the part of the tree that lies before it — unless a step on the way has to normalize, because a sort needs its whole input.

### 9.3 XDM Cache

Building the node model is not free — it can mean reading the live UI tree from the platform — so the engine can keep it between evaluations in an **`XdmCache`**. A cache holds one XDM tree, keyed by the runtime id of the context node it was built from; an evaluation from another context node replaces it. It is `Clone + Send + Sync`, so several threads can share one cache.

There are three ways to evaluate:

- **Without a cache** (`Runtime::evaluate`, `evaluate_iter`, `evaluate_iter_owned`, `evaluate_single`): each evaluation builds a new tree, so it sees the UI as the providers report it at that moment.
- **With a cache of your own**, created with `Runtime::create_cache()` and passed to `evaluate_cached()`, `evaluate_iter_cached()`, `evaluate_iter_owned_cached()` or `evaluate_single_cached()`. `platynui-cli watch` works this way.
- **With the runtime's shared cache** (`evaluate_runtime_cached()`, `evaluate_single_runtime_cached()`, `evaluate_iter_owned_runtime_cached()`). The runtime creates this cache in its constructor, and `Runtime::clear_cache()` empties it. Every evaluation of the Python binding goes through it: `Runtime.evaluate`, `evaluate_single` and `evaluate_iter`.

**The snapshot model.** A cached tree is a snapshot of the UI. It is built while a query runs, and it does not change while that query runs, so a query cannot break on a tree that changes under it. The next query may reuse the snapshot and search deeper in it: what no earlier query visited is read from the providers when a query first touches it. When the caller does not find what it looks for, or cannot operate what it found, it clears the cache, and its next query reads the current UI.

A reused snapshot is **revalidated lazily**, not rebuilt:

- Before each evaluation, the cached attributes are cleared and the per-node "children already validated" flags are reset, also for the ancestors that an earlier query built upward from its context element.
- When a query first touches a cached list of children, it asks each cached child whether it is still valid (`UiNode::is_valid`, §5.1). If one is not, the whole list is read again from the provider; otherwise the list is reused as it is.
- A cached root that is no longer valid is replaced by a new tree.

So a snapshot does not return an element that has gone away, as long as that element's node type implements `is_valid`. The application nodes of UI Automation, the Java Access Bridge and the Java agent do: each answers from the pid and start time of the process it was created for, and the agent's also from its agent session. So a root pinned to `/app:Application[@ProcessId=…]` is looked up again once its application has ended. What a snapshot does not show is change that still leaves every cached node valid: a node added under a parent whose cached children are all still valid, and attribute values that a provider itself keeps from an earlier read. A query sees the UI as it was when the snapshot was taken. A caller that needs such changes clears the cache.

**A snapshot's lifetime.** A snapshot lives as long as something holds it: the cache, or a query or stream that is still running. It is released, together with every provider node that only it held, when the caller clears the cache, when a query from another context node replaces it, when a query without a cache has returned and its results are dropped, when a stream is dropped, when a query fails, and when the runtime shuts down; `Runtime::shutdown` releases the shared snapshot before it shuts down the providers. A reused snapshot does not grow: a list of children that revalidation reads again releases the wrappers it held. Releasing a large snapshot does real work, such as releasing COM elements, JAB references and D-Bus proxies, on the thread that drops it. Release never recurses per level of the tree, and it never runs while a lock of the snapshot is held.

A node that a query returns, or that `children()` or a hit test hands out, keeps its own chain of ancestors alive while it is held, but not the rest of its snapshot (the provider rule of §7.3). A held element can therefore still activate its window or walk `parent()` after its snapshot was released, and it costs no more than its ancestors.

Within a snapshot, an element links to the wrapper whose list of children holds it only weakly. If another query on the same snapshot has meanwhile read that list again, while a stream of an earlier query is still running, the link is gone and the element's parent is read from the provider instead. The result is correct but reflects the current UI, so the running stream's walk over siblings may end early. That is the same kind of effect as the revalidation of a list during a query.

PlatynUI.BareMetal follows this model. Its element resolution clears the cache after each attempt that finds nothing, `Query` clears it before it evaluates, and `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value` clear it on each attempt; `Wait Until Exists` is element resolution. The `PlatynUI.core` object model follows it too: a lookup that finds nothing discards the snapshot (`AdapterFactory.discard_snapshot()`), and a context discards it before each retry of a lookup or check and when it gives up, while a lookup that succeeds keeps it. Code that uses `platynui_native.Runtime` directly does the same: it calls `clear_cache()` when it did not find what it looks for, before it asks again.

### 9.4 Evaluation API

The central entry point is `evaluate`. You give it an XPath string, a set of options, and an *optional* context node to start from — and if you pass no context, evaluation starts at the desktop root. The options let you choose whether to use a cache or not, supply a node resolver so the engine can re-resolve a context node by its `RuntimeId` (see §5), and hand in a **cancel flag**: a shared boolean the engine checks at each XPath axis step, so a long-running query can be cooperatively interrupted from another thread. For language bindings there is also an owned-iterator form (`EvaluationStream`) that returns results without any borrowed references, plus a cancellable variant of it for interruptible streaming.

Whatever the query selects, each result comes back as an **`EvaluationItem`**, which is one of three kinds:

- a **Node**,
- an **Attribute** (its owner, its name, and its value), or
- a **Value** (a plain `UiValue`).

A few supporting pieces shape how expressions are understood:

- **Static prefixes.** The static context registers a fixed set of namespace prefixes you can use in queries: `control`, `item`, `app`, and `native`.
- **Context-dependence.** `is_context_dependent(xpath)` answers whether evaluating an expression reads its context node: a relative path, `.`, or a function that falls back to the context when an argument is left out (`name()`, `position()`, `lang('en')`). It looks into every operand, condition, binding and function argument, so `count(.//x)` is dependent and `count(//x)` is not, but not into predicates and the later steps of a path, which have their own focus. BareMetal uses it twice: `Set Root` drills into the current root for a dependent selector and starts from the desktop for an independent one, and a keyword looks its root up only for a dependent selector or expression.
- **Typed values.** Every node must be able to report a typed value, returned as XDM-conformant atomics (`xs:boolean`, `xs:integer`, `xs:double`, `xs:string`). Complex structures such as Rect, Point, and Size stay as JSON-encoded strings, but their derived components (`Bounds.X`, and the like) come back as numeric atomics.

## 10. Platform Implementations

Up to this point the architecture has been deliberately platform-neutral: nodes, providers, patterns, and the runtime all describe *what* PlatynUI does without committing to *how* any one operating system delivers it. The platform implementations are where that abstraction finally meets reality — where a "focusable" pattern becomes a concrete UIA call on Windows or an AT-SPI2 request on Linux, and where pointer and keyboard input turn into actual device events. Because each platform brings its own accessibility technology, window manager, and input plumbing, those details are large enough to live in their own documents rather than crowd the core design here. Start with the file for the platform you are working on:

- **Windows** (UIA, Win32 devices, WindowManager): [`dev-docs/platform-windows.md`](platform-windows.md)
- **Linux** (X11 devices, AT-SPI2, EWMH WindowManager): [`dev-docs/platform-linux.md`](platform-linux.md)
- **Linux Wayland** (input, screenshots, and window management per compositor): [`dev-docs/platform-linux-wayland.md`](platform-linux-wayland.md)
- **Java UI toolkits** (detection and coverage per toolkit and platform, the in-JVM agent): [`dev-docs/java-toolkits.md`](java-toolkits.md)

### Platform support matrix

Which backend implements each capability on each platform. The README carries a plain-language version of this table for users; when support changes, update both.

| Component | Windows | Linux X11 | Linux Wayland | PlatynUI compositor | macOS | Mock |
|-----------|---------|-----------|---------------|---------------------|-------|------|
| UI tree provider | ✅ UIA | ✅ AT-SPI2 | ✅ AT-SPI2, window-relative coordinates | ✅ AT-SPI2 + compositor window geometry | ❌ AX stub | ✅ |
| Java provider backends | ⚠️ Swing/AWT: in-JVM agent (opt-in, Web Start included) + JAB (Java Access Bridge) fallback | ❌ planned in-JVM agent | ❌ planned in-JVM agent | ❌ planned in-JVM agent | — (JDK implements AX natively) | — |
| Pointer | ✅ SendInput | ✅ XTest | ⚠️ portal (Mutter/KWin) / virtual-input (wlroots) / EIS | ✅ EIS | ❌ stub | ✅ |
| Keyboard | ✅ SendInput | ✅ XTest | ⚠️ portal (Mutter/KWin) / virtual-input (wlroots) / EIS | ✅ EIS | ❌ stub | ✅ |
| Desktop info | ✅ Win32 | ✅ XRandR/root geometry | ⚠️ `wl_output` + Mutter/KWin D-Bus enrichment | ✅ `wl_output` | ❌ stub | ✅ |
| Screenshot | ✅ GDI | ✅ XGetImage | ❌ planned (portal / screencopy) | ✅ compositor capture | ❌ stub | ✅ |
| Highlight | ✅ layered window | ✅ override-redirect windows | ❌ | ✅ compositor overlay | ❌ stub | ✅ |
| Window management | ✅ Win32 | ⚠️ partial EWMH | ❌ | ✅ | ❌ stub | ✅ |
| Inspector | ✅ | ✅ | ⚠️ tree and queries only | ✅ | ❌ | ✅ with mock feature |
| Inspector live mouse picker | ✅ UIA | ✅ AT-SPI | ❌ no live cursor position | ✅ | ❌ AX stub | — |

- **Linux** chooses between X11 and Wayland at runtime (*Linux Session Mediator*, §3). X11 is the most complete Linux path. On generic Wayland compositors, capabilities that need the compositor's cooperation — global coordinates, screenshots, highlighting, window management, live picking — are not available yet; portal- and protocol-based backends are planned ([`platform-linux-wayland.md`](platform-linux-wayland.md)).
- **PlatynUI compositor** is [`apps/wayland-compositor`](../apps/wayland-compositor/README.md), the project's own Wayland test compositor. It exposes input injection (EIS), window geometry and management, screenshots, highlighting, and modifier state over a control socket, optionally hosts X11 applications through XWayland, and backs the Linux acceptance lanes (windowed or headless). Its usage, configuration, and control protocol are documented in [`apps/wayland-compositor/docs/`](../apps/wayland-compositor/docs/).
- **Java on Linux:** `java-atk-wrapper` is deliberately not part of the strategy — it is fragile and requires changing the target application's launch. Swing/AWT and JavaFX are planned through the in-JVM agent that serves Swing/AWT on Windows; SWT is covered by the native providers on every platform ([`java-toolkits.md`](java-toolkits.md)).
- **PID namespaces:** PlatynUI can also run beside the application in a PID namespace of its own, as a container sidecar does ([`platform-linux.md`](platform-linux.md), *Process Identity and PID Namespaces*).
- **Mock** is the in-memory platform and provider for tests ([`testing-strategy.md`](testing-strategy.md) §4); it stays out of user-facing docs.

### Patterns in the providers

§6 defines the patterns without naming a technology. This section is the snapshot of how each provider fulfils them: where it reads an attribute, and what it calls for a runtime action. It covers the providers that exist and the ones that are planned, and describes the mapping each of them is meant to follow; the code is the reference for what a provider does today. A dash means the technology has no source for that attribute.

#### Roles

Roles keep each technology's own vocabulary, normalized to PascalCase (§5.7); the platform's original term stays in `native:Role`. The same kind of element can therefore carry different roles on different technologies — a single-line text field is an `Edit` under UI Automation and an `Entry` under AT-SPI2. Within one technology the role can also depend on the toolkit: AT-SPI2 knows both a frame and a window role, and whether a toolkit reports its top-level windows as `Frame` or as `Window` is the toolkit's choice. Items inside containers may sit in the `item:` namespace (§5.2). The table shows typical correspondences, not a guarantee:

| Kind of element | UI Automation | AT-SPI2 | macOS AX (planned, source role) |
|---|---|---|---|
| Button | Button | Button | AXButton |
| Check box | CheckBox | CheckBox | AXCheckBox |
| Radio button | RadioButton | RadioButton | AXRadioButton |
| Combo box | ComboBox | ComboBox | AXComboBox / AXPopUpButton |
| Single-line text field | Edit | Entry | AXTextField |
| Multi-line text | Edit / Document | Text | AXTextArea |
| List / list item | List / ListItem | List / ListItem | AXList / AXStaticText |
| Menu / menu item | Menu / MenuItem | Menu / MenuItem | AXMenu / AXMenuItem |
| Tab list / tab | Tab / TabItem | Tab / TabItem | AXTabGroup / AXRadioButton |
| Tree / tree item | Tree / TreeItem | Tree, TreeTable / TreeItem, TableRow | AXOutline / AXRow |
| Table / row / cell | DataGrid, Table / DataItem | Table / TableRow / TableCell | AXTable / AXRow / AXCell |
| Slider / progress bar | Slider / ProgressBar | Slider / ProgressBar | AXSlider / AXProgressIndicator |
| Top-level window | Window | Frame or Window, depending on the toolkit | AXWindow |
| Dialog | Window | Dialog, or Frame / Window, depending on the toolkit | AXSheet / AXDialog |

The Java backends map the `javax.accessibility` roles onto the AT-SPI2 column wherever the two vocabularies coincide, so a Swing application answers the same selectors on Windows and Linux.

#### Windows: UI Automation

UI Automation offers each capability as a pattern of its own, and a property per pattern says whether an element supports it (`IsTogglePatternAvailable`, `IsSelectionItemPatternAvailable`, …). Those properties decide what a node announces.

| Pattern | Attribute or action | UI Automation source |
|---|---|---|
| common | Name, Id, Description | CurrentName; AutomationId; FullDescription |
| Focusable | focus; IsFocused | SetFocus; HasKeyboardFocus |
| Activatable | activate | restore the window if minimized, then bring it to the foreground |
| Minimizable, Maximizable, Restorable | minimize, maximize, restore; IsMinimized, IsMaximized; CanMinimize, CanMaximize | WindowPattern.SetWindowVisualState; WindowVisualState; CanMinimize, CanMaximize |
| Closeable | close | WindowPattern.Close |
| Movable, Resizable | move, resize; CanMove, CanResize | TransformPattern.Move, Resize; CanMove, CanResize |
| Responsive | accepts input? | IsEnabled and in view, plus WaitForInputIdle |
| Element | Bounds; IsVisible; IsEnabled; IsInView | BoundingRectangle; not IsOffscreen and a non-empty rectangle; IsEnabled; not IsOffscreen |
| ActivationTarget | ActivationPoint; ActivationArea | GetClickablePoint, else the centre of BoundingRectangle; BoundingRectangle |
| TextContent | Text | TextPattern document range, else ValuePattern.Value |
| Editable | IsReadOnly; MaxLength | ValuePattern.IsReadOnly, for a range RangeValuePattern.IsReadOnly; — |
| Toggleable | ToggleState | TogglePattern.ToggleState; for a radio button, which offers SelectionItem instead, SelectionItemPattern.IsSelected |
| Selectable | IsSelected | SelectionItemPattern.IsSelected |
| SelectionProvider | CanSelectMultiple; IsSelectionRequired | SelectionPattern.CanSelectMultiple; SelectionPattern.IsSelectionRequired |
| Expandable | IsExpanded; CanExpand | ExpandCollapseState Expanded or PartiallyExpanded; ExpandCollapseState other than LeafNode |
| StatefulValue | Value, MinValue, MaxValue | RangeValuePattern.Value, Minimum, Maximum |
| WindowState | IsActive; IsTopmost, IsModal | the window is the foreground window; WindowPattern.IsTopmost, IsModal |
| TextSelection | SelectedText, SelectionStart, SelectionEnd | TextPattern.GetSelection |
| Scrollable | scroll percentages, view sizes, scrollability | ScrollPattern |

ValuePattern.Value is a string, so it feeds `Text`, never the numeric `Value`.

#### Linux: AT-SPI2

AT-SPI2 describes most capabilities as states of an element (checked, selected, expanded, …) and some as interfaces (Text, Selection, Value). Whether an element *can* have a state is a state of its own — checkable, selectable, expandable — and decides what a node announces. Window actions and window state come from the platform's window manager device (§8.5): EWMH on X11, the control socket of the PlatynUI compositor on Wayland.

| Pattern | Attribute or action | AT-SPI2 source |
|---|---|---|
| common | Name, Id, Description | Accessible.Name alone (the object attributes stay readable in `native:Accessible.Attributes`); Accessible.AccessibleId, else the object attribute `accessible-id`, `accessible_id` or `id`; Accessible.Description |
| Focusable | focus; IsFocused | Component.GrabFocus; state FOCUSED |
| window runtime patterns | the window actions; IsMinimized, IsMaximized | window manager |
| Responsive | accepts input? | states SENSITIVE and SHOWING |
| Element | Bounds; IsVisible, IsInView; IsEnabled | Component extents, the window manager for top-level windows; state VISIBLE or SHOWING; state ENABLED or SENSITIVE |
| ActivationTarget | ActivationPoint; ActivationArea | centre of the Component extents; Component extents |
| TextContent | Text | Text.GetText(0, -1) |
| Editable | IsReadOnly; MaxLength | state READ_ONLY, for text also EDITABLE; — |
| Toggleable | ToggleState | states CHECKED and INDETERMINATE (PRESSED for a toggle button where a toolkit reports it so); checkable by the state CHECKABLE or by a check, radio or toggle role |
| Selectable | IsSelected | state SELECTED, for a radio button CHECKED; selectable by the state SELECTABLE |
| SelectionProvider | CanSelectMultiple; IsSelectionRequired | state MULTISELECTABLE on an element with the Selection interface; — |
| Expandable | IsExpanded; CanExpand | state EXPANDED; state EXPANDABLE — a tree item without it is announced by its role, with `CanExpand` = false |
| StatefulValue | Value, MinValue, MaxValue | Value.CurrentValue, MinimumValue, MaximumValue |
| WindowState | IsActive, IsTopmost; IsModal | window manager; state MODAL |
| TextSelection | SelectedText, SelectionStart, SelectionEnd | Text.GetSelection |
| Scrollable | — | no scroll interface; derivable from the scroll bars' Value |

#### Java: Access Bridge and in-JVM agent

The Java provider serves Swing and AWT on Windows through two backends: the Java Access Bridge, and the agent PlatynUI loads into the JVM. Both read the `javax.accessibility` model, in which capabilities are states (checked, selected, expanded, …) and interfaces (AccessibleText, AccessibleSelection, AccessibleValue); the agent can also read the toolkit's own model, for example the selection of a table. Planned: the agent on Linux, and adapters for JavaFX and SWT ([`java-toolkits.md`](java-toolkits.md)). Java windows are native windows, so the window actions and window state come from the platform's window manager.

| Pattern | Attribute or action | Access Bridge / agent source |
|---|---|---|
| common | Name, Id, Description | the accessible name the bridge reports / Swing's `getAccessibleName()` for every element, with a window's title in `native:WindowTitle` and a table cell's model value in `native:TableCell.ModelValue`; none / `Component.getName()` when set with `setName` (cells, rows and other accessibility-only children have none); the accessible description the bridge reports / Swing's `getAccessibleDescription()`, which Swing derives from the tool tip text when nothing else is set |
| Focusable | focus; IsFocused | requestFocus; state `focused` / `Component.isFocusOwner` |
| window runtime patterns | the window actions; IsMinimized, IsMaximized | window manager |
| Element | Bounds; IsVisible; IsEnabled; IsInView | context bounds / component location and size; `visible` and `showing` / `Component.isVisible`; `enabled` / `Component.isEnabled`; `showing` / `Component.isShowing` |
| ActivationTarget | ActivationPoint | centre of the bounds (the Access Bridge has no clickable-point call) |
| TextContent | Text | AccessibleText (the Access Bridge reads it in chunks) / the component's text |
| Editable | IsReadOnly; MaxLength | `editable` / `JTextComponent.isEditable`; — |
| Toggleable | ToggleState | states `checked` and `indeterminate` of check boxes, radio buttons and toggle buttons |
| Selectable | IsSelected | state `selected`, for a radio button `checked`; agent table rows: `JTable.isRowSelected` |
| SelectionProvider | CanSelectMultiple; IsSelectionRequired | `multiselectable` on an element with AccessibleSelection / the selection mode of the list, table or tree; — |
| Expandable | IsExpanded; CanExpand | state `expanded`; state `expandable` — a tree node without it (a leaf) is announced by its role, with `CanExpand` = false |
| StatefulValue | Value, MinValue, MaxValue | AccessibleValue current, minimum and maximum value |
| WindowState | IsActive, IsTopmost; IsModal | window manager / `Window.isActive`, `isAlwaysOnTop`; state `modal` / `Dialog.isModal` |
| TextSelection | SelectedText, SelectionStart, SelectionEnd | AccessibleText selection |
| Scrollable | — | derivable from the scroll bars' AccessibleValue |

#### macOS: Accessibility (AX), planned

The AX provider is a stub. Its mapping is planned as follows:

| Pattern | Attribute or action | AX source |
|---|---|---|
| common | Name, Id, Description | AXTitle or AXDescription, depending on the role; AXIdentifier; open (AXHelp is help text, not a description) |
| Focusable | focus; IsFocused | set AXFocused; AXFocused |
| Activatable | activate | AXRaise and AXFocused |
| Minimizable, Restorable | minimize, restore; IsMinimized | AXMinimized |
| Maximizable | maximize; IsMaximized | AXFullScreen (an approximation) |
| Closeable | close | the AXClose action |
| Movable, Resizable | move, resize | AXPosition, AXSize |
| Responsive | accepts input? | AXEnabled |
| Element | Bounds; IsEnabled | AXFrame; AXEnabled |
| ActivationTarget | ActivationPoint; ActivationArea | centre of AXFrame; AXFrame |
| TextContent | Text | AXValue |
| Editable | IsReadOnly | whether the value attribute (AXValue) is settable |
| Toggleable | ToggleState | AXValue (0, 1, 2) |
| Selectable | IsSelected | AXSelected |
| SelectionProvider | CanSelectMultiple; IsSelectionRequired | AXAllowsMultipleSelection; — |
| Expandable | IsExpanded; CanExpand | AXExpanded / AXDisclosing; whether the row has children |
| StatefulValue | Value, MinValue, MaxValue | AXValue, AXMinValue, AXMaxValue |
| WindowState | IsActive; IsModal | AXMain; AXModal |
| TextSelection | SelectedText, SelectionStart, SelectionEnd | AXSelectedText; AXSelectedTextRange |

#### Mock

The mock provider declares patterns and attributes per node in its tree file (`crates/provider-mock/assets/mock_tree.xml`): what a node announces is what the file says. It follows the same rules as every provider, and it is deliberately partial ([`testing-strategy.md`](testing-strategy.md) §4).

#### Application nodes

Every provider on a platform reads the process attributes through the one reader of `platynui-process`, bound to the process identity the node recorded; the Java provider's JAB and agent nodes on Windows included. Formats and the presence rule: spec `application-process-attributes`. `ProcessId` is an application node's identity: application nodes carry no `Id` on any provider (§5.5).

| Attribute | Windows (UIA, JAB, Java agent) | Linux (AT-SPI2) | macOS AX |
|-----------|-----|---------|----------|
| ProcessId | CurrentProcessId; the window's or the agent session's process | D-Bus peer credentials | AXPid (via kAXPIDAttribute) |
| ProcessName | File name of ExecutablePath, trailing `.exe` removed | File name of ExecutablePath | — |
| ExecutablePath | QueryFullProcessImageNameW (Win32 form) | `sysinfo` `exe()` (/proc/PID/exe, without ` (deleted)`) | — |
| CommandLine | NtQueryInformationProcess(ProcessCommandLineInformation), verbatim | /proc/PID/cmdline, arguments joined by spaces as they are | — |
| UserName | Process token → LookupAccountSidW, `DOMAIN\user` | `sysinfo` effective UID → getpwuid_r | — |
| StartTime | Creation time the identity recorded (GetProcessTimes), to the second | `sysinfo` `start_time()` (/proc/PID/stat field 22), to the second | — |
| Architecture | GetProcessInformation(ProcessMachineTypeInfo), then IsWow64Process2 | — (Linux keeps no architecture per process) | — |

## 11. Companion Documentation

This document covers the Rust core and the cross-platform model that everything else stands on. A handful of companion docs go deeper into the parts that live at the edges of the system — where PlatynUI meets Python, the command line, the Inspector UI, and the logging infrastructure. Reach for them when your work touches one of those areas.

- **Python Bindings** — how the Rust core is exposed to Python via PyO3, how types are mapped across the boundary, and how threading is handled: [`dev-docs/python-bindings.md`](python-bindings.md)
- **CLI** — the available commands and the snapshot model the CLI works against: [`dev-docs/cli.md`](cli.md)
- **Inspector** — the architecture of the Inspector's TreeView: [`dev-docs/inspector.md`](inspector.md)
- **Logging** — the logging concept for Rust and Python: what each level means, where a failure is logged, once-per-episode reporting, the Python loggers and the level knob: [`dev-docs/logging.md`](logging.md); the checklist for code is [`.github/instructions/logging.instructions.md`](../.github/instructions/logging.instructions.md)
