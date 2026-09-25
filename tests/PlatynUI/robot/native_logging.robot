*** Settings ***
Documentation     Fixture for ``tests/PlatynUI/test_native_logging_rf.py``: drives native diagnostics
...               into the Robot Framework log, and the pytest reads them back from ``output.xml``.
...               It lives outside ``tests/BareMetal`` because its assertions are in the pytest; run on
...               its own it only proves that nothing fails. The pytest sets ``NATIVE_LOG_LEVEL`` and
...               RF's ``--loglevel`` per run. The hidden emitter exists in mock builds only.
Library           PlatynUI.BareMetal    use_mock=${True}    native_log_level=${NATIVE_LOG_LEVEL}


*** Variables ***
${NATIVE_LOG_LEVEL}       ${None}


*** Test Cases ***
A Native Trace Is Logged Inside The Query That Evaluated It
    Query    trace(count(//*), 'node-count')    only_first=${True}

A Warning On The Calling Thread Is An RF Warning
    Evaluate    platynui_native._native._emit_log_for_tests('warn', 'warning on the calling thread')

A Background Thread's Warning Names Its Thread
    Evaluate    platynui_native._native._emit_log_for_tests('warn', 'warning from a background thread', on_background_thread=True)

RF's Own Log Level Still Applies
    Query    trace(1, 'before-set-log-level')    only_first=${True}
    Set Log Level    DEBUG
    Query    trace(2, 'after-set-log-level')    only_first=${True}
