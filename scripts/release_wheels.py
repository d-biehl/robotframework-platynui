"""The wheel set of one CI run, and its upload to PyPI.

Shared by ``publish_dev.py`` (nightly dev builds) and ``publish.py`` (releases).
Both download the wheel artifacts of one CI run into one directory per artifact,
check here that they form one complete set of one version, and upload them here.
"""

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

from packaging.utils import InvalidWheelFilename, NormalizedName, parse_wheel_filename
from packaging.version import Version

# The wheel artifacts of .github/workflows/ci.yml. Renaming one there, or adding a
# platform, needs the same change here: publishing refuses an incomplete set.
ARTIFACTS: tuple[str, ...] = (
    'wheels-pure-python',
    'wheels-provider-java',
    'wheels-linux-x86_64',
    'wheels-linux-aarch64',
    'wheels-macos-latest',
    'wheels-windows-latest',
    'wheels-windows-11-arm',
)

# The artifact with the wheel of ROOT_PROJECT, which tells the version of a run.
ROOT_ARTIFACT = 'wheels-pure-python'

# robotframework-PlatynUI pins the other four exactly, so it is uploaded after them.
ROOT_PROJECT = 'robotframework-platynui'
DEPENDENCY_PROJECTS: tuple[str, ...] = (
    'platynui-native',
    'platynui-cli',
    'platynui-inspector',
    'platynui-provider-java',
)
# The projects with one wheel per platform; each must cover the same platforms.
PLATFORM_PROJECTS: tuple[str, ...] = ('platynui-native', 'platynui-cli', 'platynui-inspector')

PYPI_SIMPLE_INDEX = 'https://pypi.org/simple/'
UPLOAD_TIMEOUT_SECONDS = 600


class WheelSetError(Exception):
    """The downloaded wheels are not one complete set of one version."""


class UploadError(Exception):
    """An upload to PyPI failed."""


@dataclass(frozen=True)
class WheelSet:
    version: Version
    dependencies: tuple[Path, ...]
    root: tuple[Path, ...]


@dataclass(frozen=True)
class _Wheel:
    path: Path
    project: NormalizedName
    version: Version
    platforms: frozenset[str]


def _parse(path: Path) -> _Wheel:
    try:
        project, version, _build, tags = parse_wheel_filename(path.name)
    except InvalidWheelFilename as e:
        raise WheelSetError(f'{path.name} is not a valid wheel file name: {e}') from e
    return _Wheel(path, project, version, frozenset(tag.platform for tag in tags))


def root_version(directory: Path) -> Version:
    """Return the version of the one ``robotframework-platynui`` wheel in ``directory``."""
    wheels = [wheel for wheel in map(_parse, sorted(directory.glob('*.whl'))) if wheel.project == ROOT_PROJECT]
    if len(wheels) != 1:
        raise WheelSetError(f"expected one wheel of {ROOT_PROJECT} in '{directory}', found {len(wheels)}")
    return wheels[0].version


def collect_wheels(dist: Path) -> WheelSet:
    """Check that ``dist`` holds one complete wheel set of one version, and split it for the upload."""
    missing_artifacts = [name for name in ARTIFACTS if not any((dist / name).glob('*.whl'))]
    if missing_artifacts:
        raise WheelSetError(f"the wheel artifacts {', '.join(missing_artifacts)} are missing in '{dist}'")

    wheels = [_parse(path) for path in sorted(dist.glob('*/*.whl'))]

    versions = sorted({wheel.version for wheel in wheels})
    if len(versions) != 1:
        raise WheelSetError(f'the wheels carry more than one version: {", ".join(map(str, versions))}')

    projects: set[str] = {wheel.project for wheel in wheels}
    expected = {ROOT_PROJECT, *DEPENDENCY_PROJECTS}
    if missing_projects := sorted(expected - projects):
        raise WheelSetError(f'no wheels of {", ".join(missing_projects)}')
    if unexpected_projects := sorted(projects - expected):
        raise WheelSetError(f'wheels of unexpected projects: {", ".join(unexpected_projects)}')

    platforms = {
        project: frozenset[str]().union(*(wheel.platforms for wheel in wheels if wheel.project == project))
        for project in PLATFORM_PROJECTS
    }
    all_platforms = frozenset[str]().union(*platforms.values())
    gaps = [
        f'{project} lacks {", ".join(sorted(all_platforms - covered))}'
        for project, covered in platforms.items()
        if covered != all_platforms
    ]
    if gaps:
        raise WheelSetError(f'the platform wheels do not cover the same platforms: {"; ".join(gaps)}')

    return WheelSet(
        version=versions[0],
        dependencies=tuple(wheel.path for wheel in wheels if wheel.project != ROOT_PROJECT),
        root=tuple(wheel.path for wheel in wheels if wheel.project == ROOT_PROJECT),
    )


def _publish_command(files: tuple[Path, ...]) -> list[str]:
    # Trusted Publishing: uv gets a short-lived token through the workflow's OIDC
    # identity, so no credential is stored or appears here. --check-url skips files
    # PyPI already has, so a repeated upload only sends what is missing.
    return ['uv', 'publish', '--trusted-publishing', 'always', '--check-url', PYPI_SIMPLE_INDEX, *map(str, files)]


def upload(wheels: WheelSet) -> None:
    """Upload the dependency wheels, and only after they all arrived the root project's wheel."""
    for group, files in (('dependency', wheels.dependencies), (ROOT_PROJECT, wheels.root)):
        print(f'uploading the {len(files)} {group} wheels of {wheels.version} to PyPI', flush=True)
        try:
            subprocess.run(_publish_command(files), check=True, timeout=UPLOAD_TIMEOUT_SECONDS)
        except (subprocess.SubprocessError, OSError) as e:
            raise UploadError(f'the upload of the {group} wheels of {wheels.version} to PyPI failed: {e}') from e


def write_output(name: str, value: str) -> None:
    """Hand ``name=value`` on to later workflow steps, when running in GitHub Actions."""
    output = os.environ.get('GITHUB_OUTPUT')
    if output:
        with open(output, 'a', encoding='utf-8') as f:
            f.write(f'{name}={value}\n')
