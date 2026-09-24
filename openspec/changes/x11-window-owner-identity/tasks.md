# Tasks

Test-first throughout: every test below is written to fail against the current code before
the step that makes it pass. Spec scenarios are in `specs/element-at-point/spec.md`; the
"how" and the alternatives are in `design.md`.

## 1. Confirm the two facts the design assumes

- [x] 1.1 Confirm X-Resource v1.2 is offered by every X server this crate meets: the
      developer's session server, the `Xvfb` used by the X11 acceptance lane, and XWayland
      under `apps/wayland-compositor`. Verify by recording `xdpyinfo -queryExtensions | grep
      -i x-resource` plus the server version for each, and note any server that lacks it —
      that is the *unknown* branch of the decision table and must stay usable (design 2).
      Run of 2026-09-24: every server offers X-Resource. The session server is Xwayland 24.1.13
      under xwayland-satellite. The lane's Xvfb is 21.1.24 and answers `QueryVersion` with 1.2.
      Xephyr 21.1.24 was run nested in an Xvfb. The PlatynUI compositor's Xwayland is 24.1.13,
      started through `startcompositor.sh --xwayland`. No server lacks it.
- [x] 1.2 Confirm that `setup().resource_id_base` is accepted as a `ClientIdSpec.client` for
      our own connection (design 3). Verify with a throwaway probe against a live server that
      prints the returned `LocalClientPID` and `getpid()`; if the server rejects or ignores
      it, switch the design's spec source to an XID from `generate_id()` and record that in
      `design.md` §3 before continuing.
      Run of 2026-09-24, with a throwaway `x11rb 0.14.0` probe against Xvfb 21.1.24:
      - Same namespace: `resource_id_base` is accepted, and the server reports our `getpid()`.
      - Probe in a child PID namespace (`getpid()` = 1): the server reports the probe's host
        PID.
      - Xvfb and probe in sibling PID namespaces: the server reports `0`.

      Both of the last two land in *foreign*. §3 records the verified fact.
- [x] 1.3 Enable the `res` feature for `x11rb` in `crates/platform-linux-x11/Cargo.toml`
      (alongside `xtest`, `xfixes`, `randr`, `shape`) and verify `cargo check -p
      platynui-platform-linux-x11` succeeds with `x11rb::protocol::res` imported. Verify the
      lockfile is unchanged apart from feature resolution — `x11rb 0.14.0` already carries
      the feature, so no new dependency may appear.

## 2. Unit tests for the decision table (red before green)

- [x] 2.1 In `crates/platform-linux-x11/src/window_manager.rs`'s `#[cfg(test)]` module, add
      tests for a pure decision function that takes the server's view of our own connection,
      our own PID and a window's reported PID, and answers "skip this window or not"
      (design 5). Cover all six cells: *verified* × (own window, foreign window), *foreign* ×
      (window reporting our number, window reporting another number), *unknown* × (own
      window, foreign window). Verify the tests fail to compile until 3.1 introduces the
      function, then pass unchanged afterwards.
- [x] 2.2 Add a test that a window with no `_NET_WM_PID` at all is never skipped in any of the
      three modes, so the popup path's missing-property case (`window_manager.rs:432`,
      `:562`) cannot regress into an accidental skip. Verify it fails against a naive
      implementation that treats "no PID" as a match.
- [x] 2.3 Add a test that the per-connection decision is taken exactly once: drive the caching
      helper with a counted query closure, call it several times, and verify the closure ran
      once and every call returned the same mode (spec scenario *The ownership decision is
      taken once per connection and stated in the log*). Verify it fails against an
      implementation that queries per call.
- [x] 2.4 Run `just test-crate platynui-platform-linux-x11` and verify the new tests are the
      only failures, for the expected reason (missing function), before writing any
      implementation.

## 3. The ownership decision in the window manager

