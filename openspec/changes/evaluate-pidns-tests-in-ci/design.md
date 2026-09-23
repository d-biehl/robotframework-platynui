# Design

## Context

See `proposal.md` — Why. This section only records the state the design has to work with.

**What this change does not own.** The tests themselves belong elsewhere: the mode-decision unit tests and the `unshare` harness (an `#[ignore]`d test module, `crates/provider-atspi/src/pidns_harness.rs`, plus its `just` recipe `test-atspi-pidns`, which takes the bus daemon as a parameter) are part of `atspi-process-identity`; the compositor, Wayland-capability and X11-identity tests belong to `fix-compositor-foreign-pidns-clients`, `wayland-sidecar-capabilities` and `x11-window-owner-identity`. Across the series, those namespace tests fail with the missing prerequisite named — blocked user namespaces, a daemon that is not installed, a binary that was not built — and never skip. This change decides **where those run**, measures the one fact that decides it, and writes the outcome down. It can be researched in parallel with the fixes but can only be adopted after the harness exists.

**The harness shape is measured, in a VM, not in CI.** Everything the identity chain cares about is reachable with two sibling PID namespaces and one shared socket directory — no container runtime, no images, no display server:

- namespace #1: `unshare --user --map-current-user --keep-caps --pid --fork --mount-proc` holding the bus (`dbus-daemon --config-file=…` or `dbus-broker-launch --scope user`) together with the peers that must **not** be recognised as ours,
- namespace #2: the same `unshare` invocation holding the prober, optionally forked onto an exact PID to reproduce the reported collision,
- plus a plain host-namespace run for the same-namespace case.

That produced the same mode decisions and the same per-peer verdicts as the whole podman matrix (123 container probe runs + 22 VM peer checks + 12 CLI runs, 0 false positives for the chosen design).

**The single blocking unknown is AppArmor, and it was measured — on a VM.** On stock Ubuntu 24.04 (`kernel.apparmor_restrict_unprivileged_userns = 1`) that exact `unshare` invocation fails as an ordinary user with `write failed /proc/self/uid_map: Operation not permitted` and `apparmor="DENIED" operation="capable" profile="unprivileged_userns" capability=21 capname="sys_admin"`. After `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` (in-VM) the same command line works as uid 1000. The hosted-runner side is read from files, never run: the 26.04 `apparmor` package ships `/usr/lib/sysctl.d/10-apparmor.conf` with `kernel.apparmor_restrict_unprivileged_userns = 1` *and* `…_unconfined = 1`; the runner-images build scripts under `images/ubuntu/` never touch apparmor, userns or those sysctls (grepped at commit `938aed2`); there is an AppArmor profile for `podman`, `crun`, `runc`, `rootlesskit` and `slirp4netns` but **none for `/usr/bin/unshare`**. **No public log shows the live sysctl values, `unshare -Ur`, or the effect of the sysctl write on a GitHub-hosted runner.**

**What `ubuntu-latest` resolves to** (read from the runner-images READMEs, the release SBOMs and the Launchpad archive):

| | until the rollout: `ubuntu-24.04` | after the rollout: `ubuntu-26.04` (GA 2026-09-17) |
|---|---|---|
| kernel | 6.17.0-azure | 7.0.0-azure |
| dbus | 1.14.10 | 1.16.2 |
| dbus-broker | not installed; `35-2ubuntu0.1` in universe | not installed; `37-5` in universe |
| at-spi2-core | not installed; 2.52 installable | not installed; 2.60.4 installable |
| podman | 4.9.3 | 5.7.0 |
| pidfs | yes | yes |

