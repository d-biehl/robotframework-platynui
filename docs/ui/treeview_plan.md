# TreeView Umsetzung – Plan & Backlog

Status: Phase 1-4 Complete, Ready for Properties Sync
Owner: Inspector Team
Last Updated: 2025-10-15

Dieser Plan leitet konkrete Arbeitspakete aus `docs/ui/treeview_spec.md` ab. Ziel ist ein schrittweiser, jederzeit lauffähiger MVP mit klaren Acceptance-Kriterien.

## Phasenübersicht
1. ✅ **Skeleton & Demo** – UI-agnostische TreeView mit Dummy-Modell
2. ✅ **Interaktion** – Auswahl, Maus & Tastatur, Scroll-Into-View, Programmatic API, Fokus
3. ✅ **Adapter & ViewModel** – Flattened Visible-Rows, Expand/Collapse, Basic Lazy Loading
4. ✅ **UiNode Integration** – Echte Datenquelle statt Demo
5. 🔄 **Properties Sync** – Selection an Properties Panel koppeln
6. **Robustheit & UX** – Error/Loading, Kontextmenü, Tastatur-Details
7. **Performance & Refactor** – Messen, Optimieren, optionaler File-Split

## Aktuelle Architektur (nach Phase 3)

```
apps/inspector/src/ui/tree/
├── adapter.rs          # TreeViewAdapter trait (UI-Port)
├── viewmodel.rs        # ViewModel (impl TreeViewAdapter, konsumiert TreeData)
├── data.rs            # TreeData trait (read-only Datenquelle)
└── data/demo.rs       # DemoData + TreeNode + demo_root() (Demo-Implementation)
```

**Entkopplung erreicht:**
- `TreeData` trait: read-only Zugriff auf beliebige Baumstrukturen (Demo, UiNode, Remote)
- `TreeViewAdapter` trait: UI-Port mit flachem Model und Commands
- `ViewModel`: implementiert beide, flacht Baum zu sichtbaren Reihen ab
- Demo isoliert in `data/demo.rs`, keine Kopplung an generische Module

## Phase 1 – Skeleton & Demo ✅ COMPLETE
Ziel: Sichtbare TreeView mit Palette-Theming und rekursiver Anzeige statischer Testdaten.

**Erledigt:**
- ✅ `TreeView` + `TreeRow` Komponenten mit Palette-Theming
- ✅ Public API: Properties/Callbacks/Functions gemäß Spec
- ✅ Flaches Demo-Modell mit Tiefe/Indentation, Disclosure-Triangle, Icon-Slot
- ✅ Integration in `app-window.slint`
- ✅ Rust-Demo-Model in `main.rs` verdrahtet

**Acceptance:** ✅ Statische Demo wird angezeigt, Palette greift, UI-Komponenten funktional

## Phase 2 – Interaktion ✅ COMPLETE
Ziel: Vollständige Single-Selection und Navigation auf dem flachen Demo-Modell.

**Erledigt:**
- ✅ FocusScope + `request_focus()` Implementation
- ✅ Maus: Klick auf Row → Selection; Klick auf Disclosure → Toggle-Events
- ✅ Tastatur: Up/Down/Home/End/PageUp/PageDown Navigation
- ✅ Scroll-Into-View bei Selection
- ✅ Events: `node-selected`, `node-toggled`, `request-children`, `request-parent`
- ✅ Programmatic API: `set_selected_node`, Toggle-Functions
- ✅ Styling: Selection/Focus mit Palette

**Acceptance:** ✅ Flüssige Maus/Tastatur-Navigation, Events emittiert, Programmatic API funktional

## Phase 3 – Adapter & ViewModel ✅ COMPLETE
Ziel: Echte Baumstruktur mit Expand/Collapse und Flattened Visible-Rows.

**Erledigt:**
- ✅ `TreeData` trait (read-only Datenquelle-Interface)
- ✅ `TreeViewAdapter` trait (UI-Port für flaches Model + Commands)
- ✅ `ViewModel` implementiert beide, flacht Baum zu sichtbaren Reihen ab
- ✅ Expand/Collapse verändert sichtbare Zeilen real
- ✅ Left/Right Tastatur: Right=Expand/FocusChild, Left=Collapse/FocusParent
- ✅ Demo-Lazy-Loading: `request-children` erzeugt Kinder on-demand
- ✅ Saubere Modularisierung: Demo isoliert in `data/demo.rs`

**Acceptance:** ✅ Expand/Collapse funktional, Lazy Loading demonstriert, Clean Architecture

## Phase 4 – UiNode Integration ✅ COMPLETE
Ziel: Echte PlatynUI UiNode-Datenquelle statt Demo-Daten.

**Tasks:**
- [x] `UiNodeData: TreeData` Implementation
  - [x] `crates/runtime` UiNode als TreeData-Quelle
  - [x] ID-Mapping: UiNode RuntimeId ↔ TreeView String-IDs
  - [x] Lazy Loading: echte `children()` und `parent()` Aufrufe
- [x] Error Handling in TreeData
  - [x] Funktionen können Fehler zurückgeben → Error-State in UI


**Acceptance:**
- ✅ Inspector zeigt echte Desktop-App-Hierarchie statt Demo
- ✅ Error-States werden sichtbar und sind retry-fähig

## Phase 5 – Properties Sync
Ziel: Properties-Panel erhält Selection-Änderungen.

