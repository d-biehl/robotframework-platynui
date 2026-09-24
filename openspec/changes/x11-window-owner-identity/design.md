# Design

## Context

See `proposal.md` — Why. This section records only the state the approach has to fit.

**Where the decision lives today.** `X11EwmhWindowManager::window_at_point`
(`crates/platform-linux-x11/src/window_manager.rs:543`) reads `std::process::id()` once per
call (`:548`) and hands it to `managed_window_at` (`:499`) and `popup_window_at` (`:433`).
Both skip a candidate when `pid == Some(self_pid)` (`:507`, `:459`), where `pid` comes from
the window's `_NET_WM_PID` property (`get_window_pid`), falling back for popups to the
managed window's PID (`:562`). Nothing else in the crate compares PIDs.

**What the numbers mean.** `_NET_WM_PID` is written by the client from its own `getpid()`,
so it is a number in the *client's* process-namespace. `std::process::id()` is a number in
*ours*. The two are comparable only while both namespaces are the same one — which is
exactly the assumption the sidecar deployment breaks, measured as `element-at-point`
returning *No element* under a forced collision while control runs at neighbouring
identifiers resolved `Button 'Press me'` (in the podman pod described in the proposal).

**What the X server knows.** X-Resource v1.2 `QueryClientIds` with the `LocalClientPID`
mask returns the PID the *server* derived from the client connection's socket credentials —
a number in the **server's** namespace, and therefore the only value on this protocol that
is not self-reported. `dev-docs/java-toolkits.md:41-50` already settles this as the repo's
preferred window→process source, with `_NET_WM_PID` as a fallback gated on
`WM_CLIENT_MACHINE`.

**What the dependency already offers** (verified against the locked sources, not assumed):
`Cargo.lock` pins `x11rb 0.14.0` for this crate; `x11rb-0.14.0/Cargo.toml:115` defines
`res = ["x11rb-protocol/res"]`; the request is `res::query_client_ids`
(`x11rb-0.14.0/src/protocol/res.rs:88`) with `ClientIdSpec { client, mask }`
(`x11rb-protocol-0.14.0/src/protocol/res.rs:168`), `ClientIdMask::LOCAL_CLIENT_PID` (`:122`)
and a reply of `ClientIdValue { spec, value: Vec<u32> }` (`:208`) inside
`QueryClientIdsReply { ids }` (`:957`). `crates/platform-linux-x11/Cargo.toml` currently
enables `["xtest", "xfixes", "randr", "shape"]` — `res` is missing. Prior art for the call
shape: `smithay-0.7.0/src/xwayland/xwm/surface.rs:935-958` (`get_client_pid`), which queries
one window's owner and reads `reply.ids.first()?.value.first()`.

**Connection lifetime.** `X11Connection` is per runtime by design
(`crates/platform-linux-x11/src/x11util.rs:11-24`), and the window manager is built with it
per runtime (`crates/platform-linux-x11/src/lib.rs:115`). The interned-atom cache is
deliberately process-global (`window_manager.rs:8-11`, `:49`) because atoms are stable for
the X server's lifetime — a property the answer to "who does this server think we are" does
**not** share.

## Goals / Non-Goals

**Goals:**

- One ownership decision per X connection, derived from the display server, cached, and
  stated once in the log.
- Identical behaviour to today wherever the runtime and the display server agree on process
  numbering, including every current desktop and acceptance lane.
- A decision function that is unit-testable without an X server, plus one local namespace
  test that reproduces the measured failure.

**Non-Goals:**

- Replacing `_NET_WM_PID` as the per-window PID source in `WindowHit` — consumers keep
  receiving the window's reported PID (`provider-atspi` needs it to correlate to an AT-SPI
  application, `crates/provider-atspi/src/lib.rs:310-322`).
- Making `element_at_point` work for an application the accessibility daemon cannot see at
  all. Measured as unfixable from this layer: the window reports its in-namespace PID while
  the daemon reports `0`/unknown, so `application_for_pid` finds nothing — on `main`, on
  PR #5, with and without collision, on all three bus daemons, in the same pod. Recorded as
  a known limitation below.
