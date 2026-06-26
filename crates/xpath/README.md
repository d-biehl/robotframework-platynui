# platynui-xpath

`platynui-xpath` is the query engine used by PlatynUI. It lets the runtime and tools find UI elements with XPath-like expressions such as:

```xpath
//control:Window//control:Button[@Name='OK']
```

In PlatynUI, XPath is not used for XML files. It is used to navigate desktop UI trees from providers such as Windows UIA and Linux AT-SPI2.

## Why it exists

Desktop automation needs selectors that are readable, precise, and portable across platforms. This crate provides the parsing and evaluation layer behind those selectors so the CLI, Inspector, Python bindings, and Robot Framework libraries can share the same query behavior.

## Current role

- Parse and evaluate XPath-style expressions for PlatynUI UI trees.
- Support the common XPath 2.0 features needed by the automation layer.
- Keep query behavior consistent across CLI, Python, Robot Framework, and tests.

## For contributors

Most contributors do not need to use this crate directly. Work here when you are changing selector parsing, query evaluation, built-in functions, or XPath-specific tests.

Useful commands from the repository root:

```sh
cargo nextest run -p platynui-xpath
cargo test -p platynui-xpath
```

## More information

- [docs/](docs/) - current working notes for XPath coverage.
- [../../dev-docs/](../../dev-docs/) - project developer notes, including architecture context.
- [../../README.md](../../README.md) - project overview.

The files in `docs/` (here) and `../../dev-docs/` are developer documentation for now and will be consolidated into user-facing docs later.

## License

Apache-2.0. See the repository's LICENSE file.
