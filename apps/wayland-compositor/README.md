# PlatynUI Wayland Compositor

The PlatynUI Wayland compositor is a controlled Linux desktop environment for UI automation tests. It can run nested during development or headless in CI, so tests do not depend on whatever desktop session happens to be open on the machine.

Use it when you need repeatable window placement, screen size, keyboard layout, screenshots, and input behavior for Linux automation work.

## Run it

From the repository root:

```sh
cargo run -p platynui-wayland-compositor -- --backend winit
```

For headless CI-style runs:

```sh
cargo run -p platynui-wayland-compositor -- --backend headless --exit-with-child -- your-test-command
```

For X11 applications through XWayland:

```sh
cargo run -p platynui-wayland-compositor -- --backend winit --xwayland --print-env
```

## Notes

- `winit` is the easiest backend for local development because it opens a nested window.
- `headless` is intended for automated test runs.
- `--print-env` prints environment variables that child applications can use to connect to the compositor.
- The companion control tool is documented in [../wayland-compositor-ctl/README.md](../wayland-compositor-ctl/README.md).

## More information

- [docs/](docs/) - current working notes for compositor usage, configuration, and the control protocol.
- [../../docs/](../../docs/) - current project working notes and planning material.
- [../../README.md](../../README.md) - project overview.

The files in these `docs/` directories are working documentation for now and will be replaced or consolidated into proper user documentation later.

## License

Apache-2.0. See the repository's LICENSE file.