GitHub has announced that `ubuntu-latest` moves from 24.04 to 26.04, rolling out between **2026-10-19 and 2026-11-19** ([actions/runner-images#14748](https://github.com/actions/runner-images/issues/14748)). The Linux jobs in `.github/workflows/ci.yml` run on `ubuntu-latest` (lines 24, 83, 134, 199, 301, 353, and the x86-64 wheel entry at 417) and stay there (maintainer decision). The only Linux exception is the arm64 wheel entry (`ubuntu-24.04-arm`, :421), which this change does not touch. The normal lanes are unaffected by the bus change: application and runtime share a PID namespace in every one of them, and every difference in the next table concerns a peer the daemon cannot see.

**The bus behaviours, and where each is covered** (all measured at protocol level for a peer the daemon cannot see):

| bus | credentials `ProcessID` | `GetConnectionUnixProcessID` | `ProcessFD` | covered by |
|---|---|---|---|---|
| dbus-daemon 1.12 / 1.14 | present, `0` | `0` | absent | unit tests + local podman matrix; also `ubuntu-latest`'s stock bus until the rollout |
| dbus-broker 29 / 33 | present, `0` | `0` | absent | unit tests + local podman matrix |
| dbus-broker 35 / 37 | omitted | success `0` | present | unit tests + local podman matrix |
| dbus-daemon ≥ 1.15.10 (1.16.2) | omitted | `Error.UnixProcessIdUnknown` | present | unit tests + local podman matrix; also `ubuntu-latest`'s stock bus after the rollout |

The unit tests are the ones over the recorded credential shapes that `atspi-process-identity` owns. A CI job meets whichever stock bus its runner ships; it is not how any of these cells is covered (D5).

**The repo already has a local-only live lane, and it is the model here:** the `#[ignore = "needs a JVM and the built fixture"]` tests in `crates/java-agent/tests/live_fixture.rs` (from `:317`) run only via `just test-java-agent-live` (`justfile:407`), and `.github/workflows/ci.yml:346` states in prose that they are deliberately not run in CI yet. That recipe also sets the series' rule for missing prerequisites: "a missing artifact fails the run rather than silently skipping the coverage" (`justfile:403-404`). `dev-docs/testing-strategy.md:351` ("every automated lane runs in CI") therefore already coexists with an on-demand lane; the point of this change is to make the PID-namespace case an explicit, argued entry rather than a second silent exception.

## Goals / Non-Goals

**Goals:**

- Answer one question with a measurement: can a GitHub-hosted runner run the `unshare` harness without contortions?
- Fix the acceptance bar *before* the measurement, so the outcome is read off, not argued.
- Make the "no" outcome as complete a deliverable as the "yes": a recorded decision, the reason, and where the coverage lives instead.
- If "yes": guard what no unit test reproduces — the harness's verdicts across a real PID-namespace boundary between a real bus and a real prober — on the runners CI already uses, with assertions that hold on any bus.
- Leave the existing CI jobs untouched, and make no runner decision.

**Non-Goals:**

- Designing the harness, the probe, or any test assertion — `atspi-process-identity` owns those; this design only states the properties a CI-hosted harness must have (D7).
- Any podman-based lane in CI: pods, a second display server, a fixture-launch bridge, RF suites in a sidecar topology. Evaluated and rejected — ~1 GB images, an X server or software-GL compositor, and on 26.04 a 15 s teardown stall per rootless container (AppArmor blocks podman's SIGTERM to `pasta`, LP #2154379, Confirmed).
- Reproducing a pre-6.9 kernel anywhere in CI. Both images `ubuntu-latest` resolves to around the rollout run kernels ≥ 6.17, so the anon-inode trap (every pidfd `anon_inode:[pidfd]` with `st_ino = 69`, `f_type = 0x09041934`) is unreachable there by construction.
- Choosing runners. The existing jobs stay on `ubuntu-latest`, an adopted job runs there too, and no job in this series is pinned to an image or given one of its own (maintainer decision).
- Covering daemon-specific cells in CI. Which identity mode a given bus produces is covered by unit tests and the local podman matrix; CI does not install or select a particular bus to reach a cell (D5).

## Decisions

### D1: Decide by running it on a branch, not by reading documentation

**Chosen:** a throwaway branch with one workflow job, five or more runs, then a written decision.

The one fact that decides everything — whether `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` is permitted and effective on a hosted runner — is **not** derivable from what is public. The sysctl default is in a shipped file, the absence of an `unshare` profile is in a shipped file, passwordless sudo is documented, and none of that says what the runner's provisioner (closed source) leaves in place at job start. One job answers it in minutes.

*Alternatives considered:* **adopt straight away** and fix it if it breaks — rejected, because the failure mode is a red job on every PR at exactly the moment the fixes land, or worse, a job that "passes" because the harness silently did nothing. **Decide "no" now without measuring** — rejected, because the price of finding out is one branch, and the harness's cross-namespace verdicts are the one thing in this series no unit test reproduces: a real kernel PID-namespace boundary between a real bus and a real prober.

### D2: `unshare`, not podman, for anything that could go into CI

**Chosen:** the two-sibling-namespace harness described in Context.

It was measured to produce the same answers as the whole container matrix, needs no images, no registry, no X server and no root, and its runtime is dominated by starting a bus. The podman matrix stays a **local** tool and its seven images already exist; nothing about this decision deletes them.

*Alternative:* podman pods in CI, to exercise the full sidecar topology end to end. Rejected on cost (see Non-Goals) and on value: every identity-relevant result the pods produced was reproduced by the `unshare` harness.

### D3: Run on the runners the existing jobs use; assert what holds on any bus

**Chosen:** the spike and any adopted job run on `ubuntu-latest`, like the other Linux jobs in `ci.yml`. The job asserts only verdicts that do not depend on the daemon: no foreign peer is claimed as our own; our own peer is recognised where the bus identifies it; a lost collision or a missing prerequisite fails (D7). Per topology, it logs the daemon and its version, the decided mode and that mode's inputs.

The maintainer decided not to pin runners, and the assertion shape follows from that. A job that asserted "on this image the daemon answers `0`" would tie the test to one image. A job that asserts verdicts is unaffected by the announced rollout: the rollout changes which bus the job exercises, not what the job checks. Which mode a given daemon yields is a property of that daemon's answers, and it is covered where the daemon can be named explicitly (D5).

*Alternative, not chosen:* branching on the detected dbus version inside the job to assert a per-daemon mode. It hides the axis under test inside the test, and a wrong branch reads as a pass; per-daemon expectations are better served by unit tests that name the daemon they model.

### D4: One elevation step, with a measured fallback — and the fallback is the tiebreak for "contortion"

**Chosen (primary):** `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`, then run the harness as the normal runner user, exactly the invocation measured in the VM.

**Fallback, to be measured in the same spike:** `sudo unshare …` with `setpriv` dropping to uid 1000 inside the namespace. Running the bus and the peers under different uids would change D-Bus EXTERNAL authentication relative to every measurement taken so far, so uid consistency is a property of the fallback, not a detail. **This variant has never been run — here, in the VM, or on a runner.**

The decision rule (D6) treats the fallback as "one step further" and a chain beyond it as a contortion.

*Alternative:* `aa-exec` style escapes — rejected in advance: 26.04 additionally ships `kernel.apparmor_restrict_unprivileged_unconfined = 1`, which is aimed precisely at those.

### D5: The CI job runs the runner's stock bus; daemon-specific cells live in unit tests and the local podman matrix

The job runs the harness against the bus the runner ships — dbus-daemon 1.14.10 on today's `ubuntu-latest`, 1.16.2 after the rollout — started privately by the harness at an explicit socket path in its own namespace, never the runner's session bus. The recipe takes the daemon as a parameter, so the job runs `just test-atspi-pidns dbus-daemon`. It installs no other bus, and it does not run the `dbus-broker` variant, which the stock image does not ship.

The cells that depend on a particular daemon are covered elsewhere, not by choosing, pinning or provisioning runners. The first is the legacy `ProcessID = 0` answer of dbus-daemon 1.12/1.14 and dbus-broker 29/33. The second is dbus-broker 35/37's successful `0` next to `ProcessFD`, the cell where PR #5's `Some(0)` drops a real application. The third is dbus-daemon 1.16.2's `ProcessFD` plus `Error.UnixProcessIdUnknown`. All three are covered by unit tests over the recorded credential shapes, owned by `atspi-process-identity`, and by the local podman image matrix.

What no runner can give, and where it goes instead: a pre-6.9 kernel → a unit test with an injected `f_type` (owned by `atspi-process-identity`). Note that the runner images carry **no** `at-spi2-core` either; the harness must therefore drive a plain bus, not an accessibility bus. That is what the VM harness did, and it is sufficient, because the identity decision is a property of the D-Bus connection, not of AT-SPI.

### D6: The acceptance bar, written down before the measurement

Adopt only if **all** of these hold:

1. the harness runs on the hosted runner after at most the primary elevation step (D4), or after the single fallback variant;
2. five consecutive runs of the job are green, with no forced-PID retry exhaustion and no bus-start flake;
3. the added wall time stays in single-digit minutes, measured, not estimated — the earlier estimate of "seconds of runtime" was never clocked;
4. the harness's verdicts hold on the runner's stock bus, and the job log names the daemon version and the decided mode for every topology.

Otherwise the outcome is "keep them local", recorded with the failing criterion named. A partial result may produce a partial adoption — for example the same-namespace and sibling-namespace topologies without the forced-PID collision, if only the collision proves flaky. A partial adoption must say in the workflow which topology it does *not* cover and where that coverage lives instead.

### D7: Three properties the harness must have before it may run in CI

These are constraints this change places on the harnesses the other changes build. They apply primarily to `atspi-process-identity`, whose harness is the candidate for adoption and whose harness tasks (6.1, 6.2) carry them, and by the same reasoning to the namespace tests of `fix-compositor-foreign-pidns-clients`, `wayland-sidecar-capabilities` and `x11-window-owner-identity`. They are constraints, not implementation:

- **A missing prerequisite fails, never skips.** User namespaces blocked, a daemon not installed, a binary not built: the run fails with the prerequisite named, as the Java agent's live checks do (`crates/java-agent/tests/live_fixture.rs`, `justfile:403-404`). In CI this is what turns a refused sysctl into a red job instead of a green one that exercised nothing.
- **A lost collision must fail, not pass.** The forced-PID step forks until the child lands on the target PID; the measured helper exits non-zero when it overshoots. A harness that silently proceeds without the collision would turn the most important topology into a green job that proves nothing.
- **The PID is taken before anything else starts in that namespace.** Forced PIDs are deterministic only then. Under thread churn, 600 forced spawns needed up to 7 attempts each (~5.5 % miss per attempt; 265/300 first try under steady churn, 296/300 under burst churn), with a 50-attempt bound and 0 failures. In the `unshare` harness the prober is the first thing in its namespace, which is why the VM runs needed no retries. The bounded retry stays anyway, because "nothing else starts" is a property of the harness that a future edit can break.

### D8: The translated-descriptor mode and what it means for CI

`atspi-process-identity` includes the descriptor's process ID in our namespace — the `Pid:` line of `/proc/self/fdinfo/<ProcessFD>` — as the second of its four identity modes ("translated descriptor"), taken when a descriptor exists but is not on `pidfs`. On the kernel-6.8 VM it was measured as a perfect own-process discriminator: 0 false positives over 22 peer checks, and 4 false negatives against 6 for a chain without it. Its CI consequence is **none**. The mode is reachable only on kernels without `pidfs`, and both images `ubuntu-latest` resolves to around the rollout run ≥ 6.17, where every descriptor is on `pidfs` and the chain takes process-fd mode first. A CI job's topologies therefore never enter it; it is covered by that change's unit tests with an injected filesystem magic and `fdinfo` text, and by the VM.

### D9: Where the decision is written down

`dev-docs/testing-strategy.md` — §6 (CI) gains the PID-namespace coverage and its lane placement, §8 (Decisions) gains one row. The doc is normative and durable: it states what the lane is, where it runs and what stays local, and it does not carry a status report of the spike. The spike's numbers live in this change's tasks; only the rule survives into the doc.

## Risks / Trade-offs

- **The sysctl is permitted but ineffective (e.g. a runner-side LSM stack that ignores it).** → The spike prints the sysctl before and after and runs `unshare -Ur id` at both points, so "refused", "written but ineffective" and "works" are three distinguishable outcomes rather than one failure.
- **A green job that proves nothing** — the collision was lost, a prerequisite was missing, or the harness quietly left out a topology. → D7's fail-loud rules, plus a job step that prints the decided mode and its inputs per topology, so the log names what was actually exercised.
- **Flaky forced PIDs in CI**, where the runner is noisier than the VM. → Bounded retry plus the five-run bar; flakiness is an explicit reason to answer "no" rather than something to stabilise later.
- **The announced `ubuntu-latest` rollout (2026-10-19 … 11-19) lands during the spike**, so its runs may land on different images. → Each run logs its image, kernel and daemon version (task 2.2), and the acceptance bar counts runs regardless of image because the assertions do not depend on the daemon (D3). A run that fails on one image only is a finding for `atspi-process-identity`, not a reason to pin.
- **Adopting it hides the local tools.** The unit tests and the podman matrix are the only coverage of the daemon-specific cells and of old buses, and the VM is the only way to reach a pre-pidfs kernel; all of that is easy to forget once a green badge exists. → The `dev-docs` entry names what CI does *not* cover, next to what it does.
- **Cost of the "no" outcome:** the PID-namespace fixes stay covered by unit tests in CI and by an on-demand local harness — the same position the Java agent's live checks are in today (`.github/workflows/ci.yml:346`). The risk it accepts is a regression that only a namespace boundary reveals, caught at the next manual run rather than on the PR.

## Migration Plan

**Additive and tooling-only.** No Rust, Python or Robot Framework source is touched, so **no native rebuild** and no change to anything a user observes. The spike lives on a throwaway branch and is never merged; only its outcome is.

1. Branch, add the spike job, run it (D1), record the numbers in this change's tasks.
2. Read the outcome against D6.
3. **Yes:** add one job to `.github/workflows/ci.yml`, on `ubuntu-latest` like the other Linux jobs. At first it does not gate the wheel jobs (`build-wheels*` currently need `lint, rust, python, acceptance-linux`); leaving the new job out of that list keeps a flaky newcomer from blocking releases. Promote it to a gate only after it has been green for a while.
4. **No:** no workflow change at all.
5. Either way: update `dev-docs/testing-strategy.md` (D9) and delete the spike branch.

**Rollback:** delete the job from `.github/workflows/ci.yml` and revert the doc paragraph — one commit, no state anywhere else, nothing installed on a developer machine, and the local harness and recipe (owned by `atspi-process-identity`) are unaffected either way.

## Open Questions

None open.

**Settled with the maintainer:**

- **Runners.** The existing jobs stay on `ubuntu-latest`, an adopted job runs there too, and this series makes no runner decision — no pinned image, no job for a particular image (D3). The daemon-specific cells are covered by unit tests and the local podman matrix (D5).
- **Commit credit.** This change carries no `Co-authored-by` trailer. The credit for PR #5's author is scoped to changes 1–4, and this change contains none of that work.
