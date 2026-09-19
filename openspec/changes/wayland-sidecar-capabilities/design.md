# Design

## Context

See proposal.md — Why for the motivation. This section holds the current state and the constraints the approach works within. Everything marked *verified* was read in the working tree at `1b6e3ad`; everything marked *measured* comes from the podman pod `pidns-wl` runs (compositor + egui app in one container, CLI in a sibling container, no PID sharing) reported in the PR #5 investigation. Nothing here is inferred unless it says so.

**Detection today (verified):**

- `detect_compositor` (`crates/platform-linux-wayland/src/capabilities.rs:54-58`) tries `detect_via_peercred` first, then `detect_via_env`.
- `detect_via_peercred` (`:62-88`) reads `SO_PEERCRED` from the Wayland socket through `rustix::net::sockopt::socket_peercred`, then `readlink /proc/<pid>/exe`, then `classify_binary_name` (`:91-108`), whose first branch already matches `platynui`.
- `detect_via_env` (`:114-136`) matches only gnome/kde/plasma/hyprland/sway. There is no `platynui` branch, although `scripts/startcompositor.sh:193` exports `XDG_CURRENT_DESKTOP=platynui`.
- Detection runs exactly once, in `connect_and_enumerate` (`src/connection.rs:243-253`), before the registry roundtrip. The result is stored in the process-global `WaylandGlobal` (`:220-231`) and every gate reads it through `compositor_type()`.
- Gates: `window_manager::backend()` (`src/window_manager/mod.rs:99-110`), `screenshot::capture` (`src/screenshot.rs:33-36`), `highlight` (`src/highlight.rs:14-17`, `:33-35`), input-backend selection (`src/input/mod.rs:93-100`), display-config enrichment (`src/desktop/display_config.rs:49-58`).
- The control channel is `control_ipc::send_command` (`src/control_ipc.rs:10-51`): it resolves `PLATYNUI_CONTROL_SOCKET`, else `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control` (`:53-59`), connects per call, sets a 5 s read/write timeout, and requires `status == "ok"`. Its connect error already names the socket path (`:16-19`).
- The compositor's `status` (aliased by `ping`) is `build_status_response` (`apps/wayland-compositor/src/control.rs:391`, `:819-850`). It reports version, backend, uptime, socket name, XWayland state, window counts and outputs — nothing that identifies the server as a PlatynUI compositor. The protocol is documented as unversioned v0, with unknown commands answered by an error so clients can be forward-compatible (`apps/wayland-compositor/docs/ipc-protocol.md:475-480`).

**What the namespace boundary does (measured):**

- From the app container: `compositor process identified pid=1198 exe=/opt/pbin/platynui-wayland-compositor`. From the sidecar: `SO_PEERCRED failed … error=Unknown error -1000` and `pid=0`.
- The client-side call **is** undefined behaviour, even though it looks like an ordinary error. rustix's `UCred.pid` is a non-zero `Pid`, and its `getsockopt` reads the kernel's bytes into a `MaybeUninit<UCred>` and returns `value.assume_init()` (`rustix-1.1.4/src/backend/linux_raw/net/sockopt.rs:45-61`, **verified**). A kernel-reported PID of `0` violates that type's invariant; the `Err(Unknown error -1000)` we measured is an artifact of how the compiler laid out `Result<UCred, Errno>` around the non-zero niche, not a defined error path, and may differ between compiler versions and optimisation levels. The code itself gives the layout away: `-1000` is the peer's uid 1000 read as an `Errno` (`Errno(u16)`, whose `raw_os_error` negates it) — inferred from the layout, and consistent with the uid the measured pod ran under. The same un-representable value is what aborts the *compositor*, because `wayland-backend-0.3.15/src/rs/server_impl/client.rs:314` writes `socket_peercred(&self.socket).expect("getsockopt failed!?")`. One direction degrades silently, the other one crashes, and neither is safe to rely on. This change therefore stops calling `rustix::net::sockopt::socket_peercred` in `detect_via_peercred`: the foreign-compositor heuristic reads `SO_PEERCRED` through `nix::sys::socket::getsockopt` with `sockopt::PeerCredentials`, whose `UnixCredentials` wraps a plain `libc::ucred` and can therefore carry `0` — a safe call, so no `unsafe` enters the crate (maintainer decision: avoid `unsafe` where a safe wrapper exists) — treats a PID of `0` as "not visible", and the rustix accessor is added to `clippy.toml`'s `disallowed-methods` in the same commit — the entry `fix-compositor-foreign-pidns-clients` deliberately leaves to this change, because adding it earlier would fail the workspace clippy gate on this file.
- The control socket itself works across the boundary, over an absolute path — that is what makes the handshake possible at all.
- The handshake is safe to send even before `fix-compositor-foreign-pidns-clients` lands: `build_status_response` never builds a `WindowInfo`, and `window_pid` (`control.rs:1270-1278`, whose `client.get_credentials` call is at `:1277`) is reached only from `build_window_info` (`:1084`), `list_popups` (`:1203`) and the minimized-window listing (`:1236`). Measured: a foreign client that only maps a window left the compositor alive for 15 s; a single `list_windows` killed it.
- `XDG_CURRENT_DESKTOP=platynui` in the sidecar changed nothing: `compositor detected via environment ct=Unknown`.

