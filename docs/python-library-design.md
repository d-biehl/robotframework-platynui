# PlatynUI Python Library — Design & Migrationsplan

<!-- Living document. Diskussionsgrundlage für die Portierung der Python-Schicht
     aus dem Altprojekt (`/home/daniel/develop/tmp/robotframework-PlatynUI`) auf
     den neuen Rust-basierten Kern. Keine Entscheidung ist final. -->

> **Status:** Diskussionsentwurf, **Revision 20**.
>
> **Änderungen seit Rev. 4:**
> - **Rev. 5** — modernes Python 3.10+ als verbindlicher Standard
>   (Dataclasses, `match`/`case`, PEP 604, `Self`, Hybrid-Registry für
>   Decorators).
> - **Rev. 6** — ABC bleibt Default für Pattern-Interfaces (statt
>   Protocol); keine globale `PatternRegistry` (Adapter halten lokale
>   Mapping-Tabellen); `StrEnum` für Standardrollen/Technologien
>   verworfen (freie Strings).
> - **Rev. 7** — Selbst-Review: Hybrid-Form (`class X(Base, role=…)`)
>   konsequent in Code-Beispielen, §2.6 mit Konventionen-Spickzettel,
>   §5.4 „Verwendung"-Block präzisiert (kein Widerspruch zur
>   No-Registry-Aussage), §7.1 PascalCase-Konvention dokumentiert,
>   §9 `__init__.py`-Konventionen ergänzt, §13.4 Class-Registries
>   gegen No-Pattern-Registry abgegrenzt.
> - **Rev. 8** — Neuer §9a „API-Spezifikationen" mit zwölf
>   Detail-Verträgen: Settings, Exception-Hierarchie,
>   `ensure_that`/`wait_for` (inkl. Standard-Predicates),
>   Adapter-Interface, `ContextBase`-API, `@locator`-Mechanik,
>   `ElementDescriptor[PatternT]`, Lifecycle/Robot-Library-Init,
>   Devices (Mouse/Keyboard), Pattern-Defaults, Mock-Adapter,
>   Highlight/Diagnose. Damit ist jede in §§ 2–9 referenzierte API
>   konkret unterfüttert. Selbst-Review-Fixes: `## 10.`-Überschrift
>   wiederhergestellt, `HasBounds`/`Visibility`-Patterns in §5
>   deklariert, `supports_pattern` als Adapter-Convenience,
>   Element-Convenience-Properties in §A.5 (Wrapper auf Patterns),
>   Phase-1-Hinweis konsistent zu re-entrant Thread-Local-Stack,
>   Mock-Adapter ohne `MockRuntime` (direkt per `adapter=`-Parameter
>   am Desktop), Desktop-Beispiele vereinheitlicht auf `ContextBase`,
>   `Assertable`-Herkunft verlinkt.
> - **Rev. 9** — Zweite Selbst-Review über das ganze Dokument: §4.3
>   `PatternNotSupportedError` statt generisches „Exception"; §4.4
>   Adapter-Auflösung ohne `locator.technology` (erbt aus Parent-
>   Context, `Desktop` hält den Wurzel-Adapter); §5.4
>   `WindowSurfaceActivate`-Beispiel auf existierendes
>   `WindowSurface`/`HasBounds` korrigiert; §5
>   `HasIsEnabled`/`HasIsReadonly` als eigenständige Patterns
>   deklariert (aus §A.3-Standard-Predicates heraus sichtbar); §7.2
>   auf §A.5 verlinkt; §8-Beispiel auf §A.7 verwiesen; §9 Dateiliste
>   um `Deactivatable` bereinigt, `window.py` als `WindowSurface`-
>   Cluster gekennzeichnet; §11.3 Thread-Local-Widerspruch aufgelöst
>   (bleibt für Re-entrancy; ensure.py ~80 LOC statt ~50); §13.2 als
>   „resolved in §A.7" markiert; §A.8 Adapter-Bootstrap via
>   `core/technology.py` spezifiziert (einzige prozessweite
>   Adapter-Registry; Zweck: Default-Adapter fürs Desktop-Root).
> - **Rev. 10** — Neuer §11a „Testing-Strategie" mit Drei-Ebenen-
>   Pyramide (cargo nextest / pytest / Robot Framework),
>   Test-Matrix über 12 Test-Gegenstände × 5 Kanäle,
>   RF-Mock-Harness-Spezifikation (`Load Mock Tree`, `Set Mock
>   Property`, `Set Mock Pattern Behavior`), Phase-für-Phase-
>   Testreihenfolge und CI-Matrix (Haupt-CI gegen Mock, Nightly
>   gegen echte Adapter pro OS). RF-Tests begleiten die Entwicklung
>   ab Phase 3, nicht erst am Ende.
> - **Rev. 11** — §11a.6/§11a.7 überarbeitet: Mock- und Echt-
>   Provider-Tests laufen **parallel** statt sequentiell. Neue
>   „Phase 0"-Prerequisite: AccessKit-Integration in
>   `apps/test-app-egui` (aktuell ohne AccessKit → AT-SPI blind).
>   §11a.7 spezifiziert Teststrecken pro OS: Linux (Wayland-
>   Compositor + AT-SPI), Windows (UIA), macOS (AX) — alle mit
>   `test-app-egui` als gemeinsamer Ziel-App. Dual-Mode-RF-Suites:
>   dieselbe `.robot`-Datei läuft gegen Mock und Echt-Provider,
>   gesteuert über `provider=${PROVIDER_MODE}`-Variable. CI-Matrix
>   erweitert um OS × Provider-Mode Cross-Product. Echt-Provider-
>   Tests beginnen ab Phase 3 (Linux AT-SPI) statt erst Phase 7.
> - **Rev. 12** — Korrektur in §11a.6/§11a.7.1: Annahme aus Rev. 11
>   („AccessKit fehlt in `test-app-egui`") war **falsch**. Faktencheck
>   gegen `eframe 0.34.1` Feature-Flags: `accesskit` ist Default-
>   Feature und transitiv via `egui-winit/accesskit` aktiv (ab egui
>   0.35+ obligatorisch laut Upstream-PR #7701). Die App ist seit
>   Anbeginn accessibility-aktiv — der Modul-Header (`main.rs:7`)
>   deklariert sie explizit so. Phase 0 reduziert sich von „AccessKit
>   integrieren" auf „AT-SPI-Tree verifizieren + ggf. deterministische
>   Widget-Szenarien ergänzen". Echt-Provider-Tests sind damit ab
>   sofort möglich, kein Blocker mehr.
> - **Rev. 13** — **Phase-0-Smoke-Verifikation durchgeführt** (Linux/
>   Wayland/GNOME). Ergebnis in §11a.7.1/§11a.7.2 eingearbeitet.
>   Zentrale Erkenntnisse:
>   (1) `test-app-egui` ist am AT-SPI-Bus erst sichtbar, wenn
>   `org.a11y.Status.ScreenReaderEnabled == true` ist —
>   `accesskit_unix 0.21/src/context.rs:153–180` aktiviert den Adapter
>   **ausschließlich** bei aktivem Screen Reader. Das ist Design von
>   AccessKit, keine App-Codeänderung kann das umgehen. Konsequenz:
>   Linux-Testläufe müssen das Flag vor Startup toggeln
>   (Setup-Fixture/Helper-Skript). (2) Unser `platynui-cli-rs` findet
>   die App per XPath korrekt, liefert vollständige Attribute
>   (`control:*`, `native:Accessible.*`, `Bounds`, `SupportedPatterns`)
>   und iteriert den Widget-Baum (Frame → Panel → Button/Entry/CheckBox/
>   SpinButton/ScrollBar). AccessKit→AT-SPI-Roundtrip funktioniert. (3)
>   Beim Cold-Start wirft `provider-atspi` einen `D-Bus call timed out
>   elapsed_ms=1000`-Warn — Call liefert trotzdem. Eventuell Timeout-
>   Strategie überdenken (nicht in Phase 0). (4) `native:Application.
>   ToolkitName` kommt `null` zurück — AccessKit oder egui-winit setzt
>   das nicht; Nice-to-have für später.
> - **Rev. 14** — Rust-`PatternId` durchgängig auf Reverse-DNS umgestellt
>   (siehe §13.6).
> - **Rev. 15** — **Attribute-Modell konsolidiert.** Die alten
>   `Properties`-/`NativeProperties`-Patterns und die getrennten
>   `property_*`/`native_property_*`-Adapter-Methoden werden ersatzlos
>   gestrichen. Adapter (Rust und alle künftigen) exposen Attribute
>   einheitlich als `(namespace, name) → UiValue`-Schlüsselraum, parallel
>   zum Rust-Modell (`crates/core/src/ui/node.rs`,
>   `crates/core/src/ui/namespace.rs`). Vier Namespaces sind kanonisch:
>   `control` (Default), `item`, `app`, `native`. Konsequenzen:
>   (a) `Adapter`-Interface (§A.4) hat `attribute_names(namespace=...)`
>   und `attribute_value(name, namespace=...)`, sonst nichts; (b)
>   `WeightCalculator` (§4.1) hat genau ein Attribut-Kriterium
>   `attributes[(ns, name)] == v`; (c) Locator (§A.6) trägt
>   `attributes: dict[str | tuple[str, str], str | re.Pattern[str]]` —
>   bare String = Name im Default-Namespace, Tupel = expliziter
>   Namespace; (d) Page-Object kann den Default-Namespace per
>   Klassenattribut `default_attribute_namespace = "item"` o.Ä.
>   umstellen. Symmetrisch zu Rust und zur XPath-Schreibweise
>   (`@AutomationId` ist im Default-NS `control`, `@native:HWND` ist
>   explizit). Die Robot-Keywords heißen `Get Attribute` / `Get
>   Attributes` (kein `Get Property` mehr); das `Properties`-Pattern
>   entfällt.
> - **Rev. 16** — **Locator-Konstruktor akzeptiert PascalCase-Kwargs**
>   als freie Attribut-Predicates (`Locator(Name="OK", native__HWND=1)`),
>   neben den sechs typisierten snake_case-Convenience-Feldern und dem
>   `attributes`-Dict. Doppelte Schlüssel über mehrere Kanäle werfen
>   `TypeError` mit konkreter Quellenangabe — kein stilles
>   Vorrang-Verhalten. Details in §7.1 und §A.6.
> - **Rev. 20** — **Process-wide Runtime Singleton.** `PlatynUI.core.runtime`
>   exportiert ein Singleton-Objekt `runtime` (Klasse `Runtime`) mit drei
>   Verantwortlichkeiten: *Variantenwahl* (`use_default()`, `use_mock()`,
>   `use_factory(cb)` — nur vor dem ersten `current`-Zugriff erlaubt;
>   danach gesealed), *Konsum* (`current`-Property, lazy gebaut beim
>   ersten Zugriff) und *Test-Override* (`override(...)` /
>   `override_with_mock()` als Context-Manager mit garantiertem
>   Restore). Es gibt **keinen** nackten Setter wie `set(rt)` — alle
>   Wege sind entweder Variantenwahl oder scope-gebundener Override.
>   Adapter, Device-Proxies, künftige Robot-Keywords, BareMetal-Helper
>   und Inspector teilen sich denselben `platynui_native.Runtime`.
>   Konsequenzen: `UiNodeAdapter.create_root()` braucht keinen Runtime-
>   Parameter mehr; `AdapterMouseProxy` / `AdapterKeyboardProxy` greifen
>   intern auf `runtime.current` zu. RF-Library: `Library PlatynUI
>   use_mock=${True}` mappt auf `runtime.use_mock()`. Siehe §A.5.
> - **Rev. 19** — **Rust-API-Symmetrie zu Python: `PatternId` → `PatternName`.**
>   Der Newtype `PatternId` heißt jetzt `PatternName`, das Konstanten-Modul
>   `pattern_ids` heißt `pattern_names`, die Trait-Methoden `UiPattern::id()`
>   / `UiPattern::static_id()` heißen `pattern_name()` / `static_pattern_name()`,
>   und `UiNode::pattern_by_id()` heißt `pattern_by_name()`. Damit haben
>   Rust-API und Python-API dieselbe Vokabelwahl. Wire-Format ist unverändert
>   (`org.platynui.patterns.<Name>`-Strings). PyO3-Klasse `PatternName` wird
>   bewusst NICHT aus `platynui_native.__init__` re-exportiert, weil sie
>   sonst mit dem Python-TypeAlias `PlatynUI.core.types.PatternName: TypeAlias = str`
>   kollidieren würde — Python-User-Code spricht den str-Alias, der Wrapper
>   bleibt intern (`platynui_native._native.PatternName`). Siehe §13.7.
> - **Rev. 18** — **`@locator` ist jetzt eine echte Decorator-Funktion**
>   (kein `Locator`-Alias mehr). Class-Decorator-Form
>   (`@locator(name="X") class Foo: ...`) ist vollständig implementiert
>   und hängt einen `Locator` als `Foo.__locator__` an. Method-/
>   Property-Form (`@property + @locator(...) def n5(self) -> Button`)
>   ist als `LocatorMethodDescriptor`-Stub implementiert: API steht,
>   `__get__` wirft derzeit `NotImplementedError("Phase 3")`. Die volle
>   Resolution braucht `ContextBase.get(annotation, locator=…)` aus
>   Phase 3. Page-Object-Code kann beide Formen heute schreiben — der
>   Phase-3-Übergang erfordert keine Änderung am Page-Object. Details in
>   §A.6.
> - **Rev. 17** — **Pattern-Liste konsolidiert.** Die Python-Pattern-
>   Hierarchie wird an die Rust-Capability-Gruppen
>   (`crates/core/src/ui/attributes.rs`,
>   `crates/core/src/ui/identifiers.rs`) angeglichen. Konkret:
>   (a) **`HasBounds` + `Visibility` + `HasIsEnabled`** werden zu
>   einem Pattern `Element` mit `bounds`, `is_visible`, `is_in_view`,
>   `is_enabled`, `default_click_position` zusammengeführt — analog zum
>   Rust-Modul `attributes::element`. (b) **`EditableText`** wird in
>   drei Patterns aufgeteilt: `TextContent` (read-only Properties
>   `text`, `locale`, `is_truncated`), `TextEditable` (`set_text()` +
>   `is_readonly`, `max_length`, `supports_password_mode`) und
>   `Clearable` (`clear()`). `HasIsReadonly` entfällt — Read-only-
>   Status gehört zu `TextEditable`. (c) **`Toggleable` + `HasToggleState`**
>   werden zu einem Pattern `Toggleable` mit `toggle()` + `state` +
>   `supports_three_state` zusammengeführt. (d) **`Activatable`** wird
>   um `is_activation_enabled` und `default_accelerator` erweitert.
>   (e) **`Focusable`** bleibt eigenständig (`is_focused` + `focus()`).
>   `HasFocus` entfällt. (f) **`Point`/`Rect`** werden aus
>   `platynui_native` re-exportiert (kanonische pyo3-Bindings statt
>   eigener Python-Definition).
>
> **Wire-Breaking Change in Rev. 17:** Das Rust-Attribut `IsOffscreen`
> wird zu `IsInView` umbenannt und semantisch invertiert (war: „nicht
> sichtbar im Viewport"; jetzt: „im Viewport sichtbar"). Default-
> Fallback in den Providern dreht von `false` auf `true`. Da das
> Projekt unveröffentlicht ist, ist das kein SemVer-Breaking Change.
>
> **Begriffsänderung in Rev. 4:** Wir verwenden durchgehend den Begriff
> **Pattern** statt Strategy — passend zur neuen Rust-Implementierung
> (`UiPattern`, `FocusablePattern`, `WindowSurfacePattern`). Der frühere
> Begriff „Strategy" (aus dem Altprojekt) und „Pattern" sind synonym; im
> neuen Projekt heißen sie überall Pattern.
>
> **Inhaltlich (unverändert seit Rev. 3):** Patterns werden auf
> **Python-Seite** verwaltet — nicht in Rust. Rust ist *eine* konkrete
> Implementierung der Adapter-Schicht; in Zukunft können weitere Adapter
> daneben stehen (JSON-RPC, andere Protokolle, …). Adapter dürfen Patterns
> mitbringen; zusätzlich kann pro UiNode anhand seiner Attribute eine
> spezialisierte Implementierung (Context-Klasse + Proxy mit
> Patterns) ausgewählt werden. Aus User-Sicht beschreiben wir
> UI-Elemente so, wie sie sich präsentieren — und Robot-Framework-Keywords
> drücken **Outcomes** aus (`Activate`, `Toggle`, `Select`, `Set Value`,
> `Expand`), nicht Mechanismen (`Click`).
>
> Die zugrundeliegende Mental-Model-Vorlage ist die RoboCon-2025-Präsentation
> in `docs/talks/robocon_2025.md` — insbesondere die Slides zu
> „Click ist die falsche Frage" und „semantische Aktionen mit
> Outcome-Vertrag".

## 1. Ausgangslage

### 1.1 Neues Projekt (Ist-Stand)

- **Rust-Kern** (produktiv):
  - `platynui-core` — `UiNode`, `UiAttribute`, `UiValue`, `UiPattern`, Namespaces
  - `platynui-xpath` — vollwertige XPath 2.0-Engine mit Streaming-Evaluator
  - `platynui-runtime` — `Runtime`, `PointerProfile`, `KeyboardProfile`, Provider-Registry, Cache
  - Provider: Windows UIA, AT-SPI2, macOS AX (Stub), Mock
  - **Rust-Patterns aktuell:** `FocusablePattern`, `WindowSurfacePattern`
    (mehr ist möglich, aber kein zwingender Designtreiber — siehe §6).
- **Python-Bindings** (`packages/native`): PyO3-Wrapper über
  `UiNode`/`Runtime`/Patterns.
- **Robot-Framework-Schicht**:
  - `PlatynUI.BareMetal` (917 LOC) — Low-Level-Keywords direkt über
    XPath-Strings. Funktioniert.
  - `PlatynUI` (Main-Library) — aktuell Platzhalter mit `dummy_keyword`.
    **Genau hier passiert der Port.**

### 1.2 Altes Projekt (als Quelle der Konzepte)

Speicherort: `/home/daniel/develop/tmp/robotframework-PlatynUI/src/PlatynUI/`

Wichtige Bausteine, deren Konzepte 1:1 übernommen werden (Implementierung
wird teilweise stark vereinfacht):

| Modul | LOC | Rolle | Übernahme |
|---|---|---|---|
| `core/adapter.py` | 156 | `Adapter`-Interface (UiNode + Pattern-Lookup) | **Konzept ja**, dünner |
| `core/adapterproxy.py` | 170 | `AdapterProxy` + Factory + `@adapter_proxy_for` (alt) → `@pattern_proxy_for` (neu) | **Ja, zentral** |
| `core/contextbase.py` | 473 | `ContextBase` + `ContextFactory` + `@context` | **Ja**, vereinfacht |
| `core/weight_calculator.py` | 114 | Gewichtetes Multi-Kriterien-Matching | **Ja, übernehmen** |
| `core/strategybase.py`, `core/strategies/` (alt) | — | Pattern-Interfaces (Activatable, Toggleable, …) | **Ja**, neu unter `core/patterns/` |
| `core/strategyimpl.py` (alt) | — | Default-Pattern-Implementierungen | **Ja**, leicht angepasst |
| `core/ensure.py` | 155 | Retry mit Prädikaten | **Ja**, vereinfacht (~50 LOC) |
| `core/wait_for.py` | 51 | Polling bis Prädikat | **Ja, 1:1** |
| `core/settings.py` | 66 | Timeouts, Delays | **Ja, 1:1** |
| `core/technology.py` + `AdapterFactory` | — | Bridge zu C#-Providern | Ersetzt durch `Runtime`/Provider-Registry (Rust) |
| `ui/locator.py` | 433 | XPath-Builder aus Attributen | **Ja**, ~100 LOC dank Rust-XPath |
| `ui/proxies/standardproxies.py` | 408 | Standard-Proxies pro Rolle | **Ja**, das Herzstück |
| `ui/element.py`, `window.py`, `buttons.py`, … | ~1500 | UI-Klassen (Page-Object-Basis) | **Ja** |
| `keywords/*.py` | ~250 | Robot-Framework-Keywords | **Ja**, semantisch geschärft |
| `_assertable.py` | — | bereits portiert | ✅ |

**Gesamtgröße alt:** ~4.600 LOC. **Geschätzte Zielgröße:** ~2.000 LOC, weil
Adapter-/Technology-Bridge und XPath-Builder dramatisch schrumpfen.

## 2. Designprinzipien (das Mental Model)

Diese Prinzipien sind nicht verhandelbar — sie sind die Definition dessen,
was PlatynUI **ist**.

### 2.1 User-Sicht ist führend

Tests beschreiben UI-Elemente so, wie sie sich dem **User** präsentieren —
nicht so, wie sie technisch implementiert sind. Ein Element, das aussieht
und sich verhält wie ein Button, ist im Test ein `Button` — egal ob es
intern ein nativer Button, ein Label mit ClickHandler oder ein
Custom-Composite ist.

### 2.2 Keywords drücken Outcomes aus, nicht Mechanismen

Robot-Framework-Keywords sind **semantische Aktionen mit Outcome-Vertrag**
(siehe RoboCon-Slides 4–7, 10):

- `Activate` → das UI-Element wurde aktiviert; ein verifizierbares
  Outcome ist eingetreten (Dialog öffnet, Label ändert sich, …).
- `Toggle`, `Check`, `Set Check State` → der Zustand hat sich
  beobachtbar geändert.
- `Select`, `Select Item` → die Selektion hat sich beobachtbar geändert.
- `Set Value`, `Clear`, `Append` → der Wert ist beobachtbar gesetzt.
- `Expand`, `Collapse` → der Expand-Zustand hat sich beobachtbar
  geändert.
- `Focus`, `Scroll Into View` → ein verifizierbares Outcome (Caret
  sichtbar, Element im Viewport).

**`Click` ist kein Standard-Test-Keyword.** Es bleibt verfügbar als
Low-Level-Mausoperation (BareMetal-Niveau) für Sonderfälle, aber Tests
sollen es nicht als Default benutzen — es transportiert keinen
Outcome-Vertrag (Slide 4–6).

### 2.3 Pattern-Vertrag: Pre-Conditions → Perform → Postcondition

Jede semantische Aktion hat denselben Lebenszyklus (Slide 7):

1. **Pre-Conditions** sicherstellen: aktives Top-Level-Window, Element in
   View, Element enabled, … (`ensure_that`, blockierend, raises).
2. **Perform**: die rolle-/element-spezifische Aktivierungslogik
   ausführen (was *exakt* passiert, ist die Sache der Pattern-Implementierung
   — Provider-API-Call, Maus, Tastatur, oder Kombination).
3. **Postcondition**: warten bis das beobachtbare Outcome eingetreten ist
   und die App wieder ready ist (`wait_for` + `ensure_that(...,
   raise_exception=False)`).

**Welcher Mechanismus** im Schritt „Perform" verwendet wird, ist eine
Implementierungsdetail-Entscheidung der konkreten Pattern-Implementierung
für dieses Element — nicht des Tests, nicht des Keywords.

### 2.4 Adapter-Schicht abstrahiert die UI-Quelle

PlatynUI hat eine eigene Adapter-Abstraktion in Python. Sie liefert für
ein UiNode:

- Identität, Beziehungen (parent/children, runtime_id)
- Attribute (Name, Role, Bounds, ToggleState, …)
- Verfügbare Patterns (was kann das Element von Haus aus?)
- Pattern-Aufrufe (`adapter.get_pattern(Activatable).activate()`)

**Heute** ist Rust die wichtigste Adapter-Implementierung (über die
PyO3-Bindings auf `UiNode` + `Runtime`). **Morgen** können andere
Implementierungen daneben stehen — JSON-RPC-Provider, In-Process-Mocks,
Web-/Mobile-Bridges, … Der Adapter ist explizit als
**Erweiterungspunkt** designt, nicht als „Rust-Wrapper".

### 2.5 Batterien inklusive: Standard-UI-Elemente out-of-the-box

PlatynUI liefert für die gängigen UI-Rollen (Button, CheckBox, Edit,
ComboBox, List/Tree/Table, Window, Menu, Tabs, …) fertige UI-Klassen
*und* passende Default-Proxies mit. Ein Test-Projekt definiert nur
Locator/Page Objects und nutzt direkt die semantischen Keywords —
eigene Pattern-Implementierungen sind nur für App-/Framework-Spezialfälle
nötig. Details und abgedeckte Rollen: §5a.

### 2.6 Modernes Python: eingesetzte Sprachfeatures und Konventionen

Zielversion ist **Python 3.10+** (3.10–3.13, vgl. `pyproject.toml`).
Der Altcode entstand zu Python-3.0-Zeiten und nutzt überwiegend
klassische Idiome (ABC + `__init__`-Boilerplate, `Optional[X]`,
Decorator-Side-Effects). Für die Neufassung legen wir einen modernen
Mindeststandard fest. Diese Konventionen sind **verbindlich** für neuen
Code und werden in jedem nachfolgenden Abschnitt vorausgesetzt:

**Typsystem & Daten**

- **`abc.ABC` + `@abstractmethod`** für Capability-Interfaces (Patterns,
  Adapter, Devices). Erzwingt Implementierung zur Instanzierungszeit
  (echter Fehler statt stiller Protocol-Miss), ermöglicht billige
  `isinstance`-Checks über die MRO und passt zum Registry-Mechanismus
  via `__init_subclass__`. `typing.Protocol` setzen wir nur punktuell
  ein, wo strukturelles Typing echten Mehrwert bringt (z.B. kleine
  interne Marker ohne Implementierungspflicht).
- **`@dataclass(frozen=True, slots=True, kw_only=True)`** als Default
  für alle Value-Objekte (`Settings`, `MatchCriteria`, `EnsureResult`,
  `Locator`-Bestandteile). `frozen` erzwingt Immutabilität, `slots`
  spart Speicher bei vielen UiNodes, `kw_only` macht APIs robust gegen
  Reordering.
- **PEP 604 Union-Syntax**: `X | None` statt `Optional[X]`, `int | str`
  statt `Union[int, str]`. Konsequent durchziehen.
- **`typing.Self`** (3.11+, Fallback `typing_extensions` für 3.10) für
  Builder-Pattern und Methoden, die `self`-Typ zurückgeben (z.B.
  `Locator.with_role(...)`).
- **`typing.TypeAlias`**: zentrale Aliases für `PatternName`,
  `RoleName`, `TechnologyName`, `FrameworkId`. Ein Punkt zum Ändern,
  semantisch klar. Freie Strings — kein Enum-Zwang, damit
  app-spezifische Rollen/Technologien problemlos mitgeführt werden
  können.
- **Generics** für typisierte Wrapper (`PatternProxy[P]`,
  `AdapterRef[T]`) — ersetzt `cast()`-Aufrufe an Aufrufstellen.
- **`@overload`** für die Locator-API (`by_role`, `by_xpath`,
  `by_properties`) — saubere Trennung statt Mega-`__init__` mit vielen
  Optionals.

**Kontrollfluss**

- **PEP 634 Structural Pattern Matching (`match`/`case`)** für
  Outcome-/Resolution-Dispatch (Capability-Auflösung,
  Pre-/Post-Conditions, Locator-AST-Traversal). Ersetzt verschachtelte
  `isinstance`-Ketten. (Hinweis: „Pattern" hier im Sinne des
  Python-Sprachfeatures, nicht im Sinne unserer
  Capability-Marker — diese heißen ebenfalls Pattern, sind aber etwas
  ganz anderes.)
- **`functools.cached_property`** für teure Properties am `ContextBase`
  (resolved Adapter, Pattern-Lookup-Cache pro Node-Instanz).
- **`contextlib.contextmanager`** für Wait-Loops, Highlight-Sessions,
  Mouse-Drags. Async-Varianten nur an klar abgegrenzten Punkten — Robot
  Framework ist sync-zentriert.
- **`functools.singledispatchmethod`** wo Polymorphie über Typ statt
  Vererbung ausgedrückt werden soll (z.B. Locator-Bauer aus
  unterschiedlichen Quellen).

**Decorators & Registries**

- **Hybrid-Registrierung**: `__init_subclass__` als deklarativer
  Standardweg, Decorator (`@context`, `@pattern_proxy_for`) als
  additive Variante für mehrere Rollen / Sonderfälle. Beide rufen
  intern dieselbe Registrierungsfunktion. Beispiele:

  ```python
  # Deklarativ (eine Rolle, häufigster Fall)
  class Button(UiElement, role="Button"):
      ...

  # Decorator (mehrere Rollen oder feinere Kriterien)
  @context(role="Button", framework_id="WPF")
  @context(role="ToggleButton", framework_id="WPF")
  class WpfButton(Button):
      ...
  ```

- **`typing.ParamSpec` + `Concatenate`** für die Decorator-Wrapper
  (`@ensure(...)`), damit aufgerufene Keyword-Funktionen ihre
  Typsignatur exakt behalten.

**Was wir nicht einführen**

- **Kein Pydantic / kein `attrs`** — Standard-Dataclasses reichen,
  zusätzliche Runtime-Abhängigkeiten ohne Mehrwert.
- **Kein flächendeckendes `async`/`await`** — RF ist sync, nur
  punktuell einführen, wenn ein Adapter es erzwingt.
- **Kein PEP 695 `type X = …`** als Pflicht (erst ab 3.12) — `TypeAlias`
  ist der gemeinsame Nenner für 3.10+.

**Querverweise**: §5 (ABC-basierte Patterns), §5.4 (Convention statt
Registry), §7 (Locator-API mit `@overload`), §10 Phase 1 (konkrete
Anwendungsstellen für Dataclasses, `ParamSpec`, `Self`).

**Konventionen-Spickzettel** (gilt für alle Beispiele in diesem
Dokument):

```python
from dataclasses import dataclass
from typing import ClassVar, Self, TypeAlias

PatternName: TypeAlias = str
RoleName: TypeAlias = str

@dataclass(frozen=True, slots=True, kw_only=True)
class MatchCriteria:
    role: str | None = None
    framework_id: str | None = None
    properties: dict[str, object] | None = None

class Button(Control, role="Button"):       # __init_subclass__-Hybrid
    def with_timeout(self, ms: int) -> Self: ...   # Self statt "Button"
    def activate(self) -> None: ...
```

Vermeide: `Optional[X]` (→ `X | None`), `Union[A, B]` (→ `A | B`),
`@dataclass` ohne `slots=True/frozen=True` für Wertobjekte, generische
`Dict`/`List`-Imports aus `typing` (→ Builtins).

## 3. Architekturüberblick

```
┌─────────────────────────────────────────────────────────────┐
│  Robot Framework Layer                                      │
│  Keywords: Activate / Toggle / Select / Set Value / …       │
└──────────────────────┬──────────────────────────────────────┘
                       │ (semantische Verben)
┌──────────────────────▼──────────────────────────────────────┐
│  UI-Klassen (Page Objects)                                  │
│  Button, CheckBox, ListItem, Window, …                       │
│  - registriert via @context                                 │
│  - kennen ihre Pattern-ABCs (Activatable, Toggleable)       │
│  - orchestrieren Pre/Perform/Post                           │
└──────────────────────┬──────────────────────────────────────┘
                       │ adapter.get_pattern(Activatable)
┌──────────────────────▼──────────────────────────────────────┐
│  AdapterProxy-Schicht                                       │
│  - registriert via @pattern_proxy_for                       │
│  - überschreibt/ergänzt Patterns für spezifische UiNodes    │
│  - liefert Pattern-Implementierungen (z.B. Click-basiert,   │
│    Tastatur-basiert, Provider-Pattern-basiert, gemischt)    │
└──────────────────────┬──────────────────────────────────────┘
                       │ get_pattern → fallback
┌──────────────────────▼──────────────────────────────────────┐
│  Adapter-Interface (Python)                                 │
│  - liefert Patterns, die der Provider von Haus aus kann     │
│  - exposes Attribute, Beziehungen, runtime_id               │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┬──────────────┐
        ▼              ▼              ▼              ▼
   Rust-Adapter    JSON-RPC      Mock-Adapter    (zukünftig)
   (UIA, AT-SPI,   (zukünftig)   (Tests)
    macOS AX)
```

Drei Auswahlmechanismen, alle gewichtet (siehe §4):

- **Welche UI-Klasse passt für diesen UiNode?** → `ContextFactory` +
  `@context`.
- **Welcher Proxy passt für diesen UiNode?** → `PatternProxyFactory` +
  `@pattern_proxy_for`.
- **Welche Pattern-Impl liefert das gewünschte Pattern?** →
  `proxy._self_implemented_patterns` zuerst, dann `adapter`
  (additiv). Dadurch kann ein Proxy ein Provider-Pattern überschreiben
  oder ein vom Provider fehlendes Pattern hinzufügen.

## 4. Auswahlmechanismus: WeightCalculator + zwei Registries

Das ist der **Kern der Erweiterbarkeit** und wird 1:1 aus dem Altprojekt
übernommen. Idee: Klassen registrieren sich mit Kriterien-Sets;
zur Laufzeit wird für jedes UiNode das am besten passende Match gewählt.

### 4.1 Kriterien

Jede Registrierung gibt eine Teilmenge dieser Kriterien an:

| Kriterium | Beispiel | Gewicht |
|---|---|---|
| `technology` | `UiNodeTechnology` | +100000 (oder reject) |
| `role` (exakt) | `"Button"` | +10000 (oder reject) |
| `role` (in `supported_roles`) | `"ToggleButton" ∈ {"Button", "ToggleButton"}` | +5000 - i |
| `framework_id` | `"WPF"` | +1000 (oder reject) |
| `class_name` | `"Microsoft.Maui.Controls.Button"` | +500 (oder reject) |
| `tag_name` | (DOM-artig) | +400 (oder reject) |
| `attributes[(ns, name)] == v` | `{("control", "AutomationId"): re.compile("submit-.*")}`, `{("native", "HWND"): 0xABCD}` | +200 pro Match (oder reject) |

Werte können `str`, `re.Pattern` oder beliebige `==`-vergleichbare Werte
sein. Höchstes Gewicht > 0 gewinnt; bei keinem Match: Fallback
(`UnknownContext` bzw. roher Adapter ohne Proxy).

**Attribut-Namensraum.** Schlüssel im `attributes`-Dict sind entweder
ein Tupel `(namespace, name)` oder — als Convenience — ein bloßer
String, der dann als `(<default_namespace>, name)` interpretiert wird.
Der Default-Namespace ist `control` und kann von einer Page-Object-
Klasse über das Klassenattribut `default_attribute_namespace`
umgestellt werden (siehe §A.6 / §7.1). Vier Namespaces sind
kanonisch (1:1 zu `crates/core/src/ui/namespace.rs`):

- `control` — semantische UI-Attribute (Default)
- `item` — Container-Item-Attribute
- `app` — Application-Attribute
- `native` — toolkit-/provider-spezifische Roh-Attribute

### 4.2 Zwei Registries

#### `@context` — UI-Klassen-Registry

Beantwortet: *„Welche `ContextBase`-Subklasse repräsentiert diesen UiNode
aus User-Sicht?"*

```python
class Button(Control, role="Button"):
    def activate(self) -> None: ...

@context(role="Button", framework_id="WPF",
         attributes={"ClassName": re.compile("MyApp\\..*PrimaryButton")})
class MyAppPrimaryButton(Button):
    """Spezialisierung für Primary-Buttons unserer App."""
```

Gewicht entscheidet: `MyAppPrimaryButton` schlägt das generische `Button`,
wenn `framework_id="WPF"` UND ClassName matchen. Die Hybrid-Form
(Klassen-Kwarg `role="Button"` vs. expliziter Decorator) ist in §2.6
beschrieben — Klassen-Kwarg für „eine Klasse, eine Rolle", Decorator
für mehrere Rollen oder feinere Kriterien.

#### `@pattern_proxy_for` — Pattern-Implementierung-Registry

Beantwortet: *„Welche Pattern-Implementierung gilt für diesen
UiNode?"* Ein `AdapterProxy` umhüllt den Adapter und fügt
Pattern-Implementierungen hinzu/überschreibt sie.

```python
@pattern_proxy_for(role="Button")
class ButtonProxy(ControlProxy, patterns.Activatable):
    """Standard-Button-Aktivierung."""
    def activate(self) -> None:
        # Nutzt Provider-Pattern wenn vorhanden, sonst Click — die
        # konkrete Wahl trifft die Implementierung dieses Proxys.
        ...

@pattern_proxy_for(role="Label",
                   attributes={"ClassName": "MyApp.FakeButton"})
class FakeButtonProxy(ControlProxy, patterns.Activatable):
    """Label-mit-ClickHandler bedient sich wie ein Button."""
    def activate(self) -> None:
        AdapterMouseProxy(self.adapter).click()
```

Aus Sicht der UI-Klasse `Button`: sie ruft
`adapter.get_pattern(Activatable).activate()` — der `WeightCalculator`
hat den `FakeButtonProxy` ausgewählt, also läuft die Aktivierung über
dessen Click-Implementierung. **Die `Button`-Klasse weiß nichts vom
„Fake".**

### 4.3 Pattern-Auflösung im Detail

`AdapterProxy.get_pattern(PatternT)` (im Altcode `adapterproxy.py:95`,
dort noch `get_strategy`):

1. Wenn der **Proxy** `PatternT` selbst implementiert (durch Mehrfachvererbung
   von `patterns.Activatable` etc.) → der Proxy ist die Implementierung.
2. Sonst → `self.adapter.get_pattern(PatternT)` — der darunterliegende
   Adapter liefert (wenn er kann).
3. Sonst → `None` bei `raise_exception=False`, sonst
   `PatternNotSupportedError` (siehe §A.2).

`AdapterProxy.supported_patterns` ist die **Vereinigung** aus
Proxy-Patterns und Adapter-Patterns.

**Konsequenz für den Adapter (= Rust):** Was Rust an Patterns liefert,
wird genutzt. Was Rust nicht liefert, kann der Proxy ergänzen. Was Rust
liefert, der Proxy aber „besser" macht, wird durch den Proxy
überschrieben. **Es gibt kein Entweder-Oder zwischen „Provider-Pattern"
und „User-Simulation"** — pro Pattern wird der bestmögliche verfügbare
Weg gewählt, lokal pro UiNode entschieden.

### 4.4 Auflösungs-Reihenfolge bei der Element-Erzeugung

Wenn `parent.get(child_ctx, locator=...)` aufgerufen wird (siehe §A.5):

```
1. Adapter-Quelle aus dem Parent-Context erben.
   Der Parent hält bereits einen Adapter; Children werden aus
   dessen Tree resolved (Rust-XPath-Engine via `Runtime.evaluate`).
2. Locator → XPath → Adapter.evaluate(xpath) → Sequenz roher
   Adapter-Refs auf gefundene UiNodes.
3. PatternProxyFactory.find_proxy_for(adapter)
   → wickelt einen passenden Proxy darum (gewichtsbasiert).
4. ContextFactory.find_context_class_for(proxied_adapter)
   → bestimmt die UI-Klasse (gewichtsbasiert).
5. context_type(locator, parent, proxied_adapter)
   → fertige Context-Instanz (z.B. Button-Objekt).
```

Schritt 3 passiert idealerweise **innerhalb** der Adapter-Auflösung
(im Altcode: `AdapterFactory.get_adapter` ruft
`PatternProxyFactory.find_proxy_for` auf), damit jeder Code-Pfad
dieselben Proxies sieht.

**Kein Runtime-Singleton** — der Adapter wird über die Parent-Chain
weitergereicht; das Wurzel-Context-Objekt (`Desktop`, siehe §A.8)
hält den initialen Adapter und bestimmt damit die Technology für
seinen gesamten Sub-Tree. Das `technology`-Kriterium des
`WeightCalculator` (§4.1) liest die Technology aus dem
Adapter-Objekt selbst (`adapter.technology`), nicht aus dem Locator.

## 5. Patterns als Capability-Marker

Patterns sind `abc.ABC`-Klassen (Basis `PatternBase`) mit
`@abstractmethod`-Methoden. Sie definieren **was** ein Element kann,
nicht **wie**. ABC wurde gegenüber `typing.Protocol` bevorzugt, weil
(a) die Instanzierung einer unvollständigen Implementierung sofort
einen echten Fehler wirft, (b) `isinstance`-Checks billig über die MRO
laufen und (c) `__init_subclass__` ohne Umwege verfügbar ist (auch
wenn wir aktuell keine globale Registry brauchen — siehe §5.4).

**Jedes Pattern trägt einen stabilen String-Identifier** (`pattern_name`)
im Reverse-DNS-Format `org.platynui.patterns.<Name>`. Zwei Aspekte
sind zu trennen:

- **Identifier-Pflicht (hart):** Jede Pattern-ABC muss einen
  `pattern_name` setzen. Ohne ihn kann der Adapter das Pattern nicht
  über die Drahtverbindung melden — der Identifier ist Teil des
  öffentlichen Vertrags.
- **Format (Konvention):** Reverse-DNS ist die empfohlene Form,
  aber **nicht erzwungen** — symmetrisch zu Rust-`PatternName`, das
  ebenfalls keine Format-Validierung macht. Third-Party-Patterns
  *sollten* einen eigenen Reverse-DNS-Namespace
  (`com.acme.patterns.*`) verwenden, um Kollisionen zu vermeiden.

Begründung der Identifier-Pflicht:

- Externe Adapter (JSON-RPC, Remote-Provider, fremdsprachliche
  Implementierungen) können keine Python-Klassen-Objekte über die
  Drahtverbindung schicken. Sie reden über `pattern_name`-Strings — z.B.
  `supported_patterns() → ["org.platynui.patterns.Activatable",
  "org.platynui.patterns.Focusable"]` und
  `invoke_pattern("org.platynui.patterns.Activatable", "activate", …)`.
- Rust-Bindings mappen Provider-Patterns ebenfalls über den Identifier
  auf Python-Patterns (Tabelle in `core/adapters/rust.py`).
- Die Auflösung `pattern_name ↔ Pattern-Klasse` erfolgt über
  Adapter-lokale Mapping-Tabellen, nicht über eine globale Registry
  (siehe §5.4).

> **Status Rust-Code:** Der Identifier-Mechanismus existiert
> bereits Rust-seitig: `PatternName` (newtype über `Arc<str>`,
> `crates/core/src/ui/identifiers.rs:82`), `UiPattern::pattern_name()` /
> `UiPattern::static_pattern_name() -> PatternName` als Pflicht-Trait-Methoden
> (`crates/core/src/ui/pattern.rs:18`), `PatternRegistry` mit
> `register`/`get`/`get_typed`/`supported`
> (`crates/core/src/ui/pattern.rs:57`) und `supported_patterns_value`
> serialisiert die IDs als String-Array für die FFI-Grenze
> (`pattern.rs:165`).
>
> **Rust verwendet Reverse-DNS-Identifier** (Rev. 14 als `PatternId` eingeführt,
> Rev. 15 zu `PatternName` umbenannt für Symmetrie mit Python — siehe §13.6, §13.7):
> Sowohl `PatternName::from(pattern_names::FOCUSABLE)` (= `"org.platynui.patterns.Focusable"`)
> als auch Python-`Focusable.pattern_name` liefern denselben String. Die
> Konstanten leben in `core::ui::pattern_names` (`crates/core/src/ui/identifiers.rs`)
> und werden überall statt der bare names verwendet. `PatternName` selbst
> bleibt validierungsfrei (Convention statt Format-Check); die Konsistenz
> wird in den Mapping-Tabellen und beim Python-Import sichergestellt.

Der Identifier wird von `PatternBase` als `ClassVar[str]` deklariert.
Eine `__init_subclass__`-Prüfung gibt es nicht: Eine Pattern-ABC ohne
`pattern_name` würde zur Laufzeit beim ersten Adapter-Aufruf
(`pattern.pattern_name`) einen `AttributeError` werfen — das reicht
als Sicherung, weil neue Pattern-ABCs nur in `core/patterns/` und in
Third-Party-Code definiert werden, beides Code, dessen Tests genau
diesen Pfad triggern.

Beispiele:

```python
# core/patterns/base.py
from abc import ABC
from typing import ClassVar

class PatternBase(ABC):
    """Basisklasse für alle Capability-Marker.

    Jede konkrete Pattern-ABC deklariert einen Reverse-DNS-Identifier
    via `pattern_name`. Das Format ist Konvention, keine Validierung
    (symmetrisch zu Rust-PatternName).
    """
    pattern_name: ClassVar[str]


# core/patterns/activation.py
from abc import abstractmethod
from .base import PatternBase

class Activatable(PatternBase):
    pattern_name = "org.platynui.patterns.Activatable"
    @abstractmethod
    def activate(self) -> None: ...
    @property
    @abstractmethod
    def is_activation_enabled(self) -> bool: ...
    @property
    @abstractmethod
    def default_accelerator(self) -> str | None: ...


# core/patterns/toggle.py
class Toggleable(PatternBase):
    """Toggle-Aktion *und* Toggle-Status in einem Pattern.

    Konsolidiert die alten `Toggleable` und `HasToggleState`-Patterns
    (Rev. 17). Spiegel des Rust-Moduls `attributes::toggleable`.
    """
    pattern_name = "org.platynui.patterns.Toggleable"
    @abstractmethod
    def toggle(self) -> None: ...
    @property
    @abstractmethod
    def state(self) -> "ToggleState": ...
    @property
    @abstractmethod
    def supports_three_state(self) -> bool: ...


# core/patterns/text.py
class TextContent(PatternBase):
    """Read-only Textinhalt eines Elements."""
    pattern_name = "org.platynui.patterns.TextContent"
    @property
    @abstractmethod
    def text(self) -> str: ...
    @property
    @abstractmethod
    def locale(self) -> str | None: ...
    @property
    @abstractmethod
    def is_truncated(self) -> bool: ...


class TextEditable(PatternBase):
    """Editable-Status + Schreib-Operation. Ersetzt den alten
    `EditableText` und `HasIsReadonly` (Rev. 17)."""
    pattern_name = "org.platynui.patterns.TextEditable"
    @abstractmethod
    def set_text(self, value: str) -> None: ...
    @property
    @abstractmethod
    def is_readonly(self) -> bool: ...
    @property
    @abstractmethod
    def max_length(self) -> int | None: ...
    @property
    @abstractmethod
    def supports_password_mode(self) -> bool: ...


class Clearable(PatternBase):
    """Eigenständige Clear-Operation (separater Capability-Marker)."""
    pattern_name = "org.platynui.patterns.Clearable"
    @abstractmethod
    def clear(self) -> None: ...


# core/patterns/element.py
class Element(PatternBase):
    """Konsolidiertes Element-Pattern: Geometrie + Sichtbarkeit +
    Enabled-Status. Ersetzt die alten `HasBounds`, `Visibility` und
    `HasIsEnabled` (Rev. 17). Spiegel des Rust-Moduls
    `attributes::element` (`Bounds`, `IsVisible`, `IsInView`, `IsEnabled`).
    """
    pattern_name = "org.platynui.patterns.Element"
    @property
    @abstractmethod
    def bounds(self) -> Rect: ...
    @property
    @abstractmethod
    def is_visible(self) -> bool: ...
    @property
    @abstractmethod
    def is_in_view(self) -> bool: ...
    @property
    @abstractmethod
    def is_enabled(self) -> bool: ...
    @property
    def default_click_position(self) -> Point:
        """Typisch bounds.center(); Adapter überschreiben bei Bedarf."""
        return self.bounds.center()


# core/patterns/focusable.py
class Focusable(PatternBase):
    """Fokus-Status + Fokus-Aktion (`focus()` ist Python-seitig)."""
    pattern_name = "org.platynui.patterns.Focusable"
    @property
    @abstractmethod
    def is_focused(self) -> bool: ...
    @abstractmethod
    def focus(self) -> None: ...


# Expandable, HasIsExpanded, Selectable, HasIsSelected,
# Scrollable, HasNativeWindowHandle, HasValue, EditableValue,
# … — alle als ABC mit pattern_name im
# org.platynui.patterns.*-Namespace.
#
# Hinweis: Es gibt KEIN „Properties"-Pattern. Generische Attribut-
# Reads laufen direkt am Adapter über
# `adapter.attribute_value(name, namespace=...)` (§A.4) — symmetrisch
# zu `UiNode.attribute(name, namespace)` in Rust.
```

Die Pattern-Liste folgt **eng der Rust-Capability-Gruppierung** in
`crates/core/src/ui/attributes.rs` und `pattern_names` in
`crates/core/src/ui/identifiers.rs`. Konsolidierungen gegenüber dem
Altprojekt (Rev. 17):

- `HasBounds` + `Visibility` + `HasIsEnabled` → **`Element`** (Rust:
  `attributes::element` mit `Bounds`, `IsVisible`, `IsInView`,
  `IsEnabled`).
- `Toggleable` + `HasToggleState` → **`Toggleable`** (Rust:
  `attributes::toggleable` mit `ToggleState`, `SupportsThreeState`).
- `EditableText` + `HasIsReadonly` → **`TextEditable`** (Rust:
  `attributes::text_editable` mit `IsReadOnly`, `MaxLength`,
  `SupportsPasswordMode`); read-only Inhalt → **`TextContent`** (Rust:
  `attributes::text_content`); Clear-Operation → **`Clearable`** (Rust:
  leeres Modul `attributes::clearable`, da reine Aktion ohne Attribute).
- `Activatable` ist um `is_activation_enabled` und
  `default_accelerator` erweitert (Rust: `attributes::activatable`).
- `Focusable` bleibt eigenständig; `HasFocus` entfällt.
- `Point` und `Rect` werden aus `platynui_native` re-exportiert
  (kanonische pyo3-Bindings; `Rect.center()` ist eine Methode, keine
  Property).

### 5.1 Wer implementiert Patterns?

Drei Quellen, in dieser Auflösungsreihenfolge:

1. **Proxy** (`AdapterProxy`-Subklasse via `@pattern_proxy_for`) — die
   Hauptquelle für Standard- und Sonderfälle. Hier liegt die rolle- bzw.
   element-spezifische Logik.
2. **Adapter** (= Provider-Schicht, heute: Rust-Bindings) — wenn der
   Provider direkt eine Pattern-Implementierung liefert, kann der Adapter
   sie als Pattern exposen. Beispiel heute: `Focusable` über
   `FocusablePattern`, Window-Operationen über `WindowSurfacePattern`.
3. **Default-Implementierungen** (im Altcode: `core/strategyimpl.py`,
   neu: `core/patterns/defaults.py`) — generische Implementierungen, die
   nur Adapter-Basics brauchen (`bounding_rectangle`, Maus/Tastatur).
   Greift, wenn weder ein Proxy noch der Adapter eine spezifische
   Implementierung anbietet.

### 5.2 Was passiert konkret in einer Standard-Proxy-Methode?

Aus `ui/proxies/standardproxies.py` im Altcode (vereinfacht, auf neue
Begriffe übertragen):

```python
@pattern_proxy_for(role="Button")
class ButtonProxy(ControlProxy, patterns.Activatable):
    def activate(self) -> None:
        # Heuristik: wenn der Adapter ein "natives" Activatable liefert,
        # nutze das (z.B. bei AT-SPI Action-Interface). Sonst Click.
        native = self.adapter.get_pattern(patterns.Activatable,
                                          raise_exception=False)
        if native is not None and native is not self:
            native.activate()
            return
        AdapterMouseProxy(self.adapter).click()
```

(Der Altcode ist stellenweise einfacher und ruft direkt `click()`. Beide
Varianten sind legitim — die Klasse entscheidet pro Rolle, was der
robusteste Weg ist.)

### 5.3 Verifikation gehört in die UI-Klasse

Die Proxy-Methode führt nur die **Aktion** aus. Die **Verifikation des
Outcomes** ist Sache der UI-Klasse, weil sie weiß, *was* aus User-Sicht
das verifizierbare Outcome ist:

```python
class Button(Control, role="Button"):
    def activate(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class CheckBox(Button, role="CheckBox"):
    def set_state(self, target: ToggleState) -> None:
        for _ in ToggleState:
            toggleable = self.adapter.get_pattern(patterns.Toggleable)
            current = toggleable.state
            if current == target:
                return
            toggleable.toggle()
            wait_for(lambda last=current:
                     self.adapter.get_pattern(patterns.Toggleable).state != last)
        raise CannotEnsureError(f"cannot set checkbox to {target}")
```

### 5.4 Namensauflösung: Convention statt globaler Registry

Patterns tragen ihren Reverse-DNS-Identifier als `ClassVar[str]`. Das
reicht als Vertrag zwischen Adapter und UI-Schicht — eine globale
`PatternRegistry` ist **nicht** nötig:

- **Rust-Adapter** (`core/adapters/rust.py`) hält eine lokale
  Mapping-Tabelle `PatternName-String → Python-ABC`:

  ```python
  # core/adapters/rust.py
  _RUST_PATTERN_MAP: dict[str, type[PatternBase]] = {
      Focusable.pattern_name: Focusable,
      WindowSurface.pattern_name: WindowSurface,
      Element.pattern_name: Element,
      # …
  }
  ```

- **JSON-RPC-Adapter** (zukünftig) hält seine eigene Tabelle. Die
  Patterns, die er versteht, sind Teil seines Vertrags.
- **`adapter.supported_patterns()`** liefert `list[type[PatternBase]]`
  direkt — kein Lookup-Umweg.
- **`adapter.get_pattern(Activatable)`** nimmt die Python-Klasse als
  Schlüssel; die Übersetzung auf den Draht-Identifier passiert im
  Adapter.

**Vorteile dieser Entscheidung:**

- Keine Import-Side-Effects, keine globale Mutable State.
- Jeder Adapter ist explizit darüber, was er unterstützt.
- Third-Party-Patterns funktionieren per Convention: ABC mit
  `pattern_name` deklarieren, Adapter darauf vorbereiten. Kein Hook
  nötig.
- `__init_subclass__` in `PatternBase` prüft nur, dass konkrete
  Subklassen einen `pattern_name` gesetzt haben — reine
  Missbrauchssicherung, keine Registrierung.

**Verwendung am Adapter (Beispiele):**

- `adapter.get_pattern(Activatable)` — typisierter Lookup mit der
  Python-Klasse als Schlüssel; Adapter-intern wird daraus (sofern nötig)
  ein `pattern_name`-String, um an die konkrete Implementierung zu
  kommen. Dies ist der Standardweg für Python-Code.
- `adapter.get_pattern_by_name("org.platynui.patterns.Activatable")` —
  string-basierter Lookup. Wird benötigt, wenn der Aufrufer die
  Python-Klasse nicht importieren kann (externe Adapter, Debug-
  Werkzeuge, RPC-Bridges).
- `adapter.supported_patterns()` — liefert `set[type[PatternBase]]` für
  alle Patterns, die der Adapter zur Python-Klasse auflösen kann. Eine
  zusätzliche `supported_pattern_names() → set[str]` deckt auch externe
  Patterns ab, deren Python-ABC dem Adapter (noch) nicht bekannt ist.
- **`UiNodeAdapter`** übersetzt die `PatternName`-Strings, die
  `UiNode.supported_patterns()` aus dem Rust-Code liefert, über seine
  lokale Builder-Tabelle (`_NATIVE_PATTERN_BUILDERS: dict[str,
  Callable[[UiNodeAdapter], PatternBase | None]]`) in Python-Pattern-
  Implementierungen. Builder werden gewählt, weil das Wrapping pro
  Pattern leicht unterschiedlich ist (z. B. `_NativeFocusable` braucht
  Adapter + native `Focusable`, ein zukünftiger `_NativeElement` würde
  nur den Adapter brauchen). Eine Erweiterung um ein neues Pattern ist
  ein einzelner Eintrag in der Tabelle. Die Pattern-Id-Strings sind
  bereits im Reverse-DNS-Format (`org.platynui.patterns.*`), also
  identisch zwischen Rust- und Python-Seite.

## 5a. Mitgelieferte Standard-UI-Elemente (Batterien inklusive)

**Zentrale Designentscheidung:** PlatynUI liefert für die gängigen
UI-Rollen fertige **UI-Klassen *und* passende Default-Proxies**
out-of-the-box. Ein Test-Autor schreibt nur Locator/Page Objects — für
Standardfälle ist *keine eigene Implementierung* von Pattern-Proxies
oder UI-Klassen nötig.

### 5a.1 Was "fertig" heißt

Für jede Standardrolle sind zwei Dinge registriert:

- eine UI-Klasse mit `@context(role=...)` in `ui/` (Page-Object-Ebene,
  orchestriert Pre/Perform/Post, exposed Methoden wie `activate()`,
  `set_state()`, …),
- ein Default-Proxy mit `@pattern_proxy_for(role=...)` in
  `ui/proxies/` (liefert Pattern-Implementierungen: Click-basiert,
  Tastatur-basiert, Provider-Pattern-basiert — je nach Rolle).

Beide sind die **generische Fallback-Stufe** im Gewichtungs-Match. Ein
User-Projekt kann sie jederzeit durch spezifischere Registrierungen
(höheres Gewicht über `framework_id`, `class_name`, `properties`)
überschreiben, ohne die Defaults zu entfernen.

### 5a.2 Abgedeckte Standard-Rollen

Minimal-Set, das mit v1 ausgeliefert werden soll (Rollen entsprechen
`control:*` im XPath-Namespace):

| Rolle | UI-Klasse | Default-Proxy bedient Patterns |
|---|---|---|
| `Desktop` / `Application` / `Window` / `Dialog` / `Frame` | `Desktop`, `Application`, `Window`, `Dialog` | `WindowSurface`-Patterns (über Rust), `Activatable` |
| `Button`, `Link` | `Button` | `Activatable` |
| `CheckBox`, `RadioButton`, `ToggleButton` | `CheckBox`, `RadioButton`, `ToggleButton` | `Toggleable` |
| `Edit`, `Text`, `PasswordBox` | `Edit`, `Text` | `TextContent`, `TextEditable`, `Clearable`, `HasValue` |
| `ComboBox` | `ComboBox` | `Expandable`, `Selectable`, `TextEditable` (editierbar) |
| `List`, `ListItem` | `List`, `ListItem` | `Selectable`, `HasIsSelected`, `Scrollable` |
| `Tree`, `TreeItem` | `Tree`, `TreeItem` | `Expandable`, `Selectable`, `Scrollable` |
| `Table`, `Row`, `Cell`, `Header` | `Table`, `Row`, `Cell` | `Selectable`, `Scrollable` |
| `TabList`, `TabItem` | `TabList`, `TabItem` | `Selectable` |
| `Menu`, `MenuBar`, `MenuItem` | `Menu`, `MenuBar`, `MenuItem` | `Activatable`, `Expandable` |
| `Label`, `StaticText`, `Image` | `Label`, `Image` | (lesend — kein Action-Pattern) |
| `ScrollBar`, `Slider`, `Spinner`, `ProgressBar` | `Slider`, `Spinner`, `ProgressBar` | `HasValue`, `EditableValue` |

Die Liste ist bewusst am Altprojekt (`ui/proxies/standardproxies.py`,
~400 LOC) orientiert — das dortige Set hat sich in der Praxis bewährt.

### 5a.3 Konsequenzen

- **Ein neues Test-Projekt ist produktiv, ohne eine einzige Zeile
  Proxy-Code zu schreiben.** Der User definiert Locator/Page Objects und
  ruft Keywords auf — Standard-Rollen funktionieren.
- **Framework-Spezialfälle überschreiben gezielt.** Erst wenn eine
  konkrete App/ein Framework sich anders verhält (z.B. WPF-CustomControl
  mit eigener Aktivierungslogik), registriert der User einen
  spezifischeren `@pattern_proxy_for(..., framework_id="WPF",
  class_name="...")` — der Gewichtungs-Algorithmus wählt ihn automatisch
  über den Default.
- **Fake-Button-Szenario (siehe §12)** ist genau so umgesetzt: der
  Default-Proxy für `role="Button"` bleibt unberührt; zusätzlich wird
  ein `@pattern_proxy_for(role="Label", attributes={...})` registriert,
  der im Match höheres Gewicht bekommt.
- **Drei-Ebenen-Fallback pro Pattern** (von spezifisch nach generisch):
  1. App/Framework-spezifischer Proxy (User-Registrierung)
  2. Default-Proxy für die Standard-Rolle (PlatynUI)
  3. Adapter-Pattern (Provider-nativ) oder generische
     Pattern-Defaults in `core/patterns/defaults.py`
     (Click-basiertes `Activatable`, Tastatur-basiertes `TextEditable`,
     …) — greifen, wenn keine der oberen Ebenen etwas liefert.

## 6. Was Rust beitragen kann (und was nicht)

### 6.1 Was Rust heute liefert

- **UiNode-Tree** (Provider-Bäume in einer einheitlichen Struktur)
- **Attribute** (Name, Role, Bounds, IsEnabled, IsVisible, Value,
  ToggleState, IsExpanded, IsSelected, …) — über
  `ui_node.attribute(name, namespace)`
- **XPath 2.0** — für Locator-Auflösung
- **`FocusablePattern`** — Provider-natives `focus()`
- **`WindowSurfacePattern`** — Window-Manager-API für activate/min/max/
  restore/close/move/resize (zuverlässiger als Maussimulation)
- **Pointer-/Keyboard-Devices** — niedrige Maus/Tastatur-Primitiven, die
  die Proxies nutzen können

### 6.2 Was Rust *zusätzlich* liefern könnte (optional)

Weitere `UiPattern`-Traits sind sinnvoll, wenn der Provider die Operation
zuverlässiger umsetzen kann als eine User-Simulation, oder wenn die
User-Simulation einen unverhältnismäßig hohen Aufwand hätte. Kandidaten:

- Action-Interfaces (UIA `InvokePattern`, AT-SPI `Action`,
  AX `kAXPressAction`) — für Elemente, deren Mausaktivierung schwer zu
  treffen ist (winzige Hit-Boxes, virtualisierte Listen, …)
- `ScrollPattern` — programmatisches Scrollen ist oft robuster als
  Wheel-Events
- `ValuePattern` (SetValue) — für Locale-/IME-unabhängiges Setzen langer
  Werte

**Alle diese sind Erweiterungen, keine Voraussetzungen.** Die
Python-Schicht funktioniert mit oder ohne sie — der jeweilige Proxy
nutzt das, was verfügbar ist (siehe §5.2). Wir bauen sie, wenn ein
konkreter Use-Case-Schmerz das rechtfertigt, nicht prophylaktisch.

### 6.3 Was Rust *nicht* übernimmt

- Die Pattern-Auflösung (welcher Proxy/welche UI-Klasse für welches
  UiNode?) — das bleibt Python.
- Die Outcome-Verifikation (`ensure_that`/`wait_for`) — das bleibt
  Python.
- Das Page-Object-Modell — das bleibt Python.

### 6.4 Mehrere Adapter-Implementierungen — Designeintragsplatz

Der `Adapter`-Begriff in Python ist explizit **nicht** an Rust gebunden.
Eine zukünftige `JsonRpcAdapter`-Implementierung würde:

- `Adapter`-Interface implementieren (Identität, Attribute, Patterns,
  Beziehungen)
- ihre eigene `Technology`-Markierung tragen (relevant für
  `WeightCalculator`-Kriterium `technology`)
- ihren `AdapterFactory` registrieren

**Die `pattern_name`-Identifier (§5) sind der Schlüssel dazu.** Externe
Adapter können keine Python-Klassen-Objekte über die Drahtverbindung
tragen — sie reden über Strings. Ein typisches RPC-Protokoll:

```
C → S: { "op": "supported_patterns", "node": "<runtime_id>" }
S → C: { "patterns": ["org.platynui.patterns.Activatable",
                      "org.platynui.patterns.HasValue",
                      "org.platynui.patterns.Focusable"] }

C → S: { "op": "invoke", "node": "<runtime_id>",
         "pattern": "org.platynui.patterns.Activatable",
         "method": "activate", "args": [] }
S → C: { "ok": true }
```

Der `JsonRpcAdapter` auf Client-Seite hält seine eigene
Mapping-Tabelle `pattern_name → PatternBase`, um die Python-Klasse zur
Liste hinzuzufügen, und erzeugt für jede
gemeldete Capability einen dünnen Proxy, der `get_pattern(X)` in einen
`invoke`-Call übersetzt. Server-Seite ist symmetrisch: eingehendes
`pattern`-Feld landet über dieselbe Registry bei der lokalen
Implementierung.

Das System mischt sauber: ein Test kann Rust-basierte Adapter (für
Desktop-Apps) und JSON-RPC-Adapter (für Remote-Backend) parallel im
selben Lauf nutzen — beide sprechen dieselben `pattern_name`-Strings.

## 7. Locator + Page Objects

### 7.1 Locator als Beschreibung

Der Locator beschreibt, **wie** ein UiNode gefunden wird. Implementiert
als XPath-Builder, der auf der Rust-XPath-2.0-Engine aufsetzt. Vereinfacht
gegenüber dem Altprojekt (433 → ~100 LOC), weil die XPath-Engine das
schwere Heben übernimmt.

```python
@locator(name="Rechner")
class CalculatorWindow(Window):
    @property
    @locator(AutomationId="num5Button")
    def n5(self) -> Button: ...

    @property
    @locator(AutomationId="equalButton")
    def equal(self) -> Button: ...
```

`@locator` baut intern XPath. Es gibt **keine Pattern-Override-Funktion am
Locator selbst** — das wäre konzeptionell falsch verortet, weil derselbe
UiNode dann je nach Locator unterschiedlich behandelt würde. Pattern-
Auswahl gehört in die `@pattern_proxy_for`-Registry (siehe §4), wo sie
auf Eigenschaften des UiNodes selbst basiert.

> **Attribut-Namenskonvention.** Es gibt **drei** Eingangskanäle für
> Attribut-Predicates am `Locator`. Sie können frei gemischt werden,
> aber dasselbe Attribut darf nur über **genau einen** Kanal gesetzt
> werden — Konflikte werfen `TypeError` (kein stilles Vorrang-Verhalten).
>
> 1. **Sechs verdrahtete Convenience-Felder** (typisierte Parameter):
>    `name`, `id`, `class_name`, `role`, `runtime_id`, `framework_id`.
>    Diese sind **snake_case** (Python-Identifier-Konvention) und
>    werden vom Framework auf ihre PascalCase-XPath-Form gemappt
>    (`Locator(role="Button", name="Hallo")` → `Button[@Name="Hallo"]`,
>    `class_name=` → `@ClassName=`, `runtime_id=` → `@RuntimeId=`,
>    `framework_id=` → `@FrameworkId=`). Das Mapping ist abgeschlossen
>    und nicht erweiterbar — es deckt nur diese sechs hochfrequenten
>    Felder ab. Implementierung: `Locator._standard_attributes()`.
>
> 2. **Freie Kwargs am Konstruktor / Decorator**: Jeder Kwarg, der
>    *kein* reserviertes Locator-Feld ist (also nicht in
>    `Locator.RESERVED_FIELDS`), wird als Attribut interpretiert. Der
>    Kwarg-Name geht **wörtlich** in den XPath, ohne jede
>    Case-Konvertierung — per Konvention ist er PascalCase:
>    `Locator(AutomationId="x")` → `[@AutomationId="x"]`,
>    `@locator(IsEnabled="true")` → `[@IsEnabled="true"]`. Für
>    Attribute außerhalb des `control`-Default-Namespace gilt der
>    Doppelunterstrich-Trenner: `Locator(native__HWND=0xABCD)` →
>    `[@native:HWND="..."]`. Mehrere `__` im Kwarg-Namen sind nicht
>    erlaubt — komplexere Schlüssel gehen über das `attributes`-Dict.
>
> 3. **Freies `attributes`-Dict** (für programmatisch konstruierte
>    Schlüssel oder Attribute, deren Name kein Python-Identifier ist):
>    Schlüssel werden wörtlich in den XPath übernommen. Bare String =
>    Default-Namespace, Tupel `(namespace, name)` = explizit:
>    `attributes={"AutomationId": "x"}` → `[@AutomationId="x"]`,
>    `attributes={("native", "HWND"): 0xABCD}` → `[@native:HWND="..."]`.
>
> Begründung der bewussten Asymmetrie zwischen (1) und (2)/(3):
> dataclass-artige Felder *müssen* Python-Identifier sein und folgen
> damit PEP 8 (snake_case); für Attribut-Schlüssel wäre eine
> heuristische snake_case→PascalCase-Brücke unzuverlässig, weil
> Acronym-Sonderfälle (`HWND`, `OS`, `URL`, `XPath`) jede Heuristik
> brechen. Identitäts-Mapping mit explizitem User-Wording ist
> verlässlicher als „cleveres" Auto-Casing.
>
> **Empfehlungs-Reihenfolge:** Convenience-Feld vor Kwarg vor Dict.
> Den Dict-Weg primär für dynamisch konstruierte Schlüssel oder
> Cross-Namespace-Tupel verwenden.
>
> **Namensraum.** Bare Attribut-Namen (sowohl Kwargs ohne `__` als
> auch String-Schlüssel im Dict) werden im Default-Namespace der
> Page-Object-Klasse aufgelöst — standardmäßig `control`. Eine Klasse
> kann das per Klassenattribut umstellen, z.B.
> `class ListItem(Item): default_attribute_namespace = "item"`. Für
> explizite Cross-Namespace-Attribute nutzt das `attributes`-Dict
> Tupel-Keys oder der Kwarg den `__`-Trenner. Siehe §A.6.

### 7.2 Page Objects via `@context`

UI-Klassen sind Page-Object-Bausteine. `@context` registriert sie für
Auflösung über `ContextFactory` (siehe §4.2). Vollständige
`ContextBase`-API und Convenience-Properties (`bounding_rectangle`,
`is_visible`, …) siehe §A.5.

## 8. Robot-Framework-Keywords

Keywords drücken Outcomes aus. Liste der Standard-Keywords (siehe
RoboCon-Slide 10):

| Keyword | Pattern(s) | Outcome |
|---|---|---|
| `Activate` | `Activatable` (+ App-ready) | Aktivierung erfolgt + verifiziert |
| `Focus` | `Focusable` | Element hat Fokus |
| `Toggle` | `Toggleable` | Toggle-State hat sich geändert |
| `Check` / `Uncheck` / `Set Check State` | `Toggleable` | Ziel-State erreicht |
| `Select` / `Deselect` / `Select Item` | `Selectable` + `HasIsSelected` | Selektion verifiziert |
| `Expand` / `Collapse` | `Expandable` + `HasIsExpanded` | Expand-State erreicht |
| `Set Value` / `Append` | `TextEditable` + `HasValue` | Wert verifiziert |
| `Clear` | `Clearable` | Wert ist leer |
| `Scroll Into View` | `Scrollable` + `IsInView`-Check | Element im Viewport |
| `Activate Window` / `Maximize Window` / `Minimize Window` / `Close Window` | `WindowSurface`-Patterns | Window-State verifiziert |
| `Get Attribute` / `Get Attributes` | direkter Attribut-Read am Adapter (`adapter.attribute_value(name, namespace=...)`) | Wert |
| `Wait Until Exists` / `Wait Until Gone` | Locator + `wait_for` | Existenz |

Implementierung: jedes Keyword nimmt ein UI-Element-Argument (typisiert
über `ElementDescriptor[PatternT]`) und ruft die entsprechende Methode
der UI-Klasse auf.

```python
# keywords/activate.py
@keyword
def activate(element: ElementDescriptor[Activatable]) -> None:
    element().activate()
```

Der Robot-Converter prüft beim Argument-Parsing, dass das übergebene
Element das `Activatable`-Pattern unterstützt — fehlt es, wird der
Aufruf bereits *vor* der Keyword-Ausführung abgelehnt. Details zum
Parsing, zum Pattern-Check und zur Fehlerbehandlung siehe §A.7.

## 9. Vorschlag für die Zielstruktur

Konventionen für `__init__.py`-Dateien:

- **`PlatynUI/__init__.py`** — Robot-Framework-Library-Klasse, die alle
  Keywords aus `keywords/*.py` einsammelt und exportiert. Zusätzlich
  Re-Export der wichtigsten Page-Object-Symbole (`Button`, `Window`,
  `@locator`, …) für direkten Python-Import.
- **`core/__init__.py`** — Re-Export der öffentlichen API
  (`Adapter`, `AdapterProxy`, `@pattern_proxy_for`, `@context`,
  `ContextBase`, `ensure_that`, `wait_for`, …).
- **`core/patterns/__init__.py`** — sammelt alle Pattern-ABCs aus den
  Submodulen, sodass `from PlatynUI.core.patterns import Activatable`
  ohne Wissen um die Datei-Aufteilung funktioniert.
- **`ui/__init__.py`** und **`ui/proxies/__init__.py`** — Aggregate
  Re-Exports. Beide importieren ihre Submodule mit Side-Effect, damit
  die `@context`/`@pattern_proxy_for`-Registrierungen beim
  Library-Import passieren.
- **`keywords/__init__.py`** — Sammelpunkt für die Robot-Library.

```
src/PlatynUI/
├── __init__.py                     # PlatynUI Robot Library (high-level)
├── __version__.py
├── _assertable.py                  # ✅ bleibt
├── _our_libcore.py                 # ✅ bleibt
│
├── BareMetal/                      # ✅ bleibt als Low-Level-Library
│   └── __init__.py
│
├── core/                           # NEU — Infrastruktur
│   ├── __init__.py
│   ├── adapter.py                  # Adapter-Interface (~150 LOC)
│   ├── adapter_proxy.py            # AdapterProxy + PatternProxyFactory + @pattern_proxy_for (~170 LOC)
│   ├── context.py                  # ContextBase + ContextFactory + @context (~250 LOC)
│   ├── weight_calculator.py        # 1:1 aus alt (~115 LOC)
│   ├── locator.py                  # Locator, @locator, LocatorScope (~120 LOC)
│   ├── descriptor.py               # ElementDescriptor[PatternT]
│   ├── ensure.py                   # ensure_that, @predicate (~50 LOC)
│   ├── wait.py                     # wait_for (~40 LOC)
│   ├── settings.py                 # Settings dataclass (~70 LOC)
│   ├── exceptions.py               # Exception-Hierarchie
│   ├── technology.py               # Technology-Marker, AdapterFactory-Registry (~60 LOC)
│   ├── types.py                    # TypeAliases (PatternName, RoleName, FrameworkId, …)
│   ├── patterns/                   # Pattern-Interfaces (im Altprojekt: strategies/)
│   │   ├── __init__.py             # re-exports
│   │   ├── base.py                 # PatternBase (ABC) + pattern_name-Check
│   │   ├── activation.py           # Activatable
│   │   ├── toggle.py               # Toggleable (toggle + state + supports_three_state)
│   │   ├── text.py                 # TextContent, TextEditable, Clearable, HasValue
│   │   ├── element.py              # Element (bounds + visibility + enabled + click_position)
│   │   ├── focusable.py            # Focusable (is_focused + focus())
│   │   ├── expand.py               # Expandable, HasIsExpanded
│   │   ├── selection.py            # Selectable, HasIsSelected, …
│   │   ├── window.py               # WindowSurface (Activate/Close/Min/Max als Methoden)
│   │   ├── defaults.py             # Default-Implementierungen (alt: strategyimpl.py)
│   │   └── …
│   ├── devices.py                  # MouseProxy/KeyboardProxy (Wrapper über platynui_native.Runtime)
│   └── adapters/                   # Adapter-Implementierung(en)
│       ├── __init__.py
│       └── ui_node.py              # UiNodeAdapter, wraps platynui_native.UiNode
│
├── ui/                             # UI-Klassen + Standard-Proxies
│   ├── __init__.py
│   ├── element.py                  # Element (Base-UI-Klasse)
│   ├── control.py                  # Control
│   ├── window.py                   # Window, Frame, Dialog
│   ├── buttons.py                  # Button, CheckBox, RadioButton, ToggleButton, Link
│   ├── text.py                     # Text, Edit, Label
│   ├── lists.py                    # List, ListItem
│   ├── tree.py                     # Tree, TreeItem
│   ├── table.py                    # Table, Row, Cell, Header
│   ├── tabs.py                     # TabList, TabItem
│   ├── menus.py                    # Menu, MenuBar, MenuItem
│   ├── combobox.py
│   ├── desktop.py                  # Desktop (Root)
│   ├── application.py              # Application
│   └── proxies/                    # Standard-AdapterProxies pro Rolle
│       ├── __init__.py
│       ├── base.py                 # ControlProxy, ContainerProxy, ItemProxy
│       ├── standard.py             # Button/CheckBox/Menu/… Proxies (~400 LOC, port aus altem standardproxies.py)
│       ├── text.py                 # Text/Edit/ComboBox-Proxies
│       ├── list_tree.py            # List/Tree-Item-Proxies
│       └── window.py               # WindowProxy (nutzt WindowSurfacePattern)
│
└── keywords/                       # Robot-Framework-Keywords
    ├── __init__.py
    ├── activate.py                 # Activate, Focus
    ├── toggle.py                   # Toggle, Check, Uncheck, Set Check State
    ├── select.py                   # Select, Deselect, Select Item
    ├── text.py                     # Set Value, Clear, Append, Get Value
    ├── expand.py                   # Expand, Collapse
    ├── scroll.py                   # Scroll Into View, Scroll
    ├── window.py                   # Activate/Close/Min/Max Window
    ├── properties.py               # Get Attribute, Get Attributes
    ├── wait.py                     # Wait Until Exists / Gone
    └── application.py              # Start/Close Application
```

## 9a. API-Spezifikationen

Dieser Abschnitt füllt die Detail-Verträge, auf die §§ 2–9 verweisen.
Reihenfolge so gewählt, dass spätere Abschnitte auf frühere aufbauen
(Settings → Exceptions → ensure/wait → Adapter → ContextBase → Locator
→ Descriptor → Lifecycle → Devices → Defaults → Mock → Highlight).

### A.1 Settings (`core/settings.py`)

`Settings` ist eine `@dataclass(frozen=True, slots=True, kw_only=True)`
mit prozessweitem Singleton-Zugriff über `Settings.current()` und einem
`with`-Block für lokale Overrides. Felder (1:1 aus Altcode `core/settings.py:5`,
nur Defaults konsolidiert):

```python
@dataclass(frozen=True, slots=True, kw_only=True)
class Settings:
    # Wartezeiten
    wait_for_timeout: float = 1.0
    wait_for_delay: float = 0.1
    ensure_timeout: float = 15.0
    ensure_delay: float = 0.1
    exists_timeout: float = 1.0
    window_close_timeout: float = 1.0
    # Tastatur
    input_after_input_delay: float = 0.001
    keyboard_after_press_key_delay: float = 0.01
    keyboard_after_release_key_delay: float = 0.01
    keyboard_after_press_release_delay: float = 0.05
    # Maus
    mouse_before_next_click_delay_multiplicator: float = 1.5
    mouse_after_click_delay: float = 0.010
    mouse_multi_click_delay_multiplicator: float = 0.5
    mouse_press_release_delay: float = 0.010
    mouse_after_move_delay: float = 0.010
    mouse_move_delay: float = 0.001
    mouse_move_time: float = 0.2
    # Display / Diagnose
    display_screenshot_format: str = "png"
    display_screenshot_quality: int = -1
    display_screenshot_basename: str = "screenshot"
    element_highlight_time: float = 2.0
    element_highlight_ensure_timeout: float = 2.0
```

**Konfiguration:**

- **Programmatisch (default):** `Settings.set_current(Settings(ensure_timeout=30))`
  ersetzt das Singleton. Der Setter ist explizit, um „magisches Mutieren"
  einer eingefrorenen Dataclass zu vermeiden.
- **Mit-Block (skoped):** `with Settings(ensure_timeout=30): ...`
  pusht/popt das Singleton; nestable, automatisch restauriert.
- **Robot-Variablen:** Eine schmale Brücke
  (`PlatynUI/keywords/settings.py`, neu) liest beim Library-Init
  `${PLATYNUI_ENSURE_TIMEOUT}` etc. aus dem RF-Variablenscope und ruft
  `Settings.set_current(...)`. Dadurch sind Settings pro Suite/Test
  konfigurierbar, ohne Python-Code zu schreiben.
- **Kein Env-Var-Loading.** Konsequent: der Altcode hat es nicht, der
  RF-Weg ist standardisiert, und Env-Var-Loading vermischt sich
  schlecht mit dem `with`-Block.

**Scope:** prozessglobal, **nicht thread-safe** (wie im Altcode). Für
`pabot` reicht das, weil jeder Worker-Prozess eigenen State hat.

### A.2 Exception-Hierarchie (`core/exceptions.py`)

```text
PlatynUIError                         # Basis (alt: PlatyUiError, Typo gefixed)
├── PlatynUIFatalError                # nicht-recoverable; ensure/wait re-raisen sofort
│   └── AdapterNotFoundFatalError
├── AdapterError                      # Basis für Adapter-Probleme
│   ├── AdapterNotValidError          # Adapter-Lifetime abgelaufen
│   ├── AdapterNotFoundError          # Locator hat nichts gefunden (recoverable)
│   ├── PatternNotSupportedError      # alt: AdapterNotSupportsStrategyError
│   └── NotAPatternTypeError          # alt: NotAStrategyTypeError
├── EnsureError
│   └── CannotEnsureError             # ensure_that-Timeout
├── LocatorError
│   ├── NoLocatorDefinedError
│   ├── MultipleElementsFoundError    # neu: get_one mit mehreren Treffern
│   └── ElementNotFoundError          # Alias für AdapterNotFoundError im Element-Kontext
└── DeviceError
    ├── NoMouseDeviceError            # alt: NoMouseProxyError
    ├── NoKeyboardDeviceError         # alt: NoKeyboardProxyError
    └── NoDisplayDeviceError          # alt: NoDisplayProxyError

# Außerhalb der Hierarchie (Standard-Python-Erwartungen):
NotSupportedError(NotImplementedError)
InvalidArgumentError(ValueError)
```

`Ensure.that()` und `wait_for()` lassen `PlatynUIFatalError`,
`KeyboardInterrupt`, `SystemExit` ohne Retry durch.

### A.3 `ensure_that` / `wait_for` (`core/ensure.py`, `core/wait.py`)

Beide sind die **Verifikations-Primitive** der Outcome-Vertrags-Schicht
(siehe §2.3).

```python
# core/wait.py — schlanker Polling-Helper, kein Caching, kein Raise
def wait_for(
    *predicates: Callable[[], bool],
    timeout: float | None = None,    # default Settings.wait_for_timeout (1.0)
    delay: float | None = None,      # default Settings.wait_for_delay (0.1)
    invalidate: Callable[[], None] | None = None,
) -> bool:
    ...

# core/ensure.py — kraftvoller Verifikations-Driver mit Retry, Stage-Memo, Hooks
def ensure_that(
    context: object,
    *predicates: Callable[[], bool],
    timeout: float | None = None,        # default Settings.ensure_timeout (15.0)
    raise_exception: bool | None = None, # default True
    failed_func: Callable[[], None] | None = None,  # i.d.R. context.invalidate
) -> bool:
    ...
```

**`ensure_that`-Verhalten (vereinfacht ggü. Altcode `core/ensure.py:32`):**

1. **Stage-Memo:** Predicates, die bereits einmal `True` lieferten,
   werden in dieser `ensure_that`-Invocation übersprungen. Sobald
   irgendein Predicate aktuell `False` liefert, wird das Memo zurück­
   gesetzt — alle Pre-Conditions müssen *gleichzeitig* gelten.
2. **Hook:** `failed_func()` (typisch `context.invalidate`) zwischen
   Retries; cached Adapter werden so verworfen, der nächste Versuch
   resolved frisch.
3. **Re-entrant** über Thread-Local-Stack: verschachtelte Aufrufe in
   z.B. UI-Klassen-Predicates erben den äußeren Timeout, statt einen
   eigenen zu starten. Das entspricht der Altcode-Logik
   (`core/ensure.py:60` mit `_EnsureLocal`), weil verschachtelte
   Predicates im Praxis-Code (z.B. `_application_is_ready` ruft
   intern wieder `ensure_that(...)`) sonst doppelt warten würden.
4. **Timeout** → wenn `raise_exception=True`: `CannotEnsureError`
   mit der Message des zuletzt fehlgeschlagenen Predicates und
   `repr(context)` als Format-Argument.
5. **Predicates** sind zero-arg-Callables (i.d.R. gebundene Methoden);
   sie tragen via Decorator ein `message`-Attribut für die
   Fehlermeldung:

   ```python
   from PlatynUI.core import predicate

   class Element(Control):
       @predicate("element {0} is enabled")
       def _element_is_enabled(self) -> bool:
            return self.adapter.get_pattern(patterns.Element).is_enabled
   ```

**Standard-Predicates** (in `ui/element.py` als Methoden, übernommen
aus Altcode `ui/element.py:111`):

| Predicate | Message | Quelle |
|---|---|---|
| `_adapter_exists` | „{0} exists" | `ContextBase` (Adapter resolved + valid) |
| `_parent_exists` | „parent for {0} exists" | `ContextBase` (rekursiv) |
| `_element_is_visible` | „{0} is visible" | `Element.try_ensure_visible()` |
| `_element_is_enabled` | „{0} is enabled" | `is_enabled` |
| `_element_is_not_readonly` | „{0} is not readonly" | `is_readonly` negiert |
| `_application_is_ready` | „application of {0} is ready" | `Element.try_ensure_application_is_ready()`; gibt `True` wenn Adapter ungültig (idempotent) |
| `_toplevel_parent_is_active` | „toplevel parent of {0} is active" | `try_ensure_toplevel_parent_is_active()` |
| `_element_is_in_view` | „{0} is in view" | `_element_is_visible` + `try_bring_into_view()` |

**Verwendung in einer UI-Klasse:**

```python
class Button(Control, role="Button"):
    def activate(self) -> None:
        self.ensure_that(                      # Pre-Conditions
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()  # Perform
        self.ensure_that(                      # Postcondition (best-effort)
            self._application_is_ready,
            raise_exception=False,
        )
```

`wait_for` ist die schlanke Variante für Implementierungs-Details
*innerhalb* einer Pattern-Implementierung — z.B. „warte bis sich der
Toggle-State geändert hat" in `CheckBox.set_state` (siehe §5.3-Beispiel).

### A.4 Adapter-Interface (`core/adapter.py`)

```python
class Adapter(ABC):
    """Abstraktion über einen UI-Knoten zur Pattern-Wrapping-Ebene.

    In der Praxis gibt es **eine** produktive Implementierung —
    `UiNodeAdapter` (§A.4a) — die einen `platynui_native.UiNode`
    umhüllt. Die ABC existiert dennoch, weil sie den Vertrag bildet,
    den `AdapterProxy` (§4 / §A.4) per Komposition delegieren muss,
    und weil Test-Code lokale Fakes davon ableiten kann (siehe
    `tests/PlatynUI/test_adapter.py`)."""
    pattern_name: ClassVar[str] = "org.platynui.core.Adapter"

    # Identität & Lebenszeit
    @property @abstractmethod
    def valid(self) -> bool: ...
    @property @abstractmethod
    def runtime_id(self) -> str: ...
    @property @abstractmethod
    def technology(self) -> "Technology": ...

    # Strukturelle Beziehungen
    @property @abstractmethod
    def parent(self) -> "Adapter | None": ...
    @property @abstractmethod
    def children(self) -> "Sequence[Adapter]": ...

    # Such-Kriterien (für WeightCalculator)
    @property @abstractmethod
    def name(self) -> str: ...
    @property @abstractmethod
    def class_name(self) -> str: ...
    @property
    def tag_name(self) -> str: return ""
    @property @abstractmethod
    def role(self) -> str: ...
    @property @abstractmethod
    def supported_roles(self) -> set[str]: ...
    @property @abstractmethod
    def framework_id(self) -> str: ...

    # Attribute (einheitlicher (namespace, name)-Schlüsselraum,
    # symmetrisch zu UiNode in Rust:
    #   crates/core/src/ui/node.rs (`attribute(namespace, name)`,
    #   `attributes()`),
    #   crates/core/src/ui/namespace.rs (control|item|app|native).
    # Default-Namespace ist "control".
    @abstractmethod
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        """Alle Attribut-Namen. Mit `namespace=None` werden Namen aus
        ALLEN Namespaces zurückgegeben — i.d.R. nur für Inspector/
        Debug-Zwecke. Mit explizitem `namespace="native"` (oder
        anderem) nur die des jeweiligen Namespaces."""
    @abstractmethod
    def attribute_value(self, name: str, namespace: str = "control") -> object: ...
    @abstractmethod
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        """Iterator über (namespace, name, value) — direkte Entsprechung
        zu `UiNode.attributes()` in Rust."""

    # Pattern-Discovery & -Aufruf (Kern!)
    @abstractmethod
    def supported_patterns(self) -> set[type[PatternBase]]: ...
    @abstractmethod
    def supported_pattern_names(self) -> set[str]: ...

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        """Convenience-Default: prüft `supported_patterns()`. Adapter
        können effizienter überschreiben (Rust: PyO3-Call zu
        `UiNode.has_pattern`)."""
        return pattern_type in self.supported_patterns()

    @overload
    def get_pattern(self, pattern_type: type[P]) -> P: ...
    @overload
    def get_pattern(self, pattern_type: type[P], *,
                    raise_exception: Literal[False]) -> P | None: ...
    @abstractmethod
    def get_pattern(self, pattern_type, *, raise_exception=True): ...

    def get_pattern_by_name(self, pattern_name: str, *,
                            raise_exception: bool = True) -> PatternBase | None:
        """Lookup über den Reverse-DNS-Identifier; nötig für externe
        Adapter ohne Python-Klassen-Import."""
        ...

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Adapter) and self.runtime_id == other.runtime_id
```

**Visuelle/State-Properties** (`bounding_rectangle`, `is_visible`,
`is_enabled`, `is_in_view`, `is_focused`, …) sind **keine** Adapter-
Methoden, sondern werden über das jeweilige Pattern abgerufen
(`Element.bounds`, `Element.is_visible`, `Element.is_enabled`,
`Focusable.is_focused`, …). `Element` (§7.1) bietet
`@cached_property`-Convenience-Wrapper, die intern
`adapter.get_pattern(patterns.Element).bounds` etc. aufrufen. So bleibt
das Adapter-Interface schmal und Pattern-orientiert.

**Pattern-Resolution-Reihenfolge** (siehe auch §4.3):
1. `self` ist `isinstance(pattern_type)` → `self`.
2. Adapter-internes Mapping (`_pattern_impls: dict[str, PatternBase]`) →
   gecached.
3. Adapter-spezifischer Lookup (`UiNodeAdapter`: PyO3-Call zu
   `UiNode.get_pattern`) → cachen.
4. Sonst: `PatternNotSupportedError` (oder `None` bei
   `raise_exception=False`).

**`AdapterProxy`** (siehe §4 / §5.1) ist *kein* Adapter-Subtyp, sondern
eine **Komposition**: er hält einen `adapter: Adapter` und überschreibt
nur `get_pattern` (eigene Patterns zuerst, dann `adapter.get_pattern`)
plus `supported_patterns` (Vereinigung). Alle anderen Adapter-Aufrufe
delegieren transparent.

### A.4a `UiNodeAdapter` (`core/adapters/ui_node.py`)

Die einzige produktive `Adapter`-Implementierung. Wickelt einen
`platynui_native.UiNode` so ein, dass das Python-`Adapter`-Interface
(§A.4) erfüllt ist.

Aufgabenkatalog:

- **API-Mapping**: `UiNode.attribute(name, namespace) -> UiValue` (wirft
  `AttributeNotFoundError`) → `Adapter.attribute_value(name, namespace)
  -> object` (wirft `KeyError` mit Schlüssel `"<ns>:<name>"`).
  `UiNode.attributes()` liefert `UiAttribute`; der Adapter exposed sie
  als `(namespace, name, value)`-Tupel. `attribute_names(namespace)`
  filtert in Python (kein dedizierter Native-Endpoint).
- **Defensive Default-Felder**: optionale Suchkriterien-Properties
  fangen `AttributeNotFoundError` ab und liefern den ABC-Default:
  - `class_name` ↔ `attribute('ClassName', 'control')`, default `""`
  - `framework_id` ↔ `attribute('FrameworkId', 'native')`, default `""`
  - `supported_roles` enthält bis auf Weiteres nur `{role}` — bis das
    Native-Attribut-Set `SupportedRoles` exposed (siehe §13.x), gibt es
    keinen anderen Lieferanten.
- **Pattern-Resolution-Hook (`_resolve_pattern`)**: schlägt den Reverse-
  DNS-Namen in einer modul-lokalen Builder-Tabelle nach
  (`_NATIVE_PATTERN_BUILDERS`). Treffer ruft `UiNode.get_pattern(name)`
  per **String** (nicht per Python-Klasse — die native Klasse besitzt
  kein `pattern_name`-ClassVar) und wrappt das Resultat in eine
  `core.patterns.*`-Subklasse. Beispiel: `platynui_native.Focusable` →
  `_NativeFocusable(Focusable)`, das `focus()` ans native Objekt
  delegiert und `is_focused` aus `UiNode.attribute('IsFocused',
  node.namespace.as_str())` zieht. Der Namespace folgt **dem Knoten**:
  Window/Button-Focus liegt in `control:IsFocused`, ListItem/TreeItem-
  Focus in `item:IsFocused` — der Wrapper liest entsprechend dynamisch.
- **`supports_pattern`-Override**: gibt nur dann `True` zurück, wenn
  (a) die native Seite das Pattern advertised UND (b) ein Python-
  Wrapper im `_NATIVE_PATTERN_BUILDERS`-Dict steht. Ohne (b) würde
  `get_pattern` später trotzdem in `PatternNotSupportedError` laufen
  und der Vertrag (`supports → get`) wäre gebrochen. Erweiterung um
  weitere Patterns = neuer Builder-Eintrag, sonst nichts.
- **Identity / Caching**: `runtime_id` aus `UiNode.runtime_id` (stabil
  über Tree-Reloads); `__eq__` / `__hash__` folgen der Adapter-ABC
  (§A.4 Z. 1617). `parent` / `children` werden bei jedem Aufruf neu
  aus `UiNode` geholt — der Rust-Cache regelt Wiederverwendung.
- **Technology**: `UiNodeTechnology` ist ein `Technology`-Subclass mit
  klassischem `__new__`-Singleton; das Modul hält eine fertig
  konstruierte Instanz (`_TECHNOLOGY`), so dass `.technology` keine
  Allokation auslöst.

Tests laufen gegen den **Rust-Mock-Provider** (`Runtime.new_with_mock()`)
und prüfen alle Mappings end-to-end. Reine Algorithmus-Tests für die
ABC selbst (Cache, Resolution-Steps) bleiben in `test_adapter.py` mit
Inline-Fakes.



`ContextBase` ist die Wurzel aller UI-Klassen (Page-Object-Basis).
Vereinfacht ggü. Altcode (473 → ~250 LOC), siehe §11.2. Erbt von
`Assertable` (`core/_assertable.py`, portiert aus aktuellem
`src/PlatynUI/_assertable.py`), das `assert_that`/`assert_that_not`
für RF-Style-Assertions liefert.

```python
class ContextBase(Assertable):
    default_role: ClassVar[str | None] = None
    default_prefix: ClassVar[str | None] = None

    def __init__(self,
                 locator: Locator,
                 context_parent: "ContextBase | None" = None,
                 adapter: Adapter | None = None) -> None: ...

    # --- Adapter & Lifetime ---
    @property
    def adapter(self) -> Adapter: ...           # forciert ensure, raises
    def get_adapter(self, *, timeout: float | None = None,
                    raise_exception: bool = True) -> Adapter | None: ...
    def invalidate(self) -> None: ...           # rekursiv über Children
    def exists(self, *, timeout: float | None = None,
               raise_exception: bool = False) -> bool: ...

    # --- Pre/Post-Verifikation ---
    def ensure_that(self, *predicates,
                    timeout: float | None = None,
                    raise_exception: bool | None = None) -> bool:
        return ensure_that(self, *predicates, timeout=timeout,
                           raise_exception=raise_exception,
                           failed_func=self.invalidate)

    # --- Element-Suche ---
    def get(self, ctx: type[T], *args,
            locator: Locator | None = None, **kw) -> T: ...
    def get_one(self, ctx: type[T], *args, **kw) -> T: ...     # raise wenn !=1
    def get_all(self, ctx: type[T], *args, **kw) -> list[T]: ...
    def iter_all(self, ctx: type[T], *args, **kw) -> Iterator[T]: ...
    def get_child(self, ctx: type[T], *args, **kw) -> T: ...   # scope=Children
    def get_children(self, ctx: type[T], *args, **kw) -> list[T]: ...
    def ancestor(self, ctx: type[T], *args, **kw) -> T: ...
    def ancestors(self, ctx: type[T], *args, **kw) -> list[T]: ...

    # --- Iteration ---
    def __iter__(self) -> Iterator["ContextBase"]: ...   # alle Children
    @property
    def children(self) -> list["ContextBase"]: ...
    @property
    def parent(self) -> "ContextBase | None": ...

    # --- Property-Durchreichung ---
    @cached_property
    def name(self) -> str: ...
    @cached_property
    def role(self) -> str: ...
    # … class_name, framework_id, runtime_id, supported_roles,
    # supported_patterns, is_valid

    # --- Context-Manager (No-op, Convenience) ---
    def __enter__(self) -> Self: return self
    def __exit__(self, *exc) -> None: pass
```

**Generische Attribut-Reads** (für Locator-`attributes=`-Match und
Inspector-Zwecke; delegieren 1:1 an den Adapter, siehe §A.4):

```python
def attribute_names(self, namespace: str | None = None) -> set[str]: ...
def attribute_value(self, name: str, namespace: str = "control") -> object: ...
def attributes(self) -> Iterator[tuple[str, str, object]]: ...
```

`ContextFactory` lebt im selben Modul, hält die Klassen-Registry für
`@context` und liefert via `find_context_class_for(adapter)` die beste
Subklasse (`WeightCalculator`-basiert, siehe §4).

**Element-Convenience-Properties.** `Element` (§7.1, Subklasse von
`ContextBase`) ergänzt Pattern-basierte Wrapper als `@cached_property`
bzw. `@property`, damit Page-Object-Code ohne expliziten
`get_pattern`-Aufruf auskommt:

```python
class Element(ContextBase):
    """Page-Object-Basisklasse. Nicht zu verwechseln mit dem
    gleichnamigen *Pattern* `patterns.Element` (§5), das hier durch
    diese Properties gewrappt wird."""

    @property
    def _element_pattern(self) -> patterns.Element:
        return self.adapter.get_pattern(patterns.Element)

    @property
    def bounding_rectangle(self) -> Rect:
        return self._element_pattern.bounds

    @property
    def is_visible(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_visible if e is not None else True

    @property
    def is_in_view(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_in_view if e is not None else self.is_visible

    @property
    def is_enabled(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_enabled if e is not None else True

    @property
    def is_focused(self) -> bool:
        f = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        return f.is_focused if f is not None else False
```

Diese Properties sind die Quelle für Default-Predicates
(`is_visible`, `is_enabled`, …; siehe §A.3) und für Devices
(`MouseProxy.click(element)` liest `bounding_rectangle`).

### A.5 Process-wide Runtime (`core/runtime.py`)

Die PlatynUI-Bibliothek bündelt einen einzigen
`platynui_native.Runtime` pro Prozess. Provider-Cache,
Pointer-/Keyboard-Profile und der Desktop-Tree gehören diesem
Runtime-Objekt; Adapter, Device-Proxies, Robot-Keywords, der
BareMetal-Helper und der Inspector arbeiten alle gegen dieselbe
Instanz. Den Runtime explizit durch jeden Konstruktor zu reichen
würde dem Robot-Framework-Idiom (Keywords ohne Per-Call-Context)
widersprechen und die User-API unnötig aufblähen.

**Designprinzipien.** Die API trennt drei Verantwortlichkeiten
strikt:

1. *Variantenwahl* — vor der ersten Benutzung darf der Aufrufer
   wählen, *welche* Runtime gebaut werden soll (Default,
   Mock-Provider oder eigene Factory). Sobald ein Konsument
   `runtime.current` liest, wird die Wahl eingefroren („sealed").
2. *Konsum* — Konsumenten lesen ausschließlich `runtime.current`
   und bekommen immer dieselbe, einmal gebaute Instanz.
3. *Test-Override* — Tests installieren eine alternative Runtime
   stets über den Context-Manager `runtime.override(...)`, der
   den vorherigen Zustand garantiert wiederherstellt.

Es gibt **keinen** nackten Setter, der eine externe Native-Runtime
in das Singleton injiziert. Alle Wege führen entweder über eine
Variantenwahl (`use_*`) oder über einen scope-gebundenen Override.
Damit ist „vergessenes Reset" konstruktiv ausgeschlossen, und
versehentliches Tauschen während laufender Operationen wird durch
das Sealing verhindert.

**API.** `core/runtime.py` exportiert ein Singleton-Objekt
`runtime` (Klasse `Runtime`):

```python
from PlatynUI.core import runtime

# --- Konsum (für alle Aufrufer: Adapter, Devices, Keywords, …) ---
runtime.current             # property: aktive platynui_native.Runtime
                            # — sealed beim ersten Zugriff
runtime.is_initialised()    # bool: wurde current je gebaut/gewählt
runtime.is_sealed()         # bool: wurde current bereits ausgelesen

# --- Variantenwahl (nur vor Sealing erlaubt) ---
runtime.use_default()       # Auto-Discovery via inventory (=Werkseinstellung)
runtime.use_mock()           # Runtime.new_with_mock() (mock-provider Feature)
runtime.use_factory(cb)     # cb: Callable[[], _NativeRuntime] — Custom-Builder

# --- Test-Override (jederzeit erlaubt, scope-gebunden) ---
with runtime.override(factory) as rt:    # factory: Callable[[], _NativeRuntime]
    ...                     # innerhalb: rt ist aktiv
                            # exit: shutdown(rt), Vorzustand restored
with runtime.override_with_mock() as rt:
    ...                     # Convenience für Tests
```

**Variantenwahl & Sealing.**

- `use_*()`-Methoden setzen einen *Builder* (Closure), nicht die
  Instanz selbst. Mehrfach-Aufruf vor Sealing ersetzt den Builder
  still — der zuletzt gewählte gewinnt.
- Beim ersten `current`-Zugriff wird der Builder ausgeführt, das
  Ergebnis gecacht und der Accessor *sealed*.
- `use_*()` nach Sealing wirft `RuntimeError("runtime already
  initialised; use override() instead")`. Wer im laufenden Prozess
  scopiert tauschen will, nutzt `override(...)`.
- Wird vor Sealing kein `use_*()` aufgerufen, gilt `use_default()`
  implizit.

**Override-Semantik.**

- `override(factory)` akzeptiert ein zero-arg Callable
  `Callable[[], _NativeRuntime]`. Das Callable wird im Enter genau
  einmal ausgeführt; die produzierte Instanz wird aktiv.  Wer eine
  bereits gebaute Instanz wiederverwenden möchte, wrappt sie:
  `runtime.override(lambda: existing_rt)`.  Diese explizite Callable-
  Konvention vermeidet Ambiguität (Mock-Objekte sind callable —
  `callable()`-Heuristiken wären fragil).
- `override_with_mock()` ist Zucker für
  `override(NativeRuntime.new_with_mock)`.
- Im Enter wird der aktuelle Zustand (Builder *und* Instanz) auf
  einem Stack gesichert; danach ist die Override-Instanz aktiv und
  bereits sealed.
- Im Exit wird `shutdown()` der Override-Instanz aufgerufen
  (Exceptions werden geschluckt — best-effort) und der Vorzustand
  restauriert. Verschachtelte `override(...)`-Blöcke sind erlaubt
  (LIFO-Stack).

**Thread-Safety.** Der Accessor wird durch ein `RLock` geschützt.
Die zugrundeliegende `platynui_native.Runtime` hält intern ein
Rust-`Mutex` und ist sicher zwischen Python-Threads teilbar.
Pointer- und Keyboard-Methoden adressieren absolute
Bildschirmkoordinaten bzw. die OS-Event-Queue und sind damit
node-unabhängig: ein Override mitten in einer Session invalidiert
keine zuvor geholten `UiNode`-Instanzen — diese referenzieren
weiterhin ihren ursprünglichen Provider-Tree.

**Robot-Framework-Integration.** Die spätere
`PlatynUI`-Robot-Library nimmt die Variantenwahl als
Konstruktor-Argument entgegen:

```robotframework
*** Settings ***
Library    PlatynUI    use_mock=${True}
```

```python
class PlatynUI:
    def __init__(self, use_mock: bool = False) -> None:
        if use_mock:
            runtime.use_mock()
        # sonst: Default-Lazy reicht
```

**Tests.** Mock-Tests verwenden ausschließlich den Override-
Context-Manager — kein manuelles Setzen, kein vergessenes
Reset:

```python
@pytest.fixture
def native_runtime():
    from PlatynUI.core import runtime
    with runtime.override_with_mock() as rt:
        yield rt
```

`override(...)` schluckt Exceptions aus `shutdown()` (best-effort
teardown) — Test-Cleanup soll niemals den eigentlichen Test
verschleiern.

### A.6 `@locator`-Mechanik (`core/locator.py`)

`Locator` ist eine `@dataclass`-ähnliche Builder-Klasse, die intern
einen XPath-2.0-Ausdruck für die Rust-XPath-Engine baut. Die
Implementierung ist eine handgeschriebene Klasse mit `__slots__`
(kein `@dataclass`-Decorator), weil der Konstruktor zusätzlich zu den
typisierten Feldern beliebige `**kwargs` als freie Attribut-Predicates
entgegennimmt (siehe §7.1).

**Felder & Kwargs** (vereinfacht aus Altcode `ui/locator.py:21`):

```python
@dataclass(slots=True, kw_only=True)
class Locator:
    # XPath-Bestandteile
    path: str | None = None              # wörtlicher XPath, override
    node: str | None = None              # Knotenname (Rolle)
    prefix: str | None = None            # Namespace-Prefix
    use_default_prefix: bool = False
    axis: str | None = None              # raw axis-Prefix
    scope: LocatorScope | None = None
    index: int | None = None
    position: int | None = None

    # Standard-Attribute (alle in @-Notation)
    name: str | None = None              # → @Name=
    id: str | None = None                # → @Id=
    class_name: str | None = None        # → @ClassName=
    role: str | None = None              # auch Knotenname-Default
    runtime_id: str | None = None        # → @RuntimeId=
    framework_id: str | None = None      # → @FrameworkId=

    # Freie Attribute (PascalCase erwartet, siehe §7.1).
    # Schlüssel: bare String → (default_namespace, name);
    #            Tupel (namespace, name) → expliziter Namespace.
    # Default-Namespace = "control"; eine Page-Object-Klasse kann das
    # über das Klassenattribut `default_attribute_namespace`
    # umstellen (z.B. `default_attribute_namespace = "item"`). Die
    # Auflösung passiert beim Build des XPath, nicht beim Setzen der
    # Dict-Einträge — das hält die Locator-Konstruktion deklarativ.
    attributes: dict[str | tuple[str, str], str | re.Pattern[str]] = field(default_factory=dict)
    custom_attributes: list[str] = field(default_factory=list)  # raw Prädikate

    # Plus beliebige `**extra_attributes: str | re.Pattern[str]` am __init__:
    # Locator(AutomationId="x")        → attributes["AutomationId"] = "x"
    # Locator(native__HWND=0xABCD)     → attributes[("native","HWND")] = 0xABCD
    # Konflikte mit den typisierten Feldern oder dem attributes-Dict
    # (nach Namespace-Normalisierung) werfen TypeError.
```

**`LocatorScope`** als String-Enum-Alias (kein `StrEnum`-Zwang):

```python
LocatorScope: TypeAlias = Literal[
    "root", "descendants", "children", "parent", "ancestor",
    "ancestor-or-self", "descendants-or-self",
    "following", "following-sibling", "preceding", "preceding-sibling",
]
```

**XPath-Bau** (`Locator.to_xpath()`):

1. `path` gesetzt → wörtlich übernommen, alle anderen Felder ignoriert
   außer `prefix`/`axis`-Präfix.
2. Sonst: Knotenname = `node` ?? `role` ?? `default_role` ?? `*`.
3. Achse aus `axis` ?? `LocatorScope`-Mapping (`children` → leer,
   `descendants` → `.//`, `ancestor` → `ancestor::`, …).
4. Prädikate aus drei Quellen (siehe §7.1 für die User-Konvention):
   - **Convenience-Felder** (`name`, `id`, `class_name`, `runtime_id`,
     `framework_id`) werden über `_standard_attributes()` zu
     `@Name='v'`, `@Id='v'`, `@ClassName='v'`, `@RuntimeId='v'`,
     `@FrameworkId='v'` (immer im `control`-Namespace, daher
     unprefixed).
   - **Freie Kwargs** am Konstruktor werden bereits in `__init__`
     in das `attributes`-Dict gemergt: ein Kwarg ohne `__`-Trenner
     landet als Bare-String-Key (`AutomationId="x"` →
     `attributes["AutomationId"] = "x"`); ein Kwarg mit
     `<ns>__<name>`-Trenner landet als Tupel-Key (`native__HWND=...`
     → `attributes[("native", "HWND")] = ...`). Damit fließen sie
     durch denselben Pfad wie der Dict-Weg.
   - **`attributes`-Dict**:
     - Bare-String-Schlüssel werden auf `(default_namespace, name)`
       normalisiert; `default_namespace` ist `"control"`, sofern die
       verwendende Page-Object-Klasse nicht
       `default_attribute_namespace = "<ns>"` setzt.
     - `(namespace, name)` → `@<ns>:<name>='v'` (bzw. nur `@<name>` wenn
       der Namespace dem XPath-Default `control` entspricht), oder
       `matches(@<ns>:<name>, 'v')` für Regex.
   - **`custom_attributes`** werden als rohe Prädikat-Strings übernommen.
   - Alle Teilbedingungen mit ` and ` verbunden.
   - **Konflikt-Regel** (im Konstruktor durchgesetzt, nicht erst hier):
     Sammelt eine `(namespace, name)`-Map über alle drei Quellen.
     Doppelte Schlüssel (auch nach Normalisierung — z.B.
     Convenience-Feld `name=` vs. Kwarg `Name=` vs.
     `attributes={('control','Name'): ...}`) werfen `TypeError` mit
     genauer Fehlermeldung.
5. Suffix `[N]` aus `index`, `[position()=N]` aus `position`.
6. Default-Scope-Regel: ohne Parent → `children`; mit Parent →
   `children` falls Parent ein `Application`/`Desktop`, sonst
   `descendants`. (1:1 aus Altcode.)

**`@locator`-Decorator** ist eine eigene Funktion (nicht die
`Locator`-Klasse selbst — siehe Rev. 18). Sie nimmt dieselben Kwargs wie
`Locator.__init__` entgegen und liefert je nach Decorator-Target zwei
Verhalten:

| Target | Verhalten | Status |
|---|---|---|
| Klasse | hängt einen `Locator` als `__locator__`-Klassenattribut an, gibt die Klasse unverändert zurück | **Phase 1 / Rev. 18 — DONE** |
| Methode/Property | wickelt die Funktion in einen `LocatorMethodDescriptor` ein, der den Locator + die Wrapped-Function speichert; beim Instanz-Zugriff wird `ContextBase.get(annotation, locator=…)` aufgerufen | **Phase 3 — STUB** (wirft derzeit `NotImplementedError`) |

Die Method-Form ist als Stub bereits API-stabil; Page-Object-Code kann
beide Formen heute schreiben — die Resolution wird in Phase 3 transparent
nachgereicht, ohne Quelltext-Änderungen am Page-Object.

Verwendung:

```python
# Klassen-Default (am Page-Object) — funktioniert heute
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    pass

# Property-Variante (typisierter Child-Locator) — Stub bis Phase 3
class CalculatorWindow(Window, role="Window", name="Rechner"):
    @property
    @locator(AutomationId="num5Button")
    def n5(self) -> Button: ...

# Default-Namespace umstellen (z.B. für Item-Container)
class FileListItem(Item):
    default_attribute_namespace = "item"
    # bare-String-Keys im attributes-Dict landen jetzt im
    # `item:`-Namespace; expliziter Namespace bleibt jederzeit
    # via Tupel-Key möglich.

# Cross-Namespace via Tupel
@locator(attributes={
    "AutomationId": "submit",          # → @AutomationId=… (control)
    ("native", "HWND"): 0x12AB,         # → @native:HWND=…
})
class SubmitButton(Button): ...
```

**Property-Vererbung:** `Locator.copy_from(parent)` übernimmt aus dem
Parent-Locator alle Felder, die in `self` `None` sind, und mergt
`attributes`/`custom_attributes`. So vererben `@locator`-Defaults von
Klasse zu Instanz und von Parent-Page-Object zu Child.

**Match-Verhalten** (über die Rust-XPath-Engine):

- `Locator` allein liefert keinen `UiNode` — er ist eine *Beschreibung*.
  Aufgelöst wird er via `runtime.evaluate(xpath, root_node)` aus dem
  Rust-Backend.
- `get(...)` → erstes Match (oder `ElementNotFoundError`).
- `get_one(...)` → genau ein Match (oder `ElementNotFoundError` /
  `MultipleElementsFoundError`).
- `get_all(...)` → alle Matches.
- Kein-Match: retry über `ensure_that` (Pre-Condition
  `_adapter_exists`) bis Timeout, dann `ElementNotFoundError`.

### A.7 `ElementDescriptor[PatternT]` (`core/descriptor.py`)

Im Altprojekt `keywords/types.py:20` bereits vorhanden, im neuen
Projekt im `BareMetal`-Modul als `UiNodeDescriptor` (XPath-basiert).
Beide werden konsolidiert: `ElementDescriptor` lebt in
`core/descriptor.py` und wird **von `BareMetal` und `PlatynUI`
gemeinsam genutzt**.

```python
class ElementDescriptor(Generic[P]):
    """Lazy Reference auf ein UI-Element. Wird von Robot-Argument-
    Konvertern als Eingangstyp für Keywords verwendet."""

    def __init__(self,
                 locator: Locator | None = None,
                 context_type: type[ContextBase] | None = None,
                 parent: "ElementDescriptor | None" = None,
                 context: ContextBase | None = None) -> None: ...

    def __call__(self, *, full_context: bool = True) -> ContextBase:
        """Resolved den Context. Bei full_context=True wird die konkrete
        Subklasse via ContextFactory ausgewählt; bei False bleibt es
        beim generischen ContextBase (für reine Property-Reads)."""
        ...

    # Robot-Konverter (registriert in PlatynUI/__init__.py)
    @staticmethod
    def convert(value: str | ContextBase) -> "ElementDescriptor":
        if isinstance(value, ContextBase):
            return ElementDescriptor(context=value)
        return ElementDescriptor(
            Locator(path=value),
            parent=ElementDescriptor.get_root_element(),
        )

    @staticmethod
    def set_root_element(element: "ElementDescriptor | None") -> None: ...
    @staticmethod
    def get_root_element() -> "ElementDescriptor | None": ...
```

**Pattern-getypte Variante** für Keyword-Argumente:

```python
# keywords/activate.py
@keyword
def activate(element: ElementDescriptor[patterns.Activatable]) -> None:
    ctx = element()                         # resolve
    if not ctx.adapter.supports_pattern(patterns.Activatable):
        raise PatternNotSupportedError(
            f"{ctx} does not support Activatable")
    ctx.activate()                          # UI-Klasse hat die Methode
```

Die Robot-Library registriert für jede oft genutzte Pattern-ABC einen
eigenen Konverter (`ElementDescriptor[patterns.Activatable].convert`,
…), damit die Robot-IDE-Doku die richtigen Typen anzeigt und das
Pattern-Check beim Argument-Parsing greift, nicht erst beim Call.

**Root-Element:** Wird über die Robot-Variable
`${PLATYNUI_ROOT_ELEMENT}` gesteuert (gespeichert per
`EXECUTION_CONTEXTS.current.variables`). Default ist `None` ⇒
`ElementDescriptor.convert(string)` setzt einen Locator mit
`parent=None`, was via `Locator.scope`-Default in einen Desktop-relativen
XPath aufgelöst wird (`/.//control:Foo`).

### A.8 Lifecycle & Robot-Library-Init

```python
# src/PlatynUI/__init__.py
@library(scope="GLOBAL", version=__version__,
         converters={
             ElementDescriptor: ElementDescriptor.convert,
             RootElementDescriptor: RootElementDescriptor.convert,
             # Pattern-getypte Konverter (auto-generiert aus
             # patterns.__all__):
             patterns.Activatable: ElementDescriptor[patterns.Activatable].convert,
             patterns.Toggleable: ElementDescriptor[patterns.Toggleable].convert,
             patterns.TextContent: ElementDescriptor[patterns.TextContent].convert,
             patterns.TextEditable: ElementDescriptor[patterns.TextEditable].convert,
             patterns.Clearable: ElementDescriptor[patterns.Clearable].convert,
             patterns.Element: ElementDescriptor[patterns.Element].convert,
             patterns.Focusable: ElementDescriptor[patterns.Focusable].convert,
             # …
             Locator: convert_locator,
         })
class PlatynUI(DynamicCore):
    def __init__(self) -> None:
        # Side-effect: importiert ui/* und keywords/*; das löst die
        # @context- und @pattern_proxy_for-Registrierungen aus.
        from . import ui            # noqa: F401  (registers UI classes)
        from .core.adapters import rust as _rust   # default adapter
        super().__init__([
            ApplicationKeywords(),
            ActivateKeywords(),
            ToggleKeywords(),
            SelectKeywords(),
            ExpandKeywords(),
            TextKeywords(),
            ScrollKeywords(),
            WindowKeywords(),
            PropertyKeywords(),
            WaitKeywords(),
            DiagnosticsKeywords(),    # Highlight, Screenshot
        ])

    @keyword
    def set_root_element(self, element: RootElementDescriptor) -> None:
        ElementDescriptor.set_root_element(element)
```

**Bootstrap-Reihenfolge bei Suite-Start:**

1. Robot lädt `Library  PlatynUI` → `PlatynUI.__init__` wird gerufen.
2. `from . import ui` triggert alle `@context`/`@pattern_proxy_for`-
   Registrierungen (Side-Effect-Imports in `ui/__init__.py` und
   `ui/proxies/__init__.py`).
3. `core.adapters.ui_node`-Import lädt die `UiNodeAdapter`-
   Implementierung. Sie ist die einzige produktive Adapter-Klasse;
   andere Adapter sind nicht vorgesehen.
4. Die Robot-Library ist nutzungsbereit. Der **Runtime-Singleton**
   `PlatynUI.core.runtime.runtime` (Rev. 20, §A.5) liefert die
   prozessweite `platynui_native.Runtime`; `UiNodeAdapter` greift dort
   lazy zu und hält keine eigene Runtime-Referenz.

**Desktop-Root** ist konzeptionell das Wurzelelement des UI-Trees.
Praktisch gibt es ihn als Klasse:

```python
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    default_prefix = "control"
    # Stellt MouseProxy mit base_point=(0,0) und base_rect=Bildschirm
    # bereit, damit absolute Mouse-Operationen ohne Element möglich sind.
```

**Adapter-Bootstrap.** Da es nur eine produktive Adapter-Klasse gibt,
entfällt jede Multi-Backend-Registry. Der `Desktop`-Konstruktor ruft
schlicht `UiNodeAdapter.create_root()` auf, sofern kein expliziter
`adapter=`-Parameter übergeben wurde. Tests (in Python wie Rust)
geben einen über `Runtime.new_with_mock()` gebauten Adapter ein.

**Application Start.** Im Altprojekt unimplementiert
(`keywords/application.py` raised `NotImplementedError`). Im neuen
Projekt:

```python
@keyword
def start_application(command: str | list[str], *,
                      timeout: float | None = None,
                      window_locator: ElementDescriptor | None = None,
                      ) -> ElementDescriptor:
    """Startet einen Prozess und wartet auf das Top-Level-Window.

    - command: Programmpfad oder Argv-Liste (subprocess.Popen-konform)
    - window_locator: optionaler Locator, der das erscheinende Hauptfenster
      identifiziert (z.B. nach Title oder ProcessId). Default: warten auf
      ein Window mit der vom Prozess geerbten ProcessId.
    Liefert einen ElementDescriptor auf das Hauptfenster zurück.
    """
    ...
```

`exit_application` und `close_application` bleiben getrennt:
`exit_application` ruft das `Application`-Pattern (sauberes Quit per
API), `close_application` schickt ein Window-Close (X-Knopf-Äquivalent).

### A.9 Devices: Mouse / Keyboard (`core/devices.py`)

Die Python-Devices sind **dünne Element-Wrapper** über die im Rust-
Runtime bereits vollständig implementierte Pointer/Keyboard-Pipeline.
Die komplette Low-Level-Logik (Profile, `PointerOverrides`, Multi-
Click-Multiplikator, Press/Release-Delays, Move-Interpolation, Sequenz-
Parser für `<Ctrl+A>`, Modifier-Stacking) lebt in `crates/runtime` und
ist via `runtime.current.pointer_*` / `runtime.current.keyboard_*` von
Python aus direkt erreichbar.

Die Python-Schicht trägt nur noch drei Verantwortlichkeiten:

1. **Element-bezogenes Coord-Resolving**: aus einer `Point | VirtualPoint
   | None`-Override plus optionalen `x`/`y`-Offsets eine absolute
   Desktop-Koordinate berechnen, die an Rust übergeben wird.
2. **Default-Click-Position**: über die Adapter-Pattern-Kette
   (`ActivationTarget` → `Element`) den Standard-Klickpunkt eines
   Elements bestimmen.
3. **Pre/Post-Hooks**: `before_action`/`after_action` als
   Erweiterungspunkt für Element-Integration in Phase 3 (`ensure_that(
   toplevel_active, in_view)`); in `core/devices.py` selbst no-op.

Es gibt **kein** `MouseDevice`/`KeyboardDevice`-Zwischenlayer mehr —
der Rust-Runtime *ist* das Device.

#### A.9.1 Aktions-Enums und Typ-Aliase

```python
class MouseAction(StrEnum):
    MOVE = "move"
    PRESS = "press"
    RELEASE = "release"
    CLICK = "click"
    DOUBLE_CLICK = "double_click"

class KeyboardAction(StrEnum):
    TYPE = "type"
    PRESS = "press"
    RELEASE = "release"

# core/types.py
MouseButton: TypeAlias = PointerButton  # 1:1 aus platynui_native
```

`PointerButton` (LEFT=1, MIDDLE=2, RIGHT=3) wird unverändert aus dem
Rust-Modul re-exportiert. `MouseButton` ist der historische Python-Name
für dieselbe Enum.

#### A.9.2 VirtualPoint und Anchor

`VirtualPoint` ist eine reine Funktion `Rect → Point`, die einen
Anker-Punkt innerhalb eines Bounding-Rects beschreibt.

```python
@dataclass(frozen=True)
class VirtualPoint:
    name: str
    func: Callable[[Rect], Point]

    def resolve(self, rect: Rect) -> Point:
        return self.func(rect)


class Anchor:
    """Vordefinierte VirtualPoints für Element-Bounds."""
    TOP_LEFT     = VirtualPoint("top_left",     lambda r: r.position())
    TOP          = VirtualPoint("top",          lambda r: Point(r.x + r.width / 2, r.y))
    TOP_RIGHT    = VirtualPoint("top_right",    lambda r: Point(r.right(), r.y))
    LEFT         = VirtualPoint("left",         lambda r: Point(r.x, r.y + r.height / 2))
    CENTER       = VirtualPoint("center",       lambda r: r.center())
    RIGHT        = VirtualPoint("right",        lambda r: Point(r.right(), r.y + r.height / 2))
    BOTTOM_LEFT  = VirtualPoint("bottom_left",  lambda r: Point(r.x, r.bottom()))
    BOTTOM       = VirtualPoint("bottom",       lambda r: Point(r.x + r.width / 2, r.bottom()))
    BOTTOM_RIGHT = VirtualPoint("bottom_right", lambda r: Point(r.right(), r.bottom()))
```

Im Gegensatz zum Altcode (`tmp/.../core/types.py:76`) gibt es **keinen
None-Achsenwert-Trick** mehr — der neue `Point` ist immer vollständig.
Y-only-Anker (Altcode `Anchor.TOP` mit `x=None`) sind durch konkrete
Halbierung der Width abgelöst.

#### A.9.3 MouseProxy

```python
class MouseProxy(ABC):
    """Element-relativer Maus-Wrapper.

    Berechnet absolute Desktop-Koordinaten aus base_rect plus
    Override (Point | VirtualPoint | None) plus optionalen x/y-Offsets
    und delegiert die eigentliche Pointer-Aktion an den Rust-Runtime.
    """

    @property
    @abstractmethod
    def base_rect(self) -> Rect:
        """Element-Bounding-Box im Desktop-Koordinatensystem."""

    @property
    def default_click_position(self) -> Point:
        """Default-Klickpunkt (absolut). Wird verwendet wenn pos=None."""
        return self.base_rect.center()

    def before_action(self, action: MouseAction) -> None: ...
    def after_action(self, action: MouseAction) -> None: ...

    def _resolve_point(
        self,
        pos: Point | VirtualPoint | None,
        x: float | None,
        y: float | None,
    ) -> Point:
        if pos is None:
            base = self.default_click_position
        elif isinstance(pos, VirtualPoint):
            base = pos.resolve(self.base_rect)
        else:  # Point — relativ zu Element-TopLeft
            base = self.base_rect.position() + pos
        return base.translate(x or 0.0, y or 0.0)

    def move_to(
        self,
        pos: Point | VirtualPoint | None = None,
        *,
        x: float | None = None,
        y: float | None = None,
    ) -> Point:
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.MOVE)
        runtime.current.pointer_move(target.x, target.y, origin="desktop")
        self.after_action(MouseAction.MOVE)
        return target

    def click(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        times: int = 1,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.CLICK)
        runtime.current.pointer_multi_click(
            target.x, target.y, button=button, clicks=times, origin="desktop",
        )
        self.after_action(MouseAction.CLICK)

    # press / release / double_click analog
```

#### A.9.4 AdapterMouseProxy mit ActivationTarget-Fallback-Kette

```python
class AdapterMouseProxy(MouseProxy):
    """Standard-Implementierung: liest Bounds und Default-Klickpunkt
    über die Adapter-Pattern-Kette."""

    def __init__(self, adapter: AdapterFacade) -> None:
        self._adapter = adapter
        self._logger = logging.getLogger("platynui.devices")

    @property
    def base_rect(self) -> Rect:
        return self._adapter.get_pattern(patterns.Element).bounds

    @property
    def default_click_position(self) -> Point:
        # Fallback-Kette gemäß patterns.md / Spec §A.9:
        # ActivationArea-Center → ActivationPoint → Element.default_click_position
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            target = self._adapter.get_pattern(patterns.ActivationTarget)
            if target.activation_area is not None:
                return target.activation_area.center()
            return target.activation_point
        return self._adapter.get_pattern(patterns.Element).default_click_position

    def before_action(self, action: MouseAction) -> None:
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            hint = self._adapter.get_pattern(patterns.ActivationTarget).activation_hint
            if hint:
                self._logger.debug("mouse %s: %s", action.value, hint)
```

#### A.9.5 KeyboardProxy

Der Rust-Runtime parst `<Ctrl+A>`, `<Shift+.>`, Unicode-Escapes etc.
über `KeyboardSequence::parse` (pest-basiert, siehe
`crates/runtime/src/keyboard_sequence.rs`). Die Python-API ist deshalb
**kein Variadic** mehr — sie nimmt eine einzige Sequenz-String und
reicht sie unverändert an Rust weiter.

```python
class KeyboardProxy(ABC):
    """Tastatur-Wrapper. Sequenz-Format wird komplett von Rust geparst."""

    def before_action(self, action: KeyboardAction) -> None: ...
    def after_action(self, action: KeyboardAction) -> None: ...

    def type_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.TYPE)
        runtime.current.keyboard_type(sequence)
        self.after_action(KeyboardAction.TYPE)

    def press_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.PRESS)
        runtime.current.keyboard_press(sequence)
        self.after_action(KeyboardAction.PRESS)

    def release_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.RELEASE)
        runtime.current.keyboard_release(sequence)
        self.after_action(KeyboardAction.RELEASE)


class AdapterKeyboardProxy(KeyboardProxy):
    """Tastatur-Wrapper für ein Element. Aktuell ohne adapter-spezifische
    Logik — Hook für Phase 3 (Element-Fokus, Verifikation)."""

    def __init__(self, adapter: AdapterFacade) -> None:
        self._adapter = adapter
```

#### A.9.6 Element-Integration (Phase 3, Vorausschau)

In `ui/element.py` wird `Element` `mouse` und `keyboard` als property-
cached `AdapterMouseProxy`/`AdapterKeyboardProxy` exposen, deren
`before_action` dann ein echtes `ensure_that(toplevel_active, in_view)`
ausführt. In `core/devices.py` selbst sind die Hooks no-op — die
Element-Schicht hängt sich später per Subclass oder Override ein.

#### A.9.7 Logging

`AdapterMouseProxy` nutzt den Standard-Python-Logger
`platynui.devices`. Wenn ein Element ein `ActivationTarget`-Pattern
mit nicht-leerem `activation_hint` bietet, wird dieser auf DEBUG-Level
vor jeder Maus-Aktion geloggt. Default-Level ist WARNING — User müssen
explizit `logging.getLogger("platynui.devices").setLevel(logging.DEBUG)`
setzen, um die Hints zu sehen.

### A.10 Pattern-Default-Implementierungen (`core/patterns/defaults.py`)

Generische Pattern-Implementierungen, die als Fallback greifen, wenn
weder ein `@pattern_proxy_for`-Proxy noch der Adapter das Pattern direkt
liefern (Drei-Ebenen-Fallback, siehe §5a.3).

**`Activatable` (Default):** Click auf `default_click_position`.

```python
class DefaultActivatable(patterns.Activatable):
    pattern_name = patterns.Activatable.pattern_name

    def __init__(self, adapter: Adapter) -> None:
        self._adapter = adapter

    def activate(self) -> None:
        AdapterMouseProxy(self._adapter).click()
```

**`TextEditable` (Default):** Fokus + Clear-Sequenz (Ctrl+A, Del) +
`type_keys`. Properties (`is_readonly`, `max_length`,
`supports_password_mode`) liefern konservative Defaults
(`is_readonly=False`, `max_length=None`, `supports_password_mode=False`),
solange der Adapter sie nicht überschreibt.

```python
class DefaultTextEditable(patterns.TextEditable):
    pattern_name = patterns.TextEditable.pattern_name
    def set_text(self, value: str) -> None:
        self._adapter.get_pattern(patterns.Focusable).focus()
        kb = AdapterKeyboardProxy(self._adapter)
        kb.type_keys("<Control+A><Delete>")
        kb.type_keys(value)
    @property
    def is_readonly(self) -> bool: return False
    @property
    def max_length(self) -> int | None: return None
    @property
    def supports_password_mode(self) -> bool: return False
```

**`Clearable` (Default):** Fokus + Ctrl+A + Del.

**`Toggleable` (Default):** kein generischer Default — dieses Pattern
*muss* vom Adapter oder Proxy kommen, weil ohne State-Read keine
Verifikation möglich ist (`state` und `supports_three_state` lassen sich
nicht generisch bestimmen).

`core/patterns/defaults.py` enthält diese Defaults und bietet sie
**nur auf explizite Anforderung** an: Der `AdapterProxy.get_pattern`-
Lookup (siehe §A.4) erweitert sich um Stufe 4: „falls keine spezifische
Implementierung gefunden, prüfe, ob ein
`DEFAULT_PATTERN_FACTORIES[pattern_name]` existiert und instanziiere
ihn lazy". Das Mapping ist eine reine Modul-Konstante, keine globale
Registry mit Side-Effects.

### A.11 Mock-Adapter — *gestrichen*

Ursprünglich war hier ein Python-`MockAdapter` über einen Python-
`MockNode`-Tree vorgesehen. Diese Idee wurde verworfen: Tests laufen
stattdessen gegen den **Rust-Mock-Provider** (`provider-mock` mit
`Runtime.new_with_mock()`). Begründung:

- `UiNode` (aus `platynui_native`) ist die einzige *Technology* der
  Bibliothek. Eine zweite Technology auf Python-Ebene aufzubauen, nur
  um sie zu testen, doppelt die Adapter-Mechanik ohne neuen Nutzen.
- Der Rust-Mock-Provider liefert vollwertige `UiNode`-Bäume mit
  Pattern-Implementierungen — exakt das, was Tests brauchen.
- Lokale Test-Fakes für eng begrenzte Adapter-Algorithmus-Tests
  (z.B. `test_adapter.py`) bleiben als kleine Inline-Helper bestehen
  und werden nicht zu einem öffentlichen API gehoben.

Stub-Patterns für Spy-Verhalten (z.B. *„hat `activate()` genau einmal
diesen Stub aufgerufen?"*) werden über die **`AdapterProxy`-Schicht**
realisiert: ein Proxy mixt das Pattern als Spy ein und überschreibt
damit das vom Native-Node gelieferte Pattern (siehe §A.4 / §4.2).

### A.12 Highlight & Diagnose (`core/devices.py` + `keywords/diagnostics.py`)

Highlight und Screenshots gehören nicht zum Element-Vertrag, sondern
zur **Diagnose-Schicht**. Sie erweitern `Element` um zwei Methoden,
die direkt auf das `DisplayDevice` der Adapter-Technology zugreifen.

```python
class DisplayDevice(ABC):
    @abstractmethod
    def highlight_rect(self, rect: Rect, *, time: float) -> None: ...
    @abstractmethod
    def get_screenshot(self, rect: Rect, *,
                       format: str = "png",
                       quality: int = -1) -> bytes: ...

# Element-Mixin
class Element(Control):
    def highlight(self, rect: Rect | None = None,
                  time: float | None = None) -> None:
        time = time or Settings.current().element_highlight_time
        rect = rect or self.bounding_rectangle
        if not self.is_visible:
            return
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            raise_exception=False,
            timeout=Settings.current().element_highlight_ensure_timeout,
        )
        self.adapter.technology.display_device.highlight_rect(rect, time=time)

    def get_screenshot(self, rect: Rect | None = None, *,
                       format: str | None = None,
                       quality: int | None = None) -> bytes: ...
    def save_screenshot(self, filename: str | None = None, **kw) -> str: ...
```

**Robot-Keywords:**

```python
@keyword
def highlight(element: ElementDescriptor, time: float | None = None) -> None: ...

@keyword
def take_screenshot(element: ElementDescriptor | None = None, *,
                    filename: str | None = None) -> str: ...
```

**Inspector-Anbindung.** Die GUI-Inspector-App (`apps/inspector`) ist
*kein* Teil der Python-Library — sie ist ein eigenständiges Rust-
Binary mit eigener `egui`-UI. Die Python-Library exposed jedoch die
gleichen Diagnose-Bausteine (Highlight, Screenshot, Tree-Dump), damit
Tests ohne Inspector denselben Output produzieren können.


## 10. Migrations-Reihenfolge

Reihenfolge der Implementierung in Phasen, aufsteigend nach
Abhängigkeit. Jede Phase ist eigenständig testbar; spätere Phasen
bauen auf früheren auf.

### Phase 1 — Fundament

1. `core/types.py` — `TypeAlias`es (`PatternName`, `RoleName`,
   `TechnologyName`, `FrameworkId`) als freie String-Aliases; zusätzlich
   Re-Export `Point`/`Rect` aus `platynui_native` (Rev. 17)
2. `core/settings.py` — als `@dataclass(frozen=True, slots=True, kw_only=True)`,
   mit `with`-Block und RF-Variablen-Brücke (siehe §9a.1)
3. `core/exceptions.py` — Hierarchie nach §9a.2 (Typo-Fix
   `PlatyUiError` → `PlatynUIError`)
4. `core/wait.py` — `wait_for` nach §9a.3
5. `core/ensure.py` — `ensure_that` mit Stage-Memo und re-entrant
   Thread-Local-Stack (siehe §9a.3); Decorator mit `ParamSpec`/
   `Concatenate` typisiert, `match`/`case` für Outcome
6. `core/predicate.py` — `@predicate`-Decorator (für `ensure_that`-
   Standard-Predicates und Page-Object-Predicates, siehe §A.3)
7. `core/weight_calculator.py` — Port aus Altprojekt, `MatchCriteria`
   als Dataclass; `attribute_value(name, namespace)`-basiert (Rev. 15)
8. `core/technology.py` — Marker + AdapterFactory-Registry
9. `core/locator.py` — neu, XPath-basiert über Rust, API nach §A.6
   (`@overload`, Builder-Methoden geben `Self` zurück, `copy_from`-
   Vererbung; `attributes` mit Tupel- oder String-Keys; Default-NS
   per Klassenattribut `default_attribute_namespace`; freie
   PascalCase-Kwargs nach Rev. 16)
10. `core/patterns/` — **vorgezogen aus Phase 2 Punkt 11**, weil
    `Locator` und `WeightCalculator` bereits `pattern_name`
    referenzieren. Pattern-Klassen nach §5 (Element, TextContent,
    TextEditable, Clearable, Toggleable, Activatable, Focusable —
    konsolidiert in Rev. 17, parallel zu den Rust-Capability-Gruppen
    in `crates/core/src/ui/attributes.rs`). Ohne Default-
    Implementierungen (`patterns/defaults.py` bleibt Phase 4).

**Ergebnis:** ~750 LOC Core-Infrastruktur + Pattern-ABCs, unabhängig
testbar (119 pytest, mock-basiert).

### Phase 2 — Adapter-Schicht

11. `core/adapter.py` — Interface
12. `core/adapter_proxy.py` — `AdapterProxy`, `PatternProxyFactory`,
    `@pattern_proxy_for` (portiert aus altem `adapterproxy.py`, nur
    umbenannt)
13. `core/adapters/ui_node.py` — `UiNodeAdapter`, der `UiNode` aus
    `platynui_native` umhüllt; mappt Native-Patterns auf Python-Patterns
    (`platynui_native.Focusable` → `core.patterns.Focusable` usw.).
    Siehe §A.4a.
14. `core/devices.py` — `MouseProxy`/`KeyboardProxy` über
    `platynui_native.Runtime`

(Punkt 11 „`core/patterns/` — Pattern-ABCs" wurde nach Phase 1
vorgezogen, siehe Phase-1-Punkt 10. Ein dedizierter Python-`MockAdapter`
entfällt; Tests gegen den UI-Tree nutzen den Rust-Mock-Provider via
`Runtime.new_with_mock()`. Siehe §A.11.)

### Phase 3 — Context-Schicht

14. `core/context.py` — `ContextBase`, `ContextFactory`, `@context`
15. `core/descriptor.py` — `ElementDescriptor[PatternT]` (aus
    BareMetal extrahieren, gemeinsam nutzen)

### Phase 4 — UI-Klassen + Standard-Proxies

16. `ui/proxies/base.py`, `ui/element.py`, `ui/control.py`
17. `ui/proxies/standard.py` + `ui/buttons.py` — Button/CheckBox/RadioButton
18. `ui/proxies/window.py` + `ui/window.py` — Window/Dialog
19. `ui/proxies/text.py` + `ui/text.py` + `ui/combobox.py`
20. `ui/proxies/list_tree.py` + `ui/lists.py` + `ui/tree.py` + `ui/table.py`
21. `ui/menus.py`, `ui/tabs.py`, `ui/desktop.py`, `ui/application.py`

### Phase 5 — Keywords + Robot-Library

22. `keywords/*.py` — semantische Keywords
23. `__init__.py` als Robot-Library
24. Libdoc-Generierung ins CI

### Phase 6 — Iterative Erweiterungen

- Weitere Provider-Patterns in Rust hinzufügen, wenn ein konkreter Bedarf
  auftritt (siehe §6.2). Pro neues Pattern: Trait + Provider-Impl(s) +
  Mapping in `UiNodeAdapter` auf ein Python-Pattern. UI-Klassen und
  Proxies bleiben unverändert.

## 11. Drastische Vereinfachungen gegenüber dem Altprojekt

### 11.1 `Locator` (433 → ~120 LOC)

XPath-Aufbau dramatisch kürzer dank Rust-XPath-2.0-Engine. Wegfallend:
manueller Path-Aufbau mit Achsen-Präfix-Logik, Index/Position-Spezialfälle,
parent/child-Relativierungen (Rust kennt stabile `runtime_id`s).

### 11.2 `ContextBase` (473 → ~250 LOC)

Wegfallend:
- Adapter-Validierung-Schleife (`UiNode` macht Lebenszeit selbst)
- Property-Read-Convenience-Methoden (`UiNode.attribute(...)` direkt)
- WeakSet-basiertes Children-Tracking — Rust-Cache regelt
- `invalidate()`-Propagation — `runtime.clear_cache()` reicht

Bleibend (1:1):
- Locator-Verwaltung, Parent-Beziehungen
- `ensure_that` / `wait_for`
- `get`/`get_all`/`get_child`/`ancestor`/`iter_all`
- Context-Manager (`with MyWindow() as w:`)

### 11.3 `ensure.py` (155 → ~80 LOC)

Wegfallend: Predicate-Cache mit ad-hoc Eviction, Sonderfall-Hooks für
Adapter-Lifecycle, manuelle `failed_func`-Registrierung pro Predicate.
Bleibend: **Re-entrant Thread-Local-Stack** (siehe §A.3) — wird für
verschachtelte `ensure_that`-Aufrufe in Element-Predicates gebraucht
und ist nicht ersetzbar; `failed_func` als optionaler Parameter
(Default: `context.invalidate`); Stage-Memo für stabile
Multi-Predicate-Verifikation.

### 11.4 `WeightCalculator` (114 → ~110 LOC)

**Mechanismus 1:1 übernommen** (gewichtetes Multi-Kriterien-Match —
elegant, hebt Spezialfälle über generische). Einzige inhaltliche
Änderung: das alte Doppel-Kriterium `properties[k]==v` /
`native_properties[k]==v` wird zu **einem** Kriterium
`attributes[(ns, name)] == v`, weil Adapter (Rust und alle künftigen)
Attribute jetzt einheitlich über den `(namespace, name)`-Schlüsselraum
exposen (siehe §A.4 / §4.1). LOC bleibt praktisch identisch — die
Cache-Map nutzt Tupel-Keys statt zwei separate Dicts.

## 11a. Testing-Strategie

Tests laufen auf **drei komplementären Ebenen**. Zielpublikum der
Bibliothek ist Robot Framework — Acceptance-Tests in `.robot`-Syntax
sind das Oberflächenziel, aber nicht der alleinige Test-Kanal.

```
                    ┌─────────────────────────────┐
                    │  Ebene 3: Robot Framework   │  Acceptance
                    │     .robot Suites           │  (Keyword-Syntax)
                    ├─────────────────────────────┤
                    │  Ebene 2: pytest            │  Python-Unit/
                    │  (Rust-Mock via Maturin     │  -Integration
                    │   + Inline-Fakes für ABC)   │
                    ├─────────────────────────────┤
                    │  Ebene 1: cargo nextest     │  Rust-Unit/
                    │  (Rust-Mock-Provider)       │  -Integration
                    └─────────────────────────────┘
```

### 11a.1 Ebene 1 — Rust-Tests (`cargo nextest`)

**Scope:** Runtime-Orchestrierung, XPath-Parser/Evaluator, Provider-
Implementierungen, PyO3-Bindings (sofern reine Rust-Logik).

**Werkzeuge:**
- `cargo nextest run --workspace` (Standard)
- `cargo nextest run -p platynui-runtime --features mock-provider`
- Der Rust-`provider-mock` ist bereits umfangreich (~3259 LOC,
  `tree.rs`/`input.rs`/`window.rs`/`focus.rs`/`events.rs`) und
  unterstützt XML-Fixtures via `quick_xml::de`.

**Ergänzungen, die evtl. nötig werden** (on-demand, nicht auf Verdacht):
- Property-Change-Events für `wait_for`-Tests
- Scripted Behavior: MockNodes, die nach einer Aktion State ändern
- Timing-Injection: kontrollierte Delays für Timeout-Tests
- Failure-Injection: Patterns, die gezielt Exceptions werfen

### 11a.2 Ebene 2 — Python-Tests (`pytest`)

Zwei komplementäre Spielarten, je nach Test-Ziel:

**2a. Inline-Fakes für ABC-/Algorithmus-Tests**
- Kein Rust-Build nötig.
- Lokale `class _FakeAdapter(Adapter)` o.ä. direkt im Test-Modul; nicht
  als öffentliches Test-API gehoben.
- Für: `Adapter`-ABC-Resolution-Algorithmus, `AdapterProxy`-Komposition,
  `PatternProxyFactory`-Match-Logik, `WeightCalculator`,
  `wait_for`/`ensure_that`-Re-Entrancy — alles, was rein in Python
  liegt und keinen UI-Tree braucht.

**2b. Tests gegen den Rust-Mock-Provider**
- Build: `uv run maturin develop -m packages/native/Cargo.toml --features mock-provider`
- Für: `UiNodeAdapter` (Native-Pattern-Mapping, PyO3-Bindings),
  `ContextBase.get`-End-to-End, UI-Klassen-Hybrid-Form, Keyword-
  Outcome-Verträge — alles, was den realen UI-Tree-Pfad durchläuft.
- Stub-/Spy-Verhalten wird via `AdapterProxy`-Overlays realisiert
  (Proxy mixt das gewünschte Pattern als Spy ein).

### 11a.3 Ebene 3 — Robot-Framework-Acceptance-Tests

**Zielbild:** `.robot`-Suites sind *die* primäre Nutzer-Erfahrung.
Wir müssen sie früh haben, nicht erst am Ende — sobald UI-Klassen
existieren (ab Phase 3), schreiben wir parallel Smoke-Suites.

**3a. RF gegen Rust-Mock-Provider** (CI-freundlich, deterministisch)
- Python-Aufruf: `Runtime.new_with_mock()` (setzt `mock-provider`-
  Feature-Build voraus — analog zur bestehenden BareMetal-Praxis mit
  `use_mock=${true}`)
- Läuft headless auf Linux/macOS/Windows ohne Display-Server
- **Coverage:** Keyword-Semantik, Locator-Syntax, Pre/Post-
  Conditions, Outcome-Vertrag, Page-Object-Pattern

**3b. RF gegen echte Adapter** (plattformabhängig)
- Läuft gegen `apps/test-app-egui` oder plattform-native Test-Apps
- **Coverage:** Echte Accessibility-API (UIA/AT-SPI/AX), Wayland-
  Compositor, Plattform-Integration
- Separate CI-Jobs pro OS; nightly oder on-demand

### 11a.4 RF-Mock-Harness — benötigte Keywords

Für RF-Tests gegen den Rust-Mock brauchen wir ein kleines
RF-Library-Set (idealerweise als Teil von `PlatynUI.Mock` oder
als Erweiterung der BareMetal-Library):

```robotframework
*** Settings ***
Library    PlatynUI    use_mock=${true}
Library    PlatynUI.Mock    # Mock-Steuerung

*** Test Cases ***
Login Dialog Becomes Enabled After Typing
    Load Mock Tree    fixtures/login-dialog.xml
    Type Text    ${USERNAME_FIELD}    alice
    Set Mock Property    ${LOGIN_BUTTON}    IsEnabled    True
    Activate    ${LOGIN_BUTTON}
    Wait For Window    name=Dashboard
```

**Benötigte Mock-Library-Keywords:**
- `Load Mock Tree  <xml-fixture>` — deklarativer Tree-Load
- `Set Mock Property  <element>  <name>  <value>` — State-Mutation
- `Set Mock Pattern Behavior  <element>  <pattern>  <script>` —
  Scripted Actions (z.B. „Klick öffnet Window X nach 100ms")
- `Clear Mock Tree` — Tear-down

**Implementierungsgrundlage:** `tree.rs` hat bereits
`quick_xml::de::from_str` (Z.11). Set-Property/Set-Behavior sind
Erweiterungen über bestehende `MockNode`-Mutationspfade.

### 11a.5 Test-Matrix

| Test-Gegenstand                    | nextest | pytest(Inline-Fakes) | pytest(Rust-Mock) | RF(Rust-Mock) | RF(echt) |
|------------------------------------|:-------:|:--------------------:|:-----------------:|:-------------:|:--------:|
| XPath-Parser/Evaluator             |   ✅    |          —           |         —         |       —       |    —     |
| Runtime-Orchestrierung             |   ✅    |          —           |        ✅         |       —       |    —     |
| Provider-Adapter (Rust)            |   ✅    |          —           |         —         |       —       |   ✅     |
| PyO3-Bindings                      |    —    |          —           |        ✅         |       —       |    —     |
| Pattern-ABC-Verträge               |    —    |         ✅           |         —         |       —       |    —     |
| `Adapter`-Resolution-Algorithmus   |    —    |         ✅           |         —         |       —       |    —     |
| `AdapterProxy`-Komposition         |    —    |         ✅           |         —         |       —       |    —     |
| `@pattern_proxy_for` Match-Logik   |    —    |         ✅           |         —         |       —       |    —     |
| `UiNodeAdapter` (Native-Mapping)   |    —    |          —           |        ✅         |       —       |    —     |
| `wait_for`/`ensure_that` Re-entry  |    —    |         ✅           |         —         |       —       |    —     |
| `ContextBase.get` Parent-Chain     |    —    |          —           |        ✅         |       —       |    —     |
| UI-Klassen (Button, Window, …)     |    —    |          —           |        ✅         |       —       |    —     |
| Keyword-Outcome-Verträge           |    —    |          —           |        ✅         |      ✅       |   ✅     |
| Locator-Syntax in RF               |    —    |          —           |         —         |      ✅       |   ✅     |
| Plattform-Provider (UIA/AT-SPI/AX) |    —    |          —           |         —         |       —       |   ✅     |
| End-to-End Page-Object             |    —    |          —           |         —         |      ✅       |   ✅     |

### 11a.6 Reihenfolge & CI-Integration

**Grundprinzip:** Mock-Tests und Echt-Provider-Tests laufen **parallel**,
nicht sequentiell. Sobald technisch möglich, wird jede Keyword-/Pattern-
Funktion auf beiden Kanälen validiert. Der Mock ist nicht „erste Stufe"
und der Echt-Provider „zweite Stufe" — sie sind komplementär.

**Phase 0 (Verifikation):** AccessKit ist in `apps/test-app-egui` bereits
über das `eframe`-Default-Feature aktiv (siehe §11a.7.1) — keine
Code-Änderung nötig. Phase 0 ist eine Smoke-Verifikation: AT-SPI-Tree
inspizieren, ggf. deterministische Widget-Szenarien ergänzen.

**Phase 1 (Foundation):** pytest für `types`, `settings`, `wait`,
`ensure`, `weight_calculator`, `technology`, `locator`.

**Phase 2 (Patterns/Adapter):** pytest mit Inline-Fakes für die ABC-/
Resolution-Algorithmen (`Adapter`, `AdapterProxy`, `PatternProxyFactory`)
sowie pytest gegen den Rust-Mock-Provider für `UiNodeAdapter`.

**Phase 3 (Standard-UI-Klassen):** Zusätzlich zu pytest die ersten
**Smoke-RF-Suites** — und zwar **dual-mode** (siehe §11a.7): dieselbe
Suite läuft gegen Rust-Mock UND gegen `test-app-egui` auf Linux AT-SPI.
Ziel: `click_button.robot`, `wait_for_window.robot`, `type_text.robot`.

**Phase 4 (Rust-Adapter via PyO3):** pytest gegen Rust-Mock (Bindings)
+ pytest gegen echten Linux-AT-SPI-Provider.

**Phase 5 (Devices, Highlight):** pytest + RF-Keywords für Maus/
Keyboard. Mock-Pfad via `Runtime.new_with_mock()`, Echt-Pfad via
Wayland-Compositor + `test-app-egui`.

**Phase 6 (Keywords):** Vollständige RF-Suite gegen Rust-Mock UND
gegen Linux-AT-SPI im Haupt-CI. Windows-UIA + macOS-AX als Job-Matrix
dazu, sofern Runner verfügbar.

**Phase 7 (Härtung):** Zusätzliche plattform-spezifische Eigenheiten
(komplexe Controls, DPI-Scaling, Multi-Monitor, HighContrast-Themes).

**CI-Matrix:**
- **Haupt-CI (PR-Gate):**
  - Linux: Ebene 1 (nextest) + Ebene 2 (pytest Mock+Rust-Mock) +
    Ebene 3a (RF vs. Rust-Mock) + Ebene 3b-Linux (RF vs.
    Wayland-Compositor + `test-app-egui` AT-SPI).
  - Windows (falls Runner): Ebene 1 + Ebene 3b-Windows (RF vs.
    `test-app-egui` UIA).
  - macOS (falls Runner): Ebene 1 + Ebene 3b-macOS (RF vs.
    `test-app-egui` AX).
- **Nightly:** Erweiterte Echt-Provider-Suiten gegen native Apps
  (nicht nur `test-app-egui`), plattform-spezifische Edge-Cases,
  Langläufer.
- **Release-Gate:** Alle OS-Jobs müssen grün sein.

### 11a.7 Echt-Provider-Teststrecken

Jede OS-Plattform braucht eine reproduzierbare Teststrecke mit
(a) einer Ziel-App, (b) aktivem Accessibility-Stack und (c) einem
deterministischen Display-Server-Setup. Die Teststrecken werden als
Build-Abhängigkeit der RF-Acceptance-Jobs eingebunden.

#### 11a.7.1 `apps/test-app-egui` als gemeinsame Ziel-App

**Status quo:** `apps/test-app-egui/Cargo.toml` nutzt `eframe = "0.34"`
ohne `default-features = false`. Da `eframe 0.34.1` `accesskit` als
**Default-Feature** mitbringt (`accesskit (default) → egui-winit/accesskit`),
ist die App **bereits accessibility-aktiv**: AT-SPI auf Linux, UIA auf
Windows, AX auf macOS — automatisch über `accesskit_winit`. Ab
egui 0.35+ wird das Feature laut Upstream-PR #7701 obligatorisch
(immer enabled). Der Modul-Header (`main.rs:7`) deklariert die App
explizit als „Accessibility target for `PlatynUI` functional tests via
`AccessKit`/AT-SPI".

**Aktion (Phase 0):** Verifikation, nicht Integration. **Durchgeführt
in Rev. 13** auf Linux/Wayland/GNOME — Ergebnis: AccessKit→AT-SPI-
Pipeline funktioniert, aber mit einer scharfen Bedingung (siehe unten).

**Verifizierte Smoke-Strecke (reproduzierbar):**

```bash
# 1. Build
cargo build -p platynui-test-app-egui -p platynui-cli

# 2. Precondition (Linux): AccessKit aktiviert sich nur bei aktivem
#    Screen Reader. Siehe „Lazy-Activation-Falle" unten.
gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
    --method=org.freedesktop.DBus.Properties.Set \
    org.a11y.Status ScreenReaderEnabled "<true>"

# 3. App starten (detached)
setsid nohup ./target/debug/platynui-test-app-egui \
    --app-id org.platynui.testapp \
    --title "PlatynUI Smoke Test" \
    --auto-close 0 > /tmp/testapp.log 2>&1 < /dev/null &
disown

# 4. Per XPath finden und Baum auslesen
./target/debug/platynui-cli-rs query \
    "//app:Application[contains(@Name, 'test-app-egui')]"
./target/debug/platynui-cli-rs snapshot \
    "//app:Application[@ProcessId=<PID>]"

# 5. Cleanup
kill <PID>
gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
    --method=org.freedesktop.DBus.Properties.Set \
    org.a11y.Status ScreenReaderEnabled "<false>"
```

**Verifizierte Fakten:**
- `control:Role="Application"`, `control:Name="platynui-test-app-egui"`,
  `control:ProcessId`, `control:Technology="AT-SPI2"` kommen korrekt durch.
- `control:Bounds`, `control:ActivationPoint`, `control:IsFocused`,
  `control:SupportedPatterns=["org.platynui.patterns.Focusable","org.platynui.patterns.WindowSurface"]` funktionieren.
- Widget-Hierarchie sichtbar: Frame → Panel (Menubar) → Button, Entry,
  CheckBox, SpinButton, ScrollBar etc. (43 Children im Frame).
- `native:Accessible.*` (Role, RoleName, State, Interfaces,
  RelationSet, ChildCount, IndexInParent) ist vollständig populiert.

**Lazy-Activation-Falle (Linux — kritisch!):**

`accesskit_unix 0.21` implementiert „lazy activation":
`src/context.rs:153–180` verbindet sich mit `org.a11y.Status` und
aktiviert den Adapter **nur wenn `ScreenReaderEnabled == true`**. Ohne
Screen-Reader-Flag ist der Adapter zwar konstruiert (`Adapter::new` →
`AdapterState::Inactive`), aber nicht am D-Bus-Registry gelistet — die
App erscheint für den AT-SPI-Provider unsichtbar.

- Kein App-Code kann das umgehen (Design von AccessKit).
- Betrifft alle winit/egui-Apps auf Linux, nicht nur unsere.
- Für Tests: Setup-Hook muss `ScreenReaderEnabled=true` setzen und
  beim Teardown zurückstellen. Siehe §11a.7.2 und §11a.7.6.

**Offene Beobachtungen (nicht Phase-0-blockierend):**

- **AT-SPI-Timeout:** Beim ersten Call gegen die cold-started App
  wirft `provider-atspi` einen `D-Bus call timed out elapsed_ms=1000`-
  Warn-Log. Der Call liefert trotzdem sein Ergebnis — Timeout scheint
  Warnschwelle, nicht Abbruch zu sein. TODO: Timeout-Strategie im
  AT-SPI-Provider prüfen (evtl. 3000ms oder adaptiv beim Cold-Start).
- **`native:Application.ToolkitName = null`:** AccessKit oder egui-
  winit setzt dieses AT-SPI-Property nicht. Für Tests unkritisch,
  aber eine Future-Improvement-Idee wäre, in `test-app-egui` o.Ä.
  einen Toolkit-Namen zu setzen, falls AccessKit-API das erlaubt.
- **Widget-Szenarien ausreichend:** Die bestehenden Widgets in
  `test-app-egui` bieten genug Test-Targets für Phase 1–4. Nachschärfen
  (stabile IDs pro Widget, zusätzliche Szenarien wie Liste, Dialog,
  geschachtelte Fenster) erst wenn konkrete RF-Tests das brauchen.

#### 11a.7.2 Linux — AT-SPI über Wayland-Compositor

- **Compositor:** `apps/wayland-compositor` (vorhanden, ~9889 LOC,
  Smithay-basiert). Läuft headless im CI, kontrollierter Focus/Input,
  Highlight-Support eingebaut.
- **Accessibility:** AT-SPI via D-Bus. `test-app-egui` mit AccessKit
  publiziert seinen Baum automatisch; `atspi`-Provider greift zu.
- **Precondition (zwingend, siehe §11a.7.1):** Vor dem App-Start muss
  `org.a11y.Status.ScreenReaderEnabled = true` gesetzt sein. Ohne das
  Flag aktiviert `accesskit_unix` den Adapter nicht, und die App
  bleibt unsichtbar am AT-SPI-Bus. Die Fixture muss das Flag setzen
  **bevor** die App startet (Environment wird beim App-Start gelesen)
  und nach den Tests zurückstellen.
- **CI-Setup:** D-Bus Session-Bus + at-spi-bus starten →
  ScreenReaderEnabled=true setzen → Compositor starten → test-app-egui
  als Child starten → RF-Suite laufen lassen → Teardown in umgekehrter
  Reihenfolge.
- **Runner:** GitHub-hosted `ubuntu-latest` reicht (Compositor ist
  headless). Kein echter GPU nötig (`wayland-compositor` nutzt
  Software-Rendering-Backend). AT-SPI-Bus muss als System-Dependency
  installiert sein (`at-spi2-core` Paket).

#### 11a.7.3 Windows — UIA

- **App:** `test-app-egui` mit AccessKit-UIA-Backend (AccessKit
  unterstützt UIA nativ auf Windows).
- **Accessibility:** Windows UI Automation API; Provider
  `provider-windows-uia` greift zu.
- **Runner:** GitHub `windows-latest`. Kein Display-Server-Trick
  nötig (Windows hat eingebauten Desktop-Session).
- **Besonderheit:** UIA-Trees können vom AT-SPI-Tree leicht
  abweichen — Tests brauchen plattform-spezifische Locator-
  Variationen oder einen Abstraktions-Layer.

#### 11a.7.4 macOS — AX

- **App:** `test-app-egui` mit AccessKit-AX-Backend.
- **Accessibility:** macOS Accessibility API; Provider
  `provider-macos-ax` greift zu. Benötigt u.U. Assistive-Access-
  Berechtigung — im CI über einen Init-Schritt.
- **Runner:** GitHub `macos-latest` (oder selbstgehosteter M-series-
  Runner für Cross-Architektur-Tests).

#### 11a.7.5 Dual-Mode-RF-Suites

Idealfall: **dieselbe** `.robot`-Datei läuft gegen Mock und gegen
Echt-Provider, gesteuert über eine Library-Variable.

```robotframework
*** Settings ***
# ${PROVIDER_MODE} wird vom CI-Runner gesetzt: mock|atspi|uia|ax
Library    PlatynUI    provider=${PROVIDER_MODE}
Suite Setup    Run Keyword If    '${PROVIDER_MODE}' == 'mock'    Load Mock Fixture    login-dialog.xml

*** Test Cases ***
Login With Valid Credentials
    Type Text       ${USERNAME}    alice
    Type Text       ${PASSWORD}    secret
    Activate        ${LOGIN_BUTTON}
    Wait For Window    name=Dashboard
```

- **Gleiche Semantik, gleiche Keywords.** Unterschiede leben im
  Locator (plattform-spezifische Rollen-/Property-Namen) bzw. in
  der Fixture-Vorbereitung (Mock lädt XML, echte Provider starten
  App-Prozess).
- **CI-Lauf:** Matrix über `PROVIDER_MODE={mock, atspi, uia, ax}` ×
  `OS={ubuntu, windows, macos}`, ungültige Kombinationen
  (z.B. `uia` auf Linux) werden ausgefiltert.

#### 11a.7.6 Fixture-/Setup-Konventionen

- **Mock-Fixtures:** XML-Dateien unter `tests/acceptance/fixtures/mock/`.
- **Echt-App-Szenarien:** Scripts unter `tests/acceptance/fixtures/egui/`,
  die `test-app-egui --scenario <name>` mit vorkonfiguriertem Layout
  starten.
- **Linux-a11y-Helper:** `scripts/linux-a11y-enable.sh` und
  `scripts/linux-a11y-restore.sh` (noch anzulegen) toggeln
  `org.a11y.Status.ScreenReaderEnabled` via `gdbus` (siehe §11a.7.1).
  Sie müssen idempotent sein und den vorherigen Wert restaurieren
  (Read-Modify-Write-Pattern). RF-`Suite Setup`/`Suite Teardown` und
  pytest-Session-Fixtures rufen sie auf Linux auf; No-Op auf Windows/
  macOS.
- **Suite-Layout:** `tests/acceptance/{smoke,core,devices,edge}/*.robot`.
  Jede Suite deklariert via Tags, in welchen Provider-Modi sie läuft
  (`atspi_only`, `all_providers`, `mock_only`).

## 12. Erfolgskriterien

1. Eine `.robot`-Datei mit Page-Object-Pattern (Calculator-Beispiel) läuft
   gegen `apps/test-app-egui` in `wayland-compositor` und gegen einen
   nativen Calculator unter Windows UIA.
2. Alle Standard-Keywords aus §8 funktionieren.
3. Ein „Fake-Button" (Label mit ClickHandler) wird durch einen
   `@pattern_proxy_for(role="Label", attributes={...})`-Proxy korrekt als
   Button bedient — ohne dass die Test-Suite davon weiß.
4. Mindestens 30 Python-Unit-Tests gegen Mock-Adapter
   (`core/adapters/mock.py`).
5. Mindestens eine Robot-Acceptance-Suite läuft im CI.
6. Libdoc-Output ist im CI publiziert.

## 13. Offene Fragen

### 13.1 BareMetal vs. PlatynUI — Abgrenzung

- **BareMetal:** Low-Level, XPath-Strings direkt, ohne Page Objects oder
  semantische Keywords. Bleibt als Werkzeug für Quick-Skripte und für
  Diagnose-/Debug-Zwecke.
- **PlatynUI:** High-Level, Page-Object-basiert, semantische Keywords mit
  Outcome-Vertrag.

Beide nebeneinander. Mittelfristig kann BareMetal sich auf eine reine
Diagnose-/Inspector-Rolle konzentrieren.

### 13.2 `UiNodeDescriptor`-Umzug — **Resolved in §A.7**

`UiNodeDescriptor` lebt aktuell in `BareMetal/__init__.py:51`. §A.7
spezifiziert den gemeinsamen `ElementDescriptor[PatternT]` in
`core/descriptor.py`; beide Libraries nutzen ihn. Umsetzung in
Phase 3 der Migration (siehe §10).

### 13.3 `UiNode.supported_patterns` in Python

Bereits verfügbar: `UiNode.supported_patterns() -> list[str]` ist in
`packages/native/src/runtime.rs:157` implementiert und in
`_native.pyi:302` typisiert. `UiNodeAdapter` kann direkt darauf
aufsetzen — kein zusätzlicher Bindings-Bedarf.

### 13.4 Mehrere Runtimes / Pabot

`ContextFactory.registered_contexts` und
`PatternProxyFactory.registered_proxies` sind prozessweite Registries
(Class-Variables, gefüllt durch `@context` /
`@pattern_proxy_for`-Aufrufe beim Modul-Import — Implementierungs­
detail der jeweiligen Factory). Das ist bewusst und unabhängig von
der „keine globale Pattern-Registry"-Entscheidung in §5.4: dort geht
es um Pattern-Identifier, hier geht es um die Auflösungs-Tabellen
für Klassen-Auswahl.

Für `pabot` (parallele Robot-Runs im selben Prozess) ist das ok, weil
Registrierungen in Modul-Imports passieren und prozessweit gleich
aussehen. Bei Bedarf können wir später Runtime-Scoped Registries
einführen — dann würden auch die Klassen-Registries pro `Runtime`-
Instanz leben.

### 13.5 Native Read-Escape-Hatch

`UiNode.attribute(name, "native")` deckt provider-spezifische Reads ab.
Ein Write-Escape-Hatch (z.B. direkter Aufruf einer UIA-Pattern-Methode,
die wir nicht abstrahiert haben) ist offen. Vorschlag: separate
`unsafe`-Helper-API auf dem Adapter, explizit als „nicht portabel"
markiert.

### 13.6 Rust-PatternId-Umstellung auf Reverse-DNS — RESOLVED (Rev. 14)

> **Hinweis (Rev. 19):** Der Newtype `PatternId` und das Konstanten-Modul
> `pattern_ids`, die in dieser Sektion erwähnt werden, heißen seit Rev. 19
> `PatternName` und `pattern_names`. Siehe §13.7 für die Umbenennung. Der
> ursprüngliche Wortlaut bleibt hier erhalten als historisches Record der
> Reverse-DNS-Umstellung.

Rust-`PatternId` verwendet jetzt durchgängig Reverse-DNS-Identifier
(`org.platynui.patterns.<Name>`). Damit sind Rust-`PatternId.as_str()`
und Python-`pattern_name` wörtlich identisch.

**Implementierung:**

- Neues Konstanten-Modul `core::ui::pattern_ids` mit `&'static str`-
  Konstanten (z.B. `pub const FOCUSABLE: &str = "org.platynui.patterns.Focusable";`)
- Alle `PatternId::from("BareName")`-Stellen in core/runtime/provider-atspi/
  provider-mock/platform-linux-wayland umgestellt
- `assets/mock_tree.xml` enthält voll qualifizierte Patterns (kein
  Loader-Expand-Shortcut)
- PyO3-Bindings: `Pattern.id()` liefert Reverse-DNS;
  `pattern_id_from_arg` liest `pattern_name`-ClassVar von Pattern-Klassen
  (kein Klassennamen-Fallback mehr)
- `match pattern.as_str() { … }` in Provider-Code auf `if`-Ketten
  umgestellt (`&str`-Konstanten in Patterns nicht erlaubt)

**Breaking Change:** PyO3 `pattern_object`/`Pattern.id()` liefert jetzt
Reverse-DNS-Strings statt bare names; Python-Aufrufer von
`get_pattern("Focusable")` müssen auf `get_pattern("org.platynui.patterns.Focusable")`
oder `get_pattern(Focusable)` (mit `pattern_name`-ClassVar) umstellen.

### 13.7 Rust-API-Symmetrie zu Python: PatternId → PatternName — RESOLVED (Rev. 19)

Rust-Code spricht ab Rev. 19 dieselbe Vokabel wie Python: `PatternName`
(statt `PatternId`), `pattern_names` (statt `pattern_ids`),
`UiPattern::pattern_name()` (statt `id()`),
`UiPattern::static_pattern_name()` (statt `static_id()`),
`UiNode::pattern_by_name()` (statt `pattern_by_id()`).

**Motivation:** Asymmetrie zwischen Rust (`PatternId` / `id()`) und
Python (`PatternName` / `pattern_name`) verursachte mentale
Übersetzungsarbeit beim Lesen. Der Wire-Inhalt (Reverse-DNS-String)
ist auf beiden Seiten identisch — die Bezeichner sind es jetzt auch.

**Implementierung:**

- `crates/core/src/ui/identifiers.rs`: `pub struct PatternName(Arc<str>)`,
  `pub mod pattern_names`
- `crates/core/src/ui/pattern.rs`: `UiPattern::pattern_name()`,
  `UiPattern::static_pattern_name()`
- `crates/core/src/ui/node.rs`: `UiNode::pattern_by_name()`
- Alle Implementierungen in `core`, `runtime`, `provider-*`, `platform-*`,
  `cli`, `inspector`, `packages/native` mechanisch nachgezogen
  (~27 Dateien, ~150 Bezeichner-Treffer)
- PyO3-Klasse `PatternName` wird intern in `platynui_native._native`
  registriert, aber bewusst NICHT aus `platynui_native.__init__`
  re-exportiert, weil sie sonst mit dem Python-TypeAlias
  `PlatynUI.core.types.PatternName: TypeAlias = str` kollidieren würde.
  Python-User-Code spricht den str-Alias.

**Verifikation:**

- `cargo nextest run --workspace`: 1980/1980 ✅
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `uv run pytest`: 265/265 ✅
- `uv run ruff check`, `mypy`, `pyright`: clean

**Breaking Change (Rust):** Direkte Rust-Konsumenten der `UiPattern`-/
`UiNode`-Traits müssen ihre Aufrufe von `id()` → `pattern_name()`,
`static_id()` → `static_pattern_name()`, `pattern_by_id()` →
`pattern_by_name()` umstellen, und `PatternId` / `pattern_ids` durch
`PatternName` / `pattern_names` ersetzen. Wire-Format unverändert.

**Breaking Change (Python):** PyO3 exportiert keine Klasse `PatternId`
mehr aus `platynui_native`. Wer den Wrapper braucht (selten —
typischerweise nur Bridging-Code), nutzt `platynui_native._native.PatternName`.
User-Code, der nur Strings vergleicht, ist nicht betroffen.

## 14. Zusammenfassung

- **Patterns leben in Python.** Der `WeightCalculator` plus
  `@context` und `@pattern_proxy_for` ist der Kern der Erweiterbarkeit
  und wird aus dem Altprojekt übernommen (dort hießen sie Strategies).
- **Batterien inklusive:** PlatynUI liefert für alle gängigen
  UI-Rollen fertige UI-Klassen und Default-Proxies mit (siehe §5a).
  Test-Projekte sind ohne eigenen Proxy-Code produktiv; App-Spezialfälle
  überschreiben gezielt über `framework_id`/`class_name`/`attributes`.
- **Rust ist eine konkrete Adapter-Implementierung**, nicht „die"
  Adapter-Schicht. Weitere Adapter (JSON-RPC, Mock, …) können
  gleichberechtigt daneben stehen. Patterns tragen einen stabilen
  Reverse-DNS-Identifier (`org.platynui.patterns.*`), über den externe
  Adapter Capabilities string-basiert melden und aufrufen.
- **Adapter dürfen Patterns mitbringen** (heute: `Focusable`,
  `WindowSurface`). Welches Pattern bei `adapter.get_pattern(X)`
  gewinnt, entscheidet pro UiNode die Kombination aus Proxy-Override
  und Adapter-eigenen Patterns.
- **User-Sicht ist führend:** UI-Elemente werden so beschrieben, wie sie
  sich präsentieren. Ein „Fake-Button" wird über `@pattern_proxy_for` mit
  passenden Kriterien zum Button — die Test-Klasse `Button` weiß nichts
  davon.
- **Robot-Keywords sind semantische Aktionen mit Outcome-Vertrag**
  (`Activate`, `Toggle`, `Set Value`, `Expand`, …), nicht
  Mechanismus-Verben (`Click`).
- **Pre-Conditions → Perform → Postcondition** ist der universelle
  Lebenszyklus jeder Aktion. Verifikation ist erstklassig.
- **~50 % Code-Reduktion** gegenüber dem Altprojekt (4.600 → ~2.000 LOC),
  weil Adapter-/Technology-Bridge und XPath-Builder dank Rust schrumpfen
  — die Konzepte aber 1:1 erhalten bleiben.
- **Modernes Python 3.10+** ist verbindlich (siehe §2.6): ABC als
  Default für Pattern-Interfaces, Dataclasses
  (`frozen`/`slots`/`kw_only`) für Value-Objekte, `match`/`case` für
  Outcome-Dispatch, PEP 604 Union-Syntax, `Self`, `cached_property`,
  `@overload` für Locator-API. `__init_subclass__`-Hybrid mit
  Decorator für `@context`. Keine globale `PatternRegistry` —
  Convention plus Adapter-lokale Mapping-Tabellen reichen.
