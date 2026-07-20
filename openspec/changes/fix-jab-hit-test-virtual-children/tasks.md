## 1. Provider

- [ ] 1.1 `node.rs` `descend_to_hit`: per-level index fallback — when `isSameObject` matching fails, read the chain target's `indexInParent` and accept it as the enumeration index iff `0 <= index < children_count` of the current level; continue the descent with the child fetched at that index
- [ ] 1.2 `node.rs` `hit_test_node`: when the descent aborts, return the deepest matched node built so far (at minimum the window) instead of `hit_fallback_node`; keep the parentless fallback only for chains that never reach the window (`reached_window == false`)

## 2. Tests

- [ ] 2.1 `live_fixture.rs`: hover-based pick (move the real cursor over the target first — the native hit-test answers only after the JVM saw a mouse event) over a fixture data cell and a column header; assert tree RuntimeId (table path + row-major index / header entry path) and a walkable ancestor chain up to `app:Application`
- [ ] 2.2 `tests/acceptance/swing/picker.robot`: pointer-hover pick over the table resolves a node whose RuntimeId matches the top-down tree node (`BM.Pointer Move To` + `BM.Get Element At Point`, reveal-equality like the existing button scenario)

## 3. Docs & verification

- [ ] 3.1 `dev-docs/platform-windows.md` hit-testing paragraph: replace the "picks never land on a cell/column" statement with the index-fallback behavior and the deepest-matched-ancestor degradation; then `just check`, `just test`, `just build-native`, and the Windows acceptance lane green
