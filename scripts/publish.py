# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "packaging>=24",
#   "semantic-version>=2.10",
# ]
# ///
"""Release a version tag: check the tag, then upload the wheels of its CI run to PyPI.

Targets:

- ``check-tag <tag>`` writes ``skip``, ``version`` and ``is-prerelease`` to ``GITHUB_OUTPUT``.
  A ``-dev.0`` tag starts a version line and gives ``skip=true``: its dev versions come
  from the nightly upload. Every other dev tag is refused, because the nightly numbers
  after it would start again at ``.dev1`` and collide with file names PyPI has used.
- ``upload <dist> --version <version>`` checks that ``dist`` holds the complete wheel set
  of that version and uploads it, the root project last.

Run it with ``uv run scripts/publish.py <target> ...``.
"""

import argparse
import contextlib
import sys
from dataclasses import dataclass
from pathlib import Path

import semantic_version
from packaging.version import InvalidVersion, Version

if __name__ == '__main__' and not __package__:
    file = Path(__file__).resolve()
    parent, top = file.parent, file.parents[1]

    if str(top) not in sys.path:
        sys.path.append(str(top))

    with contextlib.suppress(ValueError):
        sys.path.remove(str(parent))

    __package__ = 'scripts'


from scripts.release_wheels import UploadError, WheelSetError, collect_wheels, upload, write_output


class TagError(Exception):
    """The tag names no version that can be released."""


@dataclass(frozen=True)
class TagCheck:
    skip: bool
    version: str
    is_prerelease: bool


def check_tag(tag: str) -> TagCheck:
    """Decide what the release job does with ``tag``."""
    if not tag.startswith('v'):
        raise TagError(f'{tag} is no version tag: it does not start with v')
    try:
        parsed = semantic_version.Version(tag[1:])
    except ValueError as e:
        raise TagError(f'{tag} names no SemVer version: {e}') from e
    version = str(parsed)
    try:
        Version(version)
    except InvalidVersion as e:
        raise TagError(f'{tag} names a version that PyPI cannot represent: {e}') from e

    prerelease: tuple[str, ...] = parsed.prerelease or ()
    if prerelease[:1] == ('dev',):
        if prerelease == ('dev', '0'):
            return TagCheck(skip=True, version=version, is_prerelease=True)
        raise TagError(
            f'{tag} is a dev tag: dev versions are published by the nightly upload. '
            f'Only a -dev.0 tag, which starts a version line, is accepted.'
        )

    return TagCheck(skip=False, version=version, is_prerelease=bool(prerelease))


def run_check_tag(tag: str) -> None:
    check = check_tag(tag)
    if check.skip:
        base = check.version.removesuffix('-dev.0')
        print(f'{tag} starts the version line {base}; the nightly upload publishes its dev versions')
    else:
        print(f'{tag} releases {check.version}{" as a pre-release" if check.is_prerelease else ""}')
    write_output('skip', 'true' if check.skip else 'false')
    write_output('version', check.version)
    write_output('is-prerelease', 'true' if check.is_prerelease else 'false')


def upload_release(dist: Path, version: str) -> None:
    """Upload the wheel set in ``dist``, if it carries ``version`` (SemVer, as in the tag)."""
    expected = Version(version)
    wheels = collect_wheels(dist)
    if wheels.version != expected:
        raise TagError(f'the wheels carry version {wheels.version}, but the tag names {expected}')
    upload(wheels)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    targets = parser.add_subparsers(dest='target', required=True)
    check = targets.add_parser('check-tag', help='decide what the release job does with a tag')
    check.add_argument('tag')
    upload_parser = targets.add_parser('upload', help='upload the wheel set of a tag to PyPI')
    upload_parser.add_argument('dist', type=Path, help='the directory with one subdirectory per wheel artifact')
    upload_parser.add_argument('--version', required=True, help='the version the tag names, without the leading v')
    args = parser.parse_args(argv)

    try:
        if args.target == 'check-tag':
            run_check_tag(args.tag)
        else:
            upload_release(args.dist.absolute(), args.version)
    except (TagError, WheelSetError, UploadError) as e:
        raise SystemExit(str(e)) from e


if __name__ == '__main__':
    main()