**Constraints:**

- The identification must not require the peer process to be visible, and must not require the two sides to agree on any PID.
- It must stay a single decision at initialization: `compositor_type()` is read from many call sites and is not allowed to become a per-call probe.
- It must keep working for foreign compositors, which have no control socket and no marker, and where today's `SO_PEERCRED` path is the only signal.
- The Wayland backend stays process-global-backed this phase (`src/lib.rs:68-78`), so the handshake result lives next to the compositor type, not in per-runtime state.
- The compositor's control protocol has no version negotiation, so any compositor-side addition must be additive and inert for old clients.

## Goals / Non-Goals

**Goals:**

- One identification, decided from evidence that survives a PID-namespace boundary, with the mechanism and its inputs recorded.
- The identification question and the usability question answered by the same round trip, so "identified as ours" cannot mean "and unusable, silently".
- A pure decision function over injected inputs, so the whole table is unit-testable without a compositor, a display or a namespace.
- Every refusal caused by the identification names the capability, the compositor and — where the control channel is involved — the socket path.

**Non-Goals:**

- Making capabilities work under foreign compositors. `screenshot.rs:66-77` already documents that as later protocol work.
- Per-runtime Wayland state, new configuration keys, or any change to `runtime-session-config`.
- Fixing the compositor's peer-credential crash — that is `fix-compositor-foreign-pidns-clients`, and this change depends on it.
- Fixing the identity of *applications* (AT-SPI peers) or of *X11 windows*; those are `atspi-process-identity` and `x11-window-owner-identity`. The one AT-SPI function this change does touch is the fallback for a top-level's window-manager-backed geometry, which gains a warning and keeps its value (decision 7) — no PID, no identity.
- Own-window handling in hit-test: the compositor's skip is `fix-compositor-foreign-pidns-clients`, the window-manager rule is `x11-window-owner-identity`.

## Decisions

**1. The deciding mechanism is a control-socket handshake, with the session environment second and peer credentials last.**

Order: (a) control-socket handshake → PlatynUI; (b) `XDG_CURRENT_DESKTOP` marking a PlatynUI session → PlatynUI; (c) peer credentials plus `/proc/<pid>/exe` → whatever foreign compositor they name; (d) the existing environment heuristic for foreign desktops; (e) unrecognised.

The handshake is preferred because it asks the exact channel every gated capability uses. Under a compositor we have identified, `window_manager`, `screenshot`, `highlight`, popup geometry and modifier state all run over that socket, so an identification that succeeds is also a usability proof, and the "identified but unusable" state shrinks to the env-only path (b), where it is reported by name rather than hidden. It is PID-free by construction.

- *Rejected:* keeping `SO_PEERCRED` first and using the handshake as a fallback. That leaves the namespace-dependent answer in the deciding position for the case this change exists for, and requires reconciling two mechanisms that can disagree (a PlatynUI compositor reachable by socket but reported by `/proc` as something else, e.g. through a wrapper binary).
- *Rejected:* dropping `SO_PEERCRED` entirely. It is still the only way to tell Mutter from KWin from sway when no desktop variable is set, and its failure there is benign.
- Consequence to accept: a PlatynUI compositor started with `--no-control-socket` (`apps/wayland-compositor/src/lib.rs:132-134`) can no longer be identified by the handshake. It is still identified by (b) when the session was started through `scripts/startcompositor.sh`, and by (c) when the process is visible — i.e. never worse than today. A task verifies this combination.

