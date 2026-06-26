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

### Search result limit

The inspector limits XPath search collection to the first 5000 results by default so broad queries stay usable in an interactive UI. Override the limit with a command-line option:

```sh
platynui-inspector --search-result-limit 10000
platynui-inspector --search-result-limit unlimited
```

Or use the equivalent environment variable:

```sh
PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT=10000
PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT=unlimited
```

Command-line options take precedence over environment variables. Values `0`, `off`, `disabled`, `disable`, `none`, and `unlimited` disable the guard.

### Rendering options

The inspector ships with both `wgpu` and `glow` renderers. `wgpu` is the default; `glow` can be useful in virtual machines or older graphics stacks.

Command-line options take precedence over environment variables:

```sh
platynui-inspector --renderer glow --glow-hardware-acceleration off
```

Equivalent environment variables:

```sh
PLATYNUI_INSPECTOR_RENDERER=glow
PLATYNUI_INSPECTOR_GLOW_HARDWARE_ACCELERATION=off
```

Supported renderer values are `wgpu` and `glow`. Supported glow hardware acceleration values are `required`, `preferred`, and `off`.

The glow hardware acceleration setting applies to the `glow` renderer only. When `glow` is selected, `off` is treated as a best-effort request. Some Windows systems do not expose a software-only OpenGL configuration, so the inspector falls back to `preferred` instead of failing during window creation.

The `wgpu` renderer does not use the glow hardware acceleration setting. It can be influenced through the environment variables supported by `wgpu` itself:

```sh
WGPU_POWER_PREF=low      # low, high, or none
WGPU_BACKEND=dx12        # comma-separated: dx12, vulkan, gl, metal
```

`WGPU_POWER_PREF` changes adapter preference, such as low-power versus high-performance devices. `WGPU_BACKEND` limits which graphics backends `wgpu` may try. These settings do not force pure software rendering, but they are useful when diagnosing virtual machine or driver-specific rendering behavior.

On Windows, the inspector uses Vulkan or DX12 by default, with Vulkan preferred when available. OpenGL is not tried automatically because it can make startup noticeably slower on some systems. If you need to test OpenGL anyway, start the inspector with `WGPU_BACKEND=gl`.

## Notes

- On Linux/X11, make sure accessibility is enabled and AT-SPI is running.
- On Windows, UIA is available by default, but elevated applications may require matching privileges.
- If a highlighted element is missing or empty, the application may not expose usable bounds through the platform backend.

## More information

- [../../dev-docs/](../../dev-docs/) - developer notes for Inspector behavior and platform details.
- [../../README.md](../../README.md) - project overview.

The files in `dev-docs/` are developer documentation for now and will be consolidated into user-facing docs (under `docs/`) later.

## License

Apache-2.0. See the repository's LICENSE file.
