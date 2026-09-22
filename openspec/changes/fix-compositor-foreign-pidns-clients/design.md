# Design

## Context

See proposal.md — Why for the failure and the measurements behind it. What shapes the approach:

- **The compositor owns the accepted socket for exactly one moment.** `insert_client` is called from two places, and both receive the freshly accepted `UnixStream`: the Wayland listening socket (`apps/wayland-compositor/src/backend/mod.rs:204-206`) and the `wp-security-context-v1` listener (`apps/wayland-compositor/src/handlers/security_context.rs:28-31`). Afterwards the file descriptor belongs to `wayland-backend`, and the only way back to the peer credentials is `Client::get_credentials` — the accessor that aborts. Verified: those are the only two `insert_client` call sites in the repository.
- **`wayland-backend` cannot return "unknown".** `wayland-backend-0.3.15/src/rs/server_impl/client.rs:313-317` does `socket_peercred(&self.socket).expect("getsockopt failed!?")` with the `rustix` 1.1.4 it links, whose `UCred.pid` is a non-zero `Pid` (`rustix-1.1.4/src/net/types.rs:2096`). rustix reads the kernel's bytes into a `MaybeUninit<UCred>` and returns `assume_init()` (`rustix-1.1.4/src/backend/linux_raw/net/sockopt.rs:45-61`), so a kernel-reported `0` violates the type's invariant: the read is undefined behaviour, not an error path. In the measured build it came back as an `Err`, and this code turns that into a panic. Verified against the vendored sources.
- **Per-client data already exists and is already read this way.** `ClientState` (`src/client.rs:5-10`) is attached to every client the compositor inserts, and `client.get_data::<ClientState>()` is the established retrieval pattern (`src/handlers/compositor.rs:27`).
- **The wire format is already nullable.** `WindowInfo.pid`, `PopupInfo.pid` and `MinimizedWindowInfo.pid` are `Option<u32>` (`src/control.rs:140/175/193`), and the only consumer models them as options too (`crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:18/29`, with a no-pid fallback at `:381`). Nothing has to learn a new shape.
- **XWayland is a separate path already.** `window_pid` returns `x11.pid()` before it ever looks at peer credentials (`src/control.rs:1270-1273`), and that value is the X11 client's `_NET_WM_PID` (`smithay-0.7.0/src/xwayland/xwm/surface.rs:372`). XWayland's own Wayland connection is created inside smithay (`src/xwayland.rs:62-79`), so it carries smithay's client data, not ours.
- **The control socket has an accept point of its own.** `register_control_client` (`src/control.rs:285`) receives the accepted `UnixStream` from the listener callback (`:260-265`), wraps it in `ControlClient` (`:219-228`, which already implements `AsFd`) and hands the connection to calloop. `process_command` (`:382`) takes only the request text and the state, so today a command cannot tell who asked; the caller's identity has to ride on the connection and be passed in. Verified: this is the only place a control connection is accepted.
- **The caller is a fresh connection every time.** The runtime opens a new control connection per command (`crates/platform-linux-wayland/src/control_ipc.rs:10-21`), so the peer credentials are read once per request-sized connection — there is no long-lived connection whose identity could go stale, and no per-request cost beyond the one `getsockopt` the connection already pays for being accepted.
- **The hit-test cannot skip with the call it makes today.** `window_at_point` (`src/control.rs:565-579`) takes `Space::element_under`, which returns the frontmost hit and nothing else: it walks `elements()` reversed, filters by bounding box and then tests the input region at the render location (`smithay-0.7.0/src/desktop/space/mod.rs:185-200`). There is no way to ask it for the next hit down, so skipping means running that walk ourselves — `elements()` is back-to-front, which the renderer states where it reverses it (`src/render.rs:114-115`).

## Goals / Non-Goals

**Goals:**

