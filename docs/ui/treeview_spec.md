# PlatynUI Inspector TreeView Specification

Status: Draft (MVP scope defined)
Owner: TBD
Last Updated: 2025-10-14

## 1. Purpose
Eine hierarchische TreeView-Komponente in Slint zur Darstellung beliebiger Baumdaten. Sie ist domain-agnostisch (nicht an UiNode gebunden) und wird über ein Modell (ViewModel) gespeist. Für den Inspector bildet ein Adapter die UiNode-Struktur (Accessibility-/UI-Automation-Baum) auf dieses ViewModel ab. Die TreeView dient als Navigationsquelle für Auswahl & Inspektion sowie als Einstiegspunkt für XPath-Abfragen und Attributanzeige.

## 2. Scope (MVP vs. Future)
| Capability | MVP | Future |
|------------|-----|--------|
| Rekursive Anzeige | ✔ | – |
| Expand / Collapse | ✔ | Optimierungen (Batch Expand, *) |
| Single Selection | ✔ | Multi-Select (Shift/Ctrl) |
| Mouse Navigation | ✔ | Drag & Drop (Reorder) |
| Keyboard Navigation (Arrow/Home/End) | ✔ | Type-Ahead Search |
| Lazy Loading Hook | ✔ (Trigger) | Streaming / Prefetch |
| Properties Sync | ✔ (Selection signal) | Diff/Live Updates Panel |
| Context Menu Trigger | Placeholder | Aktionen (Copy XPath, Refresh) |
| Error / Loading Placeholder | ✔ | Retry Button |
| Filtering / Suche | – | ✔ |
| Virtualisierung (perf >10k) | – | ✔ |
| Multi-root Support | ✔ | – |

## 3. Functional Requirements
1. Hierarchie beliebiger Tiefe anzeigen.
2. Jeder Node zeigt: Icon (optional), Label (Name / Role Kombination), optional Kurzinfo.
3. Expand-/Collapse-Interaktion via:
   - Klick auf Disclosure Triangle
   - Doppelklick auf Row
   - Tastatur: Right Arrow öffnet, Left Arrow schließt.
4. Selection via:
   - Einfacher Klick setzt aktive Auswahl.
   - Tastatur: Up/Down navigieren sichtbare Reihenfolge; Home/End springen zu erstem/letztem sichtbaren Node.
5. Scroll-Into-View bei Tastatur-Navigation.
6. Lazy Loading: Beim ersten Expand eines Knotens mit `has_children && !children_loaded` wird Signal `request_children(node_id)` emittiert.
7. Anzeige von Zuständen:
   - Loading Placeholder solange Kinder nachgeladen.
   - Fehler Placeholder falls Laden fehlschlug.
8. Emittierte Signale (MVP):
   - `node_selected(node_id)`
   - `node_toggled(node_id, is_expanded)`
   - `request_children(node_id)`
   - `context_menu_requested(node_id, x, y)` (später aktiv)
9. Externe API kann Selection setzen (Programmatische Auswahl).
10. Externe API kann Expansion forcieren (z.B. Pfad zu einem Node öffnen).
11. Stabiler Node Identifier unabhängig von Listenindex.

## 4. Non-Functional Requirements
- Flackerfreie Updates beim Einfügen/Entfernen einzelner Teilbäume.
- Effizient bei >2.000 sichtbaren Nodes (MVP).
- Architektur vorbereitet für Virtualisierung (sichtbare Flatten-Liste).
- Kein Blockieren des UI Threads beim Laden (Rust async liefert Ergebnisse -> Model Notify).

## 5. Data Model & State
Diese Sektion unterscheidet zwischen Domain (UiNode) und ViewModel (TreeNodeVM), damit die TreeView unabhängig vom konkreten Backend bleibt.

### 5.1 Core Node Meta (Rust)
```rust
/// NodeId ist ein String und entspricht der RuntimeId des UiNode.
/// Sie ist eindeutig während der Lebenszeit der Runtime/Session.
type NodeId = String;

struct UiNodeMeta {
    id: NodeId,            // RuntimeId des Knotens
    name: String,          // z.B. AccessibleName oder ControlName
    role: String,          // UIA/AX/AT-SPI Rolle vereinheitlicht
    has_children: bool,
    children_loaded: bool,
    icon: Option<String>,  // Optional: Symbolname oder Resource-Key (z.B. "button", "window", "image"). None => kein Icon
}
```

