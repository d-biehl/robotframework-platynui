*** Settings ***
Documentation     Parent suite for the scope-boundary checks. It pins context twice, through two
...               imports, to separate the two scopes that differ exactly here:
...
...               - the plain import uses ``scope=SUITE`` — this suite only, the same boundary Robot
...                 Framework's own suite variables draw (measured: a ``Set Suite Variable`` set here
...                 reads ``<unset>`` in a child suite);
...               - ``TREE`` uses ``scope=SUITES`` — the suite *and* the suites below it, which is
...                 what makes a directory's ``__init__.robot`` usable as a fixture.
...
...               The child suites assert both, so neither the boundary nor the opt-in past it can
...               move unnoticed. Each suite gets its own library instance and runtime, so what
...               travels is the selector, re-resolved there.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}    AS    TREE
Suite Setup       Pin The Context


*** Keywords ***
Pin The Context
    # Fully qualified: with two imports of the same library in this suite, a bare `Set Root` is
    # ambiguous. The child suites import only the one they exercise, so they can stay unqualified.
    PlatynUI.BareMetal.Set Root    //control:Window[@Name="Operations Console"]    scope=SUITE
    PlatynUI.BareMetal.Set Query Settings    {'timeout': 0.5}    scope=SUITE
    TREE.Set Root    //control:Window[@Name="Operations Console"]    scope=SUITES
    TREE.Set Query Settings    {'timeout': 0.5}    scope=SUITES
