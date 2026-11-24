# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "gitpython>=3.1.45",
#   "semantic-version>=2.10.0",
# ]
# ///
"""Write CHANGELOG.md with git-cliff for the version that scripts/tools.py derives.

``cz bump`` runs this as a pre-bump hook. It fails when git-cliff is missing or fails,
so that the bump stops before it commits and tags a release without its changelog.
"""

import contextlib
import shutil
import subprocess
import sys
from pathlib import Path

if __name__ == '__main__' and not __package__:
    file = Path(__file__).resolve()
    parent, top = file.parent, file.parents[1]

    if str(top) not in sys.path:
        sys.path.append(str(top))

    with contextlib.suppress(ValueError):
        sys.path.remove(str(parent))

    __package__ = 'scripts'


from scripts.tools import get_version

GIT_CLIFF_TIMEOUT_SECONDS = 600


def main() -> None:
    current_version = get_version()

    git_cliff = shutil.which('git-cliff')
    if git_cliff is None:
        raise SystemExit('git-cliff was not found on the PATH, so CHANGELOG.md cannot be written (see CONTRIBUTING.md)')

    if current_version.prerelease:
        command = [git_cliff, '-o', 'CHANGELOG.md']
    else:
        command = [git_cliff, '--bump', '-t', f'v{current_version}', '-o', 'CHANGELOG.md']

    print(f'writing CHANGELOG.md for {current_version}', flush=True)
    try:
        subprocess.run(command, check=True, timeout=GIT_CLIFF_TIMEOUT_SECONDS)
    except subprocess.CalledProcessError as e:
        raise SystemExit(f'git-cliff failed with exit status {e.returncode}, so CHANGELOG.md was not written') from e
    except subprocess.TimeoutExpired as e:
        raise SystemExit(f'git-cliff did not finish within {GIT_CLIFF_TIMEOUT_SECONDS} seconds') from e


if __name__ == '__main__':
    main()
