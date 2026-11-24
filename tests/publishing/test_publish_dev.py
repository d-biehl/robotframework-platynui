"""The nightly dev upload of the latest CI run on main (spec: *release-publishing*)."""

import io
import json
import subprocess
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from scripts import publish_dev
from scripts.release_wheels import ARTIFACTS, WheelSet

WheelNames = dict[str, list[str]]


def run_entry(run_id: int, status: str, conclusion: str = '') -> dict[str, Any]:
    return {'databaseId': run_id, 'status': status, 'conclusion': conclusion}


class FakeGh:
    """Stands in for ``subprocess.run`` and answers the ``gh`` calls of ``publish_dev``.

    ``run list`` answers come from ``runs``, one per call; ``run download`` creates the
    files of ``artifacts`` in the requested directory, or fails for ``failing``.
    """

    def __init__(
        self,
        runs: list[list[dict[str, Any]]] | None = None,
        artifacts: WheelNames | None = None,
        failing: str | None = None,
    ) -> None:
        self.runs = list(runs or [])
        self.artifacts = artifacts or {}
        self.failing = failing
        self.calls: list[list[str]] = []

    def __call__(self, command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        assert kwargs.get('check') is True
        self.calls.append(command)
        assert command[0] == 'gh'
        if command[1:3] == ['run', 'list']:
            return subprocess.CompletedProcess(command, 0, stdout=json.dumps(self.runs.pop(0)), stderr='')
        if command[1:3] == ['run', 'download']:
            name = command[command.index('--name') + 1]
            if name == self.failing or name not in self.artifacts:
                raise subprocess.CalledProcessError(1, command, stderr=f'no artifact matches {name}')
            directory = Path(command[command.index('--dir') + 1])
            directory.mkdir(parents=True, exist_ok=True)
            for file in self.artifacts[name]:
                (directory / file).touch()
            return subprocess.CompletedProcess(command, 0)
        raise AssertionError(f'unexpected gh call {command}')

    def downloads(self) -> list[str]:
        return [command[command.index('--name') + 1] for command in self.calls if command[1:3] == ['run', 'download']]


class Clock:
    """A clock that advances by one poll interval whenever the code sleeps."""

    def __init__(self) -> None:
        self.now = 0.0
        self.sleeps: list[float] = []

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.sleeps.append(seconds)
        self.now += seconds


@pytest.fixture
def github_output(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    output = tmp_path / 'github_output'
    output.touch()
    monkeypatch.setenv('GITHUB_OUTPUT', str(output))
    return output


def find(gh: FakeGh, monkeypatch: pytest.MonkeyPatch, clock: Clock | None = None) -> str | None:
    monkeypatch.setattr(subprocess, 'run', gh)
    clock = clock or Clock()
    return publish_dev.find_latest_run(clock=clock.monotonic, sleep=clock.sleep)


# --- A night's dev build comes from the latest run on main ---------------------------------


def test_a_successful_latest_run_is_handed_on(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    gh = FakeGh(runs=[[run_entry(101, 'completed', 'success')]])

    assert find(gh, monkeypatch) == '101'
    assert github_output.read_text() == 'run-id=101\n'


def test_the_run_list_asks_for_the_newest_push_run_of_ci_on_main(
    github_output: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    gh = FakeGh(runs=[[run_entry(101, 'completed', 'success')]])

    find(gh, monkeypatch)

    command = gh.calls[0]
    assert command[command.index('--workflow') + 1] == 'ci.yml'
    assert command[command.index('--branch') + 1] == 'main'
    assert command[command.index('--event') + 1] == 'push'
    assert command[command.index('--limit') + 1] == '1'


def test_a_failed_latest_run_gives_no_dev_build(
    github_output: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    # Only the newest run is listed (--limit 1), so an older green run is never taken.
    gh = FakeGh(runs=[[run_entry(102, 'completed', 'failure')]])

    assert find(gh, monkeypatch) is None
    assert github_output.read_text() == ''
    assert '102' in capsys.readouterr().out


def test_a_running_latest_run_is_waited_for(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    gh = FakeGh(
        runs=[
            [run_entry(103, 'in_progress')],
            [run_entry(103, 'in_progress')],
            [run_entry(103, 'completed', 'success')],
        ]
    )
    clock = Clock()

    assert find(gh, monkeypatch, clock) == '103'
    assert clock.sleeps == [publish_dev.LATEST_RUN_POLL_SECONDS] * 2


def test_a_newer_run_replaces_the_one_waited_for(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    gh = FakeGh(
        runs=[
            [run_entry(104, 'in_progress')],
            [run_entry(105, 'queued')],
            [run_entry(105, 'completed', 'success')],
        ]
    )

    assert find(gh, monkeypatch) == '105'
    assert github_output.read_text() == 'run-id=105\n'


def test_a_newer_run_that_fails_gives_no_dev_build(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    gh = FakeGh(runs=[[run_entry(104, 'in_progress')], [run_entry(105, 'completed', 'failure')]])

    assert find(gh, monkeypatch) is None
    assert github_output.read_text() == ''


def test_the_wait_ends_after_two_hours(
    github_output: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    polls = publish_dev.LATEST_RUN_WAIT_SECONDS // publish_dev.LATEST_RUN_POLL_SECONDS
    gh = FakeGh(runs=[[run_entry(106, 'in_progress')] for _ in range(polls + 1)])
    clock = Clock()

    assert find(gh, monkeypatch, clock) is None
    assert github_output.read_text() == ''
    assert clock.now == publish_dev.LATEST_RUN_WAIT_SECONDS
    assert "isn't done" in capsys.readouterr().out


def test_no_run_on_main_gives_no_dev_build(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    gh = FakeGh(runs=[[]])

    assert find(gh, monkeypatch) is None
    assert github_output.read_text() == ''


def test_a_gh_failure_ends_with_an_error(monkeypatch: pytest.MonkeyPatch) -> None:
    def failing_gh(command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        raise subprocess.CalledProcessError(4, command, stderr='HTTP 401: Bad credentials')

    monkeypatch.setattr(subprocess, 'run', failing_gh)

    with pytest.raises(SystemExit, match='Bad credentials'):
        publish_dev.main(['latest-run'])


def test_a_night_without_a_dev_build_ends_successfully(github_output: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(subprocess, 'run', FakeGh(runs=[[run_entry(102, 'completed', 'failure')]]))

    publish_dev.main(['latest-run'])

    assert github_output.read_text() == ''


# --- Dev builds of main are published to PyPI every night ---------------------------------


@pytest.fixture
def uploads(monkeypatch: pytest.MonkeyPatch) -> list[WheelSet]:
    uploaded: list[WheelSet] = []
    monkeypatch.setattr(publish_dev, 'upload', uploaded.append)
    return uploaded


def pypi_answers(found: bool) -> Callable[[str, str], bool]:
    def has_version(project: str, version: str) -> bool:
        return found

    return has_version


def publish(
    gh: FakeGh,
    dist: Path,
    monkeypatch: pytest.MonkeyPatch,
    on_pypi: bool = False,
) -> None:
    monkeypatch.setattr(subprocess, 'run', gh)
    monkeypatch.setattr(publish_dev, 'pypi_has_version', pypi_answers(on_pypi))
    publish_dev.publish_pypi('201', dist)


def test_a_new_dev_version_is_uploaded_as_one_set(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    wheel_names: Callable[[str], WheelNames],
    uploads: list[WheelSet],
) -> None:
    gh = FakeGh(artifacts=wheel_names('0.14.0.dev12'))

    publish(gh, tmp_path / 'dist', monkeypatch)

    assert len(uploads) == 1
    assert str(uploads[0].version) == '0.14.0.dev12'
    assert len(uploads[0].dependencies) + len(uploads[0].root) == 17


def test_every_artifact_is_downloaded_by_name_into_its_own_directory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    wheel_names: Callable[[str], WheelNames],
    uploads: list[WheelSet],
) -> None:
    gh = FakeGh(artifacts=wheel_names('0.14.0.dev12'))
    dist = tmp_path / 'dist'

    publish(gh, dist, monkeypatch)

    assert sorted(gh.downloads()) == sorted(ARTIFACTS)
    for command in gh.calls:
        assert command[:4] == ['gh', 'run', 'download', '201']
        name = command[command.index('--name') + 1]
        assert command[command.index('--dir') + 1] == str(dist / name)


def test_a_version_pypi_has_is_not_uploaded_again(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    wheel_names: Callable[[str], WheelNames],
    uploads: list[WheelSet],
    capsys: pytest.CaptureFixture[str],
) -> None:
    gh = FakeGh(artifacts=wheel_names('0.14.0.dev12'))

    publish(gh, tmp_path / 'dist', monkeypatch, on_pypi=True)

    assert uploads == []
    assert gh.downloads() == ['wheels-pure-python']
    assert 'already on PyPI' in capsys.readouterr().out


@pytest.mark.parametrize('version', ['1.0.0', '1.0.0rc1'])
def test_a_version_without_a_dev_label_is_left_to_its_tag(
    version: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    wheel_names: Callable[[str], WheelNames],
    uploads: list[WheelSet],
    capsys: pytest.CaptureFixture[str],
) -> None:
    gh = FakeGh(artifacts=wheel_names(version))

    publish(gh, tmp_path / 'dist', monkeypatch)

    assert uploads == []
    assert 'its tag publishes it' in capsys.readouterr().out


def test_a_missing_artifact_fails_before_any_upload(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    wheel_names: Callable[[str], WheelNames],
    uploads: list[WheelSet],
) -> None:
    gh = FakeGh(artifacts=wheel_names('0.14.0.dev12'), failing='wheels-macos-latest')
    monkeypatch.setattr(subprocess, 'run', gh)
    monkeypatch.setattr(publish_dev, 'pypi_has_version', pypi_answers(False))

    with pytest.raises(SystemExit, match='wheels-macos-latest'):
        publish_dev.main(['pypi', '--run-id', '201', '--dist', str(tmp_path / 'dist')])

    assert uploads == []


def test_pypi_is_asked_for_the_root_project_version(monkeypatch: pytest.MonkeyPatch) -> None:
    urls: list[str] = []

    def found(url: str, timeout: float) -> io.BytesIO:
        urls.append(url)
        return io.BytesIO(b'{}')

    monkeypatch.setattr(urllib.request, 'urlopen', found)

    assert publish_dev.pypi_has_version('robotframework-platynui', '0.14.0.dev12')
    assert urls == ['https://pypi.org/pypi/robotframework-platynui/0.14.0.dev12/json']


def test_a_version_pypi_does_not_know_is_reported_missing(monkeypatch: pytest.MonkeyPatch) -> None:
    def not_found(url: str, timeout: float) -> io.BytesIO:
        raise urllib.error.HTTPError(url, 404, 'Not Found', None, None)  # type: ignore[arg-type]

    monkeypatch.setattr(urllib.request, 'urlopen', not_found)

    assert not publish_dev.pypi_has_version('robotframework-platynui', '0.14.0.dev12')


def test_another_pypi_error_ends_with_an_error(monkeypatch: pytest.MonkeyPatch) -> None:
    def unavailable(url: str, timeout: float) -> io.BytesIO:
        raise urllib.error.HTTPError(url, 503, 'Service Unavailable', None, None)  # type: ignore[arg-type]

    monkeypatch.setattr(urllib.request, 'urlopen', unavailable)

    with pytest.raises(urllib.error.HTTPError):
        publish_dev.pypi_has_version('robotframework-platynui', '0.14.0.dev12')
