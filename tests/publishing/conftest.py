"""Shared fixtures for the tests of the publishing scripts in ``scripts/``.

``scripts/`` is no installed package, so the repository root goes on ``sys.path``
and the tests import the scripts as ``scripts.<module>``.
"""

import sys
from collections.abc import Callable
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
if str(REPOSITORY_ROOT) not in sys.path:
    sys.path.insert(0, str(REPOSITORY_ROOT))

# The wheels of the CI run 44391763278 on main (commit aca3bbfc), by artifact.
PLATFORM_ARTIFACTS = {
    'wheels-linux-x86_64': 'manylinux_2_34_x86_64',
    'wheels-linux-aarch64': 'manylinux_2_34_aarch64',
    'wheels-macos-latest': 'macosx_11_0_arm64',
    'wheels-windows-latest': 'win_amd64',
    'wheels-windows-11-arm': 'win_arm64',
}

WheelNames = dict[str, list[str]]


def _wheel_names(version: str) -> WheelNames:
    names = {
        'wheels-pure-python': [f'robotframework_platynui-{version}-py3-none-any.whl'],
        'wheels-provider-java': [f'platynui_provider_java-{version}-py3-none-any.whl'],
    }
    for artifact, platform in PLATFORM_ARTIFACTS.items():
        names[artifact] = [
            f'platynui_cli-{version}-py3-none-{platform}.whl',
            f'platynui_inspector-{version}-py3-none-{platform}.whl',
            f'platynui_native-{version}-cp312-abi3-{platform}.whl',
        ]
    return names


def _make_dist(dist: Path, names: WheelNames) -> Path:
    for artifact, files in names.items():
        directory = dist / artifact
        directory.mkdir(parents=True, exist_ok=True)
        for name in files:
            (directory / name).touch()
    return dist


@pytest.fixture
def wheel_names() -> Callable[[str], WheelNames]:
    """Return the file names of a complete CI wheel set of a version, by artifact."""
    return _wheel_names


@pytest.fixture
def make_dist() -> Callable[[Path, WheelNames], Path]:
    """Create empty wheel files in one directory per artifact, as the downloads leave them."""
    return _make_dist


@pytest.fixture
def complete_dist(tmp_path: Path) -> Path:
    return _make_dist(tmp_path / 'dist', _wheel_names('0.13.0.dev378'))