- Any Wayland or compositor work (`fix-compositor-foreign-pidns-clients`,
  `wayland-sidecar-capabilities`), any AT-SPI-side identity decision
  (`atspi-process-identity`), and any CI adoption of namespace tests
  (`evaluate-pidns-tests-in-ci`).
- A configuration key. The decision is derived from the display server on every connection;
  a toggle would only let a user re-enable a comparison the runtime has just proven
  meaningless.

## Decisions

### 1. Two witnesses: the window's reported PID and the server's attribution

A window is the runtime's own only when two witnesses agree:
- It reports our PID in `_NET_WM_PID`.
- The X server attributes it to the same process as our own connection.

The server's attribution is `QueryClientIds` with `LocalClientPID`. It is asked **once per
connection** for our own client, and **per window** only for a window that already reports
our PID. Both answers are numbers in the server's namespace, so they compare wherever the
server runs, even where it cannot see us and answers `0` for both.

| Deployment | Our connection | Our window | Another window reporting our PID |
|---|---|---|---|
| One namespace (every desktop) | our PID | our PID → skipped | its own PID → resolved |
| Runtime in a child namespace (a container on the host's display) | our host PID | our host PID → skipped | the application's PID → resolved |
| Server that cannot see the runtime (WSLg) | `0` | `0` → skipped | — (applications share our namespace, so none reports our PID) |
| Sidecar (server with the application, runtime in a sibling) | `0` | — | the application's PID → resolved |

*Alternative — "foreign never skips" (the first implementation, rejected in review).* Decide
once whether our numbering is the server's, and skip by `_NET_WM_PID` only where it is. That
was right for the sidecar. But wherever the server numbers the runtime differently while the
applications share the runtime's namespace, it stopped skipping the runtime's own window, and
the picker selected the Inspector itself: WSLg, which `dev-docs/inspector.md` documents, a
container on the host's display, and trusted `ssh -X`.

*Alternative — per-window XRes alone.* Compare the server's attribution of every candidate
window with its view of us, without `_NET_WM_PID`. It fails where the server sees neither
side (WSLg: `0` for every window) and under `ssh -X`, where every forwarded window is the
ssh process. `_NET_WM_PID` is the witness that tells apart processes the server cannot
separate. Asking it first also keeps the per-window round trip to the rare window that
reports our PID.

*Alternative — drop the self-skip entirely (not chosen).* It exists for a real reason: the
Inspector picks over its own window and overlay on an ordinary desktop. Removing it would
regress the normal case to fix the exotic one.

### 2. What the server's answer about our connection means, and what is logged

| Server's answer about our own connection | Logged mode | Rule |
|---|---|---|
| a PID equal to `std::process::id()` | **verified** | two witnesses; on a desktop the second always agrees |
| a different PID, `0`, or no value in the reply | **foreign** | two witnesses; this is where the second one decides |
| extension absent, too old, or the request/reply fails | **unknown** | the reported PID alone, as before, plus one warning per connection |

A window whose owner the server cannot be asked about, even though it answered about us, is
not skipped: the second witness is missing, and the spec forbids guessing. Only a server
that cannot be asked at all keeps the reported PID alone. The split between *foreign* and
*unknown* stays the point: a server that can answer settles ownership, while a missing
extension is no evidence at all, and silently weakening the picker on an old X server would
be the wrong default.

**Measured** (task 4.4, Xvfb 21.1.24). A client in a sibling PID namespace gets
`LocalClientPID = 0`: the server translates the peer's credentials into its own namespace and
finds nothing. A client in a *child* namespace of the server's gets its PID as the server sees
it, which differs from the client's `getpid()`. `crates/platform-linux-x11/tests/pidns_tests.rs`
builds the child-namespace, WSLg and sidecar rows of the table in §1 with real namespaces, and
the one-namespace row's own window. The unit table covers every cell, including another
window that reports our PID on a desktop.

### 3. Which XID names "our own connection"

`ClientIdSpec.client` identifies a client by *any* XID from that client's resource range;
the server masks it with `resource_id_mask`. Our own range is `setup().resource_id_base`,
available without a round trip and without creating a window. Prior art uses a window XID
(smithay asks about someone else's window), which is the same mechanism.

**Verified** (task 1.2, Xvfb 21.1.24): `resource_id_base` alone is accepted as a client
spec. `QueryClientIds` answers it with the same `LocalClientPID` as for an XID taken from
`generate_id()`, so no throwaway allocation is needed.

### 4. The decision is cached on the window-manager instance, not process-globally

A `OnceLock`-style field on `X11EwmhWindowManager` (which today holds only `conn`, `:421`),
so each runtime's connection decides for itself. Putting it next to the global `ATOMS` cell
(`:49`) would be wrong for the reason its own comment gives: atoms are stable per X server,
whereas a second runtime may connect to a *different* display — and the multi-suite
acceptance lanes exist precisely to catch state that wrongly outlives a runtime
(`dev-docs/testing-strategy.md` §2.6).

### 5. The comparison is a pure function

The server query is I/O; the decision is not. A small pure function — "given the server's
view of us, our own PID, and a window's reported PID: skip or not" — makes the whole
decision table unit-testable with no X server, in the style of the existing
`#[cfg(test)]` module (`window_manager.rs:788-836`, `decode_window_state` tests). Only the
one-shot query needs a live server.

### 6. `find_xid_for_pid` stays unchanged, deliberately

`find_xid_for_pid` (`:162-219`) answers a different question: given *an application's* PID
taken from its AT-SPI node (`extract_pid`, via `resolve_window` at `:521-522`), find its window.
Swapping `_NET_WM_PID` for the server's view there would compare a value in the
*accessibility daemon's* namespace with a value in the *X server's* namespace. In the
measured sidecar setup those two are the same namespace (bus daemon, display server and
application all live in the app container), so the swap would change nothing about the
failure actually observed there — which is that the daemon reports `0`/unknown for an
application it cannot see, producing four `WARN no X11 window found for PID pid=0` and a
silent fall back to AT-SPI frame extents *including decorations*
(`600,400,370,189` instead of the WM client rect `5,24,360,160`), measured in the same pod.

Hardening `find_xid_for_pid` against a window that *claims* a foreign PID is a real but
separate concern, and `java-provider-linux` already owns the helper for it: its task 1.2
and design decision 5 make `XResQueryClientIds` + `LocalClientPID` the authoritative
window→process source for attach decisions, with `_NET_WM_PID` as a `WM_CLIENT_MACHINE`-
gated fallback.

**Ownership split, so the two do not collide.** This change owns the *self* query — "who
does this server think **we** are" — and keeps it private to `window_at_point`'s decision.
`java-provider-linux` owns the general *window → owning process* helper. They share one
prerequisite, the `res` feature on `x11rb`, which this change adds to
`crates/platform-linux-x11/Cargo.toml`; whichever lands second finds it already enabled.
When the general helper exists, folding this change's one-line self query into it (a
`QueryClientIds` for our own resource base) is a mechanical follow-up — worth doing, but not
worth blocking either change on.

### 7. The provider-side re-check is gone: this is the only own-window decision on X11

**Settled with the maintainer** — this was Open Question 1 of this design and, on the other
side, `atspi-process-identity`'s D10 / Open Question 4. The own-process guard in
`provider-atspi`'s `element_at_point` (`crates/provider-atspi/src/lib.rs:317`), which took
the `WindowHit`'s PID and returned `None` when it equalled `SELF_PID`, is **dropped**. The
window system decides window ownership; the provider stops comparing PIDs.
`atspi-process-identity` owns and implements that line, and its `sidecar-deployment` spec
already prescribes it: the hit-test's exclusion "SHALL therefore rest entirely on the window
system's own ownership decision (`element-at-point`), and the provider SHALL NOT decide window
ownership itself — neither by comparing a reported process identifier with its own, nor by any
other re-derivation from a value whose numbering space it has not established".

*Scope, settled with the maintainer: Linux.* The rewritten requirement's rules — ownership
from the window system's view, verified against its view of our own connection, and no
re-derivation by the provider — apply to a hit-test that goes through a window manager, which
means the X11 window manager and the PlatynUI compositor. Windows has no such step:
`ElementFromPoint` returns the topmost element and cannot be asked for the one behind it, and
the process the operating system reports for that element is already in the runtime's own
numbering. There the UIA provider implements the exclusion itself
(`crates/provider-windows-uia/src/provider.rs:497-500`) and returns nothing over its own UI,
and the requirement states that explicitly instead of leaving Windows to read as a violation.

*Why the guard could not simply be deleted on its own.* Measured: the PlatynUI compositor's
`window_at_point` returns the **caller's own** window and applies no exclusion at all, so on
Wayland a provider that stops filtering would let the Inspector's picker select the
Inspector's own window — worse than today, where the provider's guard turns that into *no
element*. The compositor-side exclusion is therefore part of the same decision and is owned by
`fix-compositor-foreign-pidns-clients`, which already captures the identities it needs. X11
has no such gap: the window manager here already skips and resolves the window *behind*, which
is why this change only has to fix *how* ownership is established.

*What it means for this change.* The window manager's answer is now final: a window wrongly
attributed to us is hidden from the picker, and our own window wrongly attributed elsewhere is
exposed to it. That is the argument for the conservative *unknown*
branch (decision 2), for the whole table being unit-tested (decision 5), and for task 4.3, the
guard that an ordinary desktop still skips its own window. The remaining work at the seam is
observation, not implementation: `tasks.md` 5.1 runs `element-at-point` under a collision and
records that the button now resolves.

## Risks / Trade-offs

- **[A three-namespace deployment can still collide]** → The runtime, the X server and an
  application each in a separate PID namespace, with the server seeing neither: both of the
  server's answers are `0`, and an application reusing the runtime's number is skipped. That
  topology is outside `sidecar-deployment`'s supported one, which puts the display server and
  the application in one namespace.
- **[`ssh -X` attributes every forwarded window to the ssh process]** → The second witness
  agrees for every forwarded window, but the first one still tells them apart: the remote
  processes share one namespace, so only the runtime's own window reports its PID.
- **[This decision is now load-bearing on its own]** → With no second filter behind it, a
  wrong attribution would hide an application's window from the picker or expose ours. → The rule is a pure function of the reported PID and the
  server's two answers, every deployment in decision 1 is unit-tested and built in the
  namespace harness, and task 4.3 keeps the ordinary-desktop behaviour under test. No error
  path may reach `window_at_point`: every failure falls back to the reported PID alone, which is
  today's behaviour.
- **[An X server without X-Resource 1.2 keeps the unverified comparison]** → The *unknown*
  branch is today's behaviour, so this is not a regression; the one warning per connection
  makes it visible. Every server this crate meets offers the extension (task 1.1): the lanes'
  Xvfb, Xephyr, and Xwayland both under `xwayland-satellite` and under `apps/wayland-compositor`.
- **[Extra round trips]** → `QueryVersion` plus `QueryClientIds` on a connection's first
  hit-test, taken lazily by decision 4's per-instance cell and never again on that connection,
  plus one `QueryClientIds` for a window that reports our PID. That is our own window, so a
  pick over anything else costs what it costs today, and a runtime that never hit-tests never
  asks. Any error resolves to *unknown*
  rather than propagating. A reply that never comes is not a new failure mode: `x11rb` has no
  per-request timeout, and a server that stops answering this request stops answering every
  other request on the connection too — only the connect itself is bounded
  (`x11util.rs:60-80`).
- **[The change is invisible on every current lane]** → On a normal desktop the decision
  resolves to *verified*, so no existing test can tell the change landed. This is why the
  unit decision table and the namespace test are part of the change rather than optional.
- **[The local namespace harness may not run on every developer machine]** → Unprivileged
  user namespaces are AppArmor-restricted on stock Ubuntu noble (measured: writing `uid_map`
  denied; the usual remedy is `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`).
  The test is `#[ignore]`d, and where a prerequisite is missing — user namespaces blocked,
  `unshare` or `Xvfb` absent — it **fails** with a readable message naming it, never with a
  cryptic `unshare` error and never by skipping. That is the rule the Java agent's live
  checks follow (`crates/java-agent/tests/live_fixture.rs`, `justfile:403-404`): an ignored
  test runs only when someone asks for exactly this coverage, and a skip would read as a pass
  in that run. The existing graceful skip in `apps/wayland-compositor/tests/ipc_tests.rs` is
  not the precedent here. Making it dependable in CI is `evaluate-pidns-tests-in-ci`'s job.

**Known limitation, recorded and not addressed here:** `element_at_point` cannot bridge an
X11 window to an AT-SPI application the bus daemon cannot see at all, because `_NET_WM_PID`
and the daemon's answer come from different worlds (`0`/unknown). Measured on `main` and on
PR #5, with and without collision, on dbus-daemon 1.14.10, dbus-daemon 1.16.2 and
dbus-broker 35, in the pod described in the proposal. No change in this series fixes it;
it needs a window↔application correlation that does not go through a PID.
`atspi-process-identity` already writes it down as a property of the deployment — its
requirement *Correlating a native window to an application needs a process ID both sides can
express* prescribes reporting "no element" rather than correlating on an unusable value — so
this change neither restates nor tries to work around it.

## Migration Plan

- **Behavioural, and bounded.** No API, configuration or spec surface outside
  `element-at-point` changes. On any display where the server's view of our connection is our
  own PID — every current desktop, CI lane and acceptance lane — behaviour is byte-for-byte
  today's. The behaviour differs only where the old comparison was already meaningless.
- **Native rebuild:** yes for anything driven from Python or Robot Framework. The X11
  platform crate is linked into `platynui_native`, so `just build-native` must precede
  `just test-python` and the X11 acceptance lane; the Rust unit tests and the namespace test
  need no rebuild.
- **Sequencing:** this is change 4 of 5 in the series and depends on nothing in changes 1–3
  at the code level. It does depend on `atspi-process-identity` for the *end-to-end*
  observation (decision 7) — the provider-side guard has to be gone before a collision can
  resolve a control — which is why the series implements that one first. Landing this change
  before it is harmless: the guard then still discards the window, exactly as measured today.
- **Rollback:** revert the crate's diff and the `res` feature line. Nothing persists, nothing
  is migrated, no user-visible API moves; a revert restores the previous comparison exactly.
  If only the new behaviour is suspect while the diagnostics are wanted, the *foreign* branch
  can be turned back into today's behaviour in one place — the pure decision function.

## Open Questions

**Settled since this design was written:** what happens to the provider-side hit-test guard at
`crates/provider-atspi/src/lib.rs:317` (this design's former Open Question 1,
`atspi-process-identity`'s D10 / Open Question 4). It is dropped — the window system decides
ownership and the provider stops comparing PIDs. Decision 7 records the reasoning and why the
compositor half of it belongs to `fix-compositor-foreign-pidns-clients`.

1. **Should the popup path eventually use per-window XRes?** Popups often carry no
   `_NET_WM_PID` and today inherit the managed window's PID (`:562`). That inheritance is
   fine for correlation but means popup ownership is never independently known. Deferrable:
   it changes no requirement in this delta.
2. **Is the `unshare`-based harness sufficient for the X11 reproduction?** Answered: yes.
   `crates/platform-linux-x11/tests/pidns_tests.rs` reproduces the collision with `unshare`
   alone, with `Xvfb` and the probe in sibling PID namespaces, and task 5.1 reproduced the
   measured sidecar end to end the same way. `evaluate-pidns-tests-in-ci` still owns the
   CI-side answer.
