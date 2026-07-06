<!--
Scope note: this change is Rust-only, inside apps/inspector. There is NO native
(PyO3) rebuild and NO Robot Framework surface, so the RF-mock/acceptance-suite and
native-rebuild task-ordering rules do not apply here. Tests are Rust unit tests run
via `just test-crate`; interactive UI behavior is verified by running the Inspector.
Slice 1 (groups 1-4) = inspector-xpath-history; slice 2 (groups 5-6) = inspector-xpath-completion.
-->

## 1. Dependencies & store scaffolding

- [ ] 1.1 Add `directories`, `serde` (derive), and `serde_json` to `apps/inspector/Cargo.toml` dependencies
- [ ] 1.2 Create `apps/inspector/src/viewmodel/xpath_history.rs` and register it in `apps/inspector/src/viewmodel/mod.rs`
- [ ] 1.3 Define the on-disk shape: a versioned `{ "version": 1, "entries": [...] }` serde struct and the `XpathHistory` type (a bounded `VecDeque<String>`, newest-first, `MAX = 100`)

## 2. History store — tests first (slice 1: inspector-xpath-history)

- [ ] 2.1 Write `#[cfg(test)]` unit tests in `xpath_history.rs` derived from the spec scenarios: recording adds newest-first; empty/whitespace-only input is ignored; re-recording an existing entry moves it to the front with others' order preserved; the 100-cap drops the oldest on a new distinct entry and does NOT shrink below 100 on a re-record
- [ ] 2.2 Write persistence unit tests: JSON round-trip (save then load reproduces order); a missing file loads as empty; an unparseable file loads as empty (no panic); a file with >100 entries is clamped to the 100 most-recent on load
- [ ] 2.3 Implement `record(&str)` (trim, ignore-empty, dedup-move-to-front, truncate to `MAX`), `entries()`, and `clear()` to pass 2.1
- [ ] 2.4 Implement `load()`/`save()` — resolve the file path via `directories::ProjectDirs` under `platynui/inspector/xpath-history.json` (namespaced to app id `org.platynui.inspector`), serialize the versioned wrapper, and make `load()` tolerant (missing → empty, corrupt → empty + logged warning, oversized → clamped) to pass 2.2. Log the resolved path once.

## 3. Record on submit + wire into the ViewModel (slice 1)

- [ ] 3.1 Add an `XpathHistory` field and a `pending_history_entry: Option<String>` to `InspectorViewModel` (`inspector_vm.rs`); load the history once during construction
- [ ] 3.2 In `evaluate_xpath()` (`inspector_vm.rs:640`), stash the trimmed non-empty expression in `pending_history_entry` when launching the search (covers Enter, Search button, and Ctrl/Cmd+Enter — all funnel here)
- [ ] 3.3 In `poll_search()`, record on success only: on the `Ok(summary)` arm (`inspector_vm.rs:685`) take `pending_history_entry`, call `record(...)` + `save()`; on the `Err` arm (`inspector_vm.rs:688`) and the drain-error arm (`inspector_vm.rs:708`) drop it without recording; also clear it in `cancel_search()` (`inspector_vm.rs:755`) so an aborted search is not recorded
- [ ] 3.4 Manually confirm: a successful query is recorded (incl. one that matches zero nodes), a syntactically invalid query is NOT recorded (spec: "A failing expression is not recorded"), and the record→persist→restart loop works — close/reopen and verify entries are present newest-first (spec: "History survives a restart", "Save happens on record")

## 4. History dropdown UI (slice 1)

- [ ] 4.1 Add a history dropdown button beside the XPath field in `toolbar::show_search_bar` (`toolbar.rs`), opening a popup built from the existing `egui::Area` + `Frame::popup` pattern (`show_search_error_popup`, `toolbar.rs:395`), listing `entries()` newest-first; empty history shows a "no recent expressions" line
- [ ] 4.2 Add `ToolbarAction` variants for selecting a history entry (fills `search_text`) and Clear History; route them through the command dispatcher in `lib.rs` to set the field / call `clear()` + `save()`
- [ ] 4.3 Manually verify: selecting an entry fills the field; Clear History empties the dropdown AND survives a restart (spec scenarios: "Selecting a history entry fills the field", "Clearing the history", "Empty history dropdown")

## 5. Completion match logic — tests first (slice 2: inspector-xpath-completion)

- [ ] 5.1 Write unit tests for the completion match helper on `XpathHistory`: case-insensitive filtering, results ordered newest-first, an entry exactly equal to the current input is excluded, empty input yields no suggestions
- [ ] 5.2 Write unit tests for completion selection state: Down/Up move the highlight within bounds (or wrap), and the highlight is clamped to a valid entry (or cleared) when the suggestion list shrinks — never indexing outside the list
- [ ] 5.3 Implement the match helper and the pure selection-state logic to pass 5.1–5.2

## 6. Completion popup UI + keyboard integration (slice 2)

- [ ] 6.1 Render the completion popup below the field (same `Area`/`Frame::popup` pattern) only when the field is focused, non-empty, and has ≥1 match, highlighting the selected suggestion
- [ ] 6.2 Integrate keyboard handling: while the popup is open, intercept Up/Down/Enter/Tab/Esc by consuming those key events (egui `input_mut`/`consume_key`) BEFORE the multiline `TextEdit` reacts (`toolbar.rs:236`, `toolbar.rs:259`) — Up/Down move the highlight, Enter/Tab accept & fill the field and close, Esc dismisses without changing the field
- [ ] 6.3 Preserve existing behavior when the popup is closed: Enter still evaluates and Escape still cancels an in-flight search; when the popup is open, Escape closes it first (search-cancel only on a subsequent Escape). Ensure selecting from the history dropdown does not immediately reopen the completion popup for the just-filled text
- [ ] 6.4 Manually verify all completion spec scenarios interactively: matches appear/hide correctly, arrows move the highlight, Enter/Tab accept, Esc dismisses, and the field's default Enter-evaluates / Esc-cancels behavior is intact with the popup closed

## 7. Documentation

- [ ] 7.1 Update `dev-docs/inspector.md` (Features section) to describe the persisted XPath history and history-sourced completion
- [ ] 7.2 Add a short status entry to `dev-docs/inspector-improvements.md` if it helps future slices (e.g. vocabulary/tree-name completion as a future extension)

## 8. Verification

- [ ] 8.1 `just check` (fmt, clippy, ruff, mypy) is clean
- [ ] 8.2 `just test-crate platynui-inspector` passes, including the new store and completion unit tests (these are the first tests in the Inspector crate)
- [ ] 8.3 Run the Inspector end-to-end and confirm the full manual checklist from 3.3, 4.3, and 6.4 (record→persist→restart, dropdown select/clear, completion navigate/accept/dismiss, unchanged default Enter/Esc). No native rebuild and no RF acceptance lane are needed for this change.
