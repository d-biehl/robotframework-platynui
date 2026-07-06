## Context

`TextContent` is the read-only "there is text to read here" capability. Three layers are involved, and they are at very different stages:

- **Rust core** — only the pattern *name* is reserved (`pattern_names::TEXT_CONTENT`, `identifiers.rs:146`). There is no trait: ClientPatterns are not traits (unlike the closure-backed RuntimePatterns in `pattern.rs`). The contract testkit already expects a `TextContent` node to carry a required `"Text"` attribute (`contract/testkit.rs:291`, exercised at `:409`–`:505`).
- **Rust providers** — neither AT-SPI nor UIA advertises any ClientPattern; both list only `Focusable` + the window patterns (`provider-atspi/src/node.rs:277`, `provider-windows-uia/src/node.rs:279`). AT-SPI already *reads* the Text interface, but only as raw `native:Text.*` debug attributes (`node.rs:1641`, `node.rs:1922`, via the `text_proxy` helper at `node.rs:369`). There is no canonical `control:Text`.
- **Python** — the RF-facing side is the furthest along but disconnected. `ui/text.py` `Text.text` reads `get_pattern(TextContent).text`; `core/patterns/text.py` defines `TextContent` (`text`, `locale`, `is_truncated`). But there is **no** `_build_textcontent` and no attribute-only entry for it, so `get_pattern(TextContent)` returns nothing and `Text.text` fails today.

The load-bearing discovery is how ClientPatterns already surface on the Python side: not from the native `supported_patterns()`, but **synthesized from a sentinel attribute** in `_ATTRIBUTE_ONLY_PATTERNS` (`ui_node.py:509`): `Element←Bounds`, `ActivationTarget←ActivationPoint`, `Readable←IsReadOnly`, `WindowState←IsActive`. `_NativeReadable` (`ui_node.py:215`) shows the shape — a small adapter that reads one attribute. `TextContent` should follow the same convention, keyed on `Text`.

## Goals / Non-Goals

**Goals**
- Expose an element's current text as a canonical, read-only `control:Text` attribute on the real providers.
- Make `TextContent` resolvable on the Python side (so `Text.text` works) via the existing attribute-only synthesis.
- Verify the real effect against a live AT-SPI provider.
- Establish the shape (`control:Text` in Rust, attribute-only synthesis in Python) that `TextEditable`/`TextSelection` will reuse.

**Non-Goals**
- No writing (`TextEditable`), no caret/selection (`TextSelection`), no `Clearable`/`HasEditor`.
- No `IsReadOnly` in the provider contract, and no change to the existing `Readable` wiring.
- No `locale` / `is_truncated` — removed from the contract, not deferred.
- No change to the native `supported_patterns()` (whether ClientPatterns should also be advertised there is a separate, cross-cutting question).

## Decisions

**D1 — `TextContent` is a ClientPattern (attribute contract), not a RuntimePattern.** It promises a readable `control:Text` string; it has no action and is not resolved through `pattern_by_name`. Consumers read the attribute. This matches the architecture's ClientPattern definition (`architecture.md` §6.1) and needs no new trait in `pattern.rs`.

**D2 — `control:Text` is sourced only from a genuine text interface; no name fallback.**
- AT-SPI: `TextProxy.text()` = `GetText(0,-1)`, when `Interface::Text` is present (mirroring `supports_component` at `node.rs:141`).
- UIA: `TextPattern.DocumentRange.GetText(-1)` first (the actual displayed text), falling back to `ValuePattern.CurrentValue` when there is no TextPattern — `Value` can hold a formatted/adapted string that differs from the visible text, so it is the second choice, not the first.
An element whose only textual info is its accessible name (plain label/button without a text interface) does **not** get `control:Text`; its label stays in `control:Name`. This is the deliberate answer to "is Button = TextContent?": only if that element actually exposes a text interface.

**D3 — `TextContent` carries only `text`.** `locale` and `is_truncated` are removed from `core/patterns/text.py` and `ui/text.py`. `is_truncated` has no honest source on either platform; `locale` is obtainable but unneeded. Keeping abstract members nothing implements is worse than removing them; they can return in a future change with a real source.

**D4 — `IsReadOnly` is not in the provider contract.** Read-only is a client-side derivation (`TextContent ∧ ¬TextEditable`) and belongs with `TextEditable`, a separate future change. The existing `Readable` pattern (`patterns/readable.py`, wired via `_NativeReadable` reading a provider `IsReadOnly` attribute) is **left untouched** here — it is pre-existing and does not block `TextContent`.

