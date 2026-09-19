# Tasks

Research first, adoption only if the measurement earns it. Group 4 is the decision point: groups 5 and 6 are mutually exclusive, and exactly one of them is worked.

## 1. Prerequisites and local baseline (test-first)

- [ ] 1.1 Confirm the artifact under evaluation exists: the `unshare` harness and its `just` recipe from `atspi-process-identity` run locally on this machine and print, per topology, the daemon and its version, the decided mode and its inputs. Verify by running the recipe on a Linux dev box and reading the per-topology mode line; if the recipe does not exist yet, stop here — this change cannot be adopted before it does (design.md, Context)
- [ ] 1.2 Establish the harness is a real test before asking CI to host it: with the AT-SPI identity fix reverted (or the pre-fix binary), the harness must **fail** in a topology a stock bus can produce — the forced-PID collision across sibling namespaces, the reported failure the harness exists to reproduce — and pass with the fix in place. Verify by recording both outcomes and the daemon version they were recorded on; a harness that is green either way is not worth a CI job
- [ ] 1.3 Verify the harness's fail-loud properties (design.md D7): force the collision helper to overshoot its target PID and confirm the harness exits non-zero instead of continuing without a collision; then remove a prerequisite (user namespaces unavailable, or the bus binary absent) and confirm it fails with that prerequisite named instead of skipping. Verify by the recorded exit codes and messages
- [ ] 1.4 Verify the harness needs no `at-spi2-core`: it drives a plain bus (`dbus-daemon` / `dbus-broker-launch`), because neither image `ubuntu-latest` resolves to around the rollout preinstalls at-spi2-core. Verify by running it on a machine with the accessibility packages uninstalled or by inspecting the harness's dependencies

## 2. Spike: measure the hosted runner (throwaway branch, never merged)

- [ ] 2.1 On a branch, add a single workflow job with `runs-on: ubuntu-latest` — the runners the existing Linux jobs use (design.md D3) — and `workflow_dispatch` so it can be re-run on demand. Verify the job appears in the Actions UI and can be dispatched
- [ ] 2.2 First step of that job reports the starting state, before any elevation: the runner image and its version (`$ImageOS`, `$ImageVersion`), `sysctl kernel.apparmor_restrict_unprivileged_userns kernel.apparmor_restrict_unprivileged_unconfined`, `uname -r`, `dpkg -l dbus dbus-broker at-spi2-core`, `id`, and `unshare --user --map-current-user --keep-caps --pid --fork --mount-proc true` with its exit code captured rather than failing the job. Verify by reading these values out of the job log — the sysctl and `unshare` results are the fact no public log currently contains (design.md, Context)
- [ ] 2.3 Apply the primary elevation (`sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`) and re-run the same probe, capturing the exit code again. Verify the log distinguishes the three outcomes: sysctl refused / written but `unshare` still denied / `unshare` works
- [ ] 2.4 Run the harness against the runner's stock bus, started privately by the harness — `just test-atspi-pidns dbus-daemon`, since the recipe takes the daemon as a parameter (design.md D5). Verify the log names, per topology, the daemon and its version and the decided mode with its inputs; that no foreign peer is claimed and our own peer is recognised where the bus identifies it; and that nothing in the job branches on the daemon version — its assertions are the daemon-independent verdicts of design.md D3
- [ ] 2.5 Measure the cost: run the job at least five times and record per-run wall time (total and harness-only), plus any forced-PID retry counts the harness reports and the image each run landed on. Verify by a table of five runs in this change's notes — the "seconds of runtime" in the evidence was an estimate, never clocked (design.md D6.3)
- [ ] 2.6 Only if 2.3 fails: measure the fallback variant — `sudo unshare …` with `setpriv` dropping to uid 1000 inside the namespace, so every D-Bus peer authenticates under one uid (EXTERNAL auth). Verify by 2.4 passing under that variant, and record that this shape had never been run before (design.md D4)

## 3. Runner-image facts that outlive the spike

- [ ] 3.1 Confirm from the job logs what was so far only read from published files: kernel ≥ 6.9 (pidfs) and the installed dbus version, for each image `ubuntu-latest` resolved to during the spike — the announced rollout (2026-10-19 … 11-19) may put runs on both. Verify against the values in design.md's runner table and note any drift

