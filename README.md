# Robot Framework PlatynUI

Cross-platform native UI automation for Robot Framework.

> [!WARNING]
> Preview quality. Packages, keywords, CLI output, and platform behavior may change before the first stable release.

## What is PlatynUI?

PlatynUI is intended to become the Robot Framework automation layer for native desktop applications. It should let test suites inspect real application windows, find controls by stable queries, read UI state, perform user-like actions, and capture diagnostic evidence without binding tests to one operating system or accessibility technology.

PlatynUI presents the whole desktop as one tree of UI elements, the same way on every supported platform. Tests find elements in that tree with XPath queries and then act on them: focus, window control, mouse and keyboard input, highlighting, and screenshots.

PlatynUI consists of:

- `PlatynUI.BareMetal`, the Robot Framework keyword library to use today; the higher-level `PlatynUI` library is still being migrated.
- `platynui-cli`, a command-line tool to explore the desktop, try out queries, and send input.
- `platynui-inspector`, a desktop app to browse the UI tree, test queries, and pick elements with the mouse.
- `platynui-native`, the Python API the library is built on, for your own scripts and libraries.
- `platynui-provider-java`, the optional package for Java applications.

## Current status

- **Platforms:** Windows and Linux (X11 and Wayland); macOS support is planned.
- **Robot Framework library:** the high-level `Library    PlatynUI` entry point is still a placeholder while the migration continues. Use `Library    PlatynUI.BareMetal` for the current low-level keyword surface (see [Use the Robot Framework library](#use-the-robot-framework-library)).
- **CLI and Inspector:** preview tools are available as binary Python packages.
- **Java applications:** Swing/AWT on Windows, opt-in (see [Java applications](#java-applications)).
- **Requirements:** Python 3.12 or newer.

## Install preview tools

Install pre-release tool packages explicitly. The examples below assume `uv` 0.11.7 or newer. For user-level command-line tools, `uv tool` is the most convenient path:

```sh
uv tool install --prerelease allow platynui-cli
uv tool install --prerelease allow platynui-inspector
```

Inside an existing virtual environment, install the packages directly:

```sh
uv pip install --pre platynui-cli platynui-inspector
# or
pip install --pre platynui-cli platynui-inspector
```

For Java support in the Inspector, see [Java applications](#java-applications).

Try the tools:

```sh
platynui-cli list-providers
platynui-cli info --format json
platynui-cli query "//control:Window"
platynui-cli keyboard list | head -n 20
platynui-cli keyboard type "Hello <Ctrl+A>\\u00A7"
platynui-cli snapshot "//control:Window" --pretty
platynui-cli snapshot "//control:Window" --format xml --output windows.xml
platynui-cli highlight "//control:Window"
platynui-cli screenshot desktop.png
platynui-cli element-at-point 100 200
platynui-inspector
```

The CLI also offers `watch` (follow changes in the UI as they happen), `focus`, `window` (activate, minimize, move, resize, ...), and `pointer` (move, click, scroll, drag) commands; see [packages/cli/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/cli/README.md) for the full command overview.

In the Inspector, switch on **Pick Element** in the toolbar and hold Ctrl+Alt+Shift to select the element under the mouse cursor, in any application; the key combination can be changed under File → Settings. By default the Inspector follows the system's light or dark theme and shows the first 5000 search results:

```sh
platynui-inspector --theme dark
platynui-inspector --search-result-limit unlimited
```

See [packages/inspector/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/inspector/README.md) for all options.

## Use the Robot Framework library

`PlatynUI.BareMetal` is the keyword library to use today. It finds elements with XPath queries, waits for them before acting, and works the same way on Windows and Linux. Install it into the environment your tests run in:

```sh
uv pip install --pre robotframework-PlatynUI
# or
pip install --pre robotframework-PlatynUI
```

The extras `[cli]`, `[inspector]`, and `[java]` add the CLI, the Inspector, and Java support; `[all]` adds all three.

A first test:

```robotframework
*** Settings ***
Library    PlatynUI.BareMetal

*** Test Cases ***
Create A Report
    Pointer Click    Window[@Name="Editor"]//Button[@Name="New"]
    Keyboard Type    ${None}    Q3 Report
    Get Attribute    Window[@Name="Editor"]//Button[@Name="Save"]    IsEnabled    ==    ${True}
    Take Screenshot
```

To run it from a source checkout, see [CONTRIBUTING.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/CONTRIBUTING.md#run-platynui-from-a-source-checkout).

The library's full documentation — selectors, waiting, window control, input timing, and every keyword — is built in: `python -m robot.libdoc PlatynUI.BareMetal BareMetal.html` writes it to an HTML file.

## Java applications

On Windows, PlatynUI reaches Swing/AWT applications through an agent that it loads into the running application, without any change to how the application is started — Java Web Start included. This gives more precise results than the Java Access Bridge, for example stable element IDs and the exact position of table cells.

Java support is optional and comes as its own package, `platynui-provider-java`, installed with the `[java]` extra. Install it into the same virtual environment as the library or tool that uses it:

```sh
uv pip install --pre "robotframework-PlatynUI[java]"   # for test runs
uv pip install --pre "platynui-inspector[java]"        # for the Inspector
```

The agent needs a Java application that runs under your own user account, without administrator rights, and as a 64-bit process. Without the agent, PlatynUI falls back to the Java Access Bridge, which offers less detail and needs a 64-bit Java installation (found through `JAVA_HOME` or `PATH`) with the bridge switched on (`jabswitch -enable`).

SWT applications on Windows and Linux and JavaFX applications on Windows need no extra package: PlatynUI reaches them like any other application. Swing/AWT and JavaFX on Linux are planned.

## Diagnostics

When PlatynUI cannot do what it was asked — for example because an application stops responding or a setting cannot be used — it logs a warning that says what does not work as a result. A healthy run has none. In Robot Framework the warnings appear in the log of the keyword where they occurred; the CLI and the Inspector print them in the terminal.

For more detail, start the CLI or the Inspector with `--log-level debug`, or import the library with `native_log_level=debug` and run Robot Framework with `--loglevel DEBUG`. The environment variable `PLATYNUI_LOG_LEVEL` sets the default level for all of them.

To report a problem, attach the `output.xml` of a run with these settings. Do not share logs at level `trace`: they contain every key PlatynUI typed, including secrets such as passwords.

## Platform support

| Feature | Windows | Linux (X11) | Linux (Wayland) | PlatynUI compositor | macOS |
|---------|---------|-------------|-----------------|---------------------|-------|
| Find and read UI elements | ✅ | ✅ | ✅ positions relative to the window | ✅ | ❌ |
| Java applications (Swing/AWT) | ⚠️ opt-in | ❌ planned | ❌ planned | ❌ planned | ❌ |
| Mouse and keyboard | ✅ | ✅ | ⚠️ depends on the desktop | ✅ | ❌ |
| Screen and monitor information | ✅ | ✅ | ⚠️ partial | ✅ | ❌ |
| Screenshots | ✅ | ✅ | ❌ planned | ✅ | ❌ |
| Highlighting | ✅ | ✅ | ❌ | ✅ | ❌ |
| Window control | ✅ | ⚠️ partial | ❌ | ✅ | ❌ |
| Inspector | ✅ | ✅ | ⚠️ tree and queries only | ✅ | ❌ |
| Inspector live picker | ✅ | ✅ | ❌ | ✅ | ❌ |

On Linux, PlatynUI automatically detects whether it runs under X11 or Wayland and checks which capabilities the session provides.

The **PlatynUI compositor** is the project's own Wayland desktop for test environments ([apps/wayland-compositor](https://github.com/imbus/robotframework-PlatynUI/blob/main/apps/wayland-compositor/README.md)). Under it, every feature in the table except Java support works on Wayland, nested in your desktop or headless, and it can run X11 applications as well.

## Package docs

- [packages/cli/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/cli/README.md) - command-line tool and command overview.
- [packages/inspector/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/inspector/README.md) - Inspector app and its options.
- [packages/native/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/native/README.md) - Python API.
- [packages/provider-java/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/packages/provider-java/README.md) - optional Java support.
- [apps/wayland-compositor/README.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/apps/wayland-compositor/README.md) - Wayland test compositor.

## Contributing

Contributions are welcome. Start with these guides:

- [CONTRIBUTING.md](https://github.com/imbus/robotframework-PlatynUI/blob/main/CONTRIBUTING.md) - setup, building and running PlatynUI from a source checkout, contribution expectations, `just` task runner workflow, coding standards, testing guidance, PR checklist, and packaging notes.

The short version is: keep changes focused, use Conventional Commits, run the relevant `just` checks, and update docs when behavior changes.

## License

Apache-2.0. See [LICENSE](https://github.com/imbus/robotframework-PlatynUI/blob/main/LICENSE).
