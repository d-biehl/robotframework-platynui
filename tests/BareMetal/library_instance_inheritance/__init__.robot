*** Settings ***
Documentation     Parent suite for the scope-inheritance checks: it sets a suite-scoped root and
...               nothing else. Robot Framework builds a fresh library instance for every suite
...               (the library is suite-scoped), while suite variables are inherited by child
...               suites — so a child importing the library under the same name with the same
...               arguments must still see this root, and one importing it differently must not.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}
Suite Setup       Pin The Context For Every Suite Below


*** Keywords ***
Pin The Context For Every Suite Below
    Set Root    //control:Window[@Name="Operations Console"]    scope=SUITE
    Set Query Settings    {'timeout': 0.5}    scope=SUITE
