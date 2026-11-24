"""The changelog step of ``cz bump`` (spec: *release-publishing*, "A bump fails when the changelog cannot be written").

``scripts/update-changelog.py`` is no importable module name, so it is loaded by path,
with a stand-in for ``scripts.tools``: that module needs GitPython, which the dev
environment does not have.
"""

import importlib.util
import subprocess
import sys
import types
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts' / 'update-changelog.py'


@dataclass
class FakeVersion:
    text: str
    prerelease: tuple[str, ...] = ()

    def __str__(self) -> str:
        return self.text


class RecordingRun:
    def __init__(self, returncode: int = 0) -> None:
        self.returncode = returncode
        self.calls: list[tuple[list[str], dict[str, Any]]] = []

    def __call__(self, command: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
        self.calls.append((command, kwargs))
        if self.returncode and kwargs.get('check'):
            raise subprocess.CalledProcessError(self.returncode, command)
        return subprocess.CompletedProcess(command, self.returncode)


def load(monkeypatch: pytest.MonkeyPatch, version: FakeVersion) -> types.ModuleType:
    tools = types.ModuleType('scripts.tools')
    setattr(tools, 'get_version', lambda: version)  # noqa: B010 - a stand-in module has no declared attributes
    monkeypatch.setitem(sys.modules, 'scripts.tools', tools)
    spec = importlib.util.spec_from_file_location('update_changelog', SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stub(monkeypatch: pytest.MonkeyPatch, git_cliff: str | None, run: RecordingRun) -> None:
    def which(name: str) -> str | None:
        assert name == 'git-cliff'
        return git_cliff

    monkeypatch.setattr('shutil.which', which)
    monkeypatch.setattr(subprocess, 'run', run)


def test_a_missing_git_cliff_fails_the_step(monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun()
    module = load(monkeypatch, FakeVersion('1.0.0'))
    stub(monkeypatch, None, run)

    with pytest.raises(SystemExit) as excinfo:
        module.main()

    assert 'git-cliff' in str(excinfo.value.code)
    assert run.calls == []


def test_a_failing_git_cliff_fails_the_step_with_its_exit_status(monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun(returncode=2)
    module = load(monkeypatch, FakeVersion('1.0.0'))
    stub(monkeypatch, '/usr/bin/git-cliff', run)

    with pytest.raises(SystemExit) as excinfo:
        module.main()

    assert 'exit status 2' in str(excinfo.value.code)


def test_a_pre_release_writes_the_unreleased_section(monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun()
    module = load(monkeypatch, FakeVersion('0.14.0-dev.0', ('dev', '0')))
    stub(monkeypatch, '/usr/bin/git-cliff', run)

    module.main()

    [(command, kwargs)] = run.calls
    assert command == ['/usr/bin/git-cliff', '-o', 'CHANGELOG.md']
    assert kwargs.get('check') is True
    assert not kwargs.get('shell')


def test_a_release_writes_its_own_section(monkeypatch: pytest.MonkeyPatch) -> None:
    run = RecordingRun()
    module = load(monkeypatch, FakeVersion('1.0.0'))
    stub(monkeypatch, '/usr/bin/git-cliff', run)

    module.main()

    [(command, kwargs)] = run.calls
    assert command == ['/usr/bin/git-cliff', '--bump', '-t', 'v1.0.0', '-o', 'CHANGELOG.md']
    assert kwargs.get('check') is True
