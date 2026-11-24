"""The notes of a GitHub release (spec: *release-publishing*)."""

from pathlib import Path

import pytest

from scripts import extract_release_notes
from scripts.extract_release_notes import MAX_LENGTH, NotesError, release_notes

CHANGELOG_URL = 'https://github.example/owner/repo/blob/v1.0.0/CHANGELOG.md'

CHANGELOG = """\
# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Features

- **cli:** Something new after the release candidate

## [1.0.0](https://github.example/compare/v0.9.0..v1.0.0) - 2027-01-15

### Features

- **BareMetal:** Add a keyword ([abc1234](https://github.example/commit/abc1234))

  The body of the commit,
  indented by two spaces.

### Bug Fixes

- **atspi:** Fix a crash ([def5678](https://github.example/commit/def5678))

## [0.9.0] - 2026-12-01

### Features

- An older feature
"""


def test_the_section_of_a_release_is_extracted_up_to_the_next_heading() -> None:
    notes = release_notes(CHANGELOG, '1.0.0', CHANGELOG_URL)

    assert notes.startswith('### Features\n\n- **BareMetal:** Add a keyword')
    assert notes.endswith('- **atspi:** Fix a crash ([def5678](https://github.example/commit/def5678))')
    assert 'older feature' not in notes
    assert CHANGELOG_URL not in notes


def test_the_last_section_of_the_file_is_extracted_too() -> None:
    assert release_notes(CHANGELOG, '0.9.0', CHANGELOG_URL) == '### Features\n\n- An older feature'


def test_a_pre_release_takes_the_unreleased_section_its_bump_wrote() -> None:
    # cz bump writes the changelog before it creates the tag, and git-cliff gets no tag for
    # a pre-release, so the changes of a pre-release are under [Unreleased] in its commit.
    notes = release_notes(CHANGELOG, '1.1.0-rc.1', CHANGELOG_URL)

    assert notes == '### Features\n\n- **cli:** Something new after the release candidate'


def test_a_version_without_a_section_is_an_error() -> None:
    with pytest.raises(NotesError, match=r'\[2\.0\.0\]'):
        release_notes(CHANGELOG, '2.0.0', CHANGELOG_URL)


def long_section(heading_prefix: str) -> str:
    subjects = '\n'.join(f'- **scope:** Feature number {n}\n\n  ' + 'body text ' * 20 for n in range(2000))
    return (
        '# Changelog\n\n## [1.0.0] - 2027-01-15\n\n'
        f'### {heading_prefix}Features\n\n{subjects}\n\n'
        f'### {heading_prefix}Bug Fixes\n\n- **atspi:** Fix a crash\n\n'
        f'### {heading_prefix}Documentation\n\n- **docs:** Explain something\n\n'
        f'### {heading_prefix}Performance\n\n- **xpath:** Faster queries\n'
    )


@pytest.mark.parametrize('heading_prefix', ['', '<!-- 0 -->'])
def test_a_section_over_the_limit_keeps_the_subjects_of_what_users_notice(heading_prefix: str) -> None:
    changelog = long_section(heading_prefix)
    assert len(changelog) > MAX_LENGTH

    notes = release_notes(changelog, '1.0.0', CHANGELOG_URL)

    assert len(notes) <= MAX_LENGTH
    assert f'### {heading_prefix}Features' in notes
    assert '- **scope:** Feature number 0' in notes
    assert 'body text' not in notes
    assert '- **atspi:** Fix a crash' in notes
    assert '- **xpath:** Faster queries' in notes
    assert 'Explain something' not in notes
    assert notes.endswith(f'See the [full list of changes]({CHANGELOG_URL}).')


def test_subjects_that_do_not_fit_are_cut_at_the_end() -> None:
    subjects = '\n'.join(f'- **scope:** A feature with a long subject line, number {n:05}' for n in range(5000))
    changelog = f'## [1.0.0] - 2027-01-15\n\n### Features\n\n{subjects}\n\n### Bug Fixes\n\n- **atspi:** Fix\n'

    notes = release_notes(changelog, '1.0.0', CHANGELOG_URL)

    assert len(notes) <= MAX_LENGTH
    assert '- **scope:** A feature with a long subject line, number 00000' in notes
    assert '- **atspi:** Fix' not in notes
    assert notes.endswith(f'See the [full list of changes]({CHANGELOG_URL}).')


def test_main_prints_the_notes(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    changelog = tmp_path / 'CHANGELOG.md'
    changelog.write_text(CHANGELOG)

    extract_release_notes.main(['1.0.0', '--changelog-url', CHANGELOG_URL, '--changelog', str(changelog)])

    assert capsys.readouterr().out.startswith('### Features\n')


def test_main_fails_for_a_missing_section(tmp_path: Path) -> None:
    changelog = tmp_path / 'CHANGELOG.md'
    changelog.write_text(CHANGELOG)

    with pytest.raises(SystemExit, match=r'\[2\.0\.0\]'):
        extract_release_notes.main(['2.0.0', '--changelog-url', CHANGELOG_URL, '--changelog', str(changelog)])