**2. The compositor names itself in the `status` response.**

Identification needs a marker, not a shape: any JSON-lines server answering `{"status":"ok"}` would otherwise pass. The marker is added to the existing `status` response rather than to a new command, because `status` is already the liveness probe, `ping` already aliases it, and one round trip then answers both questions. The addition is additive and the protocol is documented as ignoring unknown fields.

- *Rejected:* a dedicated `identify` command — a second round trip and a second command for information the status response should carry anyway.
- *Rejected:* inferring identity from the field set (`backend` + `socket` + `outputs`). It is a fingerprint, not an identity, and it silently breaks whenever the response changes.

**3. Correlating the control channel with the Wayland connection is advisory, not a veto.**

The handshake proves *a* PlatynUI compositor answers on the configured socket path, not that it is the compositor owning our Wayland display. The one correlating datum available is the compositor's own socket name (`control.rs:844`, `state.socket_name`) against the client's `WAYLAND_DISPLAY`. The backend compares them when both are comparable and logs a mismatch, but a mismatch does not veto the identification: measured, both containers saw the same absolute path `/run/pod/xdg/wl-0`, but in a Kubernetes sidecar the two containers may well mount that socket at different paths, and vetoing would break the deployment this change exists for. **The mismatching case is therefore unmeasured** — every measurement to date had the paths agree, so what a veto would cost is reasoned, not observed. The residual false positive needs a session deliberately pointed at another compositor's control socket, in which case every capability call would reach that compositor anyway — the misidentification would be a symptom of the misconfiguration, not its cause. This is the weakness that decision 9's alternative removes.

**4. The handshake has its own short timeout, separate from the capability timeout.**

It runs on the initialization path, so a socket that accepts a connection and never answers must not stall startup for the 5 s `control_ipc` grants capability calls. A short bound (1 s, the same order as the D-Bus timeout `atspi-process-identity` uses for its identity question) keeps a dead socket from delaying every runtime construction. A timeout is treated as "the handshake did not decide", never as "not PlatynUI" — the decision falls through to (b)/(c) and the reason is recorded.

**5. The decision is a pure function; only its inputs do I/O.**

`decide_compositor(handshake_outcome, desktop_env, peer_process_name) -> (CompositorType, Basis)` mirrors the `decide_mode` shape `atspi-process-identity` uses, so the whole table — including the failure rows (no socket path, connection refused, no marker, timeout, marker on a foreign desktop) — is covered by unit tests that need no compositor, no display and no namespace. The socket connect, the environment read and the peer-credential read stay at the edges, where the namespace test exercises them.

**6. `highlight` is the only fail-open gate inside this crate; `clear` stays permissive.**

Verified: `screenshot.rs:33-36` and `window_manager/mod.rs:99-110` already return `CapabilityUnavailable` for an unsupported or undetected compositor, so the "fail open" family is one site (`highlight.rs:14-17`), not three. `highlight()` with rectangles starts returning `CapabilityUnavailable`; `clear()` keeps returning `Ok(())` because its postcondition — no highlight is shown — holds even when this backend never showed one, and because `highlight()` delegates an empty request to `clear()` (`highlight.rs:10-12`). `crates/cli/src/commands/highlight.rs:47,59` already propagates the error, so the CLI needs no change to stop printing `Highlighted 1 region(s).` for work it did not do.

**7. Geometry the window manager could not supply keeps its best-effort fallback and says so in the log — and that one provider function belongs here.**

