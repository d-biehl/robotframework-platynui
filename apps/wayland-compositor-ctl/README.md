# platynui-wayland-compositor-ctl

`platynui-wayland-compositor-ctl` controls a running [PlatynUI Wayland compositor](../wayland-compositor/README.md). It is mainly useful in scripts and tests that need to inspect the compositor, list windows, take screenshots, or shut the compositor down.

## Examples

```sh
platynui-wayland-compositor-ctl status
platynui-wayland-compositor-ctl list-windows
platynui-wayland-compositor-ctl --json list-windows
platynui-wayland-compositor-ctl screenshot -o screenshot.png
platynui-wayland-compositor-ctl shutdown
```

The tool usually finds the compositor socket automatically when it is started from the environment printed by the compositor. If needed, pass `--socket <path>` explicitly.

## More information

- [../wayland-compositor/docs/](../wayland-compositor/docs/) - current working notes for compositor usage and the control protocol.
- [../wayland-compositor/README.md](../wayland-compositor/README.md) - compositor overview.
- [../../README.md](../../README.md) - project overview.

The files in `../wayland-compositor/docs/` are working documentation for now and will be replaced or consolidated into proper user documentation later.

## License

Apache-2.0. See the repository's LICENSE file.