### 5.1.1 ViewModel (domain-unabhängig)
```rust
/// Minimale Daten, die die TreeView pro Knoten erwartet (UI-agnostisch).
struct TreeNodeVM {
    id: String,                 // opaque Id (z.B. RuntimeId)
    label: String,              // was im Baum angezeigt wird
    has_children: bool,
    icon: Option<String>,       // optionaler Symbolname/Key
    // Hierarchisches Modell (MVP): optionales Kindermodell für Repeater
    // Hinweis: Slint erwartet ein Model; in Rust als ModelRc<TreeNodeVM> übergeben
    children: Option<slint::ModelRc<TreeNodeVM>>,
    // optionale Zusatzfelder, die nur die Anzeige betreffen (z.B. role_for_badge)
}
```

Adapter-Schicht (Rust):
- Verantwortlich für die Abbildung `UiNode -> TreeNodeVM` und das Bereitstellen eines `ModelRc<TreeNodeVM>` für die Wurzel sowie für nachgeladene Kindermodelle.
- TreeView kennt nur `TreeNodeVM` und `node_id` (String) – keine UiNode-spezifischen Typen.
 - Bei Expand eines Knotens ohne geladene Kinder emittiert die TreeView `request-children(id)`; der Adapter lädt die Kinder und setzt `children = Some(ModelRc<...>)` am entsprechenden Eintrag und triggert `ModelNotify` (z. B. `data_changed`).

### 5.2 Tree Item (Flattened Visible) – Option B Vorbereitung
```rust
enum TreeItemKind { Node, Loading, Error }

struct TreeItem {
    id: NodeId,          // parent_id für Loading/Error
    depth: u16,
    kind: TreeItemKind,
    name: String,
    role: String,
    is_expanded: bool,
    has_children: bool,
    loading: bool,
    failed: bool,
}
```

### 5.3 State Maps
- `expanded: HashSet<NodeId>`
- `selection: Option<NodeId>` (MVP)
- `pending_requests: HashSet<NodeId>` (Lazy Loading in flight)
- `failed: HashSet<NodeId>`

### 5.4 Slint Model Strategy
MVP: Rekursive Darstellung via eigener `TreeNodeModel` (oder einfache verschachtelte Struktur über `Vec<ModelRc<...>>`).
Future: Wechsel auf flaches `Model` mit sichtbaren Reihen (bessere Performance + einfache Tastaturnavigation).

### 5.5 Contract: TreeView ↔ Model/Adapter
Pflichtfelder je Eintrag (Row):
- `id: string` (stabil, opaque)
- `label: string`
- `has_children: bool`
- `icon_name?: string` (optional)

Signale/Interaktion:
- TreeView emittiert `request-children(id)` sobald ein expandierter Knoten noch keine Kinder hat.
- Adapter aktualisiert das Kindermodell per `ModelNotify` (Insert/Remove/DataChanged).
- Selection/Expansion-States werden in der TreeView verwaltet, aber über `node-selected(id)` und `node-toggled(id, expanded)` nach außen gespiegelt.

Hinweis: Die TreeView benötigt keine Kenntnis über UiNode; alle Domänenbelange (Attribute, Runtime-Operationen) laufen über das Adapter-/Controller-Objekt, das `node_id` als Schlüssel verwendet.

## 6. Slint Component Architecture
```
TreeView (public)
 └─ ScrollView
     └─ Column / VerticalLayout
         └─ Repeater(TreeRootModel)
             └─ TreeRow (rekursiv) -> enthält Disclosure + Label + Children Container
```

### 6.1 Components
- `TreeView`: Exponiert Properties & Signale; kapselt ScrollView.
- `TreeRow`: Row UI + Interaktionslogik. Props: `node-id`, `depth`, `label`, `icon-name?`, `is-expanded`, `has-children`, `is-selected`, `is-loading`, `is-error`.
- `ChildrenContainer`: Falls expanded, rendert Repeater über Kindermodell.

