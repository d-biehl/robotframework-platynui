*** Settings ***
Documentation     Mock-backed checks for the explicit wait keywords — Wait Until Exists, Wait Until
...               Gone, Wait Until Query and Wait Until Attribute Value. The import sets a small
...               0.2 s default so the timeout-driven checks stay fast; waits are observed through
...               the timeout value in the raised error, exactly like query_settings.robot. The
...               captured-node "gone success" direction, and a value that changes while a keyword
...               waits, are covered in the egui acceptance lane, since the mock neither invalidates
...               a captured node nor changes a value on its own.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${MISSING}        //control:Button[@Name="NoSuchButton"]
${OPS_NAME}       //control:Window[@Name="Operations Console"]/@Name
${OPS_MAX}        //control:Window[@Name="Operations Console"]/@IsMaximized
${MISSING_CNT}    count(//control:Button[@Name="NoSuchButton"])
${MISSING_ROOT}   //control:Window[@Name="NoSuchWindow"]
# Relative, so resolving it needs the root — an absolute selector ignores the root and never
# triggers its resolution at all.
${MISSING_INSIDE_ROOT}    .//control:Button[@Name="NoSuchButton"]


*** Test Cases ***
# --- Wait Until Exists ---------------------------------------------------------

Wait Until Exists Returns The Element
    ${el}=    Wait Until Exists    ${OPS}
    Should Be Equal    ${el.name}    Operations Console
    Should Be Equal    ${el.role}    Window

Wait Until Exists Times Out With A User Facing Message
    Run Keyword And Expect Error    *No element matched*within timeout of 0.2 seconds*
    ...    Wait Until Exists    ${MISSING}

Wait Until Exists Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Exists    ${MISSING}    query_overrides={'timeout': 0.6}

Wait Until Exists Honors Scope Settings
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Wait Until Exists    ${MISSING}

Wait Until Exists Rejects A Non Element Selector
    Run Keyword And Expect Error    *did not return an element*    Wait Until Exists    count(//control:Window)

Wait Until Exists Does Not Leak Overrides Across The Shared Cache
    Set Query Settings    {'timeout': 0.5}    scope=TEST
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Exists    ${MISSING}    query_overrides={'timeout': 0.2}
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*    Wait Until Exists    ${MISSING}

Wait Until Exists With Ignore Exceptions Names The Last Error
    [Documentation]    The malformed selector raises on every attempt. ignore_exceptions keeps the wait
    ...    going until the timeout, and the failure then quotes the error it swallowed last.
    Run Keyword And Expect Error
    ...    *No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *
    ...    Wait Until Exists    //control:Window[broken    query_overrides={'ignore_exceptions': True}

Wait Until Exists With Ignore Exceptions Still Rejects A Value Selector
    [Documentation]    A selector that yields a value can never yield an element, so waiting cannot
    ...    fix it: the wait ends on the first attempt, whatever ignore_exceptions says.
    Run Keyword And Expect Error    ResultTypeError: Query 'count(//control:Window)' did not return an element*
    ...    Wait Until Exists    count(//control:Window)    query_overrides={'ignore_exceptions': True}

# --- Wait Until Gone -----------------------------------------------------------

Wait Until Gone Returns Fast When Already Absent
    Wait Until Gone    ${MISSING}

Wait Until Gone Times Out While The Selector Persists
    Run Keyword And Expect Error    *still present*within timeout of 0.2 seconds*    Wait Until Gone    ${OPS}

Wait Until Gone Times Out For A Still Valid Captured Node
    ${el}=    Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error
    ...    *Captured element Window "Operations Console" was still valid within timeout of 0.2 seconds.
    ...    Wait Until Gone    ${el}

Wait Until Gone Ignores A Stale Cached Descriptor Node
    Get Attribute Value    ${OPS}    Name
    Run Keyword And Expect Error    *still present*within timeout of 0.2 seconds*    Wait Until Gone    ${OPS}

Wait Until Gone Rejects A Value Selector
    Run Keyword And Expect Error    *Use Wait Until Query for value conditions*
    ...    Wait Until Gone    count(//control:Window)

Wait Until Gone Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Gone    ${OPS}    query_overrides={'timeout': 0.6}

Wait Until Gone With Ignore Exceptions Never Reports Gone
    [Documentation]    The malformed selector raises on every attempt, so no attempt saw the element
    ...    either way: the failure says it could not be confirmed gone, and quotes the last error.
    Run Keyword And Expect Error
    ...    ElementStillPresentError: *could not be confirmed gone within timeout of 0.2 seconds. The last error was: EvaluationError: *
    ...    Wait Until Gone    //control:Window[broken    query_overrides={'ignore_exceptions': True}

Wait Until Gone Fails On A Missing Root Even With Ignore Exceptions
    [Documentation]    The root is looked up outside the errors ignore_exceptions swallows, with its
    ...    own 0.2 s timeout: a root that cannot be found ends the wait with its own error, not
    ...    with a claim about the target.
    Set Root    ${MISSING_ROOT}    scope=TEST
    Run Keyword And Expect Error
    ...    RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; *NoSuchButton*was not evaluated.
    ...    Wait Until Gone    ${MISSING_INSIDE_ROOT}    query_overrides={'timeout': 1, 'ignore_exceptions': True}

# --- Wait Until Query ----------------------------------------------------------

Wait Until Query Default Passes On A Truthy Value
    ${n}=    Wait Until Query    count(//control:Window)
    Should Be True    ${n} > 0

Wait Until Query Default Times Out On A Falsy Value
    Run Keyword And Expect Error    *was 0 and did not become truthy within timeout of 0.2 seconds.
    ...    Wait Until Query    ${MISSING_CNT}

Wait Until Query Default Times Out On A Falsy Attribute
    [Documentation]    A fresh mock window is not maximized, so @IsMaximized is False. The default
    ...    must test the attribute's value (falsy) — not the always-true wrapper — time out, and
    ...    name that value in its failure.
    Run Keyword And Expect Error    *was False and did not become truthy within timeout of 0.2 seconds.
    ...    Wait Until Query    ${OPS_MAX}

Wait Until Query Default Names An Expression That Matched Nothing
    Run Keyword And Expect Error    *matched nothing and did not become truthy within timeout of 0.2 seconds.
    ...    Wait Until Query    ${MISSING}/@Name

Wait Until Query Default Returns The Raw Result
    ${r}=    Wait Until Query    ${OPS_NAME}
    Should Be Equal As Strings    ${r}    Operations Console

Wait Until Query Passes With A Comparison Operator
    Wait Until Query    ${OPS_NAME}    ==    Operations Console

Wait Until Query Surfaces The Assertion Diagnostic On Timeout
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${OPS_NAME}    ==    Wrong Name

Wait Until Query Expected Without Operator Does Not Raise ValueError
    ${r}=    Wait Until Query    count(//control:Window)    ${None}    ${5}
    Should Be True    ${r} > 0

Wait Until Query Times Out On An Order Operator That Stays False
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    >    ${0}

Wait Until Query Rejects The Then Operator
    Run Keyword And Expect Error    *Use 'validate'*    Wait Until Query    count(//control:Window)    then    value > 0

Wait Until Query Polls With The Validate Operator
    Wait Until Query    count(//control:Window)    validate    value > 0
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    validate    value > 0

Wait Until Query Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Query    ${MISSING_CNT}    >    ${0}    query_overrides={'timeout': 0.6}

Wait Until Query Evaluates Against A Root Node
    ${win}=    Query    ${OPS}    only_first=${True}
    ${r}=    Wait Until Query    count(.//item:ListItem)    >    ${0}    root=${win}
    Should Be True    ${r} > 0

Wait Until Query Matches Get Attribute Value For A Present Attribute
    Wait Until Query    ${OPS_NAME}    ==    Operations Console
    Get Attribute Value    ${OPS}    Name    ==    Operations Console

Wait Until Query With Ignore Exceptions Times Out On A Bad Expression
    Run Keyword And Expect Error
    ...    *did not become truthy within timeout of 0.2 seconds. The last error was: EvaluationError: *
    ...    Wait Until Query    count(//control:Window[broken    query_overrides={'ignore_exceptions': True}

Wait Until Query With An Operator Names The Last Error Instead Of Raising It
    [Documentation]    The evaluation raises on every attempt. The wait neither evaluates once more
    ...    after the timeout nor lets the error escape: it fails with an assertion error that quotes it.
    ${message}=    Run Keyword And Expect Error
    ...    *did not satisfy the assertion within timeout of 0.2 seconds. The last error was: EvaluationError: *
    ...    Wait Until Query    count(//control:Window[broken    >    ${0}    query_overrides={'ignore_exceptions': True}
    Should Start With    ${message}    Query 'count(//control:Window[broken' did not satisfy

Wait Until Query Keeps Waiting On A Value It Cannot Compare
    Run Keyword And Expect Error    *not supported between*within timeout of 0.2 seconds*
    ...    Wait Until Query    ${OPS_NAME}    >    ${5}

Wait Until Query Names The Result A Raising Check Could Not Check
    [Documentation]    The expression is evaluated, but the pattern is not a valid regular expression, so
    ...    the check raises: the failure names the result the check was given, and the error it raised.
    ${message}=    Run Keyword And Expect Error
    ...    *was 'Operations Console' and could not be checked within timeout of 0.2 seconds. The last error was: *unterminated subpattern*
    ...    Wait Until Query    ${OPS_NAME}    matches    (    query_overrides={'ignore_exceptions': True}
    Should Start With    ${message}    Query '${OPS_NAME}' was 'Operations Console'

Wait Until Query Ends At Once When A Validate Expression Raises
    [Documentation]    Robot Framework reports a validate expression that cannot be evaluated as a
    ...    RuntimeError, and a RuntimeError ends every wait at once, whatever ignore_exceptions says:
    ...    the error comes back as it is, not as a timeout.
    Run Keyword And Expect Error    Evaluating expression*failed: NameError: *
    ...    Wait Until Query    ${OPS_NAME}    validate    valu == 'x'    query_overrides={'ignore_exceptions': True}

Wait Until Query Names The Expression A Missing Root Kept From Being Evaluated
    Set Root    ${MISSING_ROOT}    scope=TEST
    Run Keyword And Expect Error
    ...    RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; 'count(.//control:Button)' was not evaluated.
    ...    Wait Until Query    count(.//control:Button)

Wait Until Query Evaluates A Computed Expression Against The Root
    [Documentation]    The relative path sits inside the function's argument, and that is enough to
    ...    need the root: the count is taken inside it, where the window holds 4 of the desktop's
    ...    8 list items.
    Set Root    ${OPS}    scope=TEST
    ${n}=    Wait Until Query    count(.//item:ListItem)
    Should Be Equal As Integers    ${n}    4

Wait Until Query Does Not Need The Root For An Absolute Expression
    [Documentation]    An absolute expression starts at the desktop, so a root that cannot be found
    ...    does not stop it: the root is never looked up.
    Set Root    ${MISSING_ROOT}    scope=TEST
    ${n}=    Wait Until Query    count(//control:Window)    >    ${0}
    Should Be True    ${n} > 0

# --- Wait Until Attribute Value ------------------------------------------------

Wait Until Attribute Value Returns A Value That Already Holds
    [Documentation]    The value comes back typed as Get Attribute Value reads it — here the boolean False.
    ${value}=    Wait Until Attribute Value    ${OPS}    IsMaximized    ==    ${False}
    ${read}=    Get Attribute Value    ${OPS}    IsMaximized
    Should Be Equal    ${value}    ${read}
    Should Be True    ${{ type($value) is bool }}

Wait Until Attribute Value Default Returns A Truthy Value
    ${name}=    Wait Until Attribute Value    ${OPS}    Name
    Should Be Equal    ${name}    Operations Console

Wait Until Attribute Value Default Times Out On A Falsy Value
    Run Keyword And Expect Error
    ...    *'IsMaximized'*was False and did not become truthy within timeout of 0.2 seconds.
    ...    Wait Until Attribute Value    ${OPS}    IsMaximized

Wait Until Attribute Value Returns The Value For Matches With Groups
    [Documentation]    AssertionEngine hands back the capture groups for ``matches``; the keyword
    ...    returns the attribute's value instead.
    ${name}=    Wait Until Attribute Value    ${OPS}    Name    matches    (Operations) (Console)
    Should Be Equal    ${name}    Operations Console

Wait Until Attribute Value Polls With The Validate Operator
    Run Keyword And Expect Error    *should validate to true*within timeout of 0.2 seconds*
    ...    Wait Until Attribute Value    ${OPS}    IsMaximized    validate    value == True

Wait Until Attribute Value Rejects The Then Operator
    Run Keyword And Expect Error    *Use 'validate'*
    ...    Wait Until Attribute Value    ${OPS}    Name    then    value.upper()

Wait Until Attribute Value Surfaces The Assertion Diagnostic On Timeout
    Run Keyword And Expect Error
    ...    *'Name'*'Operations Console' (str) should be 'Wrong Name' (str) (within timeout of 0.2 seconds)
    ...    Wait Until Attribute Value    ${OPS}    Name    ==    Wrong Name

Wait Until Attribute Value Times Out When The Element Never Appears
    Run Keyword And Expect Error    *No element matched*within timeout of 0.2 seconds.
    ...    Wait Until Attribute Value    ${MISSING}    Name    ==    OK

Wait Until Attribute Value Waits For A Missing Attribute And Names It
    [Documentation]    The window has no ToggleState: the keyword waits for it instead of failing at
    ...    once, and the timeout error names the attribute and the element.
    Run Keyword And Expect Error
    ...    *'ToggleState' did not appear on Window "Operations Console" within timeout of 0.2 seconds.
    ...    Wait Until Attribute Value    ${OPS}    ToggleState    ==    On

Wait Until Attribute Value Rejects An Unknown Namespace Prefix
    Run Keyword And Expect Error    *Unknown namespace prefix 'nosuch'*
    ...    Wait Until Attribute Value    ${OPS}    nosuch:Name    ==    Operations Console

Wait Until Attribute Value Rejects A Value Selector
    Run Keyword And Expect Error    *Use Wait Until Query for value conditions*
    ...    Wait Until Attribute Value    count(//control:Window)    Name
    Run Keyword And Expect Error    *Use Wait Until Query for value conditions*
    ...    Wait Until Attribute Value    ${OPS_NAME}    Name

Wait Until Attribute Value Honors Per Call Timeout
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    Wait Until Attribute Value    ${OPS}    Name    ==    Wrong Name    query_overrides={'timeout': 0.6}

Wait Until Attribute Value With Ignore Exceptions Never Succeeds On A Bad Selector
    Run Keyword And Expect Error    *No element matched*within timeout of 0.2 seconds.
    ...    Wait Until Attribute Value    //control:Window[broken    Name
    ...    query_overrides={'ignore_exceptions': True}

Wait Until Attribute Value Keeps Waiting On A Value It Cannot Compare
    Run Keyword And Expect Error    *not supported between*within timeout of 0.2 seconds*
    ...    Wait Until Attribute Value    ${OPS}    Name    >    ${5}
