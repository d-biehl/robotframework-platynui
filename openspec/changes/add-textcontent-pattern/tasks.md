## 1. Core contract

- [ ] 1.1 Add a `text_content` submodule with a `TEXT` constant (`"Text"`) to `crates/core/src/ui/attributes.rs`, alongside the other pattern attribute modules.
- [ ] 1.2 Replace the `"Text"` string literal in `crates/core/src/ui/contract/testkit.rs` (the `TEXT_CONTENT_ATTRS` expectation, `:291`) with the new constant; keep the existing testkit tests green.

## 2. Python adapter wiring (test-first)

- [ ] 2.1 pytest (fake node): a node exposing a `control:Text` attribute → `supports_pattern(TextContent)` is true and `get_pattern(TextContent).text` returns the value; a node without `control:Text` → not supported and `get_pattern(TextContent)` is `None`. Mirror the existing `Readable`/`_NativeReadable` tests.
- [ ] 2.2 Add `_NativeTextContent` (reads `control:Text` via `attribute_value`, like `_NativeReadable` at `ui_node.py:215`) and `_build_textcontent`; register it in `_PATTERN_BUILDERS` and add `(TextContent, 'Text')` to `_ATTRIBUTE_ONLY_PATTERNS` (`ui_node.py:509`).
- [ ] 2.3 Trim the contract: remove `locale` and `is_truncated` from `TextContent` in `src/PlatynUI/core/patterns/text.py`, and the corresponding `locale`/`is_truncated` properties from `Text` in `src/PlatynUI/ui/text.py`. Leave `TextEditable`, `Clearable`, `HasEditor`, `Readable`, and the `Edit` class untouched.

## 3. AT-SPI provider

- [ ] 3.1 Test: a node whose element implements `Interface::Text` exposes a non-null `control:Text` equal to `GetText(0,-1)`; a node without the Text interface does not expose `control:Text`. (Real AT-SPI needs a session — runs in the real-provider lane, not the mock lane.)
- [ ] 3.2 Emit a canonical `control:Text` attribute from the standard attribute iterator (`AttrsIter`/`StdAttrKind`, `crates/provider-atspi/src/node.rs`), gated on `Interface::Text` (mirror `supports_component`, `node.rs:141`), sourced from `TextProxy.text()`. Use `text_content::TEXT`. Do **not** fall back to the name; leave `native:Text.*` unchanged.
- [ ] 3.3 Preserve empty-string content: an empty text field must yield a present, empty `control:Text` (not `Null`), so an empty field keeps `TextContent`. Do not route it through the empty-to-`Null` `fetch_str` normalization (`node.rs:1657`).

## 4. egui acceptance fixture + suite (real provider)

- [x] 4.1 Confirmed from source: `Interface::Text` needs a text-input/`Label`/`Document`/`Terminal` node **with text runs** (`accesskit_atspi_common node.rs:449` → `accesskit_consumer text.rs:1404`). egui emits runs via its text-selection machinery — unconditionally for a `TextEdit`, and for a `Label` only via the default-on `selectable_labels`. → Use a `TextEdit` fixture so the Text interface is guaranteed.
- [ ] 4.2 Ensure `apps/test-app-egui` has a `TextEdit` (or a selectable `Label`) with known, `@Id`-tagged content (add one if missing) — `TextEdit` guarantees the Text interface.
- [ ] 4.3 RF acceptance suite (against egui, real AT-SPI): the widget's text (via the `Text` control class or `Get Attribute control:Text`) equals the known content; a non-text widget (e.g. a button with no text interface) exposes no `control:Text`.

## 5. Windows / UIA (implemented; runtime-verified on a Windows host)

- [ ] 5.1 Add a `control:Text` attribute (`TextAttr`) to `crates/provider-windows-uia/src/node.rs`: `TextPattern.DocumentRange.GetText(-1)` first, falling back to `ValuePattern.CurrentValue` when there is no TextPattern (Value can be a formatted/adapted string); wire it into `attribute()` (`node.rs:225`) and `AttrsIter`. No Name fallback. Absent when the element supports neither pattern.
- [ ] 5.2 `just check-windows` and `just clippy-windows` are green.
- [ ] 5.3 Runtime-verify on a real Windows desktop (UIA): run the acceptance suite and confirm `control:Text` reads the widget's content. Required; runs on a Windows host (this dev machine is Linux, so this task is completed there).

## 6. Documentation

- [ ] 6.1 `dev-docs/architecture.md` §6.3: drop the optional `IsReadOnly` from the `TextContent` row (`TextContent | Text | —`).
- [ ] 6.2 §6.4 UIA `TextContent` mapping: drop the `NameProperty →` prefix and put TextPattern first (`TextPattern.DocumentRange.GetText → ValuePattern.Value`).
- [ ] 6.3 §6.4: remove the `IsReadOnly` mapping row under `TextContent`; add a note that read-only is a client-side derivation (`TextContent ∧ ¬TextEditable`), not a provider attribute, and that `TextContent` surfaces via attribute-only synthesis.

## 7. Verification

- [ ] 7.1 `just check` (fmt, clippy, ruff, mypy) and `just test` (nextest) are green.
- [ ] 7.2 `just test-python` green (includes the new adapter pytest; rebuilds the native binding).
- [ ] 7.3 `just test-crate platynui-provider-atspi` green.
- [ ] 7.4 egui acceptance lane green on AT-SPI (`just headless=true test-acceptance-x11`, and the compositor lane).
- [ ] 7.5 `just check-windows` / `just clippy-windows` green.
- [ ] 7.6 `openspec validate add-textcontent-pattern` passes.

## 8. Follow-ups

- [ ] 8.1 When `TextEditable` lands: rework `Readable.is_readonly` to derive from `¬TextEditable` and drop the provider `IsReadOnly` sentinel (`_NativeReadable` / `_build_readable` / `_ATTRIBUTE_ONLY_PATTERNS`).
- [ ] 8.2 Cross-cutting: decide whether ClientPatterns should also be advertised in the Rust `supported_patterns()` so the CLI/`snapshot` view matches the Python view (affects `Element`, `ActivationTarget`, `TextContent`, … — out of scope here).
