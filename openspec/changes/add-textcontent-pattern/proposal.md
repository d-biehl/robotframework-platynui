## Why

The read-only text capability is half-built and disconnected. The Robot Framework surface already exists — `ui/text.py` defines a `Text` context class whose `.text` reads `get_pattern(TextContent).text` — and the pattern abstractions exist in `core/patterns/text.py`. But nothing wires them up: there is no adapter builder for `TextContent`, so `Text.text` fails today; the Rust providers expose an element's text only as raw `native:Text.*` debug attributes (AT-SPI), never as a canonical `control:Text`; and no provider makes the capability observable.

`TextContent` is also the first *content* ClientPattern to be wired end-to-end. It is deliberately the simplest one — a single read-only string — which makes it the right place to establish the pattern (attribute-only synthesis on the Python side, a canonical `control:Text` attribute on the Rust side) that later text patterns (`TextEditable`, `TextSelection`) will follow. We start on AT-SPI, where the effect is runtime-testable on this machine.

## What Changes

- **Rust core** — add a `control:Text` attribute-name constant so providers reference a constant, not a string literal (the testkit already hard-codes `"Text"`).
- **AT-SPI provider** — expose a canonical `control:Text` attribute, sourced from the element's Text interface (`GetText(0,-1)`) when `Interface::Text` is present. The existing raw `native:Text.*` debug attributes stay. **No fallback to the accessible name.**
- **Windows/UIA provider** — expose `control:Text` from `TextPattern.DocumentRange.GetText`, falling back to `ValuePattern.Value` (the Value can be a formatted/adapted string, so TextPattern is preferred). No fallback to the Name property. Implemented in this change and runtime-verified on a Windows host via a dedicated task.
- **Python adapter** — surface `TextContent` as an *attribute-only* pattern (the established convention for `Element`, `ActivationTarget`, `Readable`, `WindowState`): infer it from the presence of `control:Text` and read `text` from that attribute. No change to the native providers' `supported_patterns()`.
- **Trim the brainstormed Python contract** — remove `locale` and `is_truncated` from `TextContent` (and from the `Text` context class). They are not needed and are hard to source honestly on both platforms; keeping them would define abstract members nothing implements.
- **egui test app + acceptance** — a text widget with known, `@Id`-tagged content so the real AT-SPI read is observable.
- **Docs** — correct `dev-docs/architecture.md` §6.3/§6.4: drop `IsReadOnly` from the `TextContent` contract and drop the `NameProperty →` prefix from the UIA mapping.

Locked decisions (see design.md): `control:Text` is sourced **only** from a genuine text interface (no name fallback); `TextContent` carries **only** `text`; `IsReadOnly` is **not** in the provider contract (read-only is a client-side derivation, deferred to `TextEditable`).

Out of scope: writing text (`TextEditable`), caret/selection (`TextSelection`), `Clearable`, `HasEditor`, and the `Readable`/`IsReadOnly` wiring — all left untouched. Whether ClientPatterns should *also* be advertised in the Rust `supported_patterns()` (for the CLI view) is a broader question across all ClientPatterns and is not decided here.

## Capabilities

### New Capabilities
- `textcontent-pattern`: the `TextContent` client pattern — its `control:Text` attribute, the per-platform source (AT-SPI Text interface; UIA Value/Text pattern) with no name fallback, its read-only-and-nothing-else scope, and the attribute-only synthesis that makes the `Text` widget read it.

## Impact

- **Affected specs:** `textcontent-pattern` (new).
- **Rust core:** `crates/core/src/ui/attributes.rs` — one new attribute-name constant (`text_content::TEXT`); `contract/testkit.rs` uses it instead of the `"Text"` literal. Additive.
- **Rust AT-SPI:** `crates/provider-atspi/src/node.rs` — a canonical `control:Text` attribute in the standard attribute iterator, gated on `Interface::Text`, reusing the existing `TextProxy` plumbing (`node.rs:1922`). Additive; `native:Text.*` unchanged.
- **Rust Windows/UIA:** `crates/provider-windows-uia/src/node.rs` — a `control:Text` attribute from `TextPattern`/`ValuePattern`. Implemented in this change, compile- and clippy-checked from Linux (`just check-windows` / `just clippy-windows`) and built on the Windows CI wheel job; **runtime-verified on Windows via a dedicated task** (task 5.3 — runs on a Windows host, which this dev machine is not).
- **Python:** `src/PlatynUI/core/adapters/ui_node.py` — a `_NativeTextContent` builder and a `(TextContent, 'Text')` attribute-only entry; `src/PlatynUI/core/patterns/text.py` and `src/PlatynUI/ui/text.py` — trim `locale`/`is_truncated`.
- **Rust test fixture:** `apps/test-app-egui` — a known-content, `@Id`-tagged text widget (added if not already present).
- **Docs:** `dev-docs/architecture.md` §6.3/§6.4 corrections.
- **Native rebuild:** yes — the AT-SPI/attribute change is in the native binding; the Python/RF observation needs a `maturin` rebuild (via `just test-python`).
- **Providers / support:** AT-SPI (X11 real → runtime-verified against egui); Windows UIA (real → implemented, runtime-verified on a Windows host via a dedicated task); macOS AX stub / mock — unaffected.
- **BREAKING:** none. Additive attribute + additive adapter wiring. The `locale`/`is_truncated` removal touches only brainstormed, unwired members (`Text.text` does not work today), so nothing in use regresses.
