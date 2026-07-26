<!-- The Linux bring-up of the Java provider, not of a toolkit adapter. Builds on
     provider-java-swing (the agent backend it makes available there) and
     java-agent-core (agent, transport, Unix attach — already verified on Linux).
     Task 1.1 is a measurement and gates section 2: do not build either branch
     of design 1 before it has an answer. -->

## 1. Decide where the native window identity comes from

- [ ] 1.1 **Measure first** (design 1): does `sun.awt.X11.XBaseWindow#getWindow` yield the top-level X11 window id across JDK 8 / 17 / 21, on bare X11 **and** under XWayland, and what `--add-opens` does each need? Record the matrix in design.md next to the JDK-internals matrix `provider-java-swing` builds for Windows
- [ ] 1.2 Settle design 1 from the measurement and write the decision down: option (a) the agent reports the id — preferred, no new platform capability; option (b) `WindowManager` gains an enumeration method answered from `_NET_CLIENT_LIST`. Do not build (b) unless 1.1 rules out (a)
- [ ] 1.3 If (a): the agent reads the X11 id, and `provider-java-swing`'s PID+geometry fallback (its design 5) is recorded as Windows-only — on Linux there is no native window list to match against. If (b): the enumeration method lives in the platform layer (`platynui_core::platform::WindowManager` + `platform-linux-x11`), never in the provider, so the `x11rb`-free provider rule holds

## 2. Make the provider portable

- [ ] 2.1 `crates/provider-java`: router and agent backend build on every platform; the JAB backend moves behind `[target.'cfg(windows)'.dependencies]`, taking `libloading`, `sysinfo`, `chrono` and the `windows` features with it. The `cfg(windows)` module gating in `lib.rs` narrows to the JAB adapter module only
- [ ] 2.2 Registration on Linux: the Linux arm of `platynui_link_os_providers!` plus the Linux dependency sections of `crates/cli`, `apps/inspector`, `packages/native`, `crates/playground` — the link macro only references crates the caller declares. Exactly one Java provider registered, as on Windows
- [ ] 2.3 Window nodes on X11: geometry and the window capability patterns delegate to the platform `WindowManager` through the native handle from section 1 (the same delegation shape the JAB backend uses via `native:NativeWindowHandle`)
- [ ] 2.4 Inert absence (design 4): no agent anywhere ⇒ no nodes, no failures, nothing beyond the cost of one `handshake::discover()` directory scan that finds nothing. On Linux there is no JAB fallback whose absence would be noticed instead, so this is the only thing standing between "no Java apps" and a confusing empty tree
- [ ] 2.5 Routing stays single-backend here (design 2): the agent-presence criterion comes from `handshake::agent_present(pid)`, which needs no `JavaClassifier`; the agent-vs-JAB `Enumeration` reshape must not leak in from `provider-java-swing`

## 3. Acceptance & verification

- [ ] 3.1 **Linux lane**: the Swing fixture launched inside the existing X11 acceptance session (`just test-acceptance-x11`), served through the agent — the first proof that Swing on Linux is reachable without `java-atk-wrapper`. Suite tagging `platform:x11`; the existing Swing suites stay `platform:windows` because they assert JAB behavior
- [ ] 3.2 Geometry agreement: the agent's reported bounds against the X11 window's real geometry for the same window, confirming the physical-pixel wire contract on a second windowing system
- [ ] 3.3 XWayland (design 3): the same fixture under a Wayland session via XWayland, confirming the X11 window id is valid on the XWayland display. Document what `xwayland-satellite` is for and that no second path is built
- [ ] 3.4 Windows regression: the full Windows acceptance lane unchanged — the portability work must not disturb the JAB path. Mock lanes green; `just check`/`test`/`build-native` green, plus `just check-windows`/`clippy-windows` from Linux
- [ ] 3.5 Docs: `dev-docs/platform-linux.md` gains the Java section (what serves Java there and why the agent is the path rather than an upgrade over one), `dev-docs/java-toolkits.md`'s Linux column updated, `README.md`'s platform-support row updated