### 6.2 Styling Guidelines
- Indentation = `depth * indent_size` (indent_size default 14px).
- Keine festen Hex-Farben; sämtliche Farben kommen aus der Slint `Palette` (Theme-respektierend).
- Hover Hintergrund: `@palette.alternate-background` leicht gemischt mit `@palette.background` (z.B. via Opacity) – konkret im Code: `background: @palette.alternate-background;`
- Auswahl Hintergrund: `@palette.accent-background` oder falls nicht vorhanden Kombination aus `@palette.accent-color` mit angepasster Opazität.
- Auswahl Text: `@palette.accent-foreground` (Fallback: `@palette.foreground`).
- Fokus: Outline 1px `@palette.accent-color` oder `@palette.focus-indicator` falls verfügbar.
- Border / Divider: `@palette.border` (statt fixer Grauwerte).
- Disclosure: Triangle (Rotation via `is-expanded ? 90deg : 0deg`).
- Loading Spinner / Placeholder: `@palette.accent-color` für aktive Elemente, Fehlersymbol ggf. `@palette.negative` (wenn Theme unterstützt) sonst `@palette.accent-color` Variation.

### 6.3 Theming & Palette Mapping
| UI Element | Palette Token (Primary) | Fallback / Notes |
|------------|-------------------------|------------------|
| Hintergrund Tree Area | `@palette.background` |  |
| Alternierende / Hover Row | `@palette.alternate-background` | Bei Hover leichte Alpha auf `alternate-background` |
| Selektierte Row Hintergrund | `@palette.accent-background` | Falls nicht definiert: Rectangle mit `@palette.accent-color` * 80% Alpha |
| Selektierte Row Text | `@palette.accent-foreground` | Fallback: `@palette.foreground` |
| Normaler Text | `@palette.foreground` |  |
| Disclosure Icon (collapsed) | `@palette.foreground` | Expanded gleiche Farbe, nur Rotation |
| Fokus Umrandung | `@palette.accent-color` | Alternativ: spezielle `focus-indicator` falls Theme eingeführt |
| Border / Divider Linien | `@palette.border` |  |
| Loading Spinner | `@palette.accent-color` |  |
| Error Placeholder Text/Icon | `@palette.negative` | Fallback: gemischte Farbe aus `accent-color` + Warnsymbol |

Implementierungs-Hinweis (Slint):
```slint
import { Palette } from "std-widgets.slint";

// Beispiel Row Hintergrund abhängig von Zustand
Rectangle {
    background: if root.is-selected {
        @palette.accent-background
    } else if hover {
        @palette.alternate-background
    } else {
        @palette.background
    };
    border-width: 0.5px;
    border-color: @palette.border;
}
```

Damit bleibt die Komponente automatisch kompatibel mit Light/Dark Themes oder kundenspezifischen Farbpaletten.

### 6.4 Icon Strategy
Icons sind optional; ein Node ohne Icon verschiebt den Text nicht abrupt. Varianten:
1. Fester Platzhalter-Bereich (z.B. 16px) vor dem Label – leer wenn kein Icon.
2. Dynamisches Layout ohne Lücke (führt zu unterschiedlicher Text-Ausrichtung) – nicht empfohlen.

Entscheidung: MVP nutzt festen Platzhalter für konsistente vertikale Lesbarkeit.

Icon Quellen:
- Eingebettete Vektor-Ressourcen (Slint `.slint` Pfad) pro Rolle.
- Dynamischer Name über Property `icon-name` -> in `TreeRow` via `@image-url("icons/" + icon-name + ".svg")` (Fehlerfall: fallback Icon oder leer).

Mapping (Beispiele):
| Rolle | Icon Name |
|-------|-----------|
| Window | window |
| Button | button |
| Text | text |
| Image | image |
| List | list |

Fallback Logik: Falls `icon-name` leer oder unbekannt -> kein Icon Rendering aber reservierter Raum.

## 7. Saubere Dateistruktur (Trennung UI/Logik)
Die TreeView wird in klar getrennte Slint-Komponenten und Rust-Module aufgeteilt. Ziel ist Wiederverwendbarkeit, Testbarkeit und klare Verantwortlichkeiten.

### 7.1 Slint Dateien (UI-Schicht)
Pfad: `apps/inspector/ui/components/`

MVP (einfach & ausreichend):
- `tree-view.slint` (public API)
    - Öffentliche Komponente mit Properties/Callbacks (Selection, Toggle, Request Children)
    - Enthält `ScrollView` und den Root-Repeater
    - Enthält private, verschachtelte Unterkomponenten (z. B. `TreeRow`, Disclosure-Button) direkt in derselben Datei

