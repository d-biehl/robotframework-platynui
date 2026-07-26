## Why

The Java provider is Windows-only — and not because of the agent. The agent, its attach transport and its delivery are cross-platform, and `java-agent-core` verified the Unix attach leg against live JVMs on Linux. What is missing is smaller and more specific: **nothing on Linux turns a JVM into a top-level node.**

- `provider-atspi` enumerates the accessibility **registry** (`REGISTRY_BUS`/`ROOT_PATH` children), not windows, so it only sees applications that registered on the a11y bus. A Swing process without `java-atk-wrapper` never registers. That absence is the *premise* of serving Swing through an agent — there is no competing representation to deduplicate — but it also means AT-SPI contributes no window node the agent backend could attach to.
- `crates/provider-java` is `cfg(windows)` in its entirety, because JAB was its only backend when `unify-java-provider` created it. On Linux the crate compiles to nothing and is not linked at all.
- `platform-linux-x11` reports `java_classifier: None`. There is no Java classification on Linux: no `native:IsJvm`, no `JvmToolkit`, no agent-presence fact, so the breadcrumb that turns an empty window into "this is a JVM, it has no agent, here is the remedy" cannot be told there.

This is deliberately **not** a Swing change. Every Java adapter needs the same foundation, and the one that needs it most is JavaFX: its whole reason to exist is that JavaFX has *no* native accessibility on Linux at all. Carrying the Linux bring-up inside `provider-java-swing` would build it once for Swing and then again for the next adapter.

## What Changes

- **`crates/provider-java` becomes portable.** The router and the agent backend build everywhere; the JAB backend stays behind a `[target.'cfg(windows)'.dependencies]` edge, taking `libloading`, `sysinfo`, `chrono` and the `windows` features with it. Plus the Linux arm of `platynui_link_os_providers!` and the Linux dependency sections of `crates/cli`, `apps/inspector`, `packages/native` and `crates/playground` — the link macro only references crates the caller declares.
- **The top-level node gets a source.** Measurement first: whether the agent can read its window's X11 id in-process across JDK 8/17/21, on bare X11 *and* under XWayland. A yes makes the agent the single source and needs no new platform capability; a no means `WindowManager` gains an enumeration method that X11 answers from `_NET_CLIENT_LIST` — kept in the platform layer, because a provider growing its own `x11rb` client would break the rule that keeps `provider-atspi` windowing-agnostic.
- **Routing is trivial here and stays that way.** There is no JAB backend on Linux, so there is nothing to arbitrate: the `Enumeration` reshape that `provider-java-swing` needs for agent-vs-JAB selection is a Windows concern by construction. The routing criterion needs no classifier either — `platynui_java_agent::handshake::agent_present(pid)` is public and platform-neutral.
- **The Linux acceptance lane**: the Swing fixture served through the agent on X11 — the first proof that Swing on Linux is reachable without `java-atk-wrapper`.

## Capabilities

### Modified Capabilities

- `java-provider`: gains Linux. The provider surfaces top-level nodes for agent-carrying JVMs on platforms whose native accessibility does not enumerate windows, sourcing the top-level from the agent instead of from the native provider — where "leave it to the platform's native provider" is not a fallback but invisibility.

## Impact

- **Modified**: `crates/provider-java` (portability), `crates/link`, the Linux dependency sections of `crates/cli` / `apps/inspector` / `packages/native` / `crates/playground`, possibly `crates/core`'s `WindowManager` trait and `platform-linux-x11` (only on the measurement's "no" branch), the acceptance lane wiring, docs.
- **Not in scope: a Linux `JavaClassifier`.** It serves a different user story — diagnosing a JVM that has *no* agent — and nothing here depends on it. Worth recording that it is cheaper than it looks: `platynui_java_agent::jvm::process_runs_jvm(pid)` already answers "is this a JVM?" on Linux from `/proc/<pid>/maps` (`libjvm.so`) and is public, and `handshake::agent_present` already answers the agent half. What is genuinely missing is the toolkit discriminator, which has no X11 equivalent of a window class. Its own change.
- **Depends on**: `provider-java-swing` (the agent backend and mapping layer this makes available on Linux) and `java-agent-core` (agent, transport, delivery, the Unix attach leg already verified there). **Unblocks**: the Linux half of `provider-java-javafx` — the only path to JavaFX on Linux — and of `provider-java-swt`.
- No BREAKING changes. Windows behavior is untouched; on Linux the provider goes from absent to present.