## 4. Decision point

- [ ] 4.1 Read the measurements against the acceptance bar (design.md D6: at most one elevation step, five green runs, single-digit minutes, the verdicts holding on the runner's stock bus with daemon and mode logged per topology) and write the verdict — adopt / adopt partially / keep local — with the deciding criterion named. Verify by the written verdict in this change's notes; it selects group 5 or group 6, and the unselected group's boxes are left unticked with a one-line note saying which outcome retired them

## 5. Adoption path (only if 4.1 says adopt)

- [ ] 5.1 Add the job to `.github/workflows/ci.yml` with `runs-on: ubuntu-latest`, like the other Linux jobs, containing exactly the steps the spike validated (elevation, the harness on the stock bus, mode lines in the log). Verify by a green run on a pull request
- [ ] 5.2 Keep it out of the `needs:` list of `build-wheels` / `build-wheels-java` at first, so a flaky newcomer cannot block a release; state that intent in a comment beside the job. Verify by reading the `needs:` lists after the edit — they stay `[lint, rust, python, acceptance-linux]`
- [ ] 5.3 If the verdict was "adopt partially", name in that comment which topology the job does **not** cover and where that coverage lives instead. Verify by the comment naming the topology and the local recipe
- [ ] 5.4 Re-run the full workflow once on a pull request and confirm total CI wall time moved by no more than the measured amount from 2.5. Verify by comparing the run duration against a recent pre-change run

## 6. Local-only path (only if 4.1 says keep local)

- [ ] 6.1 Delete the spike branch and leave `.github/workflows/ci.yml` untouched. Verify by `git status` on `main` showing no workflow change
- [ ] 6.2 Record the failing criterion and the measurements that produced it, so the question is not re-opened from scratch in six months — including whether a later runner image or a lifted AppArmor default would change the answer. Verify by the recorded note in the documentation task below
- [ ] 6.3 Make the on-demand path discoverable: the `just` recipe from `atspi-process-identity` is named in the documentation entry, next to `just test-java-agent-live` as the existing precedent for an on-demand lane. Verify by the doc naming both

## 7. Documentation

- [ ] 7.1 `dev-docs/testing-strategy.md` §6 (CI): state where the PID-namespace coverage runs — CI job or on-demand local recipe — what CI cannot cover by construction (a pre-6.9 kernel), and what it deliberately leaves to other coverage (the daemon-specific cells: dbus-daemon 1.12/1.14/1.16.2, dbus-broker 29/33/35/37). Point to the unit tests and the local podman matrix that cover them. Verify by reading the section back: normative and durable, no spike numbers, no status report (design.md D9)
- [ ] 7.2 `dev-docs/testing-strategy.md` §8 (Decisions): one row for the PID-namespace tests' placement and its reason. Verify by the row rendering in the existing table
- [ ] 7.3 If adopted: note in §6 that the PID-namespace job runs on the same runners as the other Linux jobs and asserts only verdicts that hold on any bus, so a change of the runners' stock bus changes what it exercises, not what it checks. Verify by the sentence

## 8. Verification and commit

- [ ] 8.1 Re-read the evidence trail: every number in this change's notes is either a job log link, a local run, or a measurement from the PR #5 investigation, and anything still unmeasured says so. Verify by a pass over proposal.md, design.md and the notes
- [ ] 8.2 `openspec validate "evaluate-pidns-tests-in-ci" --type change --strict` passes — the change name is positional in this CLI, `--change` is not an option. Verify by the command's output
- [ ] 8.3 Regression check on the touched surface only — a full CI run on the pull request is green, including the unchanged jobs. Verify by the run's status
- [ ] 8.4 Commit. Conventional Commits, subject ≤ 72 chars: `ci(workflows): gate the PID-namespace identity tests` for the adoption path, or `docs(testing): keep the PID-namespace tests out of CI` for the local-only path (the documentation edit can ride along in either). **No `Co-authored-by` trailer** — settled with the maintainer: the credit for PR #5's author is scoped to changes 1–4, and this change carries none of that work
