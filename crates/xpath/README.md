# platynui-xpath

`platynui-xpath` is a standalone XPath 2.0 engine that powers the PlatynUI
automation toolchain. It evaluates XPath expressions against live accessibility
trees (UIAutomation on Windows, AT-SPI2 on Linux, …) so that automation actions
can be routed to the right UI nodes.

## Goals

- Full XPath 2.0 expression support (including FLWOR, quantified expressions,
  sequence operators, and type expressions) for live UI traversal.
- Rich coverage of the 2010 “XQuery 1.0 and XPath 2.0 Functions and Operators”
  library wherever it is relevant for accessibility scenarios.
- Host integration via user-defined node models: the automation host implements
  the `XdmNode` trait for its accessibility objects (UIAutomation elements,
  AT-SPI2 nodes, …) so the evaluator can walk the live tree. Optional hooks
  such as a custom regex provider can be supplied through the dynamic context
  when needed.

## Out of scope (for now)

The implementation is intentionally focused on UI automation. Consequently it
does **not** provide the following features at the moment:

- **Schema awareness** – no loading or evaluation of XML schemas; `instance of`
  and kind tests operate dynamically.
- **Persistent document/collection catalogues** – document lookup is expected
  to be implemented by the host if required.
- **F&O functions with significant host I/O or formatting requirements** (for
  example the `fn:unparsed-text*` family). These can be added once the
  automation targets require them.
- **XPath 3.x extensions** such as `fn:analyze-string`; the focus remains on the
  2010 XPath 2.0 recommendation.

## Current status

- Parser, compiler, and evaluator cover the complete XPath 2.0 grammar.
  Integration tests under `tests/` exercise both parsing and evaluation.
- The default function registry (`src/engine/functions/`) includes the majority
  of helpers typically needed when working with accessibility trees: string,
  numeric, sequence, date/time, QName, comparison, and diagnostic functions.
- The dynamic context API lets callers supply context items, variables, current
  date/time, and collations for each evaluation run.

## Quick start

```rust
use platynui_xpath::{compiler::compile_xpath, engine::runtime::DynamicContext};

let compiled = compile_xpath("Window[@Name='My Window']//Button[@Name='OK']")?;
let dyn_ctx = DynamicContext::default();
let result = platynui_xpath::engine::evaluator::evaluate(&compiled, &dyn_ctx)?;
```

In a real automation host an adapter (for example UIAutomation or AT-SPI2)
constructs `XdmNode` wrappers for the live accessibility objects and populates a
`DynamicContext` with the current context item (and optional hooks such as a
`NodeResolver`). XPath results can then be used to drive the UI interaction
layer.

## Architecture notes

- Single-threaded evaluation: The engine runs strictly single-threaded. Internals prefer `Rc`/`RefCell` over `Arc`/`Mutex` to keep overhead low.
- Streaming API and 'static: `XdmNode` does not require `'static`. Streaming sequences (`XdmSequenceStream`) intentionally own their cursors and may outlive the immediate evaluation call. Therefore the streaming layer keeps `'static` on cursor types. If we need scope-tied, borrowed nodes, we can introduce lifetimes on cursors/streams in a future refactor.
- Function registry and collations: The default function registry is created per dynamic context (no global cache). `DynamicContext::functions` is held behind `Rc`. Collation instances remain behind `Arc` as trait objects potentially shared across contexts.
