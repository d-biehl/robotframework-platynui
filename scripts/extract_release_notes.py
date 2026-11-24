# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "packaging>=24",
# ]
# ///
"""Print the release notes of a version: its section of CHANGELOG.md, shortened to GitHub's limit when needed.

Run it with ``uv run scripts/extract_release_notes.py <version> --changelog-url <url>``.
"""

import argparse
import re
from pathlib import Path

from packaging.version import InvalidVersion, Version

# GitHub rejects longer release notes.
MAX_LENGTH = 125000

# The changelog groups (see cliff.toml) whose subject lines a shortened section keeps.
NOTICED_GROUPS = ('Features', 'Bug Fixes', 'Performance')


class NotesError(Exception):
    """The changelog has no notes for the version."""


def _section(changelog: str, title: str) -> str | None:
    match = re.search(
        rf'^##\s*\[{re.escape(title)}\][^\n]*\n(?P<text>.*?)(?=^##\s|\Z)',
        changelog,
        re.MULTILINE | re.DOTALL,
    )
    return match.group('text').strip() if match else None


def _group_name(heading: str) -> str:
    # git-cliff may write an ordering prefix, as in "### <!-- 0 -->Features".
    return re.sub(r'^<!--.*?-->', '', heading.removeprefix('### ')).strip()


def _shorten(notes: str, changelog_url: str) -> str:
    """Keep only the subject lines of what users notice, as many as fit, each under its headings."""
    footer = (
        'This list shows only the features, bug fixes and performance improvements. '
        f'See the [full list of changes]({changelog_url}).'
    )

    # Subject lines start at the beginning of a line; commit bodies are indented.
    subjects: list[tuple[str, str | None, str]] = []
    type_heading = ''
    scope_heading: str | None = None
    for line in notes.splitlines():
        if line.startswith('### '):
            type_heading, scope_heading = line, None
        elif line.startswith('#### '):
            scope_heading = line
        elif line.startswith('- ') and _group_name(type_heading) in NOTICED_GROUPS:
            subjects.append((type_heading, scope_heading, line))

    def outline(count: int) -> str:
        lines: list[str] = []
        last_type: str | None = None
        last_scope: str | None = None
        for subject_type, subject_scope, subject in subjects[:count]:
            headings = []
            if subject_type != last_type:
                headings.append(subject_type)
                last_type, last_scope = subject_type, None
            if subject_scope is not None and subject_scope != last_scope:
                headings.append(subject_scope)
            last_scope = subject_scope
            for heading in headings:
                if lines and lines[-1]:
                    lines.append('')
                lines += [heading, '']
            lines.append(subject)
        return '\n'.join([*lines, '', footer]).strip()

    # The outline grows with every subject, so the longest that fits is found by bisection.
    low, high = 0, len(subjects)
    while low < high:
        middle = (low + high + 1) // 2
        if len(outline(middle)) <= MAX_LENGTH:
            low = middle
        else:
            high = middle - 1
    return outline(low)


def release_notes(changelog: str, version: str, changelog_url: str) -> str:
    """Return the notes of ``version`` (SemVer, as in its tag) from the text of CHANGELOG.md.

    A pre-release takes the ``[Unreleased]`` section: ``cz bump`` writes the changelog
    before it creates the tag, and git-cliff gets no tag for a pre-release.
    """
    try:
        is_prerelease = Version(version).is_prerelease
    except InvalidVersion as e:
        raise NotesError(f'{version} is no valid version: {e}') from e

    title = 'Unreleased' if is_prerelease else version
    notes = _section(changelog, title)
    if notes is None:
        raise NotesError(f'CHANGELOG.md has no section [{title}] for version {version}')
    return notes if len(notes) <= MAX_LENGTH else _shorten(notes, changelog_url)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument('version', help='the version, as its tag names it without the leading v')
    parser.add_argument('--changelog-url', required=True, help='where a shortened list links to the full changelog')
    parser.add_argument('--changelog', type=Path, default=Path('CHANGELOG.md'))
    args = parser.parse_args(argv)

    try:
        print(release_notes(args.changelog.read_text(encoding='utf-8'), args.version, args.changelog_url))
    except (NotesError, OSError) as e:
        raise SystemExit(str(e)) from e


if __name__ == '__main__':
    main()
