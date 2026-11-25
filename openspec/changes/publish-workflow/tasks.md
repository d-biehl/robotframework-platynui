# Tasks

The scripts are ported from RobotCode (`scripts/publish_dev.py`, `scripts/publish.py`, `scripts/extract_release_notes.py` in `~/develop/robot/robotcode`). Reuse their structure, and adapt them as `design.md` describes:
- `uv publish` instead of `hatch publish`;
- wheels only, no sdists;
- five projects instead of twelve;
- no VS Code or JetBrains targets.

The tests live in `tests/publishing/` and import the scripts as `scripts.<module>` (only `update-changelog.py` is loaded by path). They stub `subprocess.run`, `urllib.request.urlopen` and `time`, so they never reach GitHub or PyPI.

## 1. Wheel set and upload order (shared by both jobs)

- [x] 1.1 Write `tests/publishing/test_release_wheels.py` for the scenarios of "Every publish uploads one complete set of one version", using the file names of run 44391763278 (`0.13.0.dev378`, 17 wheels) as fixtures:
  - a complete set;
  - a missing artifact, whose name appears in the error;
  - mixed versions, both of which appear in the error;
  - a missing project;
  - a platform tag missing from one platform project;
  - the split into the four dependency projects and `robotframework-platynui`.

  Verify that the tests fail because the module does not exist.
- [x] 1.2 Write tests for "robotframework-PlatynUI is uploaded after the packages it pins":
  - two `uv publish` calls, dependencies first, both with `--trusted-publishing always` and `--check-url https://pypi.org/simple/`;
  - no root call after a failed dependency call;
  - the scenario "The next upload completes the version", which relies on the repeated calls passing `--check-url`;
  - no credential in any command line.

  Verify that they fail.
- [x] 1.3 Implement `scripts/release_wheels.py`:
  - the constant list of the seven `wheels-*` artifact names, with a comment pointing at `ci.yml`;
  - the set check with `packaging.utils.parse_wheel_filename`;
  - the two-step upload.

  Verify with `uv run pytest tests/publishing -v`, `just ruff` and `just mypy` (strict, through the test imports).

## 2. Nightly dev upload

- [x] 2.1 Write `tests/publishing/test_publish_dev.py` for "A night's dev build comes from the latest run on main":
  - a success writes `run-id=<id>` to `GITHUB_OUTPUT`;
  - a failed latest run writes nothing and exits 0, also when an older run succeeded;
  - a running run is polled until it succeeds;
  - a newer run that appears while waiting replaces the first;
  - the two-hour limit, through a stubbed `time.monotonic`;
  - no run at all;
  - a `gh` failure ends with an error;
  - the `gh run list` call filters on `ci.yml`, `--branch main` and `--event push`.

  Verify that the tests fail.
- [x] 2.2 Write tests for "Dev builds of main are published to PyPI every night":
  - PyPI answers 200 for the root project's version, so nothing is uploaded;
  - a version without a pre-release label is skipped with the log line that its tag publishes it;
  - every artifact is downloaded by name with `gh run download --name`;
  - a new dev version goes through the set check and the two-step upload from group 1.

  Verify that the tests fail.
- [x] 2.3 Implement `scripts/publish_dev.py` (PEP 723 header with `packaging`; targets `latest-run` and `pypi --run-id <id>`), ported from RobotCode's `find_latest_run` and `publish_python`. Verify with `uv run pytest tests/publishing -v`, `just ruff` and `just mypy`.

## 3. Release

- [x] 3.1 Write `tests/publishing/test_publish.py` for "A tag with a dev label is not released" and the version check:
  - `v0.14.0-dev.3` is refused, and the message names the nightly upload;
  - `v0.14.0-dev.0` gives `skip=true` and exits 0;
  - `v1.0.0-rc.1` gives `skip=false`, `is-prerelease=true` and `version=1.0.0-rc.1`;
  - `v1.0.0` gives `skip=false` and `is-prerelease=false`;
  - wheels whose version is not the PEP 440 form of the tag fail the check.

  Verify that the tests fail.
- [x] 3.2 Write `tests/publishing/test_extract_release_notes.py` for "The GitHub release carries the changelog of its version":
  - the `## [1.0.0]` section is extracted up to the next `## ` heading;
  - a section longer than 125,000 characters is shortened to the Features, Bug Fixes and Performance subject lines with their headings, plus a footer with the changelog link passed as an argument;
  - both the `### Features` and the `### <!-- 0 -->Features` heading styles are accepted;
  - a version without a section ends with an error.

  Verify that the tests fail.
- [x] 3.3 Implement:
  - `scripts/publish.py`, with the targets `check-tag <tag>` (writes `skip`, `version` and `is-prerelease` to `GITHUB_OUTPUT`) and `upload <dist>` (set check, version check, two-step upload);
  - `scripts/extract_release_notes.py <version> --changelog-url <url>`.

  Both are PEP 723 scripts. `semantic_version` and `packaging` are their only third-party dependencies (with a mypy override for the untyped `semantic_version`), and neither imports `scripts/tools.py`. Verify with `uv run pytest tests/publishing -v`, `just ruff` and `just mypy`.

- [x] 3.4 Write `tests/publishing/test_update_changelog.py` for "A bump fails when the changelog cannot be written". It loads `scripts/update-changelog.py` by path with a stub `scripts.tools` module in `sys.modules`, because GitPython is not in the dev environment. It stubs `shutil.which` and `subprocess.run` and checks:
  - a missing `git-cliff` exits non-zero and the message names `git-cliff`;
  - a failing `git-cliff` exits non-zero and shows its exit status;
  - a pre-release version runs `git-cliff -o CHANGELOG.md`;
  - a release version runs `git-cliff --bump -t v1.0.0 -o CHANGELOG.md`.

  Verify that the tests of the two failure cases fail before 3.5.
