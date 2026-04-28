# PlatynUI Inspector

`platynui-inspector` is a small desktop app for looking at the UI tree that PlatynUI sees. Use it to explore windows and controls, inspect attributes, test what is visible to the accessibility backend, and briefly highlight selected elements on screen.

> [!WARNING]
> Preview package. The UI and behavior may still change.

## Install

```sh
uv tool install --prerelease allow platynui-inspector
```

Inside an existing virtual environment:

```sh
uv pip install --pre platynui-inspector
# or
pip install --pre platynui-inspector
```

Windows and Linux are the active targets. macOS packages currently contain stub backend support.

## Run

```sh
platynui-inspector
```

The left side shows the desktop UI tree. Selecting an element shows its attributes on the right and, when bounds are available, highlights it on screen for a moment.

## Notes

- On Linux/X11, make sure accessibility is enabled and AT-SPI is running.
- On Windows, UIA is available by default, but elevated applications may require matching privileges.
- If a highlighted element is missing or empty, the application may not expose usable bounds through the platform backend.

## More information

- [../../docs/](../../docs/) - current working notes for Inspector behavior and platform details.
- [../../README.md](../../README.md) - project overview.

The files in `docs/` are working documentation for now and will be replaced or consolidated into proper user documentation later.

## License

Apache-2.0. See the repository's LICENSE file.
