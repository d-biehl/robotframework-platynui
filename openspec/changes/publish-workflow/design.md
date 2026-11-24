# Design

## Context

See proposal.md for the motivation, and `specs/release-publishing/spec.md` for the behavior. The state that shapes the approach was checked on 2026-11-24.

**What `publish.yml` relies on in the CI workflow (`.github/workflows/ci.yml`):**
- **Name and triggers:** it is named `CI` (`ci.yml:1`) and runs for pushes to `main` and `new_core`, for `v*` tags, for pull requests and on manual starts (`ci.yml:3-9`).
- **Concurrency:** `concurrency` per ref with `cancel-in-progress: true` (`ci.yml:14-16`). On `main`, a newer push cancels the older run.
- **Wheel artifacts:** the wheel jobs run only after every test job (`needs:` at `ci.yml:391, 450, 508, 596`). Each uploads its wheels with `if-no-files-found: error`. The seven artifacts are `wheels-pure-python`, `wheels-provider-java`, `wheels-linux-{x86_64,aarch64}` and `wheels-{macos-latest,windows-latest,windows-11-arm}`.
- **Versions:** each wheel job first runs `scripts/update-git-versions.py`, so all wheels of one run carry the same version.
- **Contents of a green run:** the last green run on `main` (44391763278, commit `aca3bbfc`) has 17 wheels, all `0.13.0.dev378`:
  - 5 each of `platynui_native` (`cp312-abi3`), `platynui_cli` and `platynui_inspector` (`py3-none-<platform>`);
  - 1 each of `platynui_provider_java` and `robotframework_platynui` (`py3-none-any`).
  - Sizes: native 20.6 MB, CLI 21.7 MB, Inspector 58.2 MB, the two pure wheels 0.1 MB each.

**Version derivation (verified):**
- `scripts/tools.py:89-116` derives the version from `git describe --tags --long --first-parent --match 'v[0-9]*'`. A tagged commit gets the tag's version. A later commit gets `<next version>-dev.<commits since the tag>`, with the next version from the conventional commits since the tag, capped at a minor bump while on 0.x.
- From the latest tag `v0.13.0-dev.2`, `next_minor()` and `next_patch()` both give `0.13.0`, so `main` builds `0.13.0-dev.<N>`. PEP 440 normalizes these to `0.13.0.devN`.
- With `packaging`, `0.13.0.dev378 > 0.13.0.dev3`. That is why a further `-dev.N` tag would rank below nightlies that are already published.
- After such a tag, the count would start again at 1 and produce `0.13.0.dev1`, which already exists as the release of `v0.13.0-dev.1`.
- From a tag `v0.14.0-dev.0`, `next_minor()` and `next_patch()` both give `0.14.0`, so the next builds are `0.14.0-dev.N`, with `N` counted from that tag. With `packaging`:
  - `0.14.0.dev0` ranks above `0.13.0.dev999`;
  - `0.14.0.dev0 < 0.14.0.dev1 < 0.14.0rc1 < 0.14.0`.

**Version places:** `0.13.0-dev.2` from the last bump appears in 21 files: the Cargo manifests, the `pyproject.toml` files, `java/agent/gradle.properties`, the `__version__.py` files and the exact pins. CI overwrites them per build. The committed value only changes with a bump.

**PyPI history:**
- On 2026-11-24 the maintainer deleted the earlier manual dev releases from PyPI, except the last two.
- PyPI never accepts a file name again, even after the file or the whole project was deleted. The Warehouse docs say so under "Filename has been previously used" (via Context7).
- Every deleted version number is therefore used up for good.

**Manifests and exact pins:** `robotframework-PlatynUI` pins `platynui-native` exactly, and pins the packages behind its extras the same way (`pyproject.toml:32, 39-47`). `update-git-versions.py` rewrites these pins with every version.