*The mechanism (verified in the code).* `resolve_window_manager_bounds` (`crates/provider-atspi/src/node.rs:1454-1457`) swallows the window manager's error from both calls it makes: the window lookup (`resolve_window`, `:1441-1446`, `.ok()?` at `:1444`) and the bounds read (`.ok()` at `:1456`). For a real platform top-level, `resolve_extents` (`:1343-1389`) reads that `None` as "no window-manager answer" and moves on. The parent chain yields nothing for a top-level — its parent is the application node, where the chain stops (`:1425-1427`) — so the answer becomes the extents the toolkit reports in screen coordinates (`:1365-1371`). The code's own comment says what those are: real screen coordinates on X11, and `0,0`-based on Wayland, where a client does not know its position. Measured as the mechanism behind the wrong bounds this change exists for: `Frame @Bounds {0,0,600,500}` against the compositor's `{10,40,600,500}`, and a click that reports success and misses by the window offset. In the measured sidecar the call that failed is the lookup, because an unrecognised compositor's window manager refuses every operation (`crates/platform-linux-wayland/src/window_manager/mod.rs:99-110`).

*The decision (maintainer).* The fallback **stays**, and it stops being silent. When either window-manager call fails for a real top-level, the provider still answers with the toolkit's extents — the same rectangle, the same success, no error — and emits one warning that names the node (role, name, AT-SPI bus name and object path), the window id where the lookup got that far, which call failed and the window manager's error, and that the toolkit's own extents were used instead. For the PlatynUI compositor that error already carries the control-socket path (`src/control_ipc.rs:16-19`), so an identified session with a dead control channel is traced to the socket from the provider's log line as well.

*Why keep a value instead of failing.* Bounds is a **read**, and a best-effort rectangle is still a useful answer. On X11 the toolkit's extents are real screen coordinates, so the fallback is usually close to what the window manager would have said. A hard error, on the other hand, would propagate: every control inside the window derives its position from the top-level's bounds through the parent chain, and the AT-SPI hit-test resolves the in-window element by searching node bounds (`element-at-point`), so one unavailable window manager would make a whole window unreadable and unpickable. The actions this change gates are different. Highlight, screenshot and the window operations are things the backend is asked to *do*, and reporting success for them claims an effect that did not happen. That is why requirement *A gated capability reports unavailable instead of success* stays exactly as it is for them: a gated action does not pretend to have acted, and a read keeps its best-effort value and says in the log that it is one.

*Why the warning is what matters on Wayland.* There the fallback is window-relative and wrong by exactly the window's offset, and nothing in the returned rectangle distinguishes it from a real position. The warning is the only thing that makes that case diagnosable. The detection fix in this change removes its measured cause — an unrecognised compositor in the sidecar — so what remains are window managers that fail for other reasons: a control channel that died mid-session, or a window the compositor cannot match.

*Rate limit: once per top-level for as long as its window-manager reads keep failing.* The provider holds, next to the injected window manager and with the same per-runtime scope, the set of top-levels whose last window-manager read failed. The key is the node's AT-SPI identity (bus name plus object path), which is stable across re-enumeration. The first failed read for a key warns and adds it; further failures for that key log at `debug`; a successful read removes the key, so a window that recovers and fails again is reported again. The alternatives were considered and rejected:

- *Per node:* rejected, because nodes are rebuilt on every enumeration and extents are memoised per node (`:1344`), so an Inspector that refreshes the tree would warn on every refresh — the flood the limit exists to prevent.
- *Once per window-manager outage (one global flag):* rejected, because it names only the first window. On X11 the lookup fails per window — `find_xid_for_pid` can fail for one application and succeed for another, measured as `WARN no X11 window found for PID pid=0` in the X11 sidecar pod (`x11-window-owner-identity`, design 6) — and the question a user debugging a missed click asks is about one particular window.

The set is bounded by the number of distinct top-levels whose window-manager read failed in the runtime's lifetime — tens in any session, one short key each — and it goes away with the runtime, so a second runtime reports its own fallbacks. A control inside such a window inherits the substituted origin through the parent chain and is covered by its window's warning, not warned about separately. Grafted popups keep their popup-geometry path (`:1373-1385`) and are outside this rule.

*Why a `provider-atspi` change lives in a Wayland change:* the theme of this change is "a capability that cannot deliver says so instead of passing off a plausible answer". The `highlight` gate is that theme at the producing end; this warning is the same theme one consumer further along, and it is the second half of the very failure the proposal measures. Splitting them would leave the change fixing detection while the visible symptom — wrong coordinates, with nothing saying why — still occurred silently whenever the window manager was unavailable for any other reason. The boundary with `atspi-process-identity` is explicit and holds in both directions: that change owns process identity and every PID-valued attribute in `node.rs`; this one owns the diagnostics of a top-level's window-manager-backed geometry. The two touch different functions in the same file.

