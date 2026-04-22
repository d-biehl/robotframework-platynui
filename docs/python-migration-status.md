# Python-Migration — Status-Tracker

Lebendes Dokument, das den Fortschritt der Migration des Python-Teils
aus dem alten PlatynUI-Projekt (`/home/daniel/develop/tmp/robotframework-PlatynUI`)
in das neue Rust-basierte Projekt verfolgt.

Bezugsdokument: [`python-library-design.md`](./python-library-design.md)

**Stand:** 2026-04-22
**Aktuelle Revision:** Rev. 14 (PatternId Reverse-DNS)

---

## Übersicht

| Phase | Status | Bemerkung |
|---|---|---|
| Phase 0 — Smoke-Verifikation | DONE | Commit `ceb3057` |
| §13.6 Rust-PatternId Reverse-DNS | DONE | Rev. 14, uncommitted |
| Phase 1 — Fundament | PENDING | Wartet auf §13.6-Commit |
| Phase 2 — Adapter-Schicht | PENDING | unblockiert nach §13.6 |
| Phase 3 — Context-Schicht | PENDING | — |
| Phase 4 — UI-Klassen + Standard-Proxies | PENDING | — |
| Phase 5 — Keywords + Robot-Library | PENDING | — |
| Phase 6 — Iterative Erweiterungen | PENDING | — |

---

## Abgeschlossen

### Phase 0 — Smoke-Verifikation (Commit `ceb3057`)

- Linux a11y / AT-SPI-Bridge Roundtrip via `accesskit_unix` verifiziert
  (`accesskit_unix-0.21.0/src/context.rs:153-180` Gate auf
  `org.a11y.Status.ScreenReaderEnabled == true` dokumentiert)
- Helper-Scripts angelegt:
  - `scripts/linux-a11y-enable.sh`
  - `scripts/linux-a11y-restore.sh`
- Designdoc Rev. 13 mit Smoke-Befunden aktualisiert

### §13.6 Rust-PatternId-Umstellung auf Reverse-DNS (Rev. 14, **uncommitted**)

Ziel: `PatternId.as_str()` (Rust) und `pattern_name` (Python ClassVar)
liefern wörtlich denselben String (`org.platynui.patterns.<Name>`).

**Implementiert:**

- [x] Konstanten-Modul `core::ui::pattern_ids` in
      `crates/core/src/ui/identifiers.rs` (12 `&'static str`-Konstanten)
- [x] `crates/core` umgestellt: `pattern.rs`, `node.rs`, `contract.rs`,
      `contract/testkit.rs`
- [x] `crates/runtime`: `runtime/desktop.rs`, `runtime/test_fixtures.rs`
- [x] `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`
- [x] `crates/provider-atspi/src/node.rs` — `match` durch `if`-Ketten
      ersetzt (`&str`-Konstanten in Patterns nicht erlaubt)
- [x] `crates/provider-mock/src/tree.rs` und `tests.rs` — analog
- [x] `crates/provider-mock/assets/mock_tree.xml` — alle 5 Pattern-
      Varianten voll qualifiziert (kein Loader-Expand-Shortcut)
- [x] PyO3 Bindings in `packages/native/src/runtime.rs`:
  - `PyFocusable::id()` / `PyWindowSurface::id()` → Reverse-DNS via
    `pattern_ids::FOCUSABLE` / `pattern_ids::WINDOW_SURFACE`
  - `pattern_object`-Mapper nutzt Konstanten in `match`
  - `pattern_id_from_arg` liest **`pattern_name`-ClassVar** von
    Pattern-Klassen (siehe Designdoc §5 Z. 624) statt Klassenname
  - Doc-Strings für `has_pattern`, `get_pattern`, `ancestor_pattern`
    aktualisiert
- [x] Pre-existing clippy-Lint in `crates/provider-atspi/src/process.rs:72`
      behoben (Toolchain-Update auf Rust 1.95.0)
- [x] Designdoc-Updates:
  - §13.6 als RESOLVED (Rev. 14) markiert
  - §5 Rev.-13-Hinweis auf aktuellen Stand gebracht
  - XPath-Beispiel in §11a.7 (Z. 2438) auf Reverse-DNS

