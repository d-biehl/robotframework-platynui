# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "packaging>=24",
# ]
# ///
"""Publish the wheels of the latest CI run on main to PyPI as a dev version, if that run succeeded.

Targets:

- ``latest-run`` finds the newest CI run for a push to main, waits while it runs, and
  hands its id on through ``GITHUB_OUTPUT`` when it succeeded. An older successful run
  is never taken: during feature work, the newest run is often red and the one before
  it an unfinished state.
- ``pypi --run-id <id>`` uploads the wheels of that run, unless PyPI has the version
  already or it is no dev version, which its tag publishes instead.

Run it with ``uv run scripts/publish_dev.py <target>``; ``gh`` reads ``GH_TOKEN`` and ``GH_REPO``.
"""

import argparse
import contextlib
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path
from typing import Any

if __name__ == '__main__' and not __package__:
    file = Path(__file__).resolve()
    parent, top = file.parent, file.parents[1]

    if str(top) not in sys.path:
        sys.path.append(str(top))

    with contextlib.suppress(ValueError):
        sys.path.remove(str(parent))

    __package__ = 'scripts'


from scripts.release_wheels import (
    ARTIFACTS,
    ROOT_ARTIFACT,
    ROOT_PROJECT,
    UploadError,
    WheelSetError,
    collect_wheels,
    root_version,
    upload,
    write_output,
)

# The workflow file whose runs are published; see .github/workflows/ci.yml.
CI_WORKFLOW = 'ci.yml'

# How long and how often the latest run on main is waited for. CI took about 20 minutes
# on 2026-11-18; two hours also cover a run that a push starts just before the upload.
LATEST_RUN_WAIT_SECONDS = 2 * 60 * 60
LATEST_RUN_POLL_SECONDS = 60

GH_TIMEOUT_SECONDS = 120
DOWNLOAD_TIMEOUT_SECONDS = 600
PYPI_TIMEOUT_SECONDS = 60


class GhError(Exception):
    """A ``gh`` call failed."""


def gh_json(*args: str) -> Any:
    command = ['gh', *args]
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=GH_TIMEOUT_SECONDS)
    except subprocess.CalledProcessError as e:
        raise GhError(f'{" ".join(command)} failed with exit status {e.returncode}: {(e.stderr or "").strip()}') from e
    except (subprocess.SubprocessError, OSError) as e:
        raise GhError(f'{" ".join(command)} failed: {e}') from e
    return json.loads(result.stdout)


def newest_main_run() -> dict[str, Any] | None:
    runs: list[dict[str, Any]] = gh_json(
        'run',
        'list',
        '--workflow',
        CI_WORKFLOW,
        '--branch',
        'main',
        '--event',
        'push',
        '--limit',
        '1',
        '--json',
        'databaseId,status,conclusion',
    )
    return runs[0] if runs else None


def find_latest_run(
    clock: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], None] = time.sleep,
) -> str | None:
    """Return the id of the latest CI run on main and hand it on, if that run succeeded.

    While the latest run is not done, a newer run that a push starts takes its place.
    """
    deadline = clock() + LATEST_RUN_WAIT_SECONDS
    run = newest_main_run()

    while run is not None and run['status'] != 'completed':
        if clock() >= deadline:
            print(f"the latest run {run['databaseId']} on main isn't done in time, so there is no dev build")
            return None
        print(f'waiting for the latest run {run["databaseId"]} on main, which is {run["status"]}', flush=True)
        sleep(LATEST_RUN_POLL_SECONDS)
        run = newest_main_run()

    if run is None:
        print(f'{CI_WORKFLOW} has no run for a push to main, so there is no dev build')
        return None

    run_id = str(run['databaseId'])
    if run['conclusion'] != 'success':
        print(f"the latest run {run_id} on main ended with '{run['conclusion']}', so there is no dev build")
        return None

    print(f'the latest run {run_id} on main succeeded')
    write_output('run-id', run_id)
    return run_id


def pypi_has_version(project: str, version: str) -> bool:
    try:
        with urllib.request.urlopen(f'https://pypi.org/pypi/{project}/{version}/json', timeout=PYPI_TIMEOUT_SECONDS):
            return True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return False
        raise


def download_artifact(run_id: str, name: str, dist: Path) -> None:
    print(f'downloading the artifact {name} of run {run_id}', flush=True)
    command = ['gh', 'run', 'download', run_id, '--name', name, '--dir', str(dist / name)]
    try:
        subprocess.run(command, check=True, timeout=DOWNLOAD_TIMEOUT_SECONDS)
    except (subprocess.SubprocessError, OSError) as e:
        raise WheelSetError(f'the artifact {name} of run {run_id} could not be downloaded: {e}') from e


def publish_pypi(run_id: str, dist: Path) -> None:
    """Upload the wheels of ``run_id`` as a dev version, unless PyPI has it or it is no dev version."""
    # The pure-Python artifact tells the version, so nothing else is downloaded for nothing.
    download_artifact(run_id, ROOT_ARTIFACT, dist)
    version = root_version(dist / ROOT_ARTIFACT)

    if not version.is_devrelease:
        print(f'{version} is no dev version, its tag publishes it')
        return

    # robotframework-platynui is uploaded last, so PyPI has the whole version when it has that project.
    if pypi_has_version(ROOT_PROJECT, str(version)):
        print(f'{version} is already on PyPI')
        return

    for name in ARTIFACTS:
        if name != ROOT_ARTIFACT:
            download_artifact(run_id, name, dist)

    upload(collect_wheels(dist))


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument(
        'target',
        choices=['latest-run', 'pypi'],
        help='latest-run finds the run to publish, pypi uploads its wheels',
    )
    parser.add_argument('--run-id', help='the CI run whose wheels to upload, as latest-run hands it on')
    parser.add_argument('--dist', type=Path, default=Path('dist'), help='where the artifacts are downloaded to')
    args = parser.parse_args(argv)

    try:
        if args.target == 'latest-run':
            find_latest_run()
            return

        if not args.run_id:
            parser.error('the target pypi needs --run-id')
        publish_pypi(args.run_id, args.dist.absolute())
    except (GhError, WheelSetError, UploadError) as e:
        raise SystemExit(str(e)) from e


if __name__ == '__main__':
    main()