- One place decides a client's identity, once per connection, where the answer is actually available.
- "Unknown" is a first-class value the compositor can carry and report, not an error path.
- The identity the compositor establishes is also *used*: the window at a point is never the caller's own window, which is what makes the compositor a window system that answers the `element-at-point` question rather than one that hands the problem to its consumer.
- The ban makes the regression impossible to reintroduce silently, within the limits of the current workspace (see Decision 5).

**Non-Goals:**

- Answering *which* process an unidentified client is. The compositor reports "not identified"; correlating such a client with an AT-SPI application is a different problem and is not solved by anything in this change.
- Excluding own windows from anything other than the point lookup. The listings answer "what exists" and keep showing the caller's windows; a consumer that wants to filter them has the `pid` field.
- Making the exclusion work where identity does not exist. Across sibling PID namespaces the compositor can resolve neither the client nor the caller, so the exclusion is inactive by construction (Decision 8).
- A namespace-independent identity token (see the `SO_PEERPIDFD` alternative in Decision 2). This change needs "known or unknown", nothing stronger.
- Anything in `crates/platform-linux-wayland`. Its detection, its bounds fallback and its window matching across namespaces belong to `wayland-sidecar-capabilities`.

## Decisions

### 1. Capture at accept time, not lazily at first use

The peer credentials are read in the accept callback, before `insert_client`, and stored in `ClientState`. `window_pid` and the security-policy check read the stored value.

*Why:* it is the only point where the compositor holds the socket without going through the panicking accessor (Context). It is also correct to cache: the peer of an accepted `AF_UNIX` connection is fixed, and `SO_PEERCRED` is a snapshot taken at `connect`/`socketpair` time, so a later read could not produce a fresher answer anyway.

*Alternative considered — a non-panicking wrapper around the backend accessor at each call site:* rejected. `Client::get_credentials` returns a `Result`, but only for an invalid client id; an unrepresentable peer panics inside the backend, so there is nothing to catch (`catch_unwind` would not help either — the abort comes from a panic in a destructor while the backend mutex is already poisoned, which is exactly what was measured). The panic is inside the dependency, so the only reliable fix is not to call it.

*Alternative considered — patching or forking `wayland-backend`:* rejected as disproportionate for one accessor we do not need. Nothing is reported upstream either (settled, see Open Questions): the change avoids the panicking accessor locally instead — its own `SO_PEERCRED` read (Decision 2) and the clippy ban (Decision 5).

### 2. Read `SO_PEERCRED` with `nix`, and treat a non-positive PID as unknown

The compositor gains a direct `nix` dependency with the `socket` feature and reads `SO_PEERCRED` through `nix::sys::socket::getsockopt(&stream, sockopt::PeerCredentials)`. The returned `UnixCredentials` wraps a plain `libc::ucred`, so `pid()` is a `pid_t` that can be `0` — verified in the `nix` sources. `pid <= 0` and any `getsockopt` failure map to *unknown*; a positive PID maps to that process. No `unsafe` enters the compositor for this.

*Why not `libc` directly:* it reads the same `libc::ucred` and adds no crate, but only through an `unsafe` block. Settled with the maintainer: `unsafe` is avoided wherever a safe wrapper exists. `nix` is such a wrapper; it is new to the lock (not in `Cargo.lock` today, checked), with one feature enabled, on top of the `libc` the workspace already builds.

*Why not `rustix`:* `rustix::net::sockopt::socket_peercred` cannot represent the value this change exists for. Reading a PID of `0` into its non-zero `Pid` is undefined behaviour (Context). That it came back as an `Err` with the non-errno code `Unknown error -1000` is an artifact of the layout, not a defined error path: `Result<UCred, Errno>` uses the zero PID as its `Err` niche, and `-1000` is exactly the peer's uid 1000 read as an `Errno` (`Errno(u16)`, whose `raw_os_error` negates it) — inferred from the layout, and consistent with the uid the measured pod ran under. Even taken at face value, "the peer is invisible to me" and "the socket option failed" would be indistinguishable, and the compositor would lose the very distinction its log line is supposed to show.