- [x] 3.5 Change `scripts/update-changelog.py` as described in `design.md` ("The changelog step fails loudly"). Verify with:
  - `uv run pytest tests/publishing -v`;
  - `just ruff` and `just mypy`;
  - `env PATH=/usr/bin:/bin uv run scripts/update-changelog.py`, which exits non-zero with the `git-cliff` message;
  - the same with `~/.cargo/bin` on the `PATH`, which rewrites `CHANGELOG.md` (revert that file afterwards).

## 4. Workflow

- [x] 4.1 Write `.github/workflows/publish.yml` following `design.md`:
  - the three triggers, with the schedule in `Europe/Berlin`;
  - the job `dev`, with job-level concurrency and `environment: pypi`. Its steps: `actions/checkout@v6`, `astral-sh/setup-uv@v7`, `latest-run`, then `pypi --run-id` under the `if` from the design. `GH_TOKEN` and `GH_REPO` come from `github.token` and `github.repository`.
  - the job `release`, with the `if` from the design, its permissions, its concurrency group, and these steps:
    1. checkout of `head_sha`;
    2. `check-tag`;
    3. `actions/download-artifact@v8` with `pattern: wheels-*` (one directory per artifact), `run-id` and `github-token`;
    4. release notes, with the changelog link built from `github.server_url` and `github.repository`;
    5. `softprops/action-gh-release@v3`;
    6. `upload` under the `if` from the design.

    Steps 3 to 6 run only with `steps.check.outputs.skip != 'true'`, so a `-dev.0` tag ends the job green after step 2.
  - no repository name and no secret anywhere.

  Verify with `uvx --from actionlint-py actionlint .github/workflows/publish.yml` and `uvx zizmor .github/workflows/publish.yml`. Fix every finding, or justify it in a comment.
- [x] 4.2 Add a comment at the top of `.github/workflows/ci.yml`. It says that `publish.yml` relies on the workflow name `CI`, the file name, the `v*` tag trigger and the seven `wheels-*` artifact names, and that `scripts/release_wheels.py` lists them. Verify that `git diff .github/workflows/ci.yml` shows only comment lines.

## 5. Documentation

- [x] 5.1 Rewrite `CONTRIBUTING.md` §10 "Packaging and release":
  - replace "CI has no publish job";
  - describe the nightly dev upload: which run it takes, when, the version form and the skip rules;
  - describe the release flow: `cz bump <version>`, which needs `git-cliff` on the `PATH` and stops otherwise; after such a stop, `git restore .` and repeat the bump; push `main` and the tag; then the GitHub release and the PyPI upload follow automatically. State the rule "no `-dev.N` tags; dev versions come only from the nightly upload", and that `major_version_zero` becomes `false` in the 1.0.0 release commit;
  - describe how to start a version line: `cz bump X.Y.Z-dev.0`, which the release job skips, and after which the nightlies run as `X.Y.Z.devN`;
  - add the maintainer's one-time setup: the environment `pypi` limited to `main`, a Trusted Publisher for each of the five projects, and both again after the repository moves;
  - describe how to re-run a failed release;
  - describe cleaning up PyPI: delete only older dev versions, never the newest. Explain that PyPI never accepts a deleted file name again, which the rising numbers respect.

  Verify that the section names no repository owner, and that every command in it matches the workflow and the scripts.
- [x] 5.2 Check the README's install section (`README.md`, "Install preview tools"). It must stay correct now that `--pre` installs the newest nightly dev version. Verify that no sentence there promises "the last pre-release" or names a fixed version.

## 6. Verification

- [x] 6.1 Run `just check` and `just test-python`, and verify that both are green, including `tests/publishing`.
- [x] 6.2 Only after the maintainer's explicit go-ahead to commit and push: after the push, check that the CI run on `main` starts a `publish` run whose jobs are both skipped.
- [x] 6.3 Start the 0.14 line, only after the maintainer's explicit go-ahead. Run `cz bump 0.14.0-dev.0 --yes` (signed commit and tag) with `~/.cargo/bin` on the `PATH`, then check:
  - the release commit changed `CHANGELOG.md`;
  - all 21 version places read `0.14.0-dev.0`, and `git grep -n "0.13.0-dev.2" -- ':!CHANGELOG.md' ':!openspec/'` finds nothing;
  - the tag `v0.14.0-dev.0` exists and is signed (`git tag -v v0.14.0-dev.0`).

  Push `main` and the tag only after a separate go-ahead. Then check that the tag's CI run is green, and that its `publish` release job ends green without a GitHub release once `publish.yml` is on `main`.
- [x] 6.4 Once `publish.yml` is on `main` (the environment `pypi` and the five Trusted Publishers were set up on 2026-11-24). If an upload fails with `invalid-publisher`, compare owner, repository, workflow and environment on PyPI with the run's claims, which the error message lists. Then:
  - start `publish` by hand and check that the log names the run it took;
  - check that PyPI has a `0.14.0.devN` version of all five projects, published by `publish.yml`;
  - start it a second time and check that it uploads nothing;
  - check that `uv pip install "robotframework-platynui[all]==<dev version>"` installs that version in a scratch environment.

## Workflow follow-up

- At the first release tag (`v1.0.0` or a release candidate), check the GitHub release (name, notes, pre-release flag, 17 wheels) and PyPI, then archive the change.