**Verifikation:**

- `cargo build --workspace --all-features` ✓
- `cargo nextest run --workspace --features mock-provider`
  → **1980/1980 passed** ✓
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` ✓
- `cargo fmt --all` ✓

**Breaking Change (PyO3):** `Pattern.id()` und `pattern_object` liefern
jetzt Reverse-DNS-Strings. Python-Aufrufer müssen
`get_pattern("Focusable")` auf
`get_pattern("org.platynui.patterns.Focusable")` oder
`get_pattern(Focusable)` (mit `pattern_name`-ClassVar) umstellen.

---

## Offen

### Sofort

- [ ] **§13.6-Änderungen committen** — auf User-Anweisung. Vorschlag:
      `refactor(core)!: switch PatternId to Reverse-DNS identifiers`

### Phase 1 — Fundament (Designdoc §10 Phase 1)

- [ ] `core/exceptions.py` — Exception-Hierarchie (§A.2)
- [ ] `core/settings.py` — Settings-Klasse (§A.1)
- [ ] `core/patterns/base.py` — `PatternBase` ABC mit
      `pattern_name: ClassVar[str]` (§5)
- [ ] `core/patterns/*.py` — die ~20 Standard-Patterns aus §5
      (Activatable, Focusable, WindowSurface, HasBounds, …)
- [ ] `core/ensure.py`, `core/wait.py` (§A.3)
- [ ] Tests in `packages/native/tests` und neuer `src/PlatynUI/tests`-Hierarchie

### Phase 2 — Adapter-Schicht (Designdoc §10 Phase 2)

- [ ] `core/adapter.py` — Adapter-Interface (§A.4),
      `Adapter`-Pattern, `supported_pattern_names()`
- [ ] `core/adapters/native.py` — Adapter-Implementation auf Basis von
      `platynui_native.Runtime`
- [ ] `core/adapters/mock.py` — Mock-Adapter (§A.11)
- [ ] Pattern-Default-Implementierungen (§A.10)

### Phase 3 — Context-Schicht (Designdoc §10 Phase 3)

- [ ] `core/context.py` — `ContextBase` (§A.5)
- [ ] `core/locator.py` — `@locator`-Mechanik (§A.6)
- [ ] `core/descriptor.py` — `ElementDescriptor[PatternT]` (§A.7)
- [ ] `WeightCalculator` aus Altprojekt portieren (§11.4)

### Phase 4 — UI-Klassen + Standard-Proxies (Designdoc §10 Phase 4)

- [ ] Standard-UI-Klassen (Button, TextBox, Window, …, §5a)
- [ ] `@pattern_proxy_for` Default-Proxies pro Rolle/Framework

### Phase 5 — Keywords + Robot-Library (Designdoc §10 Phase 5)

- [ ] Library-Init und Lifecycle (§A.8)
- [ ] Keywords (§8)
- [ ] Devices: Mouse/Keyboard (§A.9)
- [ ] Highlight + Diagnose (§A.12)

### Phase 6 — Iterative Erweiterungen (Designdoc §10 Phase 6)

- [ ] Application-Lifecycle Patterns
- [ ] Spezielle Container/Layout-Patterns
- [ ] Native-Read/Write-Escape-Hatches (§13.5)

---

## Offene Designfragen

Verweis auf Designdoc §13:

- §13.1 BareMetal vs. PlatynUI — Abgrenzung
- §13.3 `UiNode.supported_patterns` in Python
- §13.4 Mehrere Runtimes / Pabot
- §13.5 Native Read-Escape-Hatch (Write-Pfad)
- §13.6 ✓ RESOLVED in Rev. 14

---

## Konventionen für diesen Tracker

- Phasen entsprechen §10 des Designdokuments.
- Items werden mit `[x]` markiert, sobald Code + Tests + Lint grün.
- Bei größeren Umstellungen wird eine eigene §13.x-Sektion im Designdoc
  angelegt und hier als "RESOLVED in Rev. N" referenziert.
- Commit-Hashes nur für gemergte Arbeit; uncommittete Arbeit explizit
  als "uncommitted" markieren.