**D5 — Surface `TextContent` via attribute-only synthesis, not native advertisement.** Add `(TextContent, 'Text')` to `_ATTRIBUTE_ONLY_PATTERNS` and a `_NativeTextContent` builder that reads `control:Text`. The Rust providers only need to expose the attribute; they do not push `TEXT_CONTENT` into `supported_patterns()`. This is consistent with how every other ClientPattern already works and keeps the Rust change to "emit one more attribute."

**D6 — atspi-first; Windows implemented and verified on a Windows host.** The core contract is platform-neutral, so both providers implement the same attribute. AT-SPI is verified against egui here; the UIA path is written and kept green from Linux by `just check-windows` / `just clippy-windows` (and the Windows CI wheel build), and is runtime-verified by a dedicated task that runs the acceptance suite on a real Windows desktop. That task is required, not optional — it simply runs elsewhere, since this dev machine is Linux.

**D7 — Doc corrections ride along.** `architecture.md` §6.3 (`TextContent | Text | IsReadOnly` → drop the optional `IsReadOnly`), §6.4 UIA row (drop `NameProperty →`), and the §6.4 `IsReadOnly` mapping row (remove; note read-only is client-derived). The doc is a living intent document, so these are corrections, not a separate cleanup.

## Risks / Trade-offs

- **egui/AccessKit `Text` interface — confirmed.** AccessKit inserts `Interface::Text` when a node is a text input or `Label`/`Document`/`Terminal` **and** it carries text runs (`accesskit_atspi_common-0.18 node.rs:449` → `accesskit_consumer-0.35 text.rs:1404`). The runs come from egui's text-selection machinery (`egui text_selection/accesskit_text.rs`), not the default widget builder: a `TextEdit` emits them unconditionally (selection is intrinsic), a `Label` only because `interaction.selectable_labels` defaults to `true`. **Use a `TextEdit` fixture** and the Text interface is guaranteed. (A label's text is always *also* its AT-SPI name — `name() = value()`, `node.rs:39` — so the no-name-fallback rule correctly gives a non-selectable label no `TextContent`; its text lives in `control:Name`.)
- **Empty vs. absent text.** An empty text field has `Interface::Text` and returns `""`. `control:Text` must be present-and-empty (not absent) so `TextContent` still applies — distinct from "no text interface" (attribute absent). The AT-SPI `fetch_str` helper currently normalizes empty to `Null` (`node.rs:1657`); the canonical `control:Text` must not drop an empty string to `Null`, or an empty field would wrongly lose `TextContent`.
- **UIA Text vs. Value pattern.** `TextPattern.DocumentRange.GetText(-1)` first — it returns the actual displayed text. `ValuePattern.Value` only as a fallback when there is no TextPattern, because `Value` can hold a formatted/adapted string that differs from the visible text. Controls with neither get no `control:Text`. Verified on Windows via task 5.3, not on this Linux dev machine.
- **Windows runtime verification runs elsewhere** — implemented and compile/clippy-checked from Linux; the acceptance run happens on a Windows host (task 5.3), not on this dev machine.
- **`control:Text` duplicates content already in `native:Text.*`** — accepted: one is the canonical contract attribute, the other raw introspection.

## Migration Plan

Additive. New attribute (`control:Text`) + new Python adapter wiring; no existing behavior changes. Requires a native rebuild for the Python/RF side to see the new attribute (`just test-python` rebuilds with the mock provider; real verification uses the acceptance lane). The `locale`/`is_truncated` removal only touches unwired brainstorm members. Rollback: revert the change — nothing depends on `control:Text` yet beyond the new `Text` wiring.

## Verified vs. Assumed

- **Verified against the code:** core pattern model and reserved name; the testkit `Text` expectation; AT-SPI advertising only Focusable/window and reading text as `native:Text.*`; the UIA per-name attribute model; the Python attribute-only synthesis and `_NativeReadable` shape; that `TextContent` has no builder today; CLI `snapshot`/`query` and `BareMetal.get_attribute` as observation surfaces; `just check-windows`/`clippy-windows` and Windows CI wheel builds; that AccessKit exposes the AT-SPI `Text` interface for Label/text-input nodes carrying text runs (`accesskit_atspi_common node.rs:449`, `accesskit_consumer text.rs:1404`).
- **Assumed / to confirm during implementation:** the exact `StdAttrKind`/`AttrsIter` insertion point for the new attribute; the exact UIA pattern-availability calls; whether the egui test app already has a suitable text input (a `TextEdit` guarantees text runs → the Text interface; add one if absent).

## Open Questions

- Should the canonical `control:Text` trim/normalize whitespace like `fetch_str` does, or return the raw `GetText` result verbatim? Leaning verbatim (text content should be exact), unlike names.
