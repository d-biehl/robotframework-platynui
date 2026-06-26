# PlatynUI CLI

`platynui-cli` is the command-line tool for trying PlatynUI against the current desktop. Use it to check which providers are active, inspect the UI tree, test selectors, highlight targets, or capture quick diagnostic output.

> [!WARNING]
> Preview package. Commands and output may still change.

## Install

```sh
uv tool install --prerelease allow platynui-cli
```

Inside an existing virtual environment:

```sh
uv pip install --pre platynui-cli
# or
pip install --pre platynui-cli
```

Windows and Linux are the active targets. macOS packages currently contain stub backend support.

## Try it

```sh
platynui-cli list-providers
platynui-cli info --format json
platynui-cli query "//control:Button[@Name='OK']"
platynui-cli highlight "//control:Button[@Name='OK']" --duration-ms 1200
platynui-cli screenshot screen.png
```

Useful command groups include `query`, `snapshot`, `watch`, `focus`, `window`, `pointer`, and `keyboard`. Run `platynui-cli --help` or `platynui-cli <command> --help` for the current command syntax.

## Notes

- On Linux, make sure the accessibility stack is enabled and AT-SPI is running.
- Use `--format json` on commands that support it when scripts need stable output.
- Keyboard sequences use the same `<Ctrl+C>` style syntax as the Python and Robot layers.

## More information

- [../../dev-docs/](../../dev-docs/) - developer notes for CLI behavior, input handling, and platform details.
- [../../README.md](../../README.md) - project overview.

The files in `dev-docs/` are developer documentation for now and will be consolidated into user-facing docs (under `docs/`) later.

## License

Apache-2.0. See the repository's LICENSE file.
