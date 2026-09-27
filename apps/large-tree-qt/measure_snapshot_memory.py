"""Measure what a UI snapshot costs in memory, against a tree the repository owns.

Starts the large-tree app next to this file and waits until its widgets are on
the accessibility tree. Then it evaluates one search that matches nothing under
the app's application node, many times: once keeping the snapshot, once
discarding it before each run. It reports how much this process's private memory
grows per evaluation and per element, over several rounds so that the noise
shows, and how long ``clear_cache()`` takes after a full snapshot.

A flat result means something only next to a build that leaks. Run the same
script with the interpreter of an environment that has an older build of the
native extension, for example from a git worktree, as the positive control.

Read-only: no pointer or keyboard input. It runs on Windows (UI Automation) and
on Linux (AT-SPI). On Linux it belongs inside the lanes' session scripts, which
bring up AT-SPI and enable Qt's accessibility:

    scripts/startxsession.sh -- scripts/platynui-robot-session.sh \\
        uv run python apps/large-tree-qt/measure_snapshot_memory.py

Usage
-----
    uv run python apps/large-tree-qt/measure_snapshot_memory.py [--groups 50] [--items 20] [--runs 100]
"""

import argparse
import gc
import json
import os
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path

import platynui_native as pn
import platynui_native._native as native_extension

APP = Path(__file__).with_name('main.py')
#: Matches nothing, so every evaluation walks the whole tree below the context.
QUERY = ".//*[@Name='x-not-there']"
#: The provider settings of the measuring runtime: the Java provider has nothing to serve here.
CONFIG: dict[str, object] = {'providers': {'java': {'enabled': False}}}

if sys.platform == 'win32':
    import ctypes
    from ctypes import wintypes

    class _MemoryCounters(ctypes.Structure):
        """``PROCESS_MEMORY_COUNTERS_EX``."""

        _fields_ = [
            ('cb', wintypes.DWORD),
            ('PageFaultCount', wintypes.DWORD),
            ('PeakWorkingSetSize', ctypes.c_size_t),
            ('WorkingSetSize', ctypes.c_size_t),
            ('QuotaPeakPagedPoolUsage', ctypes.c_size_t),
            ('QuotaPagedPoolUsage', ctypes.c_size_t),
            ('QuotaPeakNonPagedPoolUsage', ctypes.c_size_t),
            ('QuotaNonPagedPoolUsage', ctypes.c_size_t),
            ('PagefileUsage', ctypes.c_size_t),
            ('PeakPagefileUsage', ctypes.c_size_t),
            ('PrivateUsage', ctypes.c_size_t),
        ]

    _kernel32 = ctypes.WinDLL('kernel32')
    _kernel32.GetCurrentProcess.restype = wintypes.HANDLE
    _psapi = ctypes.WinDLL('psapi')
    _psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(_MemoryCounters), wintypes.DWORD]
    _psapi.GetProcessMemoryInfo.restype = wintypes.BOOL

    def private_memory() -> int:
        """This process's private memory in bytes: ``PrivateUsage`` from ``GetProcessMemoryInfo``."""
        counters = _MemoryCounters()
        counters.cb = ctypes.sizeof(_MemoryCounters)
        if not _psapi.GetProcessMemoryInfo(_kernel32.GetCurrentProcess(), ctypes.byref(counters), counters.cb):
            raise ctypes.WinError()
        return int(counters.PrivateUsage)

else:

    def private_memory() -> int:
        """This process's private memory in bytes: anonymous and swapped memory from ``/proc/self/smaps_rollup``."""
        total = 0
        with Path('/proc/self/smaps_rollup').open(encoding='ascii') as rollup:
            for line in rollup:
                key, _, rest = line.partition(':')
                if key in ('Anonymous', 'Swap'):
                    total += int(rest.split()[0]) * 1024
        return total


@dataclass
class Round:
    """One round of evaluations in one mode."""

    bytes_per_evaluation: float
    bytes_per_element: float
    milliseconds_per_evaluation: float


@dataclass
class Report:
    """Everything one measurement found, as printed and as written to ``--json``."""

    native_module: str
    native_module_built: str
    platform: str
    groups: int
    items: int
    elements: int
    runs: int
    retained: list[Round] = field(default_factory=list)
    discarded: list[Round] = field(default_factory=list)
    clear_cache_milliseconds: list[float] = field(default_factory=list)


def launch(title: str, groups: int, items: int, lifetime: int, log: Path) -> subprocess.Popen[bytes]:
    """Start the large-tree app so that the returned process is the app itself.

    On Windows, uv's virtual-environment interpreter is a launcher that starts
    the base interpreter as a child, so its process id is not the app's. The
    base interpreter, started with the environment as ``__PYVENV_LAUNCHER__``,
    runs the app itself; the ``test-acceptance-windows`` recipe does the same.
    """
    interpreter = sys.executable
    env = dict(os.environ)
    base = str(getattr(sys, '_base_executable', sys.executable))
    if sys.platform == 'win32' and base != sys.executable:
        env['__PYVENV_LAUNCHER__'] = sys.executable
        interpreter = base
    command = [
        interpreter,
        str(APP),
        '--title',
        title,
        '--groups',
        str(groups),
        '--items',
        str(items),
        '--auto-close',
        str(lifetime),
    ]
    with log.open('wb') as output:
        return subprocess.Popen(command, env=env, stdout=output, stderr=subprocess.STDOUT)