- *Rejected, by maintainer decision:* failing loudly — the earlier draft of this change, in which a real top-level reported the window manager's error instead of any rectangle. It would have turned a best-effort value into an unreadable, unpickable window wherever the window manager failed, including on X11, where the fallback is usually close.
- *Rejected:* leaving it silent as a recorded divergence. It keeps the measured symptom undiagnosable: an env-only-identified PlatynUI session, or any session whose window manager becomes unusable mid-run, reports plausible wrong rectangles with nothing in the log.
- *Rejected:* doing it in `atspi-process-identity`. Its theme is process identity; a geometry diagnostic is neither PID-valued nor namespace-specific.
- Consequence to accept: no returned value changes, so there is no behavioural regression surface. `provider-atspi` is Linux-only — it serves X11 and Wayland; Windows uses the UIA provider and never reaches this function. On X11 the new warning appears wherever the X11 window manager cannot resolve a top-level, next to that window manager's own warning, now also naming the node that took the fallback.

**8. Peer credentials never decide *our* identity.**

Not only because the rustix read is undefined behaviour for exactly this case (Context) — the `nix` read removes that — but because even a well-defined answer has no meaning across a namespace boundary, and because reading its failure as "not PlatynUI" is exactly today's defect. It is kept only where its answer is meaningful: naming a foreign compositor's binary.

**9. Alternative weighed and not chosen now: a compositor-advertised Wayland global.**

Shape: the compositor creates a global (say `platynui_compositor_v1`); the client sees the interface name in the registry listing `connect_and_enumerate` already walks (`connection.rs:270-272` iterates `globals.contents().clone_list()` and matches on `global.interface`), so detection costs a string comparison on data already fetched. Its decisive advantage is exactly what decision 3 cannot prove: the global arrives **on the Wayland connection itself**, so it identifies the compositor that owns our display, with no path correlation and no forwarded-socket risk.

Its costs: a new Wayland protocol interface in the repo — an XML definition plus scanner-generated server types, since smithay's `create_global` needs a generated resource type — a versioning story for that interface, and a second identity surface to keep in sync with the control socket. And it answers only "a PlatynUI compositor is there", not "its control channel works", so an identified session with a dead socket would fail at the first capability instead of at identification — the same end state as path (b), reached with more machinery.

Verdict: not now. It is the upgrade path if the socket-path correlation proves insufficient in the field, and it composes with this design rather than replacing it: the global would become the deciding mechanism and the handshake would be demoted to the usability check it already is.

**10. The identification record.**

One log line at initialization naming the identified compositor, the deciding mechanism, the control-socket path with its handshake outcome, the desktop-environment value and the peer-credential outcome. This is the same discipline `atspi-process-identity` applies to its own identity decision, and for the same reason: a degradation that only shows up as wrong coordinates is not debuggable. It is a spec requirement, not a nicety — a silently degraded identification is the failure mode this whole change addresses.

## Risks / Trade-offs

- **A stale or forwarded `PLATYNUI_CONTROL_SOCKET` in a foreign session identifies the wrong compositor** → the marker proves the peer is a PlatynUI compositor; the socket-name comparison logs a mismatch; and every capability call in such a session would reach that same compositor regardless of what detection concluded. Accepted, with decision 9 as the escape hatch.
- **Identification now depends on the control socket in the common case** → the env marker and peer credentials remain as fallbacks, so no session that is identified today becomes unidentified. Verified by the `--no-control-socket` task.
- **Landing this before the compositor's peer-credential fix turns silent degradation into a crash** → hard ordering dependency on `fix-compositor-foreign-pidns-clients`, stated in the proposal and enforced by the first task. The local namespace test would surface it as a dead compositor, not as a silent pass.
- **Input backend selection changes in the sidecar**: once identified, `input/mod.rs:95` prefers the control socket over EIS, a path never exercised from a sibling namespace (EIS across namespaces *was* measured working). → An explicit verification task; `try_control_socket_then_eis` (`:142-147`) already falls back, but the fallback must be observed to trigger rather than hang.
- **`highlight` starts failing where it silently no-opped** → deliberate, and it only affects sessions where nothing was ever drawn. No acceptance lane highlights under a foreign compositor (the compositor lane runs under ours, X11 is unaffected), so no lane changes color for a reason unrelated to a real defect.
- **The bounds fallback still produces wrong coordinates on Wayland** → accepted by maintainer decision (decision 7). A consumer that reads a top-level's `@Bounds` while the window manager cannot answer still gets the toolkit's window-relative rectangle and a click can still miss by the window offset. What changes is that the log names the window and the reason, and the detection fix removes the measured cause, so the remaining cases are window managers that fail for some other reason.
- **A warning that fires in a healthy session is noise that teaches people to ignore it** → it fires only on a failed window-manager call for a real top-level, at most once per window until that window's reads succeed again; the X11 and compositor acceptance lanes are checked to stay free of it (tasks 5.4, 7.5).
- **The warning touches X11 too** → `provider-atspi` serves both Linux backends, so an X11 top-level the window manager cannot resolve now logs this warning as well. It changes no value there; it only names the node beside the X11 window manager's own warning. Windows and macOS do not use this provider.