- [x] 3.1 Add the pure decision function and the three-mode type (verified / foreign /
      unknown) from design 2, with doc comments that state *why* the comparison needs the
      server's view — an `_NET_WM_PID` is a number in the client's namespace, ours is a
      number in ours. Verify the 2.1/2.2 tests pass.
- [x] 3.2 Add the one-shot X-Resource query: negotiate `QueryVersion` 1.2, then
      `QueryClientIds` for our own client, mapping "extension missing", "version too old",
      request error, reply error and an empty `value` list onto the decision table — every
      error onto *unknown*, an empty or zero value onto *foreign* (design 2). Verify by
      pointing the probe from 1.2 at a server started without the extension and observing the
      *unknown* branch rather than an error propagating out of `window_at_point`.
- [x] 3.3 Cache the decision on the `X11EwmhWindowManager` instance — **not** in the global
      `ATOMS` cell (`window_manager.rs:49`) — and log it exactly once per connection with the
      server's view, `std::process::id()` and the resulting mode; make the *unknown* branch a
      `warn!` naming the display, everything else a `debug!`/`info!`. Verify the 2.3 test
      passes and that a run performing several hit-tests logs the line once.
- [x] 3.4 Use the decision in `window_at_point` (`:543`) so `managed_window_at` (`:507`) and
      `popup_window_at` (`:459`) skip by PID only in the *verified* and *unknown* modes
      (superseded by 3a: the rule is now two witnesses in every mode).
      Verify `just test-crate platynui-platform-linux-x11` is green and that the diff touches
      no other behaviour — `find_xid_for_pid`, `resolve_window`, `bounds`, activation and
      state handling stay exactly as they are (design 6).

## 3a. Two witnesses (found in the implementation review)

The first implementation skipped no window at all once the server's view of us differed from
`getpid()`. That was right for the sidecar. It also stopped skipping the runtime's own window
wherever the server numbers the runtime differently while the applications share its
namespace: WSLg, a container on the host's display, and trusted `ssh -X`. The maintainer chose
the two-witness rule (design 1).

- [x] 3a.1 Rewrite the unit tests for the rule first. A window is ours only when it reports our
      PID **and** the server attributes it to the same process as our connection. They cover:
      - the desktop;
      - the sidecar and a child namespace's colliding host application (resolved);
      - WSLg, a child namespace's own window, and TCP (skipped);
      - a server that cannot be asked, where the window's owner is never asked;
      - a window whose owner cannot be asked;
      - a window without `_NET_WM_PID`;
      - the rule that the server is asked about a window only when it reports our PID.

      Verify the tests fail against the one-witness signature.
- [x] 3a.2 Implement it. Cache the server's view of our own connection per window manager, ask
      about a window's owner only when it reports our PID. Let the reported PID decide alone only
      where the server cannot be asked at all; a window whose owner the server cannot name is not
      skipped, as the spec forbids guessing (a finding of the second review). Verify with `just test-crate platynui-platform-linux-x11`.
- [x] 3a.3 Rebuild the harness so each window is created where it is in the deployment it
      stands for:
      - the sidecar's application window from a process in `Xvfb`'s namespace;
      - a WSLg-like test, with `Xvfb` unable to see the runtime and its windows;
      - a child-namespace test, with the runtime's own window over a host window that reuses its
        in-namespace PID.

      Verify that the two new tests fail against the one-witness implementation, and that all
      seven pass with `just test-x11-pidns`.
- [x] 3a.4 Bring the spec, the design, `dev-docs/platform-linux.md` §3 and these tasks up to the
      rule. Verify `openspec validate x11-window-owner-identity --strict`.
- [x] 3a.5 Re-run task 5.1's end-to-end reproduction and the X11 lane with the rule. Verify the
      sidecar still resolves `Button 'Click Me'`, and the lane stays 81/81 with the own-window
      picker suite passing on X11.
      Run of 2026-09-24: the sidecar reproduction again resolves `Button 'Click Me'`, where the old window
      manager answers `No element`. `test-acceptance-x11` is 81/81. The own-window picker suite passes on X11,
      and the Inspector logs `mode=Verified` once.

