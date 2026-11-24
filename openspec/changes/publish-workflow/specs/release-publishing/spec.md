# Spec Delta

## Purpose

Defines what CI publishes as nightly dev builds of `main` and as releases of version tags, where it publishes them, from which build and in which order, and how a failed or repeated publish behaves. The goal is that every release and dev build reaches PyPI without a manual upload.

## ADDED Requirements

### Requirement: Dev builds of main are published to PyPI every night

Every night, and when the maintainer starts it by hand, CI SHALL upload the wheels of the latest CI run for a push to `main` to PyPI as a dev version. It SHALL do so only when that run succeeded and PyPI does not have that version yet. A run that built a version without a `dev` label SHALL NOT be uploaded this way, because its tag publishes it.

#### Scenario: New dev build

- **GIVEN** the latest CI run for a push to `main` succeeded and built the wheels of version `0.14.0.dev12`, and PyPI does not have that version of `robotframework-PlatynUI`
- **WHEN** the nightly upload runs
- **THEN** PyPI SHALL have version `0.14.0.dev12` of all five projects, with all 17 wheels of that run
- **NOTE:** The decision and the file selection are verified by pytest with a stubbed `gh` and stubbed uploads. The upload itself is verified once by a manual start of the workflow.

#### Scenario: Nothing new since the last upload

- **GIVEN** PyPI already has the version that the latest successful run on `main` built
- **WHEN** the nightly upload runs
- **THEN** it SHALL upload nothing, say so in its log, and succeed

#### Scenario: Release commit on main

- **GIVEN** the latest run on `main` built the release version `1.0.0` from the commit that the tag `v1.0.0` points to
- **WHEN** the nightly upload runs
- **THEN** it SHALL upload nothing and say that the tag publishes this version

#### Scenario: Push to main

- **GIVEN** a push to `main`
- **WHEN** its CI run succeeds
- **THEN** that run SHALL upload nothing itself, and the next nightly upload publishes its wheels if it is still the latest run on `main`

### Requirement: A night's dev build comes from the latest run on main

The nightly upload SHALL take the newest CI run for a push to `main`. It SHALL wait up to two hours while that run is not done, and SHALL follow a newer run that starts meanwhile. If the run does not succeed, or is not done in time, it SHALL publish nothing and SHALL still succeed. It SHALL never fall back to an older successful run.

#### Scenario: Broken latest build

- **GIVEN** the latest run on `main` failed, and an earlier run succeeded
- **WHEN** the nightly upload runs
- **THEN** it SHALL publish nothing, name the failed run in its log, and succeed

#### Scenario: Build still running

- **GIVEN** the latest run on `main` is still running when the upload starts
- **WHEN** that run succeeds an hour later
- **THEN** the upload SHALL publish that run's wheels

#### Scenario: Newer build while waiting

- **GIVEN** the upload waits for the latest run on `main`
- **WHEN** a push to `main` starts a newer run before the first one is done
- **THEN** the upload SHALL wait for the newer run instead, and publish its wheels only if it succeeds

#### Scenario: Build not done in time

- **GIVEN** the latest run on `main` is still running two hours after the upload started
- **WHEN** the wait ends
- **THEN** the upload SHALL publish nothing, say so in its log, and succeed

#### Scenario: No run on main yet

- **GIVEN** the CI workflow has no run for a push to `main`
- **WHEN** the nightly upload runs
- **THEN** it SHALL publish nothing and succeed

### Requirement: Every publish uploads one complete set of one version

A dev build or a release SHALL be uploaded only as the complete set that one CI run built: the wheels of all its wheel artifacts (today seven artifacts with 17 wheels), covering all five projects and carrying one version. If an artifact or project is missing, or versions are mixed, CI SHALL upload nothing and fail the run with a message naming the problem.

#### Scenario: Missing wheel artifact

- **GIVEN** a successful run whose artifacts lack the wheels of one platform
- **WHEN** the upload prepares that run's wheels
- **THEN** it SHALL upload nothing and fail with a message naming the missing artifact

#### Scenario: Mixed versions

- **GIVEN** a download directory with a wheel of `0.14.0.dev11` next to the wheels of `0.14.0.dev12`
- **WHEN** the upload prepares the wheels
- **THEN** it SHALL upload nothing and fail with a message naming both versions

### Requirement: robotframework-PlatynUI is uploaded after the packages it pins

The upload SHALL first upload the wheels of `platynui-native`, `platynui-cli`, `platynui-inspector` and `platynui-provider-java`. It SHALL upload `robotframework-PlatynUI` only after all of these succeeded, so that PyPI never offers a version of it whose exact dependency pins cannot be met.

#### Scenario: Failed upload of a dependency

- **GIVEN** an upload of `0.14.0.dev12`
- **WHEN** the upload of the `platynui-inspector` wheels fails
- **THEN** `robotframework-PlatynUI` `0.14.0.dev12` SHALL NOT be uploaded, and the run SHALL fail

