"""The release of a version tag (spec: *release-publishing*)."""

from collections.abc import Callable
from pathlib import Path

import pytest

from scripts import publish
from scripts.release_wheels import WheelSet

WheelNames = dict[str, list[str]]


@pytest.fixture
def github_output(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    output = tmp_path / 'github_output'
    output.touch()
    monkeypatch.setenv('GITHUB_OUTPUT', str(output))
    return output


def outputs(output: Path) -> dict[str, str]:
    return dict(line.split('=', 1) for line in output.read_text().splitlines())


# --- A tag with a dev label is not released -------------------------------------------------


def test_a_dev_tag_is_refused_with_a_pointer_to_the_nightly_upload(github_output: Path) -> None:
    with pytest.raises(SystemExit, match='nightly upload'):
        publish.main(['check-tag', 'v0.14.0-dev.3'])

    assert outputs(github_output) == {}


def test_a_dev_0_tag_starts_a_version_line_and_publishes_nothing(
    github_output: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    publish.main(['check-tag', 'v0.14.0-dev.0'])

    assert outputs(github_output)['skip'] == 'true'
    assert 'version line' in capsys.readouterr().out


def test_a_release_candidate_is_a_pre_release(github_output: Path) -> None:
    publish.main(['check-tag', 'v1.0.0-rc.1'])

    assert outputs(github_output) == {'skip': 'false', 'version': '1.0.0-rc.1', 'is-prerelease': 'true'}


def test_a_release_is_no_pre_release(github_output: Path) -> None:
    publish.main(['check-tag', 'v1.0.0'])

    assert outputs(github_output) == {'skip': 'false', 'version': '1.0.0', 'is-prerelease': 'false'}


@pytest.mark.parametrize('tag', ['1.0.0', 'v1.0', 'vnext', 'v1.0.0-foo.1'])
def test_a_tag_that_names_no_publishable_version_is_refused(tag: str, github_output: Path) -> None:
    with pytest.raises(SystemExit, match=tag):
        publish.main(['check-tag', tag])

    assert outputs(github_output) == {}


# --- The wheels must carry the tag's version ------------------------------------------------


@pytest.fixture
def uploads(monkeypatch: pytest.MonkeyPatch) -> list[WheelSet]:
    uploaded: list[WheelSet] = []
    monkeypatch.setattr(publish, 'upload', uploaded.append)
    return uploaded


def test_wheels_of_the_tag_version_are_uploaded(
    tmp_path: Path,
    wheel_names: Callable[[str], WheelNames],
    make_dist: Callable[[Path, WheelNames], Path],
    uploads: list[WheelSet],
) -> None:
    dist = make_dist(tmp_path / 'dist', wheel_names('1.0.0rc1'))

    publish.main(['upload', str(dist), '--version', '1.0.0-rc.1'])

    assert [str(wheels.version) for wheels in uploads] == ['1.0.0rc1']


def test_wheels_of_another_version_are_not_uploaded(
    tmp_path: Path,
    wheel_names: Callable[[str], WheelNames],
    make_dist: Callable[[Path, WheelNames], Path],
    uploads: list[WheelSet],
) -> None:
    dist = make_dist(tmp_path / 'dist', wheel_names('0.14.0.dev12'))

    with pytest.raises(SystemExit, match=r'0\.14\.0\.dev12.*1\.0\.0'):
        publish.main(['upload', str(dist), '--version', '1.0.0'])

    assert uploads == []


def test_an_incomplete_set_is_not_uploaded(
    tmp_path: Path,
    wheel_names: Callable[[str], WheelNames],
    make_dist: Callable[[Path, WheelNames], Path],
    uploads: list[WheelSet],
) -> None:
    names = wheel_names('1.0.0')
    del names['wheels-linux-aarch64']
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(SystemExit, match='wheels-linux-aarch64'):
        publish.main(['upload', str(dist), '--version', '1.0.0'])

    assert uploads == []