def number(runtime: pn.Runtime, expression: str, context: pn.UiNode) -> int:
    """The integer an XPath expression such as ``count(...)`` evaluates to under *context*."""
    result = runtime.evaluate(expression, context)
    value = result[0] if result else None
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f'{expression} gave {value!r}, not a number')
    return int(value)


def application(runtime: pn.Runtime, pid: int, expected: int, timeout: float) -> pn.UiNode:
    """The app's application node, once all *expected* named widgets are on the tree."""
    deadline = time.monotonic() + timeout
    seen = 0
    while time.monotonic() < deadline:
        runtime.clear_cache()
        node = runtime.evaluate_single(f'/app:Application[@ProcessId={pid}]')
        if isinstance(node, pn.UiNode):
            seen = number(runtime, "count(.//*[starts-with(@Name, 'item-') or starts-with(@Name, 'group-')])", node)
            if seen >= expected:
                return node
        time.sleep(0.25)
    raise SystemExit(f'the app did not show its {expected} widgets within {timeout:.0f} s (saw {seen})')


def measure(runtime: pn.Runtime, node: pn.UiNode, runs: int, discard: bool, elements: int) -> Round:
    """Evaluate the query *runs* times under *node* and return the growth per evaluation."""
    for _ in range(3):
        if discard:
            runtime.clear_cache()
        runtime.evaluate(QUERY, node)
    gc.collect()
    before = private_memory()
    started = time.perf_counter()
    for _ in range(runs):
        if discard:
            runtime.clear_cache()
        runtime.evaluate(QUERY, node)
    elapsed = time.perf_counter() - started
    gc.collect()
    growth = (private_memory() - before) / runs
    return Round(growth, growth / elements, elapsed / runs * 1000)


def clear_cache_time(runtime: pn.Runtime, node: pn.UiNode) -> float:
    """Milliseconds ``clear_cache()`` takes after a full snapshot of the tree below *node*."""
    runtime.clear_cache()
    runtime.evaluate(QUERY, node)
    started = time.perf_counter()
    runtime.clear_cache()
    return (time.perf_counter() - started) * 1000


def run(args: argparse.Namespace) -> Report:
    """Start the app, measure, and end the app."""
    title = f'PlatynUI Large Tree {os.getpid()}'
    log = Path(tempfile.gettempdir()) / f'large-tree-qt-{os.getpid()}.log'
    app = launch(title, args.groups, args.items, args.lifetime, log)
    runtime = pn.Runtime(CONFIG)
    try:
        node = application(runtime, app.pid, args.groups * (args.items + 1), args.timeout)
        runtime.clear_cache()
        elements = number(runtime, 'count(.//*)', node)
        module = Path(native_extension.__file__)
        built = time.strftime('%Y-%m-%d %H:%M:%S', time.localtime(module.stat().st_mtime))
        report = Report(str(module), built, sys.platform, args.groups, args.items, elements, args.runs)
        for _ in range(args.rounds):
            report.retained.append(measure(runtime, node, args.runs, discard=False, elements=elements))
        for _ in range(args.rounds):
            report.discarded.append(measure(runtime, node, args.runs, discard=True, elements=elements))
        report.clear_cache_milliseconds = [clear_cache_time(runtime, node) for _ in range(args.rounds)]
        return report
    except SystemExit:
        print(f'app output: {log}', file=sys.stderr)
        raise
    finally:
        runtime.shutdown()
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()


def show(report: Report) -> None:
    """Print *report* as a table."""
    print(f'native module: {report.native_module} (built {report.native_module_built})')
    print(f'platform: {report.platform}; {report.elements} elements under the application node')
    print(f'{report.groups} groups of {report.items} widgets; {report.runs} evaluations per round')
    for mode, rounds in (('retained', report.retained), ('discarded', report.discarded)):
        for index, found in enumerate(rounds, start=1):
            print(
                f'{mode:9s} round {index}: {found.bytes_per_evaluation / 1024:9.1f} KiB per evaluation, '
                f'{found.bytes_per_element:8.1f} B per element, {found.milliseconds_per_evaluation:7.1f} ms each'
            )
    times = ', '.join(f'{value:.1f}' for value in report.clear_cache_milliseconds)
    print(f'clear_cache() after a full snapshot: {times} ms')


def main(argv: list[str] | None = None) -> int:
    """Parse the arguments, measure, and print the result."""
    parser = argparse.ArgumentParser(
        prog='measure-snapshot-memory',
        description='Measure the memory a UI snapshot costs, against the large-tree app.',
    )
    parser.add_argument('--groups', type=int, default=50, help='Group boxes in the app (default 50).')
    parser.add_argument('--items', type=int, default=20, help='Widgets per group box (default 20).')
    parser.add_argument('--runs', type=int, default=100, help='Evaluations per round (default 100).')
    parser.add_argument('--rounds', type=int, default=3, help='Rounds per mode (default 3).')
    parser.add_argument('--timeout', type=float, default=60, help='Seconds to wait for the app (default 60).')
    parser.add_argument('--lifetime', type=int, default=1800, help='Seconds after which the app closes itself.')
    parser.add_argument('--json', type=Path, help='Also write the report to this file.')
    args = parser.parse_args(argv)

    report = run(args)
    show(report)
    if args.json is not None:
        args.json.write_text(json.dumps(asdict(report), indent=2), encoding='utf-8')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