**Commitizen:**
- `[tool.commitizen]` (`pyproject.toml:104-122`) has `version_provider = "scm"`, `tag_format = "v$version"`, `major_version_zero = true` and signed annotated tags.
- Its `pre_bump_hooks` write the version into the manifests, regenerate `CHANGELOG.md` through git-cliff and refresh the lock files.
- `cz bump <version>` thus produces the release commit and its tag, and that commit carries a `## [<version>]` changelog section.

**Changelog generation:**
- `scripts/update-changelog.py` runs `git-cliff -o CHANGELOG.md` for a pre-release. For a release it runs `git-cliff --bump -t v<version> -o CHANGELOG.md`.
- It calls `subprocess.run(..., shell=True)` without `check=True` inside a `run()` helper that only prints exceptions. A missing `git-cliff` or a failing run is therefore printed and ignored.
- `cliff.toml` writes release headings as `## [<version>](<compare link>) - <date>`. It ignores `alpha`, `beta` and `dev` tags (`ignore_tags`), so pre-release history collects under `## [Unreleased]` until the next release.
- Evidence of the silent failure:
  - `CHANGELOG.md` was last changed by the bump to `0.12.0-dev.7` on 2026-03-11, and not by the bumps to `0.13.0-dev.1` and `0.13.0-dev.2` in July.
  - On the maintainer's machine, `git-cliff` 2.13.1 is in `~/.cargo/bin`, which is on the `PATH` of neither zsh nor `fish -c`.
- All CI wheel jobs install `git-cliff` through `taiki-e/install-action`.

**Repository facts:**
- The repository is public and has no Actions secrets.
- `origin` is `d-biehl/robotframework-platynui`, and it will move to `imbus` later. Nothing in the workflow may name the repository; `github.repository` is the source.
- Of the 365 days before 2026-11-24, 96 had commits on `main`.

**PyPI and uv (from their docs via Context7):**
- **Trusted Publishing for GitHub:**
  - It matches owner, repository and workflow file name, and optionally a GitHub environment.
  - PyPI forbids it only for the triggers `pull_request_target` and `issue_comment`, so `schedule` and `workflow_run` are allowed.
  - Fork pull requests cannot get an OIDC token.
- **`uv publish`:**
  - It supports `--trusted-publishing always`.
  - On PyPI it succeeds without error for a file that already exists with identical content.
  - `--check-url <index>` skips such files before the upload.

**Lint scope:** `scripts/` is outside ruff's `include` and in mypy's `exclude` (`pyproject.toml:257-272`). `tests/` is in both, and mypy follows imports from tests into the modules they load.

## Goals / Non-Goals

**Goals:**
- The behavior of RobotCode's `publish.yml`, adapted to wheels-only Python packages and to PlatynUI's `just`/`uv` tooling.
- Publishing works without any stored credential.
- The decision logic (which run, which files, which order, skip or refuse) is testable with pytest.

**Non-Goals:**
- Changing `ci.yml`, beyond keeping its names stable.
- Building anything in `publish.yml`. It only moves files that CI built and tested.
- Signing or attesting the wheels beyond what PyPI's Trusted Publishing records.

## Decisions

### One workflow `publish.yml` with the jobs `dev` and `release`

This is the structure of RobotCode's `publish.yml`.

**Triggers:**
- `schedule` at `23 2 * * *` with `timezone: "Europe/Berlin"`;
- `workflow_dispatch`;
- `workflow_run` for the workflow `CI`, with `types: [completed]`.

**Jobs:**
- `dev` runs when the event is not `workflow_run`. It has its own concurrency group `publish-dev` with `cancel-in-progress: false`. That group is at job level, so that the `workflow_run` run that every CI run starts cannot replace a pending nightly run.
- `release` runs only for a successful `workflow_run` of a version tag of this repository. It has the same `if` as RobotCode:
  - the conclusion is `success`;
  - the event is `push`;
  - `head_branch` starts with `v`;
  - `head_repository.full_name == github.repository`.

  CI runs for pushes only on `main`, `new_core` and `v*` tags, so a push run whose branch starts with `v` is a tag. Its concurrency group is `publish-release-<tag>`.

