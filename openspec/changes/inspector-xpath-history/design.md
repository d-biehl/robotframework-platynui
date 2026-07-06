## Context

The Inspector's XPath search field is a multiline `egui::TextEdit` bound directly to `InspectorViewModel::search_text: String` (field declared at `apps/inspector/src/viewmodel/inspector_vm.rs:75`, widget at `apps/inspector/src/view/toolbar.rs:179`, stable id `inspector_xpath_search_field` at `toolbar.rs:44`). There is **no persistence of any kind in the Inspector today**: the `eframe::App` impl (`apps/inspector/src/lib.rs`) implements no `save`, and eframe's `persistence` feature is not enabled (`apps/inspector/Cargo.toml:22` — `eframe = { version = "0.34", features = ["glow"] }`, no `persistence`; verified in `Cargo.lock` — no `ron`/`directories`/`home` pulled in). The only precedent for an on-disk config path in the repo is `apps/wayland-compositor/src/config.rs:5`, which hand-rolls `$XDG_CONFIG_HOME/platynui/...` — but that app is Linux/Wayland-only, so it is not a good model for the cross-platform Inspector.

All three submit paths converge on one function, which makes recording clean: plain Enter in the field (`toolbar.rs:236-257`), the Search button (`toolbar.rs:284`), and the Ctrl/Cmd+Enter shortcut (`lib.rs:508-511`) all emit `EvaluateXPath`, routed to `InspectorViewModel::evaluate_xpath()` (`inspector_vm.rs:640`, invoked from `lib.rs:545`). `evaluate_xpath` already trims `search_text` and launches an **async, streaming** search via `egui-async` — so success is not known synchronously at the call site.

Existing building blocks this change reuses (verified in code):
- A floating popup anchored to the search field: `show_search_error_popup` uses `egui::Area` + `egui::Frame::popup` with an inner scroll area and Esc/outside-click dismissal (`toolbar.rs:395-430`), open/close tracked via `ctx.data_mut` temp flags (`toolbar.rs:366-381`). This is the template for both the history dropdown and the completion popup.
- A focus-lock event filter that, while the field has focus, claims `horizontal_arrows`, `vertical_arrows`, and `escape` (`toolbar.rs:259-273`), plus Escape-cancels-in-flight-search (`toolbar.rs:275-277`). This is exactly the machinery a keyboard-driven completion popup needs to hook into.
- egui/eframe **0.34** (`Cargo.lock` → 0.34.1), app id `org.platynui.inspector` (`lib.rs:857`).

The Inspector has **no unit tests today** (no `#[cfg(test)]`/`tests/` in `apps/inspector`), so this change introduces the first ones.

## Goals / Non-Goals

**Goals:**
- Persist a most-recent-first, de-duplicated, 100-entry XPath history across restarts.
- Make the store a pure, egui-independent unit so its rules (dedup, cap, ordering, round-trip, tolerant load) are unit-testable under `cargo nextest`.
- Add a discoverable history dropdown in the search bar (browse / select / clear), independent of completion.
- Add as-you-type completion from the history, keyboard-navigable, without regressing the field's current Enter/Escape behavior.
- Keep the MVVM split: the view stays pure rendering; state and logic live in the ViewModel/store.

**Non-Goals:**
- No broad settings/persistence framework for the Inspector — just this one history artifact. (It is designed so a later general store could absorb it.)
- No completion from the XPath vocabulary (axes, node tests, functions) or from live tree names — the XPath crate exposes no vocabulary list today (`crates/xpath`), and tree-name harvesting is its own design. History-only completion is the whole completion surface here.
- No change to XPath evaluation, results, or any Python/RF/CLI surface.

## Decisions

### D1 — Persist to a dedicated JSON file via the `directories` crate

Store the history as its own JSON file at the OS per-user config dir, resolved with the cross-platform `directories` crate (`ProjectDirs`), namespaced to match the app id `org.platynui.inspector` (`lib.rs:857`) and the compositor's `platynui/` namespace — e.g. `~/.config/platynui/inspector/xpath-history.json` on Linux, the platform equivalents on Windows/macOS. Add `directories`, `serde`, and `serde_json` to `apps/inspector/Cargo.toml` (`serde` v1 is already in the workspace lockfile; the inspector crate does not yet depend on it).

