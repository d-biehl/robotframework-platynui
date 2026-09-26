"""A test build used without its mock backend says so once (spec: *diagnostic-logging*).

Runs in a subprocess, because the warning is once per process and the tests of this package
share one. Needs the mock build of ``platynui_native``, which links no real platform or provider.
"""

import json
import os
import subprocess
import sys
import textwrap

_SCRIPT = textwrap.dedent(
    """
    import json
    import logging

    records = []

    class Collect(logging.Handler):
        def emit(self, record):
            records.append((record.levelname, record.name, record.getMessage()))

    logging.getLogger('platynui').addHandler(Collect())
    logging.getLogger('platynui').setLevel(logging.WARNING)

    from platynui_native import Runtime

    Runtime()
    after_first = list(records)
    Runtime()
    after_second = list(records)
    Runtime.new_with_mock()
    print(json.dumps({'first': after_first, 'second': after_second, 'mock': records[len(after_second):]}))
    """
)


def test_a_test_build_without_its_mock_backend_warns_once() -> None:
    env = {k: v for k, v in os.environ.items() if k not in ('RUST_LOG', 'PLATYNUI_LOG_LEVEL')}
    result = subprocess.run(
        [sys.executable, '-c', _SCRIPT], capture_output=True, text=True, timeout=120, env=env, check=False
    )
    assert result.returncode == 0, result.stderr
    seen = json.loads(result.stdout)
    [(level, _name, message)] = seen['first']
    assert level == 'WARNING'
    assert 'test build' in message, message
    assert 'mock-provider' in message, message
    assert seen['second'] == seen['first'], 'the second runtime adds no record'
    assert seen['mock'] == [], 'the mock backend is what a test build is for'