Optionales Refactoring bei wachsender Komplexität:
- `tree-row.slint` (Row-Komponente)
- `tree-disclosure.slint` (Expand/Collapse Icon/Knopf)
- `tree-icons.slint` (Icon-Mapping)
- `tree-theme.slint` (zusätzliche Style-Tokens)

Assets:
- `apps/inspector/ui/icons/` – SVG/PNG Ressourcen für optionale Rollen-Icons

### 7.2 Rust Module (Adapter/Model/Controller)
Pfad-Vorschlag: `apps/inspector/src/ui/tree/`
- `mod.rs` – Re-Export/Facade der Tree-UI-Integration
- `viewmodel.rs` – Definition `TreeNodeVM` (id, label, has_children, icon)
- `model.rs` – Implementierung eines Slint-kompatiblen `Model` (hierarchisch für MVP)
- `adapter.rs` – Abbildung von Domäne (UiNode) → `TreeNodeVM` + Laden von Kindern (Lazy)
- `controller.rs` – Event-Handling (Selection/Toggle/Context) und Weiterleitung an Runtime/Properties

Integration in `apps/inspector/src/main.rs`:
- Konstruktion des Adapters (UiNode-Quelle), Aufbau Root-Model
- Übergabe von `ModelRc` an die Slint-`TreeView`
- Verbinden der Callbacks: `node-selected`, `node-toggled`, `request-children`, `context-menu-requested`

Aufgabenteilung:
- Slint (View): Rendering, Interaktion, Theming
- Rust Model/Adapter (ViewModel/Controller): Datenbeschaffung, Lazy Loading, Backend-Operationen
Diese Trennung stellt sicher, dass die TreeView-Komponente unabhängig von der UiNode-Domäne bleibt und in anderen Kontexten wiederverwendet werden kann.
## 8. Keyboard & Mouse Interaction Mapping
| User Action | Verhalten |
|-------------|-----------|
| Left Click Row | Selection setzen |
| Left Click Disclosure | Toggle expand/collapse |
| Double Click Row | Toggle expand/collapse |
| Right Click Row | Kontextmenü Signal |
| Up Arrow | Vorherige sichtbare Row fokussieren + select |
| Down Arrow | Nächste sichtbare Row |
| Right Arrow | Expand oder wenn expanded erstes Kind select |
| Left Arrow | Collapse oder wenn collapsed auf Parent gehen |
| Home | Erste sichtbare Row |
| End | Letzte sichtbare Row |

Weitere Hinweise:
- TreeView scrollt die selektierte Zeile bei Tastatur- und programmatischer Auswahl automatisch ins Sichtfeld (Scroll-Into-View).
- Fokus: Die TreeView ist ein FocusScope; die Pfeilnavigation funktioniert, sobald die Komponente Fokus hat (per Klick oder via public `focus()` Funktion).

## 9. Signals & Properties (Slint API Draft)
```slint
export component TreeView inherits Rectangle {
    in property <bool> show-icons; // optional
    in property <int> indent-size;
    // Model entry point (MVP: hierarchical) – externally set
    in property <Model> root-model; // Pseudotyp, real: ModelRc<TreeNode>

    // Programmatic selection
    in property <string> selected-node-id; // "" = none (oder optional wenn verfügbar)

    // Signals out
    callback node-selected(node_id: string);
    callback node-toggled(node_id: string, is_expanded: bool);
    callback request-children(node_id: string);
    callback context-menu-requested(node_id: string, x: length, y: length);

    // Programmatic control (public functions)
    public function set_selected_node(id: string);
    public function expand_node(id: string);
    public function collapse_node(id: string);
    public function reveal_node(id: string); // expandiert Pfad zu id und scrollt ins Sichtfeld
    public function focus();
}
```

Hinweis zur Lebensdauer: Die `NodeId` (RuntimeId) ist stabil innerhalb einer Runtime/Provider-Session. Bei Neustart des Providers oder einer kompletten Reinitialisierung des Baums gelten alte `NodeId`-Referenzen als invalid; UI-State (Selection/Expansion) sollte dann defensiv zurückgesetzt oder über einen neu auflösbaren Pfad rekonstruiert werden.