#### Scenario: The next upload completes the version

- **GIVEN** an earlier upload of `0.14.0.dev12` stopped after some of the dependency wheels
- **WHEN** the upload runs again for that version
- **THEN** it SHALL upload the missing wheels and then `robotframework-PlatynUI`, and leave the wheels PyPI already has

### Requirement: Dev versions only rise

A build without a tag SHALL get the version `<next version>-dev.N`, which PyPI receives as `<next version>.devN`. The next version comes from the last tag and the commits since it, and `N` is the number of first-parent commits since that tag, also when the tag is a `-dev.0` tag. Every dev version thus ranks above every version published before it, including deleted ones, whose file names PyPI never accepts again.

#### Scenario: First nightlies after the start tag

- **GIVEN** the latest tag is `v0.14.0-dev.0`, and `main` has 12 first-parent commits after it
- **WHEN** CI builds the latest commit
- **THEN** its wheels SHALL carry `0.14.0.dev12`, which ranks above every `0.13.0` version
- **NOTE:** Verified after the start bump: the next CI run on `main` names this version in its wheel file names.

#### Scenario: After a release

- **GIVEN** the latest tag is `v1.0.0`, and `main` has 3 commits after it, all of type `fix:`
- **WHEN** CI builds the latest commit
- **THEN** its wheels SHALL carry `1.0.1.dev3`, which ranks above `1.0.0`

### Requirement: Users get a dev version only on request

Dev builds SHALL carry development-release versions in the sense of PEP 440 (`X.Y.Z.devN`), so that installers choose them only when asked for pre-releases or when a dev version is pinned.

#### Scenario: Plain install

- **GIVEN** PyPI has `0.14.0.dev12` of `platynui-cli` and no newer version
- **WHEN** a user runs `pip install "platynui-cli==0.14.0.dev12"`
- **THEN** pip SHALL install that dev version; without the pin and without `--pre`, pip SHALL NOT choose it

### Requirement: A release is published after the CI run of its tag succeeded

When the CI run of a version tag succeeds, CI SHALL create the GitHub release of that tag with the 17 wheels of that run. It SHALL then upload the same wheels to PyPI, in the order this spec requires. If the tag's run fails, CI SHALL publish nothing for that tag.

#### Scenario: Release tag

- **GIVEN** the maintainer ran `cz bump 1.0.0` and pushed `main` and the tag `v1.0.0`
- **WHEN** the CI run of `v1.0.0` succeeds
- **THEN** the GitHub release `v1.0.0` SHALL hold the 17 wheels of that run, and PyPI SHALL have version `1.0.0` of all five projects from these files
- **NOTE:** Verified at the first real tag; the job's decision logic is verified by pytest.

#### Scenario: Failed tag build

- **GIVEN** the tag `v1.0.0` is pushed
- **WHEN** a job of its CI run fails
- **THEN** no GitHub release SHALL be created and PyPI SHALL NOT receive `1.0.0`

### Requirement: Only a version tag of this repository publishes a release

CI SHALL publish a release only for a CI run of a version tag (`v*`) pushed to this repository. Runs of pull requests (also from forks), of pushes to branches, and of manual starts SHALL NOT publish a release.

#### Scenario: Pull request from a fork

- **GIVEN** a pull request from a fork
- **WHEN** its CI run succeeds
- **THEN** nothing SHALL be published

#### Scenario: Push to main

- **GIVEN** a push to `main`
- **WHEN** its CI run succeeds
- **THEN** no release SHALL be published

### Requirement: A tag with a dev label is not released

The release job SHALL refuse a tag with a `dev` pre-release label and a number other than 0, for example `v0.14.0-dev.3`. It SHALL publish nothing for it and SHALL fail with a message that dev versions come from the nightly upload. A `-dev.0` tag starts a version line: the release job SHALL publish nothing for it and succeed. Other labels, such as `rc`, SHALL be released as pre-releases.

#### Scenario: Dev tag

- **GIVEN** the maintainer pushes the tag `v0.14.0-dev.3`
- **WHEN** its CI run succeeds
- **THEN** the release job SHALL publish nothing and fail with a message saying that dev versions are published by the nightly upload

#### Scenario: Start of a version line

- **GIVEN** the maintainer ran `cz bump 0.14.0-dev.0` and pushed `main` and the tag `v0.14.0-dev.0`
- **WHEN** the CI run of that tag succeeds
- **THEN** the release job SHALL create no GitHub release, upload nothing and succeed, and the next nightly upload SHALL publish `0.14.0.dev0` if the tagged commit is still the latest on `main`

#### Scenario: Release candidate

- **GIVEN** the maintainer pushes the tag `v1.0.0-rc.1`
- **WHEN** its CI run succeeds
- **THEN** the GitHub release SHALL be marked as a pre-release, and PyPI SHALL receive version `1.0.0rc1`

### Requirement: A bump fails when the changelog cannot be written

