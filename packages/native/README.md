# platynui-native

`platynui-native` exposes the PlatynUI runtime to Python. It is mainly used by the Robot Framework libraries in this repository, but it is also useful for smoke tests, experiments, and custom Python wrappers.

Most users should start with the root [README](../../README.md), the CLI, or `PlatynUI.BareMetal`. Use this package directly when you need Python access to runtime queries, nodes, pointer/keyboard actions, screenshots, or mock providers.

## Local development

```sh
uv sync --dev --all-packages --all-groups --all-extras
uv run maturin develop -m packages/native/Cargo.toml --uv --features mock-provider
```

The `mock-provider` feature is useful for tests because it exposes mock UI trees and mock platform devices without relying on the real desktop.

## Tiny smoke test

```python
from platynui_native import Runtime

runtime = Runtime.new_with_mock()
nodes = runtime.evaluate("//control:Window", None)
print(len(nodes))
```

Run package tests with:

```sh
uv run pytest -q packages/native/tests
```

## Notes

- Build with `--features mock-provider` before using `Runtime.new_with_mock()`.
- Platform operations such as pointer, keyboard, screenshots, and highlights depend on the selected backend.
- The public Robot Framework API is expected to live above this package; this package is the lower-level bridge.

## More information

- [../../docs/](../../docs/) - current working notes for Python bindings, Robot Framework library design, and platform behavior.
- [../../README.md](../../README.md) - project overview.

The files in `docs/` are working documentation for now and will be replaced or consolidated into proper user documentation later.

## License

Apache-2.0. See the repository's LICENSE file.
