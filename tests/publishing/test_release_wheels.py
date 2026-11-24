"""The wheel set of one CI run and its upload order (spec: *release-publishing*)."""

import subprocess
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any

import pytest
from packaging.version import Version

from scripts import release_wheels
from scripts.release_wheels import WheelSetError, collect_wheels

WheelNames = dict[str, list[str]]


def test_a_complete_set_has_one_version_and_17_wheels(complete_dist: Path) -> None:
    wheels = collect_wheels(complete_dist)

    assert wheels.version == Version('0.13.0.dev378')
    assert len(wheels.dependencies) + len(wheels.root) == 17


def test_the_set_splits_into_the_dependencies_and_the_root_project(complete_dist: Path) -> None:
    wheels = collect_wheels(complete_dist)

    assert [path.name for path in wheels.root] == ['robotframework_platynui-0.13.0.dev378-py3-none-any.whl']
    dependency_projects = {path.name.split('-', 1)[0] for path in wheels.dependencies}
    assert dependency_projects == {'platynui_native', 'platynui_cli', 'platynui_inspector', 'platynui_provider_java'}
    assert len(wheels.dependencies) == 16


def test_a_missing_artifact_is_named(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    del names['wheels-windows-11-arm']
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match='wheels-windows-11-arm'):
        collect_wheels(dist)


def test_an_empty_artifact_directory_counts_as_missing(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    names['wheels-provider-java'] = []
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match='wheels-provider-java'):
        collect_wheels(dist)


def test_mixed_versions_name_both_versions(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.14.0.dev12')
    names['wheels-pure-python'] = ['robotframework_platynui-0.14.0.dev11-py3-none-any.whl']
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match=r'0\.14\.0\.dev11.*0\.14\.0\.dev12'):
        collect_wheels(dist)


def test_a_missing_project_is_named(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    for artifact, files in names.items():
        names[artifact] = [name for name in files if not name.startswith('platynui_inspector-')]
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match='platynui-inspector'):
        collect_wheels(dist)


def test_an_unexpected_project_is_refused(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    names['wheels-pure-python'].append('platynui_extra-0.13.0.dev378-py3-none-any.whl')
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match='platynui-extra'):
        collect_wheels(dist)


def test_a_platform_missing_from_one_platform_project_is_named(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    names['wheels-windows-11-arm'] = [
        name for name in names['wheels-windows-11-arm'] if not name.startswith('platynui_cli-')
    ]
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match=r'platynui-cli.*win_arm64'):
        collect_wheels(dist)


def test_an_invalid_wheel_file_name_is_named(
    tmp_path: Path, wheel_names: Callable[[str], WheelNames], make_dist: Callable[[Path, WheelNames], Path]
) -> None:
    names = wheel_names('0.13.0.dev378')
    names['wheels-pure-python'].append('not-a-wheel.whl')
    dist = make_dist(tmp_path / 'dist', names)

    with pytest.raises(WheelSetError, match=r'not-a-wheel\.whl'):
        collect_wheels(dist)


class RecordingRun:
    """Stands in for ``subprocess.run`` and fails the calls whose index is in ``fail``."""

    def __init__(self, fail: Sequence[int] = ()) -> None:
        self.calls: list[list[str]] = []
        self.fail = set(fail)

    def __call__(self, command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        assert kwargs.get('check') is True
        self.calls.append(command)
        if len(self.calls) - 1 in self.fail:
            raise subprocess.CalledProcessError(1, command)
        return subprocess.CompletedProcess(command, 0)


def files_of(command: list[str]) -> list[str]:
    return [Path(argument).name for argument in command if argument.endswith('.whl')]


def test_the_dependencies_are_uploaded_before_the_root_project(
    complete_dist: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    run = RecordingRun()
    monkeypatch.setattr(subprocess, 'run', run)

    release_wheels.upload(collect_wheels(complete_dist))

    assert len(run.calls) == 2
    first, second = (files_of(command) for command in run.calls)
    assert len(first) == 16
    assert not any(name.startswith('robotframework_platynui-') for name in first)
    assert second == ['robotframework_platynui-0.13.0.dev378-py3-none-any.whl']


def test_every_upload_uses_trusted_publishing_and_skips_files_pypi_has(
    complete_dist: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    run = RecordingRun()
    monkeypatch.setattr(subprocess, 'run', run)

    release_wheels.upload(collect_wheels(complete_dist))

    for command in run.calls:
        assert command[:2] == ['uv', 'publish']
        assert command[command.index('--trusted-publishing') + 1] == 'always'
        assert command[command.index('--check-url') + 1] == 'https://pypi.org/simple/'


def test_no_credential_is_on_a_command_line(complete_dist: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun()
    monkeypatch.setattr(subprocess, 'run', run)

    release_wheels.upload(collect_wheels(complete_dist))

    for command in run.calls:
        assert not {'--token', '-t', '--username', '-u', '--password', '-p'} & set(command)


def test_a_failed_dependency_upload_keeps_the_root_project_back(
    complete_dist: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    run = RecordingRun(fail=[0])
    monkeypatch.setattr(subprocess, 'run', run)

    with pytest.raises(release_wheels.UploadError, match=r'0\.13\.0\.dev378'):
        release_wheels.upload(collect_wheels(complete_dist))

    assert len(run.calls) == 1


def test_a_failed_root_upload_fails_the_upload(complete_dist: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun(fail=[1])
    monkeypatch.setattr(subprocess, 'run', run)

    with pytest.raises(release_wheels.UploadError, match='robotframework-platynui'):
        release_wheels.upload(collect_wheels(complete_dist))

    assert len(run.calls) == 2


def test_a_repeated_upload_sends_the_same_two_calls(complete_dist: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    # "The next upload completes the version": uv skips the files that --check-url finds on PyPI,
    # so a repeated upload of the same set sends the same calls and uploads only what is missing.
    first, second = RecordingRun(fail=[0]), RecordingRun()
    monkeypatch.setattr(subprocess, 'run', first)
    with pytest.raises(release_wheels.UploadError):
        release_wheels.upload(collect_wheels(complete_dist))

    monkeypatch.setattr(subprocess, 'run', second)
    release_wheels.upload(collect_wheels(complete_dist))

    assert second.calls[0] == first.calls[0]
    assert len(second.calls) == 2


def test_a_missing_uv_fails_the_upload(complete_dist: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    def missing_uv(command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        raise FileNotFoundError(command[0])

    monkeypatch.setattr(subprocess, 'run', missing_uv)

    with pytest.raises(release_wheels.UploadError):
        release_wheels.upload(collect_wheels(complete_dist))


def test_github_outputs_are_appended(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    output = tmp_path / 'output'
    output.write_text('earlier=1\n')
    monkeypatch.setenv('GITHUB_OUTPUT', str(output))

    release_wheels.write_output('run-id', '42')

    assert output.read_text() == 'earlier=1\nrun-id=42\n'


def test_without_github_output_nothing_is_written(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv('GITHUB_OUTPUT', raising=False)

    release_wheels.write_output('run-id', '42')