## Known divergences (recorded, not fixed here)

1. **`window --list`'s default expression `//control:Window` matches nothing** against AT-SPI/AccessKit, whose top level is `control:Frame` — measured inside the app container too, so unrelated to namespaces.

Three entries that used to stand here have found owners and are no longer divergences:

- **Application `@app:*` attributes read from the runtime's own `/proc`** with the daemon-reported PID, which across namespaces describes an unrelated local process (measured: `ProcessName="python3"` for the egui application). `atspi-process-identity` reads them only through a PID valid in the runtime's namespace and leaves them absent otherwise.
- **The compositor's own-window skip in hit-test** — X11's `window_at_point` skipped windows reporting the runtime's own PID while the compositor backend (`src/window_manager/platynui_ipc.rs:212-241`) returned whatever the compositor reported, so a picker over the runtime's own window got that window back instead of the one behind it. `fix-compositor-foreign-pidns-clients` now adds the skip inside the compositor, which is the only party that sees both the requesting client and the surface. The wider rule is settled across the series: the window system decides ownership, `x11-window-owner-identity` rewrites `element-at-point` accordingly, and `atspi-process-identity` removes the provider's own re-derivation so the decision is not undone one layer up.
- **The silent bounds fallback** (`crates/provider-atspi/src/node.rs:1454-1457`) — made visible here with a rate-limited warning, decision 7. The fallback value itself stays by maintainer decision.

## Migration Plan

Behavioral in exactly one place — `highlight` under an unsupported compositor now fails instead of reporting success — plus one new diagnostic: a real top-level whose window-manager call fails keeps its fallback rectangle and now says so in a rate-limited warning. Everything else is additive: a new field in the compositor's status response, a new branch in the environment heuristic, a new mechanism ahead of the existing ones. No API, keyword or configuration change, and no returned geometry changes, so nothing downstream has to migrate.

Sequence: land `fix-compositor-foreign-pidns-clients` first (a runtime that identifies the compositor from another namespace immediately calls `list_windows`, which aborts an unfixed compositor). Then the compositor's status field, then the detection change, then the highlight gate, and the bounds warning last — it changes no value, so its position in the sequence is a matter of review order, not of risk.

Verification of the Python/Robot Framework surface needs `just build-native` first — the crate is linked into `packages/native`, so an unrebuilt native module keeps the old behavior.

Rollback: revert the `platform-linux-wayland` commit; detection returns to `SO_PEERCRED`-first and highlight to its permissive gate. The bounds warning is a separate commit in `provider-atspi`; reverting it removes a log line and nothing else. The compositor's extra status field is inert for clients that do not read it and can stay in place, so a rollback does not have to touch `apps/wayland-compositor`.

## Open Questions

- Should a mismatch between the compositor's reported socket name and the client's `WAYLAND_DISPLAY` stay a warning (chosen) or become a configurable veto once real sidecar deployments show whether the paths line up? Deferrable: it changes a log line, not the specs or the task breakdown.
- Should the handshake be retried once on a transient connect error before the decision falls through to the environment marker? Deferrable: with the env marker in place, the fallthrough is already the identified-but-unusable path rather than a misidentification.