- **Alternative: enable eframe's `persistence` feature and use `eframe::Storage` / `App::save`.** Rejected as the primary mechanism: it is the "native egui" path and auto-resolves the OS dir, but it (a) also persists window geometry and egui memory — a broader behavior change than this focused feature wants, (b) pulls in `ron`/`home`, and (c) couples the store to the eframe lifecycle, making it harder to unit-test in isolation. `App::save` also isn't guaranteed on force-kill, which the Inspector (an always-on-top debugging tool) invites.
- **Alternative: hand-roll `$XDG_CONFIG_HOME` like the compositor (`apps/wayland-compositor/src/config.rs:243`).** Rejected: correct only on Linux; the Inspector must also do the right thing on Windows (`%APPDATA%`) and macOS (`~/Library/Application Support`). `directories` handles all three.
- On-disk format: a small versioned wrapper `{ "version": 1, "entries": [<newest-first strings>] }` rather than a bare array, so the format can evolve without a silent misread. *(Exact `ProjectDirs::from(qualifier, org, app)` arguments and the final path string are confirmed at apply time — see Open Questions.)*

### D2 — Record on successful evaluation, in the async completion path

Only expressions that actually evaluate are recorded (user decision: no failed/invalid expressions in the history). Evaluation is async/streaming, so success is not known at submit time — it surfaces in `poll_search()` when the background task resolves: `Ok(summary)` is success (`inspector_vm.rs:685`), `Err(err)` is a parse/query failure (`inspector_vm.rs:688`). Recording therefore lives in the completion path, not in `evaluate_xpath()`.

- Mechanism: `evaluate_xpath()` (`inspector_vm.rs:640`) already trims `search_text` and moves it into the task (`inspector_vm.rs:669`); it also stashes the trimmed non-empty expression in a new `pending_history_entry: Option<String>` on the ViewModel. `poll_search()` then, on the `Ok(summary)` arm (`inspector_vm.rs:685`), takes the pending entry and calls `history.record(...)` + `history.save()`; on the `Err` arm (`inspector_vm.rs:688`) and the drain-error arm (`inspector_vm.rs:708`) it drops the pending entry without recording. `cancel_search()` (`inspector_vm.rs:755`) also clears the pending entry, so a search aborted before it completes is not recorded.
- **Success = compiled and evaluated without error, regardless of result count.** A valid expression that matches zero nodes still reaches `Ok(summary)` and is recorded; only genuine parse/query errors (the `Err` arm, which already drives the search-error popup) are excluded.
- The store's own rules (trim, ignore-empty, dedup, cap) still apply — the call site just hands the pending text to `record(...)`.

### D3 — A pure `XpathHistory` store in a new `viewmodel/xpath_history.rs`

Model the history as a bounded, de-duplicated collection (a `VecDeque<String>`, newest at the front) in a new module, owned by `InspectorViewModel`. It exposes `record(&str)`, `entries()`, `clear()`, plus `load()`/`save()` and the completion match helper (D5). Keeping it free of egui/eframe types is what makes the spec's rules unit-testable (`cargo nextest`), consistent with `dev-docs/testing-strategy.md` and the crate's view-is-pure-rendering posture.

- `record`: trim; drop if empty; remove any existing equal entry; push front; truncate to `MAX = 100` from the back.
- `save` is called synchronously right after each successful `record` (see D4).
- `load` clamps to the 100 most-recent even if the file holds more (defensive against hand-edits).

### D4 — Save synchronously on record; load once at startup

Write the whole list (≤100 short strings, well under ~10 KB) to disk synchronously after each record, and load once during `InspectorViewModel` construction. A full rewrite each time is simplest and robust against a non-graceful exit — no append/compaction logic, no reliance on a clean-shutdown hook. Load failures (missing/corrupt) degrade to an empty history with a logged warning; they never block startup or surface an error to the user.

- **Alternative: debounced / background / on-exit write.** Rejected for now: the payload is tiny and the write is infrequent (only on submit), so UI-thread blocking is negligible. If profiling ever shows otherwise, moving `save` to a debounced background task is a localized change behind the store's API.

### D5 — History dropdown and completion popup reuse the existing popup + focus-lock machinery

Both new surfaces are built from the `egui::Area` + `Frame::popup` pattern already used by `show_search_error_popup` (`toolbar.rs:395`), anchored below the search field, with open/close tracked in `ctx.data_mut` temp state like the existing error popup (`toolbar.rs:366`).