**Tasks:**
- [ ] `controller.rs`: on node-selected(id) → Fetch UiNode Attributes → Properties aktualisieren
- [ ] Debounce bei schneller Navigation
- [ ] `app-window.slint`-Binding: Properties-View reagiert auf Selection
- [ ] Error-Handling: Properties-Load kann fehlschlagen

**Acceptance:**
- Auswahl in TreeView aktualisiert Properties-Bereich sichtbar
- Schnelle Navigation performant (debounced)

## Phase 6 – Robustheit & UX
Ziel: Runde UX inkl. Retry, Kontextmenü, Tastatur-Details.

**Tasks:**
- [ ] Error-Placeholder anklickbar → erneut request-children(id)
- [ ] `context-menu-requested(node_id, x, y)` emittieren + UI-Stub
- [ ] Tastatur-Details: Repeat-Verhalten, Performance bei schneller Navigation
- [ ] Loading-States: Spinner für lange `children()` Aufrufe
- [ ] Bounds-Tests: Navigation-Edge-Cases (erster/letzter Node)

**Acceptance:**
- Retry funktioniert; Kontextmenü-Event feuert; Tastaturnutzung robust
- Loading-Feedback bei langsamen UiNode-Operationen

## Phase 7 – Performance & Refactor
Ziel: Skalierbarkeit und saubere Struktur.

**Tasks:**
- [ ] Performance-Messung mit großen Bäumen (≥ 2k sichtbare Nodes)
- [ ] Inkrementelle Model-Updates (Insert/Remove statt set_vec)
- [ ] Virtual Scrolling (falls nötig)
- [ ] Optional: Slint-Datei splitten (tree-row.slint, disclosure, icons)
- [ ] Code-Cleanup: Unused imports, Documentation

**Acceptance:**
- Performance-Target erreicht; optionaler Refactor sauber dokumentiert

## Technische Errungenschaften & Leitplanken

**Erreichte Clean Architecture:**
- ✅ Domain-Agnostik: TreeView kennt nur TreeNodeVM/String-IDs, keine UiNode-Details
- ✅ Trait-basierte Entkopplung: TreeData (Quelle) ↔ TreeViewAdapter (UI-Port)
- ✅ Demo isoliert: Keine Demo-Logik in generischen Modulen
- ✅ Type-erased Adapter: main.rs nutzt `Rc<RefCell<dyn TreeViewAdapter>>`

**Design-Patterns umgesetzt:**
- ✅ MVC: View (Slint), Model (ViewModel), Controller (Event-Callbacks)
- ✅ Adapter Pattern: TreeViewAdapter abstrahiert UI-Anforderungen
- ✅ Strategy Pattern: TreeData erlaubt verschiedene Datenquellen
- ✅ Flattening: Hierarchische Daten → flache UI-Liste mit Depth

**Technische Qualität:**
- ✅ Theming: Ausschließlich Palette (`Palette.*`), keine Hex-Farben
- ✅ IDs: RuntimeId als String; konsistent zwischen Slint ↔ Rust
- ✅ Memory: Efficient mit Rc/RefCell für geteilte UI-State
- ✅ Updates: set_vec() für Model-Changes (inkrementell geplant)

**Tests & Validation:**
- ✅ Build: Inspector compiles warning-free
- ✅ Rust Tests: Workspace tests passing
- ✅ Manual Testing: Maus/Tastatur-Navigation, Expand/Collapse funktional

## Nächste Schritte (Priorität 1: UiNode Integration)

**Sofortiger Bedarf:**
1. **UiNodeData implementieren** (`src/ui/tree/data/uinode.rs`)
   - TreeData trait für echte PlatynUI UiNodes
   - ID-Mapping RuntimeId ↔ String harmonisieren
2. **Error-Handling erweitern** (failed children(), retry UI)
3. **Performance-Baseline** mit echten Desktop-Apps

**Integration-Punkt:**
- `main.rs` ändert nur eine Zeile: `DemoData::new(demo_root())` → `UiNodeData::new(runtime)`
- Gesamte UI-Logic bleibt unverändert (Clean Architecture zahlt sich aus)

## Langfristige Roadmap (nach MVP)

**Erweiterte Features:**
- Suche/Filter (Type-Ahead, XPath-Highlight)
- Multi-Select mit Ctrl/Shift
- Drag & Drop für Node-Manipulation
- Persistenz von Expansion/Selection zwischen Sessions

**Performance-Optimierungen:**
- Virtual Scrolling für sehr große Bäume
- Lazy Model-Loading mit Pagination
- Background-Threading für langsame UiNode-Calls

**UX-Verbesserungen:**
- Kontextmenüs mit Copy/Inspect Actions
- Keyboard-Shortcuts (Ctrl+F für Suche)
- Breadcrumb-Navigation für tiefe Hierarchien

## Risiko-Management

**Bekannte Risiken & Mitigation:**
- ✅ **Große Bäume** → Phase 7: Performance-Messung & Virtual Scrolling
- ✅ **Async Loading** → Phase 4: Error-States & Retry-Mechanismus
- ✅ **Focus-Verlust** → Phase 2: Explizite focus()-API implementiert
- ✅ **Memory Leaks** → Rc/RefCell pattern, automatische Cleanup

**Neue Risiken (UiNode Integration):**
- **UiNode-API Instabilität** → TreeData abstrahiert davon weg
- **Platform-spezifische Bugs** → Error-Handling in TreeData-Layer
- **Performance bei echten Apps** → Baseline-Messung in Phase 4