**Permissions:**
- At workflow level, `actions: read` and `contents: read`.
- `release` adds `contents: write` for the GitHub release.
- Both jobs add `id-token: write` and run in `environment: pypi`.

**Alternatives:**
- **A publish job inside `ci.yml`.** Trusted Publishing would then trust the workflow that also runs every pull request's code. It would also publish on every push instead of once a night.
- **Separate workflows for nightly and release.** Every Trusted Publisher would need two entries, and the shared upload rules would drift apart.

### The nightly upload takes the newest run on `main`, never an older green one

`scripts/publish_dev.py latest-run` is ported from RobotCode:
1. It lists the newest run with `gh run list --workflow ci.yml --branch main --event push --limit 1`.
2. While that run is not `completed`, it polls every 60 seconds, for at most two hours in all. A run that a newer push starts meanwhile replaces it.
3. On `success` it writes `run-id=<id>` to `GITHUB_OUTPUT`. Otherwise it only logs why there is no dev build.
4. It fails only when `gh` itself fails.

The `--branch main` filter leaves out `new_core` and tag runs. The upload step runs only with `if: ${{ !cancelled() && steps.build.outputs.run-id != '' }}`.

Why two hours: CI took about 20 minutes on 2026-11-18, and RobotCode chose two hours for its own builds of up to 110 minutes. The limit is a module constant.

Alternative: the most recent successful run. It would publish half-finished feature states while `main` is red. RobotCode's maintainer ruled this out on 2026-10-19, for the same reason here.

### One complete set per version, checked before any upload

A shared module collects the wheels and refuses incomplete or mixed sets. `publish_dev.py` uses it for the run it downloads, and `publish.py` for the artifacts the release job downloaded. The module does the following:
- It downloads every artifact named in a constant list of the seven `wheels-*` names. The nightly path uses `gh run download <id> --name <artifact>` per name, so that a missing artifact is an error that names it. The release job downloads them through `actions/download-artifact` with `pattern: wheels-*` and without `merge-multiple`, so that each artifact lands in a directory of its own name. The script then checks for the same names. Both paths leave `dist/<artifact>/*.whl`.
- It parses every wheel file name with `packaging.utils.parse_wheel_filename`. It requires exactly one version and all five projects, and it requires that the three platform projects (native, CLI, Inspector) have the same set of platform tags.
- It splits the files into "dependencies" (the four `platynui-*` projects) and "root" (`robotframework-platynui`).

The list of artifact names is the explicit interface to `ci.yml`. A new platform there needs a line here, and a forgotten line fails the set check, not the users.

Alternative: a fixed count of 17. It hides which part is missing, and it breaks silently when a platform is added and another removed.

### Upload with `uv publish`, dependencies first, root last

Each of the two groups is one call:

```
uv publish --trusted-publishing always --check-url https://pypi.org/simple/ <files>
```

The root call runs only after the dependency call succeeded.

**Nightly path:** before any upload, the script asks `https://pypi.org/pypi/robotframework-platynui/<version>/json`. A 200 means the version is complete, because the root project goes last, so the script ends without uploading. A version without a pre-release label is skipped with a log line that the tag publishes it.

**Release path:**
- There is no "complete" shortcut. `--check-url` makes a re-run upload only the missing files.
- The files are byte-identical across re-runs, because they come from the same CI run's artifacts.

`uv` comes from `astral-sh/setup-uv`. The scripts are PEP 723 scripts with `packaging` as their only dependency. CI needs no `just bootstrap`, no Rust and no JDK in `publish.yml`.

**Alternatives:**
- **`pypa/gh-action-pypi-publish`.** It uploads one directory in an order we do not control, and it would need a staging directory per group. It also has no "already complete" check for the nightly path.
- **`twine`.** It is one more tool, where `uv` is already the project's installer and is used throughout CI.