- **History dropdown:** a small button beside the field toggles a popup listing `entries()` newest-first (clicking one sets `search_text`), with a Clear History action (calls `clear()` + `save()`). Empty state shows a "no recent expressions" line.
- **Completion popup:** shown only when the field is focused and non-empty and there is ≥1 match. Candidates come from the store's match helper — a case-insensitive filter over `entries()`, preserving newest-first order, excluding an exact-equal-to-current entry. The helper and its selection-index clamping are pure and unit-tested.
- **Keyboard integration** is the load-bearing part. The field already installs a focus-lock filter claiming `vertical_arrows` + `escape` while focused (`toolbar.rs:259-273`), but a multiline `TextEdit` will otherwise consume Up/Down (cursor movement), Enter (evaluate, via `toolbar.rs:236`), Tab, and Esc. When the completion popup is open we must intercept those keys **before** the `TextEdit` reacts to them — i.e. consume the matching key events from the input queue (egui `input_mut` / `consume_key`) and route them to the completion (Up/Down move the highlight, Enter/Tab accept & fill, Esc dismiss). When the popup is closed, none are consumed, so Enter still evaluates and Esc still cancels an in-flight search exactly as today. This ordering — consume-then-add-TextEdit — is verified by running the Inspector, since it depends on egui 0.34 event-queue behavior (see Risks).

### D6 — Ship in two sequenced slices

`inspector-xpath-history` first (store + persistence + dropdown, fully usable on its own), then `inspector-xpath-completion` on top. This lets history land even if the completion keyboard integration needs iteration.

## Risks / Trade-offs

- **[Completion keyboard interception is the fiddly part]** A multiline `TextEdit` naturally eats Up/Down/Enter/Tab/Esc. If interception order or event consumption is wrong, arrow keys move the text cursor instead of the highlight, or Enter both accepts *and* evaluates. → Mitigation: consume the specific key events from the input queue before adding the `TextEdit` while the popup is open; gate all of it on "popup open"; verify interactively (Up/Down highlight, Enter accepts without evaluating, Esc closes without cancelling search, and all default behavior returns once the popup is closed). This is the primary reason completion is a separate, later slice (D6).
- **[Escape has two meanings now]** Escape must close the completion popup when open, but still cancel an in-flight search when the popup is closed (`toolbar.rs:275`). → Mitigation: explicit precedence — popup-close wins while open; the search-cancel path is only reached when the popup is closed. Covered by a spec scenario.
- **[New dependencies on a lean crate]** `directories` + `serde`/`serde_json` are new to `apps/inspector`. → Mitigation: all are already in the wider workspace lockfile or are tiny, well-established crates; the surface added is small and confined to the store module.
- **[On-disk path assumption]** The exact `ProjectDirs` arguments/path are assumed, not yet verified against a real run on each OS. → Mitigation: confirm the resolved path at apply time (Open Questions) and log it once at startup for support.
- **[Recording tied to the async completion path]** Recording moves from the synchronous submit to `poll_search()`'s `Ok`/`Err` arms, plus a small `pending_history_entry` field, so failed expressions stay out of the history (per the user decision). → Mitigation: set the pending entry only when launching a non-empty search, take-and-record it exactly on the `Ok` arm, and clear it on every non-success terminal state (`Err`, drain error, cancel) so nothing is recorded on failure and nothing lingers into the next search.
- **[No headless UI test]** Interactive popup behavior can't be asserted by `nextest`. → Mitigation: extract all logic (store rules, match filter, selection clamping) into the pure store so it *is* unit-tested; verify the UI by running the Inspector, consistent with the crate's current test posture.

## Migration Plan

- **Additive, not behavioral.** No existing behavior changes when the history is empty and the popups are closed; the search field works exactly as before. New surfaces (dropdown, completion) are purely additive.
- **No native rebuild.** The Inspector is a standalone `platynui-inspector` binary, not the `packages/native` PyO3 module — nothing in the Rust↔Python boundary is touched, and no `maturin` rebuild is required. No Python/RF/CLI surface changes.
- **Rollback:** revert the change. The history file left on disk is inert and harmless if the feature is removed; a user can delete `xpath-history.json` at any time with no effect beyond an empty history.
- **Rollout order:** slice 1 (history + persistence + dropdown), then slice 2 (completion), per D6.

## Open Questions

- **Persistence path specifics:** exact `directories::ProjectDirs::from(qualifier, organization, application)` arguments and the resulting per-OS path — confirm at apply and log it once at startup.
- **File format wrapper:** versioned `{version, entries}` (D1) vs a bare JSON array — confirm the small wrapper is worth it for forward-compat.

Resolved by the user: record **only on successful evaluation** (D2 — failed/invalid expressions are never recorded), and selecting a history entry **only fills the field** (never auto-evaluates).
- **egui 0.34 event interception:** confirm the exact API for consuming Up/Down/Enter/Tab/Esc before the `TextEdit` reads them (`input_mut`/`consume_key`) behaves as intended in 0.34 — verified while implementing slice 2.