*Why not `std`:* `UnixStream::peer_cred()` returns exactly the right shape (`UCred.pid: Option<pid_t>`), but it is unstable behind `peer_credentials_unix_socket` (rust-lang/rust#42839) and is not available on stable — verified in the toolchain sources of rustc 1.98.1. Once it is stabilized it is the natural replacement for the `nix` call, and the dependency can go again.

*Alternative recorded, not chosen — `SO_PEERPIDFD` (Linux 6.5+):* the compositor could take a pidfd for the peer at accept time. Measured in the podman spike: a server's peer pidfd inode matched the client's own `pidfd_open(getpid())` inode across all three topologies (host, sibling namespace, same namespace), i.e. it is a namespace-independent identity — the same property the AT-SPI identity chain (`atspi-process-identity`) relies on. Not chosen here because this change only needs "known or unknown", and using a pidfd as an *identity* would drag in the pidfs-vs-`anon_inode` kernel check that is load-bearing there (on kernel 6.8 every pidfd shares `st_ino = 69`, measured). It is the natural extension if the compositor ever has to answer "is this client the automation runtime itself?" or to correlate a window with an AT-SPI application across namespaces — and it would need no `unsafe` either, because `nix` exposes it as `sockopt::PeerPidfd`.

### 3. `null` means unknown; `0` is never emitted

The identity is stored as an option and flows unchanged into the existing `Option<u32>` fields, so `serde_json` emits `null`. No sentinel, no field omission.

*Why it matters that `0` never appears:* `0` is a syntactically valid PID-shaped value and would be matched against by any consumer that filters by PID (`platynui_ipc.rs:333`), silently associating unrelated windows. This is the same class of mistake the AT-SPI work is correcting on the bus side, where `Some(0)` from the daemon was measured to drop real applications.

*Alternative considered — omitting the field:* rejected. The field is documented (`apps/wayland-compositor/docs/ipc-protocol.md:293`) and a missing key and a null value would then both have to be handled by every consumer.

### 4. XWayland keeps its own source, and the distinction is explicit

`window_pid` keeps returning `_NET_WM_PID` for X11-backed windows and only then falls back to the stored Wayland identity. XWayland's own connection has no `ClientState`, which under this design reads as *unknown* — harmless, because no window resolves its PID through it, but the code has to say so rather than leave it to chance.

*Trust:* `_NET_WM_PID` is set by the X11 client and is a value in *that client's* namespace; the compositor cannot verify it and does not try. It does normalize one value: smithay reports the property as it stands (`smithay-0.7.0/src/xwayland/xwm/surface.rs:767` stores any `CARDINAL`), so a client that writes `0` would otherwise put a `0` on the wire through the one source Decision 3 does not cover. A declared `0` is therefore reported as *no* process, like a window that declares none. The consequences of trusting it are the subject of `x11-window-owner-identity`, not of this change.

### 5. Ban only the accessor this change removes

`clippy.toml` gains a `disallowed-methods` entry for `wayland_server::Client::get_credentials`, and `[workspace.lints.clippy]` gains `disallowed_methods = "deny"` next to the existing `disallowed_types = "deny"` so the ban is explicit rather than dependent on `-D warnings`.

The obvious companion entry, `rustix::net::sockopt::socket_peercred`, is **not** added here: `crates/platform-linux-wayland/src/capabilities.rs:67` still calls it, and `just clippy` runs `cargo clippy --workspace --all-targets -- -D warnings` (`justfile:171-173`), so adding it now would fail the gate on a file this change does not own. `wayland-sidecar-capabilities` rewrites that detection and should add the entry in the same commit.

### 6. Tests: a unit test for the mapping, a namespace test for the behaviour

- **Deterministic, in `just test`:** the credential-to-identity mapping (`0`, negative, positive, read failure) and the serialization of an unknown PID (`"pid": null`) are pure functions over injected input and belong in the crate's unit tests, per dev-docs/testing-strategy.md §2.1.
- **Behavioural, local only:** the abort can only be reproduced with two PID namespaces. The compositor runs under `unshare --user --map-current-user --pid --fork --mount-proc`, the test process stays outside and connects as an ordinary Wayland client through the shared `XDG_RUNTIME_DIR`, so the compositor's `SO_PEERCRED` read reports `0`. This topology is measured, not assumed: in the podman spike a host client was reported to a pod compositor with `SO_PEERCRED pid 0`, the same answer as for a sibling namespace.

  The client fixture already exists — `popup_client::open_toplevel_with_popup` (`apps/wayland-compositor/tests/ipc_tests.rs:377-531`) connects over an absolute socket path and maps a toplevel *and* a popup, which covers the window listing and the popup listing from one fixture.

  The test is `#[ignore]`d and reachable through a `just` recipe, keeping namespace tests local unless `evaluate-pidns-tests-in-ci` decides otherwise: unprivileged user namespaces are not universally available (on stock Ubuntu 24.04 `unshare -Ur` was measured to be blocked by `kernel.apparmor_restrict_unprivileged_userns=1`).

  It preflights that topology and **fails** when a prerequisite is missing — user namespaces blocked (naming the sysctl remedy), `unshare` not installed, a compositor that cannot start or whose sockets never appear, a fixture client that cannot connect — with a message naming the prerequisite. It never skips. This follows the Java agent's live checks (`crates/java-agent/tests/live_fixture.rs`, whose module doc states that a missing artifact "fails loudly rather than skipping the coverage", and the matching rule on `test-java-agent-live` at `justfile:403-404`), and deliberately *not* the graceful skip of the existing `ipc_tests.rs`: an ignored test only runs when someone asks for exactly this coverage, and a skip there reads as a pass in the one run that was meant to exercise it. The existing `ipc_tests.rs` tests keep their skip, and so do the new default-lane cases added to that file (tasks 1.7/1.8), which follow the file's conventions; the fail-loud rule is for the namespace test.
- **The positive control already exists:** `ipc_tests.rs:635` asserts that the popup's `pid` equals `std::process::id()`. It holds unchanged under this design — compositor and test share a PID namespace in the default lane — and it is what proves the nullable contract did not turn into "always null". It must stay green without edits.

### 7. The unidentified case logs at `warn`, the identified case at `debug`

The compositor's default log level is `warn` (`src/lib.rs:224-246`: `RUST_LOG`, then `--log-level`, then `PLATYNUI_LOG_LEVEL`, then `warn`). An `info` or `debug` line would therefore be invisible in an ordinary session, which is exactly where the sidecar degradation has to be noticeable. So: a client that cannot be identified is one `warn` per connection; a client that is identified is one `debug` per connection. Both are per connection, never per request — `list_windows` runs on every window operation, and a per-request line would drown the log it is meant to make readable.

### 8. The window system decides who owns a window — on Wayland that is the compositor

`window_at_point` skips a window whose client is the process that asked, and answers with the window behind it. The caller is the peer of the control connection the request arrived on; the owner is the identity captured at accept time. Both are the compositor's own view, and a window is excluded **only** when both were established and match.

*Settled with the maintainer, and this is the decision that made it necessary.* The own-process guard in the AT-SPI provider (`crates/provider-atspi/src/lib.rs:317`) is dropped — `atspi-process-identity` owns that line, and the question both changes had recorded as open (its D10 / Open Question 4, `x11-window-owner-identity`'s Open Question 1) is answered: the display server decides ownership, the provider stops comparing PIDs. A provider comparing `std::process::id()` with a number that reached it through a window is comparing two namespaces it never established a relation between; that is the defect, not its location.

*Why the compositor must take the other half in the same series:* measured in the podman pod, `window_at_point` returns the caller's own window today and the compositor excludes nothing. Deleting the provider guard without this exclusion would make the Inspector's picker select the Inspector's own window — worse than today, where the guard turns that answer into *no element*. X11 has no such gap: its window manager already returns the window behind, which is why `x11-window-owner-identity` only has to fix *how* ownership is established there.

*Why only on a positive match:* the alternative — excluding whenever the identities are not provably different — would hand back an occluded window, or nothing, for every client the compositor cannot see. A picker that returns the caller's own window is a visible, self-explanatory wrong answer; a picker that silently returns the window *underneath* something is not. This is the same rule the rest of this capability follows and the rule `atspi-process-identity` states for its own exclusions: exclude on a positive match, never on an unresolved one.

*Why an XWayland window is never excluded:* its process id is `_NET_WM_PID`, a value the X11 client wrote about itself (Decision 4). The compositor did not establish it and cannot verify it, so it is not an identity in this capability's sense — an X11 client that declares the caller's number must not be able to make its window unpickable.

*Limit, measured:* on a control connection from a **sibling** PID namespace the compositor cannot resolve the caller either, so the exclusion is genuinely inactive there. That is the documented boundary of this mechanism, not a defect in it, and closing it would need a namespace-independent identity (the `SO_PEERPIDFD` option in Decision 2) on both sides. The topologies it does cover are the ones that matter today: a session in one namespace, and a caller in a child namespace of the compositor's.

### 9. Skipping means walking the stack, not post-filtering one answer

`Space::element_under` returns the frontmost hit and cannot be asked for the next one (Context), so `window_at_point` does the walk itself: `elements()` reversed — front-to-back, the order the renderer relies on (`src/render.rs:114-115`) — with the same hit criterion smithay applies (bounding box, then the input region at the window's render location, `smithay-0.7.0/src/desktop/space/mod.rs:185-200`), taking the first hit the caller does not own. The index reported in the response stays the index of the window actually returned, which the current code already derives by position (`src/control.rs:572`).

*Verified:* the ordering and the hit criterion, from the smithay source and the renderer's own comment. *Assumed, and task 4.2 checks it:* that the render location can be reconstructed through the public `Space` API (element location minus the window's geometry offset) so the walk keeps matching `element_under`'s answer exactly when nothing is excluded. If it cannot, the walk must be built on whatever public accessor does reproduce it — an exclusion that changes which window is "frontmost" for an ordinary caller would be a regression, which is why task 1.7 also asserts the unexcluded answer.

## Risks / Trade-offs

- **The security-policy path is reasoned, not measured.** `SecurityPolicy::is_client_allowed` (`src/security.rs:90-112`) uses the same accessor, so it is on the same abort path under `--restrict-protocols`. The podman spike explicitly found it has **no callers today** and that `--restrict-protocols` plus a foreign client did not crash — because nothing calls it. → It is fixed anyway (the ban forces it) and its behaviour is specified; the requirement is written against the decision, not against an end-to-end observation, and a unit test over the identity is the honest level for it.
- **One extra `getsockopt` per connection.** → Negligible: once per client, at accept, on a path that already does a socket insert.
- **A new crate in the lock (`nix`).** → One feature (`socket`), built on the `libc` the workspace already compiles. The alternatives were an `unsafe` block (avoided by maintainer decision) or a dependency that provably cannot express the case; once `std`'s `peer_cred` is stable, `nix` can be dropped again.
- **The namespace test does not run in CI.** → Deliberate (`evaluate-pidns-tests-in-ci` owns that question). The unit tests cover the mapping in every run, and the ban prevents the accessor from coming back; what CI cannot see is a *new* aborting path built on something else.
- **Reporting `null` makes a previously "matched" window unmatchable by PID.** Across namespaces a consumer filtering by PID (`platynui_ipc.rs:333`) now finds nothing where it previously crashed the compositor. → Strictly better, and closing that gap is `wayland-sidecar-capabilities`' job; this change must not paper over it with a fabricated PID.
- **A future `insert_client` call site could forget to capture.** → Mitigated by making the capture part of constructing the per-client data, so an uncaptured client is not expressible without going out of one's way; a client inserted without an accepted socket reads as *unknown*, which is the safe answer.
- **The exclusion changes an answer the default lane can see.** A process that maps a window and then asks for the window at a point inside it now gets the window behind, or nothing. → Intended, and specified; no test asks `window_at_point` today (verified: `window_at_point` appears nowhere in `apps/wayland-compositor/tests/`), so nothing existing changes meaning, and the new `ipc_tests` case makes the new answer the documented one.
- **A window the caller wants to inspect could be hidden from it.** An automation tool that legitimately wants its *own* window at a point can no longer get it from this command. → Accepted: that is what the capability asks for, and the listings still report the caller's own windows with their geometry, which is the better route for "where is my window" anyway.
- **The exclusion is inactive exactly where the sidecar lives.** Across sibling namespaces neither side resolves, so a picker there gets the caller's own window back — if the caller has a window on that display at all, which a headless sidecar does not. → Recorded as the measured limit (Decision 8) rather than papered over with a comparison of numbers that mean nothing; a namespace-independent identity would be a separate change on both sides.
- **One more `getsockopt`, now per control connection.** The runtime opens one connection per command (`control_ipc.rs:10-21`), so this is one syscall per command. → Negligible next to the command it serves (a window listing walks the space; a screenshot copies a framebuffer), and it is the same read the Wayland side already does.

## Migration Plan

- **Behavioral, not additive, and not breaking.** For clients the compositor can resolve, every reported *value* is unchanged (asserted by `ipc_tests.rs:635`). Two behaviours change: an abort disappears, and `window_at_point` stops naming the caller's own window. The first cannot be depended on; the second is what `element-at-point` already required and what its only consumer — a picker looking for the window behind — wants. The IPC schema, the field names and their types stay as they are.
- **No native rebuild.** Nothing in `packages/native`, `src/PlatynUI` or the Robot Framework surface is touched; the acceptance lanes run against the same control-socket contract. `just build-native` is not a prerequisite for verifying this change.
- **Deployment:** the compositor is a binary in the session; it takes effect the next time a session starts. There is no persisted state, no protocol version and no client-side change to coordinate.
- **Rollback:** revert the commit. The externally visible differences afterwards are the return of the abort and of the caller's own window at a point; no data or configuration has to be migrated back. Because the `clippy.toml` entry and the `nix` dependency land in the same commit, a revert leaves the workspace consistent. If only the exclusion is suspect, it can be turned off in one place — the decision function of Decision 8 — without touching the identity capture.
- **Ordering:** this is change 1 of the five-change series and is implemented first. It must land before `wayland-sidecar-capabilities` (change 3): that change makes the provider detect the compositor across the namespace boundary, after which the runtime issues a window query for every window operation — turning today's latent abort into a reproducible one. It shares no code with `atspi-process-identity` (change 2), `x11-window-owner-identity` (change 4) or `evaluate-pidns-tests-in-ci` (change 5), so nothing else in the series blocks it. It is, however, what makes dropping the provider-side guard in change 2 safe on Wayland (Decision 8): if the two land separately, this one goes first, or a Wayland picker over its own window selects that window until it does.

## Open Questions

None open.

**Settled with the maintainer — no upstream reports.** Neither the `wayland-backend` panic (`Client::get_credentials` cannot express an unidentifiable peer, for any compositor) nor `rustix`'s non-zero `Pid` in `UCred` is reported upstream. This change avoids the panicking accessor locally: the compositor reads `SO_PEERCRED` itself through `nix`'s safe `getsockopt`, which can represent `0` (Decision 2), and `wayland_server::Client::get_credentials` is banned through clippy's `disallowed-methods` (Decision 5), so nothing in the workspace can call it again.

**Settled, recorded here because two other changes carried it as open:** what happens to the provider-side own-process guard at `crates/provider-atspi/src/lib.rs:317` (`atspi-process-identity`'s D10 / Open Question 4, `x11-window-owner-identity`'s Open Question 1). The guard is dropped; the window system decides ownership — the compositor here, the X server in `x11-window-owner-identity`. The reason it could not simply be deleted on its own is in Decision 8: measured, the compositor returns the caller's own window and excludes nothing, so a provider that stops filtering would let the picker select the Inspector's own window, which is worse than today's *no element*.