When the changelog step of a version bump cannot run its changelog generator, or the generator fails, the step SHALL exit with an error that names the cause. The bump SHALL then stop before it creates the release commit and the tag.

#### Scenario: Changelog generator missing

- **GIVEN** `git-cliff` is not on the `PATH`
- **WHEN** the maintainer runs `cz bump 1.0.0`
- **THEN** the changelog step SHALL fail with a message that `git-cliff` was not found, and no commit and no tag `v1.0.0` SHALL be created
- **NOTE:** The script's exit status is verified by pytest with a stubbed `subprocess.run`. That `cz bump` stops on a failing pre-bump hook is commitizen's behavior and is checked once by hand.

#### Scenario: Changelog generator fails

- **GIVEN** `git-cliff` exits with a non-zero status
- **WHEN** the changelog step runs
- **THEN** it SHALL exit with a non-zero status and show `git-cliff`'s exit status

#### Scenario: Release section is written

- **GIVEN** `git-cliff` is on the `PATH`
- **WHEN** the maintainer runs `cz bump 1.0.0`
- **THEN** the release commit's `CHANGELOG.md` SHALL contain a `## [1.0.0]` section

### Requirement: The GitHub release carries the changelog of its version

The GitHub release SHALL be named after the version. Its notes SHALL be the version's section of `CHANGELOG.md`, for a pre-release the `[Unreleased]` section its bump wrote, shortened to GitHub's limit when needed with a link to the full changelog. It SHALL be marked as a pre-release exactly when the version has a pre-release label.

#### Scenario: Release notes

- **GIVEN** the tag `v1.0.0`, whose commit has a `## [1.0.0]` section in `CHANGELOG.md`
- **WHEN** the release job creates the GitHub release
- **THEN** the release SHALL be named `v1.0.0`, its notes SHALL be that section, and it SHALL NOT be marked as a pre-release

#### Scenario: Notes of a pre-release

- **GIVEN** the tag `v1.1.0-rc.1`, whose commit has the changes since `1.0.0` under `## [Unreleased]` in `CHANGELOG.md`
- **WHEN** the release job creates the GitHub release
- **THEN** its notes SHALL be that `[Unreleased]` section, and it SHALL be marked as a pre-release
- **NOTE:** `cz bump` writes the changelog before it creates the tag, and git-cliff gets no tag for a pre-release, so a pre-release has no section of its own.

#### Scenario: Changelog section too long

- **GIVEN** a changelog section longer than GitHub's limit of 125,000 characters
- **WHEN** the release notes are extracted
- **THEN** they SHALL list only the subject lines of the features, bug fixes and performance improvements, and link to `CHANGELOG.md`

### Requirement: Publishing uses no stored PyPI credential

Every upload to PyPI, nightly or release, SHALL authenticate through PyPI Trusted Publishing from the one workflow file `publish.yml`, running in the GitHub environment `pypi`. The repository SHALL NOT need a stored PyPI token.

#### Scenario: Release and dev build come from the same workflow

- **GIVEN** the release `1.0.0`, and later the dev build `1.0.1.dev3`
- **WHEN** both are uploaded to PyPI
- **THEN** both uploads SHALL come from runs of `publish.yml`, and PyPI SHALL list that workflow as the publisher of both files
- **NOTE:** Verified on PyPI after the first uploads; it cannot be tested locally.

### Requirement: A failed upload fails the run without hiding what was done

A rejected upload SHALL fail the run and SHALL show PyPI's response in the log. When creating the GitHub release fails, CI SHALL still upload the release to PyPI and then fail the run. A night without a dev build because `main` is red SHALL NOT fail the run.

#### Scenario: Failed GitHub release

- **GIVEN** a successful CI run of the tag `v1.0.0`
- **WHEN** creating the GitHub release fails
- **THEN** PyPI SHALL still receive `1.0.0`, and the run SHALL fail

#### Scenario: Rejected upload

- **GIVEN** PyPI has no Trusted Publisher for `platynui-cli`
- **WHEN** the nightly upload tries to upload its wheels
- **THEN** the run SHALL fail, its log SHALL show PyPI's answer, and `robotframework-PlatynUI` SHALL NOT be uploaded

### Requirement: A re-run of a publish uploads only what is missing

When a publish runs again for a version, CI SHALL skip every wheel PyPI already has and SHALL update the existing GitHub release instead of failing on it. A re-run after a failed upload thus uploads only what is missing.

#### Scenario: Re-run after a failed upload

- **GIVEN** the upload of `1.0.0` failed after the dependency wheels, and the maintainer fixed the cause
- **WHEN** the release job is re-run
- **THEN** only the missing wheels SHALL be uploaded, and the run SHALL succeed

#### Scenario: Re-run of a complete release

- **GIVEN** PyPI and the GitHub release already have every wheel of `1.0.0`
- **WHEN** the release job runs again
- **THEN** nothing new SHALL be uploaded, and the run SHALL succeed
