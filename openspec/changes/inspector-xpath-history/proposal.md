## Why

The Inspector's XPath search field starts empty every time and forgets everything the moment the app closes. Iterating on a query — a common debugging loop — means retyping long expressions from scratch, and there is no way to recall an expression that worked yesterday. Every other query-oriented tool (browser address bars, shells, DB clients) keeps a recallable history; the Inspector is the one place in PlatynUI where the user's own past input is thrown away. A persisted history (and, building on it, as-you-type completion) turns the search field into something you can actually work in.

## What Changes

- **Persisted XPath history.** Every non-empty expression that **evaluates successfully** (no parse/query error; triggered via Enter, the Search button, or Ctrl/Cmd+Enter) is recorded to a most-recent-first history — failed/invalid expressions are not kept, and a valid expression that matches nothing still counts as a success. The history is de-duplicated (re-running an expression moves it to the top rather than adding a duplicate), capped at **100 entries** (oldest dropped), and **saved to disk** so it survives restarts. It is loaded once at startup.
  - A new **history dropdown** in the search bar (a small button beside the XPath field) lists the recent expressions newest-first; selecting one loads it into the field. It also offers **Clear History**. This is the base feature's discoverable access path and does not depend on completion.
- **As-you-type completion (builds on history).** While typing in the XPath field, a suggestion popup below the field offers matching entries **from the history**, keyboard-navigable (Up/Down to move, Enter/Tab to accept, Esc to dismiss), integrated with the field's existing plain-Enter-evaluates and Escape behavior. This is the "would be cool" part the user asked for; it is a separate, sequenced slice on top of history so history can ship even if completion slips.
  - Out of scope for now (noted as future extensions, not this change): completion candidates drawn from the **XPath vocabulary** (axis names, node tests, function names) or from **live tree names** (element/attribute names, runtime ids). The XPath crate has no exported vocabulary list today, so that would be new `crates/xpath` work; tree-name harvesting is a separate design. History-sourced completion delivers most of the value with none of that surface.
- **First on-disk persistence in the Inspector.** The Inspector currently persists nothing; this introduces a small, self-contained history file under the OS config directory. Not a broad settings/persistence framework — just this one artifact.

Not BREAKING: purely additive UI/behavior in the Inspector. No change to query semantics, results, or any keyword/API.

## Capabilities

### New Capabilities
- `inspector-xpath-history`: recording an XPath expression on submit; de-duplication with move-to-front; the 100-entry cap; on-disk persistence (load at startup, save on record, tolerant of a missing/corrupt/oversized file); and the history-dropdown UI to browse, select, and clear.
- `inspector-xpath-completion`: the as-you-type suggestion popup sourced from the history — match filtering, keyboard navigation (Up/Down/Enter/Tab/Esc), and its interaction with the field's existing Enter-evaluates / Escape-cancels behavior and focus-lock key handling.

### Modified Capabilities
<!-- None. The Inspector's existing XPath search behavior is not yet spec'd, so nothing is respecified here. -->

## Impact

- **Layer:** Rust only, entirely within `apps/inspector`. **No native rebuild** — the Inspector is a standalone binary, not the `packages/native` PyO3 module. **No Python/Robot Framework surface, no provider or platform-device behavior changes.** Platform-agnostic: works the same on every backend in the README support table; only the on-disk history path differs per OS (resolved per-platform).
- **New dependency:** a cross-platform config-directory resolver (`directories`) plus `serde`/`serde_json` for the inspector crate, so the history file lands correctly on Windows / Linux / macOS. Note the Linux-only, hand-rolled `$XDG_CONFIG_HOME` precedent in `apps/wayland-compositor/src/config.rs` — deliberately **not** reused, because the Inspector is cross-platform. (Alternative considered: eframe's `persistence` feature; deferred to design.)
- **Code touched (view):** `apps/inspector/src/view/toolbar.rs` — the search bar gains the history dropdown button and the completion popup, reusing the existing `egui::Area` + `Frame::popup` pattern (`show_search_error_popup`, toolbar.rs:395) and the field's existing focus-lock filter (toolbar.rs:259).
- **Code touched (viewmodel):** `apps/inspector/src/viewmodel/inspector_vm.rs` — record on the submit paths that reach `evaluate_xpath` (inspector_vm.rs:640); own the history store and completion state. Likely a new small module (e.g. `viewmodel/xpath_history.rs`) for the persistence-backed, bounded, de-duplicated store and the completion match logic, kept pure so it is unit-testable.
- **Persistence location:** OS config dir under `platynui/inspector/` (e.g. `~/.config/platynui/inspector/xpath-history.json` on Linux), namespaced to match the existing app id `org.platynui.inspector` (lib.rs:857) and the compositor's `platynui/` namespace.
- **Tests (Rust nextest, per dev-docs/testing-strategy.md):** the Inspector has **no tests today**, so this adds the first ones — unit tests for the store (dedup/move-to-front, 100-cap, ignore empty, newest-first order, JSON round-trip, tolerant/oversize-clamping load) and for completion matching (case-insensitive filter, ordering, selection clamping/wraparound). Interactive popup/keyboard behavior is verified by running the Inspector, matching the crate's existing (view-is-pure-rendering) test posture.
- **Docs:** `dev-docs/inspector.md` (Features) and the improvements doc as appropriate.
