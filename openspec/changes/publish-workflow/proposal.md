# Proposal

## Why

PlatynUI reaches PyPI only when the maintainer uploads it by hand. Each pre-release is a `-dev.N` tag (`v0.13.0-dev.2` is the latest, from 2026-07-14) whose wheels are collected from a CI run and uploaded manually. CI builds all 17 wheels for every push to `main` and then discards them (`.github/workflows/ci.yml`, "CI has no publish job" in `CONTRIBUTING.md` §10). Between two manual uploads, users cannot try a fix. The first stable release, 1.0.0, should come from nothing more than `cz bump` and a pushed tag.

RobotCode solved the same problem with one workflow, `publish.yml`. It publishes the latest green build of `main` as a dev build every night, and after a green tag build it publishes the release. This change ports that model.

## What Changes

- **A new workflow `.github/workflows/publish.yml`, named "publish", with two jobs:**
  - **`dev`**: every night at 02:23 Europe/Berlin, and on a manual start. It takes the latest CI run for a push to `main`. If that run is still running, it waits for it, at most two hours, and follows a newer run that starts meanwhile. If the run succeeded, the job uploads that run's 17 wheels to PyPI as dev versions, for example `0.14.0.dev12`. A red or unfinished run means no dev build that night, and the job still ends green. A version PyPI already has is skipped. A release version is skipped too, because its tag publishes it.
  - **`release`**: after the "CI" run of a version tag pushed to this repository succeeds. It creates the GitHub release with that run's wheels, with the tag's `CHANGELOG.md` section as notes and the pre-release flag for a pre-release version. Then it uploads the same wheels to PyPI. A re-run uploads only what is missing.
- **Upload order:** `platynui-native`, `platynui-cli`, `platynui-inspector` and `platynui-provider-java` go first. `robotframework-PlatynUI` goes last, because it pins them exactly. PyPI therefore never has an installable root version without its dependencies.
- **PyPI login through Trusted Publishing (OIDC).** There is no stored token. The repository has no secrets today. Both jobs run in a GitHub environment `pypi` and request `id-token: write` for themselves only.
- **Versions follow RobotCode's model:**
  - A tag is a release: later `v1.0.0`, or a pre-release such as `v1.0.0-rc.1`.
  - Dev versions come only from the nightly upload, as `<next version>.dev<commits since the last tag>`. `scripts/tools.py` already derives these versions; this change keeps them.
  - A `-dev.0` tag starts a new version line. The release job publishes nothing for it and ends green, and the nightly upload publishes `X.Y.Z.dev0` and its successors.
  - The release job refuses every other `dev` tag, for example `v0.14.0-dev.3`. After such a tag, the nightly numbers would start again at `.dev1` and collide with dev versions that PyPI already has or once had. PyPI never accepts a deleted file name again.
- **0.13 is skipped.** From the latest tag `v0.13.0-dev.2`, every derived version is `0.13.0-dev.N`. The maintainer therefore starts the new scheme with `cz bump 0.14.0-dev.0`. That bump:
  - writes `0.14.0-dev.0` into the 21 version places (Cargo, pyproject, `gradle.properties`, `__version__.py`, exact pins);
  - regenerates the changelog and the lock files;
  - creates the signed tag `v0.14.0-dev.0`.

  The nightlies then run as `0.14.0.dev0`, `0.14.0.dev1` and on. These rank above every 0.13 version, also above dev versions deleted from PyPI.
- **New scripts** `scripts/publish_dev.py` (targets `latest-run` and `pypi`) and `scripts/publish.py` (release upload), and a release-notes extractor. They are ported from RobotCode, run with `uv run` and are covered by pytest tests with a stubbed `gh` and stubbed uploads.
- **A bump fails when the changelog cannot be written.** `scripts/update-changelog.py`, which `cz bump` runs as a pre-bump hook, ignores the exit status of `git-cliff` and keeps going when `git-cliff` is missing. The bumps to `0.13.0-dev.1` and `0.13.0-dev.2` left `CHANGELOG.md` untouched, last changed on 2026-03-11, most likely for this reason. Without the changelog section of a release, the release job has no notes. The script now fails with a message naming the cause, so `cz bump` stops before the commit and the tag.
- **`CONTRIBUTING.md` §10** describes the new flow:
  - the nightly dev builds;
  - releasing with `cz bump <version>` and a pushed tag;
  - "no `-dev.N` tags, except `-dev.0` to start a version line";
  - the one-time Trusted Publisher setup on PyPI;
  - deleting old dev versions from PyPI to stay under the size limit, never the newest one.

Behavior changes that users notice:
- `pip install --pre <package>` and `uv … --prerelease allow`, as the README shows them, now install the newest nightly dev version instead of the last manual pre-release. Until 1.0.0 every PlatynUI version is a pre-release, so this is the README's documented path.
- `pip install robotframework-platynui` without `--pre` is unchanged: it still installs the newest final version on PyPI, which comes from the old .NET prototype.

Not part of this change:
- a user-docs page about the dev builds;
- deleting old dev versions from PyPI automatically: PyPI offers no API for it, so it stays manual;
- making the bump itself automatic: `cz bump` stays the maintainer's step.

## Capabilities

### New Capabilities

- `release-publishing`: what CI publishes as nightly dev builds and as releases, where, from which build, in which order, and how a failed or repeated publish behaves.

### Modified Capabilities

_None._

## Impact

- **Workflows:**
  - `.github/workflows/publish.yml` is new.
  - `.github/workflows/ci.yml` stays as it is. Its name "CI", its triggers (pushes to `main` and `v*` tags) and its seven `wheels-*` artifacts are the interface `publish.yml` relies on. Renaming the workflow or an artifact breaks the publishing.
- **Scripts:** `scripts/publish_dev.py`, `scripts/publish.py` and `scripts/extract_release_notes.py` are new. They are PEP 723 scripts, so CI needs no `just bootstrap` for them. `scripts/update-changelog.py` fails on a missing or failing `git-cliff`, which also stops the CI wheel jobs if `git-cliff` is not installed there; today all of them install it. `scripts/tools.py` and `scripts/update-git-versions.py` are unchanged.
- **Tests:** new pytest modules under `tests/publishing/`, run by `just test-python`.
- **Versions:** the start bump `cz bump 0.14.0-dev.0` changes the 21 version places, `CHANGELOG.md` and the lock files in one release commit. It is a maintainer step and is listed in the tasks.
- **Docs:** `CONTRIBUTING.md` §10 and the `AGENTS.md` command list, if a `just` recipe is added.
- **External setup by the maintainer**, which cannot be done from the repository:
  - a GitHub environment `pypi`, limited to `main`;
  - a Trusted Publisher on PyPI for each of the five projects, naming owner, repository, `publish.yml` and `pypi`;
  - the same setup again after the repository moves to `imbus`.
- **PyPI volume:**
  - about 100 MB per nightly dev build, 58 MB of it `platynui-inspector` (5 wheels);
  - at the 96 days with commits on `main` in the past year, that is about 5.6 GB a year for `platynui-inspector`;
  - PyPI's default limit is 10 GB per project.
- **No Rust, Python-library or native-module change**, so no native rebuild and no platform or provider behavior change.
