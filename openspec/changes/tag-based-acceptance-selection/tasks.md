# Tasks: Tag-Based Acceptance Lane Selection

## 1. Lane profiles in robot.toml (selection mechanics first)

- [x] 1.1 Add `real-x11`, `real-wayland`, `real-windows` profiles to `robot.toml` per design D2 (`inherits = ["real"]`, `excludes` of the two foreign `platform:*` tags each); update the file's header comment to describe the tag vocabulary
- [x] 1.2 Verify inheritance semantics before anything depends on them: `uv run --no-sync robotcode --profile real-x11 discover tests` must list exactly the `real` selection (no tags exist yet, so all three lane profiles must equal `real`); if `inherits` drops `paths`/`includes`, fall back to repeating them per profile (design risk 2)

## 2. Tag the platform-bound suites and tests (spec: acceptance-lane-selection)

- [x] 2.1 `tests/acceptance/egui/config_display.robot`: add `platform:x11` to `Test Tags`, remove the `Skip If '%{XDG_SESSION_TYPE=}' != 'x11'` guard (line 44) and rewrite the suite documentation that explains the skip
- [x] 2.2 `tests/acceptance/egui/coexisting_runtimes.robot`: same — tag `platform:x11`, drop the guard (line 54), fix the suite docs
- [x] 2.3 `tests/acceptance/egui/inspector_window_controls.robot`: add `[Tags]    platform:wayland` to the Wayland-only test and remove its inline `Skip If` (line 150)
- [x] 2.4 `tests/acceptance/swing/__init__.robot`: add `platform:windows` to `Test Tags` (inherited by all swing suites); update its "every suite skips" documentation
- [x] 2.5 Prove the selection with discovery (per-scenario acceptance check, no session needed): compare `robotcode --profile real-wayland discover tests` vs `real-x11` vs `real` — X11-only suites absent from the Wayland profile, the Wayland-only test absent from X11, swing absent from both Linux profiles, untagged suites present everywhere

## 3. Lane entry points pick the matching profile

- [x] 3.1 `scripts/platynui-robot-session.sh`: derive the default profile from `XDG_SESSION_TYPE` (`wayland` → `real-wayland`, `x11` → `real-x11`, else warn on stderr and use `real`); keep explicit args as full override; update the header comment's example commands
- [x] 3.2 `justfile` `test-acceptance-windows`: default robotcode invocation becomes `--profile real-windows run`

## 4. Swing prerequisites: skip → hard fail (spec: swing-test-app delta)

- [x] 4.1 `tests/acceptance/swing/resources/testapp.resource` `Require Swing Prerequisites`: delete the `os.name != "nt"` skip; convert the launcher and classes checks from `Skip If` to hard failures keeping the actionable `just build-test-app-swing` messages; update keyword docs
- [x] 4.2 `justfile` `test-acceptance-windows`: remove the `-` soft-fail prefix from `build-test-app-swing` and drop the `Test-Path`-guarded warning branch so a failed fixture build fails the lane before Robot starts (design D4)

## 5. Documentation (dev-docs/testing-strategy.md is the normative home)

- [x] 5.1 Extend `dev-docs/testing-strategy.md`: complete tag vocabulary (`mock`, `real`, `platform:*` semantics incl. "no tag = every lane"), the profile table (`mock`, `real`, `real-{x11,wayland,windows}`) with which recipe/script selects which, and the rule that environment fitness is selection — runtime-unknowable prerequisites fail, never skip; update §2.6, §5, and the §8 quick-reference accordingly (normative rules, not current-state inventory)
- [x] 5.2 Sweep stale skip references: justfile comments (`test-acceptance-windows`), `robot.toml` header, `CONTRIBUTING.md` if it describes the soft-skip behavior

## 6. Verification

- [x] 6.1 `just check` (robocop/ruff/mypy gate over the edited `.robot`/`.resource`/toml files)
- [x] 6.2 Linux lanes green with zero environment skips in the reports: `just test-acceptance-compositor` and `just test-acceptance-x11`, results checked via `robotcode results` (X11-only suites must actually *run* on the X11 lane — they were the skip-prone ones)
  - Re-verified after the interim QML fixture commit (`e559e35`): selection stays correct (104 → 74/75/93 per lane) with zero skips; the 17 QML tests fail on both Linux lanes with a pre-existing issue (fixture window never appears on the AT-SPI tree — `add-qml-test-app`'s open Linux verification; main CI has been red since that commit, independent of this change). All non-QML tests pass.
- [ ] 6.3 On Windows (real-provider only; the lane runs manually today, its CI job is planned separately): `just test-acceptance-windows` runs swing suites via `real-windows`; spot-check the fail-not-skip path by invoking `robotcode --profile real-windows run` without the fixture env vars and confirming the actionable failure
