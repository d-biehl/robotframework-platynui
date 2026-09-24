# Proposal

## Why

The X11 window manager decides that a window is "our own UI" by comparing the window's
client-claimed `_NET_WM_PID` with `std::process::id()`
(`crates/platform-linux-x11/src/window_manager.rs:548`, applied at `:459` and `:507`).
Both numbers are only comparable while the runtime and the application share one PID
namespace. In the sidecar deployment this change is written for — the display server,
the AT-SPI bus and the application in one container, the PlatynUI runtime in a sibling
container without `shareProcessNamespace` — two unrelated processes routinely carry the
same number, and the picker then throws away exactly the window it was asked about.

This was measured, not inferred: with the CLI forced onto the application's PID,
`element-at-point` returned *No element* on both `main` and PR #5, while control runs of
the same machinery at neighbouring PIDs (800/801/802) resolved `Button 'Press me'`; under
collision the trace stops before the window manager ever reports a resolved window
(measured in a podman pod: `Xvfb`, the bus daemon and a GTK application in one container,
the CLI in a sibling container without PID sharing). The failure is
independent of the AT-SPI-side own-process filter that `atspi-process-identity` fixes: it
survives that fix, because it happens one layer below, in the window manager.

## What Changes

- The X11 window manager asks the **display server** who owns our own X connection —
  X-Resource v1.2 `QueryClientIds` with the `LocalClientPID` mask — and decides **once per
  connection** whether a local-PID comparison means anything on this display:
  - a window counts as ours only when two witnesses agree: its `_NET_WM_PID` equals our PID,
    **and** the server attributes the window to the same process as our own connection. The
    server is asked about a window only when it reports our PID. A foreign window that merely
    reuses our number is resolved normally, and our own window is still skipped where the
    server numbers us differently (WSLg, a container on the host's display);
  - X-Resource **unavailable** (extension missing, query or reply fails) → today's
    behaviour, with a one-time warning naming the display, so a silent loss of the check
    is visible in the log.
- The decision and its inputs (server view, `getpid()`, resulting mode) are logged **once
  per connection**, so a degradation to the fallback is diagnosable from a normal run — the
  same discipline `atspi-process-identity` applies to its own identity decision.
- This becomes the **only** own-window decision on X11. The provider-side own-process guard
  in `element_at_point` (`crates/provider-atspi/src/lib.rs:317`) is dropped — settled with
  the maintainer, owned and implemented by `atspi-process-identity`: the display server
  decides window ownership, the provider stops comparing PIDs. What the window manager skips
  is therefore what the picker never sees, and what it keeps reaches the Inspector unfiltered.
  Nothing in this change's substance moves because of that; what moves is the stake, which is
  why the *unknown* branch stays conservative and why the whole decision table is unit-tested.
- `x11rb` gains the `res` feature in `crates/platform-linux-x11/Cargo.toml` (currently
  `xtest`, `xfixes`, `randr`, `shape`). No new third-party dependency: `x11rb 0.14.0`
  already carries `res = ["x11rb-protocol/res"]` and the request
  (`x11rb-0.14.0/src/protocol/res.rs:88`).
- Not a breaking change. On an ordinary single-namespace desktop the server's view of our
  own connection *is* `getpid()`, so the decision resolves to today's behaviour and the
  picker keeps skipping its own window. The behaviour only differs where the comparison was
  already meaningless.
- **Deliberately unchanged** (see design for the reasoning): `find_xid_for_pid`
  (`window_manager.rs:162-219`) keeps matching `_NET_WM_PID` against the PID carried by the
  AT-SPI node; the popup path keeps inheriting the managed window's PID as its
  `fallback_pid`; `resolve_window`, `bounds`, activation and state handling are untouched.

## Capabilities

### New Capabilities

None. The deployment model itself (PlatynUI and the application in different PID
namespaces) is the `sidecar-deployment` capability introduced by `atspi-process-identity`;
this change references it instead of restating it.

### Modified Capabilities

- `element-at-point`: the requirement *Hit-test excludes the host process's own UI* changes
  from "own UI is what reports our PID" to "own UI is what the display server attributes to
  our own connection". On X11 a window is excluded only when it reports our PID and the
  server attributes it to our own connection's process; a window the server attributes
  elsewhere is resolved, whatever PID it reports. A new scenario covers
  an application whose PID number equals the runtime's being resolved normally. The rewritten
  requirement also stops treating the provider as a second line of defence: the window system
  decides ownership and the provider does not re-derive it, which is what
  `atspi-process-identity` implements at that call site and what
  `fix-compositor-foreign-pidns-clients` implements for Wayland. These rules are scoped to a
  hit-test that goes through a window manager, i.e. Linux (maintainer decision). On Windows,
  where UIA resolves the element at the point directly and there is no window-manager step,
  the provider keeps implementing the exclusion itself, and the requirement says so.

## Impact

- **Rust — `platynui-platform-linux-x11`**: `src/window_manager.rs` (the self-skip decision,
  its per-connection cache, the new pure decision function and its unit tests) and
  `Cargo.toml` (`res` feature). The cache belongs to the `X11EwmhWindowManager` instance,
  **not** to the process-global atom cache at `window_manager.rs:49` — a second runtime may
  connect to a different display whose answer differs.
- **Rust — consumers, unchanged but affected**: `platynui-provider-atspi`'s
  `element_at_point` (`crates/provider-atspi/src/lib.rs:303-317`) and, through it, the
  Inspector's live picker and `Get Element At Point`. The window hit that used to be `None`
  under collision now carries a PID, and the second comparison that used to discard it —
  against the provider's own `SELF_PID` (`:317`) — is gone: `atspi-process-identity` drops
  that guard under its `sidecar-deployment` requirement *Own-process exclusion is applied
  consistently wherever the provider hides its own UI*, which already says the hit-test's
  exclusion "SHALL therefore rest entirely on the window system's own ownership decision
  (`element-at-point`)" and that "the provider SHALL NOT decide window ownership itself". So
  the two layers do not sit in series any more; this change's decision is the whole answer
  on X11, and the end-to-end
  scenario in the spec becomes observable as soon as that guard is gone. This change verifies
  the seam (task 5.1) rather than owning that call site.
- **Python / Robot Framework**: no API change, but the X11 platform is compiled into
  `platynui_native`, so verifying through the Python or RF lanes needs a native rebuild
  (`just build-native`) before `just test-python` / the X11 acceptance lane.
- **Platform reach**: Linux X11 only (README's platform table: the most complete Linux
  path). Wayland has its own owner for the same principle: the compositor decides ownership
  from the identities it captured, in `fix-compositor-foreign-pidns-clients` (which also
  fixes the abort), while detection across a namespace boundary stays with
  `wayland-sidecar-capabilities`. Windows UIA and macOS AX are untouched. Windows' hit-test
  has no window-manager step — `ElementFromPoint` returns the topmost element and cannot be
  asked for the one behind it — so the UIA provider's own exclusion
  (`crates/provider-windows-uia/src/provider.rs:497-500`) is what the rewritten requirement
  prescribes there, not a divergence from it.
- **Tests**: new unit tests in `crates/platform-linux-x11/src/window_manager.rs` (the
  decision table, in the style of the existing `#[cfg(test)]` module at `:788`), plus one
  `#[ignore]`d namespace test with a `just` recipe, following `test-java-agent-live`'s
  `--run-ignored ignored-only` pattern (`justfile:393-411`) — including its rule that a
  missing prerequisite (user namespaces blocked, `unshare` or `Xvfb` absent) fails the run
  with the prerequisite named and never skips. CI adoption is deliberately **not** part of
  this change; `evaluate-pidns-tests-in-ci` owns it.
- **Coordination**: `java-provider-linux` plans an XRes-based *window → owning process*
  helper (its task 1.2 and design decision 5) for a different question (which JVM to attach
  to). `design.md` states which change owns which helper and how they align.
