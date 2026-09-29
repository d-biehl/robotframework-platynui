"""`Get Attribute`, the deprecated alias of `Get Attribute Value` (spec: *baremetal-attribute-reading*).

The alias is tested here rather than in a mock suite: a suite that calls it would put a deprecation
warning into every run of the mock lane.
"""

import subprocess
import sys
import textwrap
from pathlib import Path
from typing import Any

from robot.api import ExecutionResult
from robot.libdocpkg import LibraryDocumentation

_SUITE = textwrap.dedent(
    """\
    *** Settings ***
    Library    PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}

    *** Variables ***
    ${OPS}    //control:Window[@Name="Operations Console"]

    *** Test Cases ***
    The Alias Returns What Get Attribute Value Returns
        FOR    ${attribute}    IN    Name    IsMaximized
            ${old}=    Get Attribute    ${OPS}    ${attribute}
            ${new}=    Get Attribute Value    ${OPS}    ${attribute}
            Should Be Equal    ${old}    ${new}
            Should Be True    ${{ type($old) is type($new) }}
        END

    The Alias Checks An Assertion Once
        [Timeout]    10 seconds
        ${old}=    Run Keyword And Expect Error    *
        ...    Get Attribute    ${OPS}    Name    ==    Wrong Name    query_overrides={'timeout': 30}
        ${new}=    Run Keyword And Expect Error    *
        ...    Get Attribute Value    ${OPS}    Name    ==    Wrong Name    query_overrides={'timeout': 30}
        Should Be Equal    ${old}    ${new}
    """
)
# How often the suite above calls the alias: twice in the loop, once for the assertion.
_ALIAS_CALLS = 3


def test_libdoc_marks_the_alias_as_deprecated() -> None:
    keywords = {kw.name: kw for kw in LibraryDocumentation('PlatynUI.BareMetal').keywords}
    alias = keywords['Get Attribute']
    assert alias.deprecated
    assert 'Get Attribute Value' in alias.short_doc, alias.short_doc
    assert '1.0' in alias.short_doc, alias.short_doc
    assert not keywords['Get Attribute Value'].deprecated


def test_the_alias_behaves_like_get_attribute_value_and_warns(tmp_path: Path) -> None:
    suite = tmp_path / 'alias.robot'
    suite.write_text(_SUITE, encoding='utf-8')
    output = tmp_path / 'output.xml'
    run = subprocess.run(
        [sys.executable, '-m', 'robot', '--output', str(output), '--log', 'NONE', '--report', 'NONE', str(suite)],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    assert run.returncode == 0, run.stdout + run.stderr
    result: Any = ExecutionResult(str(output))
    warnings = [str(error.message) for error in result.errors.messages if 'deprecated' in str(error.message)]
    assert len(warnings) == _ALIAS_CALLS, warnings
    for warning in warnings:
        assert "'PlatynUI.BareMetal.Get Attribute'" in warning, warning
        assert 'Get Attribute Value' in warning, warning