### Versions stay as `scripts/tools.py` derives them; `-dev.0` starts a line, other dev tags are refused

`tools.py` already yields RobotCode's scheme: a release for a tagged commit, `<next>.devN` after it. The rule that makes the numbers rise strictly is "tags are releases, or the start of a version line". The release job enforces it:
- `publish.py check-tag <tag>` parses the tag (without the `v`) with `semantic_version`, as RobotCode does, and requires a PEP 440 form of it. `semantic_version` ships no type information, so `pyproject.toml` has a mypy override with `ignore_missing_imports` for it.
- `dev` as the first label with the number `0` writes `skip=true` to `GITHUB_OUTPUT` and exits 0. Every later step of the release job runs only when `steps.check.outputs.skip != 'true'`, so this tag publishes nothing and the job stays green. The nightly upload publishes the tagged commit's `X.Y.Z.dev0` like any other dev build.
- `dev` with any other number fails the job before the GitHub release step, with a message that dev versions come from the nightly upload.
- Other labels (`rc`, `alpha`, `beta`) and releases write `skip=false`, `is-prerelease=true|false` and `version=<semver>` for the following steps.

**Why a start tag and not another mechanism:**
- The next version follows the last tag. From `v0.13.0-dev.2` every build is `0.13.0-dev.N`, and that range holds numbers PyPI has already used.
- A start tag `v0.14.0-dev.0` moves the base to 0.14 and resets the count. It is made with the maintainer's usual `cz bump 0.14.0-dev.0`, which writes all 21 version places, the changelog and the lock files and signs the tag.
- The same mechanism opens later lines too, for example `v2.0.0-dev.0`.

**Alternative:** a version floor read from the committed manifests, so that a manifest edit without a tag moves the base. This would make the committed version an input to CI. In RobotCode's model, the committed version is only the record of the last bump, and two sources of truth for the base would have to be kept in step.

The script also checks that the downloaded wheels carry the PEP 440 form of the tag's version. A tag on a commit whose build derived another version thus publishes nothing.

Releasing 1.0.0 is then the maintainer's `cz bump 1.0.0`, followed by pushing `main` and the tag. `major_version_zero` has to become `false` in that release commit, or later bumps keep capping breaking changes at a minor bump. This goes into `CONTRIBUTING.md`, not into code.

