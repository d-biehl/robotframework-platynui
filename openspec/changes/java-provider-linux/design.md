## Context

`unify-java-provider` created the single Java provider with a backend trait; `provider-java-swing` puts the agent behind that trait and proves it on Windows, where a Java window enters the tree because the JAB backend enumerates native top-levels itself and the platform's `JavaClassifier` answers "is this a JVM?". Neither mechanism exists on Linux. This change is the Linux bring-up of the provider — not of a toolkit adapter.

The scope is smaller than "Java on Linux" sounds, because two thirds of the problem is already solved elsewhere: **JVM discovery** is `handshake::discover()` from `java-agent-core`, which scans the per-user agent directory and is platform-neutral, and **which windows a JVM has** is something the agent knows from inside the process. What is genuinely open is only how a window's *native identity* is obtained, and that decides whether a new platform capability is needed at all.

## Goals / Non-Goals

**Goals:**

- `crates/provider-java` builds and is linked on Linux, with the JAB backend target-gated out.
- An agent-carrying JVM's windows appear as top-level nodes on X11, with working geometry and window patterns.
- The Swing fixture served through the agent on X11 — Swing on Linux without `java-atk-wrapper`.

**Non-Goals:**

- Toolkit adapters. The Swing reader is `provider-java-swing`; JavaFX and SWT are their own changes. This change makes the *provider* work on Linux; whichever adapters exist then work there.
- A Linux `JavaClassifier` (`native:IsJvm`/`JvmToolkit`/agent-presence attributes on Linux). Diagnostic value only, nothing here depends on it — see the proposal's Impact for why it is cheaper than it looks.
- Native Wayland. See decision 3.
- Changing anything about Windows.

## Decisions (proposed) and Open Questions

1. **Where the top-level's native identity comes from (OPEN — the measurement decides).** Options, cheapest first:

    - **(a) The agent reports it.** `sun.awt.X11.XBaseWindow#getWindow` (and its per-JDK forms, with the `--add-opens` matrix on 9+) yields the X11 window id in-process. This needs no new platform capability at all, and it makes `provider-java-swing` decision 5's PID+geometry fallback *Windows-only by construction* — on Linux there is no native window list to match against anyway, so the fallback has nothing to fall back to.
    - **(b) `WindowManager` gains an enumeration method**, answered on X11 from `_NET_CLIENT_LIST`/`_NET_CLIENT_LIST_STACKING` — which `platform-linux-x11` already reads internally but does not expose. Keep it in the platform layer: a provider growing its own `x11rb` client would break the rule that keeps `provider-atspi` free of windowing dependencies and working identically on X11 and Wayland.

    Option (a) is preferred and (b) is the fallback, but the choice is a measurement, not a preference — see the verification item. Do not build (b) speculatively.

2. **Routing on Linux is a single-backend case, and stays one.** There is no JAB backend to compete with, so the `Enumeration` reshape `provider-java-swing` needs for agent-vs-JAB selection is a Windows concern and must not leak here. The criterion needs no classifier: `handshake::agent_present(pid)` is public and platform-neutral, so "is there an agent in this JVM?" is answerable on Linux today without the platform bundle carrying a `JavaClassifier`.

3. **Wayland is reached through XWayland, deliberately (decided).** The shipped JDKs have no AWT Wayland backend (Project Wakefield is not in), so a Swing application on a Wayland session runs as an X11 client through XWayland — and `xwayland-satellite` covers compositors that ship none. The X11 path therefore covers Wayland for Java, and building a second path would be building for a JDK that does not exist yet. What this change owes is a verified statement of *where* the X11 window id is valid (the XWayland display, not the Wayland compositor) and the note in the docs, not a second implementation.

4. **Absence stays inert (decided, inherited).** The provider on Linux with no agent anywhere contributes no nodes and logs nothing beyond what `handshake::discover()` already costs — a directory scan that finds nothing. This is the same inert-absence rule `java-provider` already states for a missing backend; it matters more here because on Linux there is no JAB fallback whose absence would be noticed instead.

## Risks / Trade-offs

- [The measurement says (b), and the change grows a core trait change] → then it grows, and that is the right place for it: an enumeration method on `WindowManager` is useful beyond Java (the same "list native top-levels" question exists for any provider that needs to correlate with the windowing system). Scoped to X11 first; other backends may report `CapabilityUnavailable` as they already do for unimplemented window operations.
- [An agent-served window has no native representation to fall back to] → that is the premise, not a regression: without an agent the window is invisible to PlatynUI on Linux today. The consequence to state in the docs is that on Linux the agent is not an upgrade over a native path, it *is* the path.
- [Linux lane cost] → the X11 acceptance session already exists (`just test-acceptance-x11`); what is new is the Swing fixture launched inside it and the suite tagging.

## Migration Plan

Additive. On Windows nothing changes; on Linux the provider goes from absent to present, so a session that previously saw nothing for a Java application starts seeing it — which is the point. Rollback: `providers.java.enabled = false`, or reverting the link-macro arm so the crate is not registered.

## Verification items (approach decided; mechanics to confirm)

- **X11 handle reliability** (1): does `sun.awt.X11.XBaseWindow#getWindow` yield the top-level X11 window id across JDK 8 / 17 / 21, on bare X11 **and** under XWayland, and what `--add-opens` does each need? This single measurement decides between option (a) and option (b) and must run before either is built.
- **Geometry agreement** (1): the agent's reported bounds against `GetWindowRect`'s X11 equivalent for the same window, so the physical-pixel wire contract is confirmed on a second windowing system.
- **Inert absence on Linux** (4): a session with no agent anywhere comes up normally and contributes no Java nodes.