## 4. Local namespace reproduction (the measured failure)

- [x] 4.1 Build the harness under the crate's `tests/`: start an `Xvfb` display, map a fixture
      window, set its `_NET_WM_PID` to a chosen value, and run the probe inside `unshare
      --user --map-current-user --keep-caps --pid --fork --mount-proc` so the probe's own
      `getpid()` is a fresh-namespace number while the X server stays outside. Use the same
      invocation as the other harnesses in this series, not `--map-root-user`: task 5.1 runs
      AT-SPI inside this harness, and the bus daemon's EXTERNAL authentication rejects a peer
      that claims uid 0 while its socket credentials say otherwise (measured); `--keep-caps`
      keeps the capability needed to force a PID for the collision. Verify the harness reports the
      fixture's `_NET_WM_PID`, the probe's `getpid()` and the server's view of the probe.
      A missing prerequisite **fails** the test with a readable message naming it — never a
      skip, and never a raw `unshare` error: unprivileged user namespaces blocked (measured
      as the stock-Ubuntu-noble case, `uid_map` write denied; name the
      `kernel.apparmor_restrict_unprivileged_userns=0` remedy), `unshare` or `Xvfb` not
      installed, or a display that never comes up. This is the rule of
      `crates/java-agent/tests/live_fixture.rs` and `justfile:403-404` ("a missing artifact
      fails the run rather than silently skipping the coverage"), not the graceful skip of
      `apps/wayland-compositor/tests/ipc_tests.rs`. The harness SHALL assert that the
      collision it set up actually took effect (the fixture's `_NET_WM_PID` equals the
      probe's in-namespace `getpid()`) and fail rather than continue without it, so a lost
      collision can never read as a pass — the fail-loud property `evaluate-pidns-tests-in-ci`
      D7 requires of every harness in this series.
- [x] 4.2 Add the `#[ignore]`d reproduction test: with the fixture window's `_NET_WM_PID` set
      to the probe's in-namespace `getpid()`, `window_at_point` over the fixture SHALL return
      that window (spec scenario *An application whose process identifier equals the
      runtime's is still resolved*). Verify it fails against the pre-3.4 code with exactly
      the measured symptom — no window resolved — and passes afterwards.
- [x] 4.3 Add the companion `#[ignore]`d test for the *verified* mode outside the namespace:
      same display, same fixture, `_NET_WM_PID` set to the test process's real PID → the
      window SHALL still be skipped (spec scenario *A point over the host process's own
      window is skipped*). Verify it passes both before and after 3.4 — this is the guard
      that the ordinary desktop is untouched.
- [x] 4.4 Record in the test module what the X server actually returned for a client from a
      sibling PID namespace (a zero, a foreign number, or no value), closing the
      "Not measured" note in `design.md` §2; update that note to the observed answer.
- [x] 4.5 Add a `just` recipe that runs these tests (`cargo nextest run -p
      platynui-platform-linux-x11 --run-ignored ignored-only`, following
      `test-java-agent-live`'s pattern at `justfile:400-412`), mark it `[unix]`, give it the
      same kind of comment that recipe carries — its prerequisites are hard, and a missing one
      fails the run rather than skipping the coverage — and document it in `CONTRIBUTING.md`
      next to the other live recipes. Verify the recipe runs the two tests, that it exits
      non-zero with the prerequisite named when user namespaces are unavailable (a machine
      that restricts them, or temporarily `sysctl -w user.max_user_namespaces=0`), and that
      `just test` still does **not** run them.

## 5. The seam with the dropped provider-side guard (verification — see design 7)

- [x] 5.1 With 3.4 in place and `atspi-process-identity` implemented — that change drops the
      provider-side own-process guard (`crates/provider-atspi/src/lib.rs:317`), settled with
      the maintainer — run the CLI's `element-at-point` inside the 4.1 harness against a real
      AT-SPI application whose PID number equals the runtime's, and verify that the button now
      resolves where it measurably returned *No element* before (spec scenario *An application
      whose process identifier equals the runtime's is still resolved*). If it still returns
      nothing, determine which layer dropped it — this change's decision (a *verified* verdict
      where it should be *foreign*: check the log line from 3.3) or the daemon's inability to
      resolve the application at all, which is the known limitation in `design.md` and not
      fixable here.
      Run of 2026-09-24, reproducing the measured sidecar. The application container runs Xvfb, icewm,
      a private AT-SPI bus and the egui test app, whose in-container PID was 50. The CLI runs in a sibling
      PID namespace, forced onto PID 50. With the old window manager the CLI answers `No element at
      (42, 88)`, the measured symptom. With this change it logs `server_view=Reported(0) mode=Foreign`
      once and resolves `Button 'Click Me'` in `Frame 'Sidecar App'`.
- [x] 5.2 Only if `atspi-process-identity` has not landed yet: record that 5.1 cannot be
      observed end to end, verify instead that the window manager's own answer is right (the
      4.2 test, plus the log line showing the *foreign* verdict), and report the dependency
      rather than touching `provider-atspi` — that call site belongs to that change.
      Not applicable: `atspi-process-identity` has landed (`45424fc`), and 5.1 was observed end to end.
- [x] 5.3 Verify that no own-window comparison is left in the X11 path other than this one:
      `grep -rn "process::id()" crates/platform-linux-x11/src` returns only the ownership
      decision and its tests, and the guard at `crates/provider-atspi/src/lib.rs:317` is gone.
      Any other survivor is a second, unverified exclusion behind a verified one and must be
      reported before this change is called done.
      Run of 2026-09-24: `std::process::id()` in `crates/platform-linux-x11/src` appears only as the
      decision's input (`window_manager.rs`, `window_at_point`) and in its explaining comment. The guard in
      `crates/provider-atspi/src/lib.rs` is gone; the only `process::id()` left there names a test's
      temporary path.

## 6. Documentation

- [x] 6.1 Extend `dev-docs/platform-linux.md` §3 (WindowManager (EWMH), around line 127) with
      two or three sentences on how own windows are identified and what happens in each of
      the three modes, pointing at `dev-docs/java-toolkits.md:41-50` for the X-Resource rule
      rather than restating it, and stating that this verdict is the only own-window exclusion
      in the hit-test path — the provider no longer re-derives ownership from the window's
      reported PID. Verify the section still reads as intent, not as a transcript of the code.

## 7. Verification

- [x] 7.1 Run `just check` and `just test` and verify both are green with no new clippy
      findings in `platynui-platform-linux-x11`.
- [x] 7.2 Run the new namespace recipe from 4.5 and verify every `#[ignore]`d test passes, with
      the log line from 3.3 present exactly once per connection in the output.
- [x] 7.3 Run `just build-native` and then the X11 acceptance lane (`just
      test-acceptance-x11`) on a real display, and verify the Inspector picker and
      `Get Element At Point` behave exactly as before — this is the *verified* mode, so
      nothing may change (real-provider check; the mock cannot exercise it).
      Run of 2026-09-24: `just headless=true test-acceptance-x11` 81/81, and `inspector_picker_own_window.robot` on X11
      passes. Every Inspector logs exactly one decision, `mode=Verified` with the server reporting its own PID,
      and no warning.
- [x] 7.4 Run `just pre-commit` and verify it passes before committing.
      Run of 2026-09-24 on the final state: green, with 2227 Rust tests and 793 Python tests.

## 8. Commit

- [x] 8.1 Commit the change as `fix(platform-linux-x11): identify own windows via the X
      server` (Conventional Commits, subject ≤72 characters), with a body naming the sidecar
      deployment and the measured collision, and the trailer
      `Co-authored-by: Fabian Tsirogiannis <fabian.tsirogiannis@imbus.de>`. Verify
      `git log -1 --format='%s%n%b'` shows the subject and the trailer, and that the commit
      contains no unrelated files.