**Alternatives:**
- **Continuing the dev number across dev tags** (`v0.13.0-dev.2` + 378 → `dev380`, with every dev tag taking the derived number). It keeps manual dev tags possible, but it binds every tag to a number CI computes. The maintainer chose RobotCode's model instead.
- **Releasing dev tags anyway.** This breaks the nightly upload on the first night after the tag (see Context).
- **Failing on `-dev.0` too, and pushing the start tag before `publish.yml` exists.** This works only once, depends on timing (the tag's CI run must finish before `publish.yml` lands on `main`), and leaves no way to open a later line.

### GitHub release from the tag's commit

The release job:
1. checks out `head_sha`. It needs no history: the version comes from the tag name and the notes from the committed `CHANGELOG.md`;
2. runs `uv run scripts/extract_release_notes.py <version>`;
3. creates the release with `softprops/action-gh-release`: `tag_name` is `head_branch` (the job's own ref is `main`), the name is `v<version>`, `prerelease` comes from the check step, the body is the extracted notes, and the files are the 17 wheels.

The extractor is ported from RobotCode:
- It takes the `## [<version>]` section of `CHANGELOG.md`, up to the next `## ` heading or the end of the file. For a pre-release it takes `## [Unreleased]`: `cz bump` runs the changelog hook before it creates the tag, and `update-changelog.py` passes git-cliff no tag for a pre-release, so the pre-release's changes are under `[Unreleased]` in its commit.
- Above 125,000 characters, it shortens the section to the subject lines of Features, Bug Fixes and Performance, with their group and scope headings, and appends a footer that links to `CHANGELOG.md` on `github.server_url/github.repository`. The link is passed in by the workflow and is not written into the script.
- PlatynUI's `cliff.toml` writes the groups as `### Features`, with no ordering prefix. The parser accepts both forms.

The upload step runs with `if: ${{ !cancelled() && steps.check.outcome == 'success' && steps.check.outputs.skip != 'true' && steps.download.outcome == 'success' }}`. It therefore also runs when only the GitHub release failed. A failed GitHub release still fails the job, through that step's own status. On a re-run, the action updates the existing release and replaces files of the same name.

### The changelog step fails loudly

`update-changelog.py`:
- finds `git-cliff` with `shutil.which`;
- runs it with an argument list and `check=True`, without a shell;
- exits non-zero with a message naming the cause when `git-cliff` is missing or fails.

A failing pre-bump hook ends `cz bump` with `RunHookError` (exit code 26). The hooks run after the version files are updated and before the commit and the tag, according to the commitizen docs via Context7. The release commit can thus no longer lack its `## [<version>]` section, which the release job's notes depend on.

The earlier hook `update-git-versions.py` has already rewritten the version places when the changelog hook fails. After the abort, the maintainer resets the working tree with `git restore .` and runs the bump again with `git-cliff` on the `PATH`. `CONTRIBUTING.md` says so.

The CI wheel jobs run the same script. They install `git-cliff` today; a job that loses it would now fail instead of building wheels with a stale changelog.

**Alternatives:**
- **Checking for the section in the release job only.** The tag would already be pushed and signed when the gap shows.
- **A separate check before `cz bump`.** It is easy to skip. The hook runs on every bump anyway.

### Actions pinned to commit hashes in `publish.yml`

`publish.yml` pins every action to the commit of its release, with the version as a comment (`# v6.1.0`). `ci.yml` keeps its tags.

`publish.yml` is the only workflow that can get a PyPI token. A moved tag of a third-party action would run with that identity, and zizmor reports unpinned actions there as high findings.

The pins are updated by hand. There is no Dependabot configuration in the repository.

### Environment `pypi` limited to `main`

- Both jobs use `environment: pypi`, and every Trusted Publisher names it.
- Its deployment branch rule allows only `main`. GitHub runs `schedule` and `workflow_run` on the default branch, so both jobs qualify, and a workflow on any other ref cannot get a PyPI token.
- The environment has no required reviewers, because the nightly job must run unattended.

This is maintainer setup outside the repository. `CONTRIBUTING.md` lists it.

### Tests

The tests live in `tests/publishing/`. Their `conftest.py` puts the repository root on `sys.path`, so they import the scripts as `scripts.<module>`, and mypy follows these imports and checks the scripts. Only `update-changelog.py`, whose name is no module name, is loaded by path with `importlib.util.spec_from_file_location`, with a stand-in for `scripts.tools`. The test directory is not named `scripts`, which would shadow the package.

They stub `subprocess.run` (for `gh` and `uv publish`), `urllib.request.urlopen` (the PyPI lookup) and `time.monotonic`/`time.sleep`. With these stubs they cover each spec scenario that does not need PyPI or GitHub:
- run selection, waiting and the timeout;
- the set check;
- the upload order, and the stop before the root after a failed dependency;
- the skips for complete versions and releases;
- the skip for a `-dev.0` start tag, the refusal of other dev tags, and pre-release detection;
- notes extraction and shortening.

mypy checks the scripts through these imports, so they are written for `strict`.

## Risks / Trade-offs

- **[PyPI's 10 GB project limit]**
  - `platynui-inspector` grows by about 58 MB per nightly dev build. At about 96 build days a year that is roughly 5.6 GB a year, so the limit is reached after about 1.5 to 2 years.
  - → Dev builds are skipped when nothing changed, and when `main` is red.
  - → Old dev versions can be deleted on PyPI by hand, as the maintainer did on 2026-11-24.
  - → RobotCode's research found that PyPI is reluctant to raise limits for nightly builds. If that becomes pressing, a follow-up can move the nightlies to a separate index.
- **[`publish.yml` runs from the default branch]** → Changes to it can only be tried on `main`. The first real release run is the first `v*` tag. A failed upload is re-run after fixing its cause, and `--check-url` uploads only the missing files. A fix in the workflow itself does not reach a re-run, which uses the original workflow file. Then the maintainer uploads the rest by hand from the GitHub release.
- **[Nightlies after a release candidate rank below it]** → After `v1.0.0-rc.1`, `tools.py` derives `1.0.0-dev.N`, and PEP 440 orders `1.0.0.devN < 1.0.0rc1`. Users with `--pre` keep getting the rc until 1.0.0. This is accepted: release candidates are short-lived.
- **[Deleting the newest dev version]**
  - The nightly decides by asking PyPI for the root project's version, and a deleted version answers 404. If the maintainer deletes the version that the latest run on `main` built, every night retries it, and PyPI refuses it with "Filename has been previously used". The nightly stays red until the next commit on `main`.
  - → `CONTRIBUTING.md` says to delete only older dev versions, never the newest.
  - → Deleting older ones is safe: the numbers only rise, so no build ever needs a deleted name again.
- **[A dev tag other than `-dev.0` is pushed anyway]** → The release job fails at once and names the rule. Nightlies after such a tag would restart at `.dev1` and collide with used file names. The maintainer deletes the tag and pushes a `-dev.0` tag of a higher version instead.
- **[The release commit on `main` and the tag are pushed separately]** → If `main` is pushed before the tag, its run builds `<previous base>-dev.N+1` with the content of the release, and a nightly may publish it. That is harmless: it ranks below the release.
- **[The repository moves to `imbus`]** → The five Trusted Publishers and the environment have to be set up again for the new owner. Until then, every upload fails visibly, and no wrong upload happens. `CONTRIBUTING.md` names this step.
- **[A rename in `ci.yml`]** (workflow name, file name or an artifact) → It breaks `publish.yml`. The artifact list and the workflow names are constants with a comment that points at `ci.yml`, and `ci.yml` gets a matching comment.
- **[Nightly runs are delayed by GitHub under load]** → The schedule is at :23 and not on the full hour. A delay only shifts the upload.

## Migration Plan

The change is additive. It adds no Rust and no native-module change, so no native rebuild, and nothing in the libraries changes. The start bump changes only version strings, the changelog and the lock files.

1. Merge the change. The first CI run on `main` starts a `publish` run whose jobs are all skipped. This checks that the `workflow_run` trigger and the `if` conditions are sound.
2. The maintainer does the one-time setup:
   - creates the environment `pypi` (deployment branches: `main`). This was done on 2026-11-24 with `gh api`: custom branch policy `main`, no reviewers, no secrets;
   - on PyPI, for each of the five projects, adds a Trusted Publisher with the owner, the repository, the workflow `publish.yml` and the environment `pypi`. The maintainer did this on 2026-11-24. It can only be confirmed by the first upload, because PyPI shows the configuration only to the project's owners.
3. The maintainer starts the 0.14 line with `cz bump 0.14.0-dev.0` and pushes `main` and the tag. It does not matter whether this happens before or after `publish.yml` is on `main`: the release job skips a `-dev.0` tag either way.
4. The maintainer starts `publish` by hand and checks:
   - the dev job names the run it took;
   - all five projects have a `0.14.0.devN` version on PyPI, uploaded by `publish.yml`;
   - a second start uploads nothing;
   - `pip install "robotframework-platynui[all]==<dev version>"` installs in a scratch environment.
5. At the first release tag, the maintainer checks the GitHub release and PyPI.

**Rollback:** delete `publish.yml`, or disable the workflow in GitHub. Published versions stay on PyPI, and the next manual or tagged release works as before this change.