Binding-Hinweis (Rust ↔ Slint):
- `root-model` wird in Rust als `slint::ModelRc<TreeNodeVM>` gesetzt.
- Updates erfolgen inkrementell via `ModelNotify` (z. B. `row_added`, `row_removed`, `data_changed`).
- Für rekursive Darstellung liefert jeder `TreeNodeVM` optional sein Kindermodell (`children`).

## 10. Lazy Loading Sequence
1. User expandiert Node mit `has_children && !children_loaded`.
2. Slint setzt temporär Status `loading=true`, zeigt Spinner/Placeholder.
3. Signal `request-children(node_id)` nach Rust.
4. Rust lädt Kinder, aktualisiert Backend-State & baut Child-Model.
5. TreeView erhält Model-Update (ModelNotify) -> Repaint.
6. Ladezustand entfernt.
7. Fehlerfall: Set `failed=true` -> Error Row + evtl. Retry Icon.

## 11. Update & Mutation Handling
- Insert Children: Model fügt Kind-Knoten hinzu, ruft `row_added` / `data_changed`.
- Remove: `row_removed`.
- Attributeänderung: `data_changed` für betreffende Row.
- Expansion Änderung: UI setzt `is-expanded` und zeigt/verbirgt Kind-Repeater.

## 12. Performance Considerations
- Rekursive Variante ausreichend für moderate Baumgröße (Pilot).
- Ab ~5-10k Nodes Umschwenken auf flaches sichtbares Model:
  - Flatten Operation bei Expand/Collapse inkrementell (Segment Insert/Remove).
  - Navigation = Indexarithmetik.
- Mögliches Future: Virtual Scrolling (nur sichtbare Range im Model).

## 13. Error & Loading UI Patterns
| Zustand | Darstellung |
|---------|-------------|
| Loading Children | Row mit Spinner + "Loading…" |
| Load Failed | Row mit Warn-Icon + "Failed" + Click=Retry (emit request_children) |
| Empty Root | Text Placeholder "No nodes" |

Retry-Verhalten: Klick auf den Error-Placeholder triggert erneut `request-children(id)`; der Adapter kann Fehlerzustände zurücksetzen und einen erneuten Ladevorgang starten.

## 14. MVP Implementation Steps (Backlog)
1. Statisches Demo-Modell (3 Ebenen) hardcoded.
2. Slint Komponenten `TreeView` + `TreeRow` Grundlayout.
3. Mouse Selection + Toggle Disclosure.
4. Keyboard Navigation Up/Down + Expand/Collapse (Left/Right).
5. Signals `node-selected` & `node-toggled`.
6. Lazy Loading Hook (simulate delay) + Loading Placeholder.
7. Error Placeholder Simulation.
8. Programmatic Selection API (extern setzen).
9. Integrate with Properties Panel (listen to selection).
10. Refactor für Flatten Model (option switch).

## 15. Open Questions & Risks
| Thema | Frage / Risiko | Mitigation |
|-------|----------------|-----------|
| Multi-Select | Wird gebraucht? | Später optional, Architektur entkoppelt Selection State |
| Virtualisierung | Wann nötig? | Metric sammeln (Node Count, FPS) |
| Model Ownership | Wer hält Master-Truth? | Rust als Source of Truth, Slint spiegelt Sicht |
| Async Loading | Synchronisationsprobleme? | Queue + idempotente Updates (Check existing children) |
| IDs | RuntimeId (String) stabil genug? | Ja, zur Session eindeutig; bei Provider-Restart invalidieren und UI-State säubern |

## 16. Future Enhancements (Ideas)
- Breadcrumb Pfad-Bar über Tree.
- Persistierung von Expansion & Selection pro Session.
- Suche mit Hervorhebung + Navigieren zwischen Treffern.
- XPath Live Highlight Synchronisation (mark matching nodes).
- Filter Chips (Role=Button, Editable=true etc.).
- Performance Profiler Overlay (Count, Render ms).

## 17. Reference (Slint APIs Potentially Used)
- `Model`, `ModelRc`, Custom `Model` Implementation (notify via `ModelNotify`)
- `Repeater { model: ... }`
- `TouchArea` / `MouseArea` für Row Interaktion
- `FocusScope` für Tastatur Handling
- `@ { }` JS Ausdrücke für dynamische Styles

---
End of Specification.
