# PlatynUI Python Library — Design & Migrationsplan

<!-- Living document. Diskussionsgrundlage für die Portierung der Python-Schicht
     aus dem Altprojekt (`/home/daniel/develop/tmp/robotframework-PlatynUI`) auf
     den neuen Rust-basierten Kern. Keine Entscheidung ist final. -->

> **Status:** Diskussionsentwurf, **Revision 12**.
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
| `technology` | `RustAdapterTechnology` | +100000 (oder reject) |
| `role` (exakt) | `"Button"` | +10000 (oder reject) |
| `role` (in `supported_roles`) | `"ToggleButton" ∈ {"Button", "ToggleButton"}` | +5000 - i |
| `framework_id` | `"WPF"` | +1000 (oder reject) |
| `class_name` | `"Microsoft.Maui.Controls.Button"` | +500 (oder reject) |
| `tag_name` | (DOM-artig) | +400 (oder reject) |
| `properties[k] == v` | `{"AutomationId": re.compile("submit-.*")}` | +200 pro Match (oder reject) |
| `native_properties[k] == v` | `{"UIA.IsKeyboardFocusable": True}` | +200 pro Match (oder reject) |

Werte können `str`, `re.Pattern` oder beliebige `==`-vergleichbare Werte
sein. Höchstes Gewicht > 0 gewinnt; bei keinem Match: Fallback
(`UnknownContext` bzw. roher Adapter ohne Proxy).

### 4.2 Zwei Registries

#### `@context` — UI-Klassen-Registry

Beantwortet: *„Welche `ContextBase`-Subklasse repräsentiert diesen UiNode
aus User-Sicht?"*

```python
class Button(Control, role="Button"):
    def activate(self) -> None: ...

@context(role="Button", framework_id="WPF",
         properties={"ClassName": re.compile("MyApp\\..*PrimaryButton")})
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
                   properties={"ClassName": "MyApp.FakeButton"})
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
   dessen Tree resolved (Rust-XPath-Engine bei RustAdapter,
   MockNode-Walk bei MockAdapter, …).
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
  aber **nicht erzwungen** — symmetrisch zu Rust-`PatternId`, das
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
> bereits Rust-seitig: `PatternId` (newtype über `Arc<str>`,
> `crates/core/src/ui/identifiers.rs:82`), `UiPattern::id()` /
> `UiPattern::static_id() -> PatternId` als Pflicht-Trait-Methoden
> (`crates/core/src/ui/pattern.rs:18`), `PatternRegistry` mit
> `register`/`get`/`get_typed`/`supported`
> (`crates/core/src/ui/pattern.rs:57`) und `supported_patterns_value`
> serialisiert die IDs als String-Array für die FFI-Grenze
> (`pattern.rs:165`).
>
> **Aktuell verwendet Rust bare names** (`PatternId::from("Focusable")`,
> `PatternId::from("WindowSurface")`). Das ist mit der hier gewählten
> Reverse-DNS-Konvention noch **nicht synchron**. **TODO als Teil
> der Python-Migration:** alle `PatternId::from("…")`-Stellen in
> `crates/core/src/ui/pattern.rs` (Tests + Default-Impls), `crates/
> core/src/ui/node.rs:134` und Provider-Crates auf
> `org.platynui.patterns.<Name>` umstellen, sodass Rust- und
> Python-Identifier wörtlich übereinstimmen. `PatternId` selbst bleibt
> validierungsfrei (Convention statt Format-Check); die Konsistenz wird
> in den Mapping-Tabellen und beim Python-Import sichergestellt.

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
    (symmetrisch zu Rust-PatternId).
    """
    pattern_name: ClassVar[str]


# core/patterns/activation.py
from abc import abstractmethod
from .base import PatternBase

class Activatable(PatternBase):
    pattern_name = "org.platynui.patterns.Activatable"
    @abstractmethod
    def activate(self) -> None: ...


# core/patterns/toggle.py
class Toggleable(PatternBase):
    pattern_name = "org.platynui.patterns.Toggleable"
    @abstractmethod
    def toggle(self) -> None: ...


class HasToggleState(PatternBase):
    pattern_name = "org.platynui.patterns.HasToggleState"
    @property
    @abstractmethod
    def state(self) -> "ToggleState": ...


# core/patterns/text.py
class EditableText(PatternBase):
    pattern_name = "org.platynui.patterns.EditableText"
    @abstractmethod
    def set_text(self, value: str) -> None: ...


class Clearable(PatternBase):
    pattern_name = "org.platynui.patterns.Clearable"
    @abstractmethod
    def clear(self) -> None: ...


# core/patterns/geometry.py
class HasBounds(PatternBase):
    """Geometrische Position des Elements auf dem Screen."""
    pattern_name = "org.platynui.patterns.HasBounds"
    @property
    @abstractmethod
    def bounds(self) -> Rect: ...
    @property
    def default_click_position(self) -> Point:
        """Typisch bounds.center; Adapter überschreiben bei Bedarf."""
        return self.bounds.center


class Visibility(PatternBase):
    """Sichtbarkeits-Status (inkl. In-View-Unterscheidung)."""
    pattern_name = "org.platynui.patterns.Visibility"
    @property
    @abstractmethod
    def is_visible(self) -> bool: ...
    @property
    @abstractmethod
    def is_in_view(self) -> bool: ...


# core/patterns/state.py
class HasIsEnabled(PatternBase):
    """Aktivierbarkeits-Status; eigenständig von Activatable, weil
    ein Element `enabled` sein kann, ohne selbst aktivierbar zu sein
    (z.B. ein Container, dessen Kinder enabled sind)."""
    pattern_name = "org.platynui.patterns.HasIsEnabled"
    @property
    @abstractmethod
    def is_enabled(self) -> bool: ...


class HasIsReadonly(PatternBase):
    """Schreibschutz-Status für Editable-Elemente."""
    pattern_name = "org.platynui.patterns.HasIsReadonly"
    @property
    @abstractmethod
    def is_readonly(self) -> bool: ...


# Expandable, HasIsExpanded, Selectable, HasIsSelected, Focusable,
# HasFocus, Scrollable, HasNativeWindowHandle, HasValue, EditableValue,
# Properties, … — alle als ABC mit pattern_name im
# org.platynui.patterns.*-Namespace.
```

Die Patterns sind aus dem Altprojekt **weitgehend unverändert** zu
übernehmen (nur umbenannt: Strategy → Pattern, `strategy_name` →
`pattern_name`). Die Liste der ~20 Patterns hat sich in der Praxis
bewährt.

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
            current = self.adapter.get_pattern(patterns.HasToggleState).state
            if current == target:
                return
            self.adapter.get_pattern(patterns.Toggleable).toggle()
            wait_for(lambda last=current:
                     self.adapter.get_pattern(patterns.HasToggleState).state != last)
        raise CannotEnsureError(f"cannot set checkbox to {target}")
```

### 5.4 Namensauflösung: Convention statt globaler Registry

Patterns tragen ihren Reverse-DNS-Identifier als `ClassVar[str]`. Das
reicht als Vertrag zwischen Adapter und UI-Schicht — eine globale
`PatternRegistry` ist **nicht** nötig:

- **Rust-Adapter** (`core/adapters/rust.py`) hält eine lokale
  Mapping-Tabelle `PatternId-String → Python-ABC`:

  ```python
  # core/adapters/rust.py
  _RUST_PATTERN_MAP: dict[str, type[PatternBase]] = {
      Focusable.pattern_name: Focusable,
      WindowSurface.pattern_name: WindowSurface,
      HasBounds.pattern_name: HasBounds,
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
- **`RustAdapter`** übersetzt die `PatternId`-Strings, die
  `UiNode.supported_patterns()` aus dem Rust-Code liefert, über seine
  lokale Mapping-Tabelle (`_RUST_PATTERN_MAP`) in Python-Pattern-Klassen.
  Sobald Rust-PatternIds auf das Reverse-DNS-Format umgestellt sind
  (siehe Status-Box in §5), reduziert sich die Tabelle auf eine reine
  String→Klasse-Zuordnung — die Strings sind dann wörtlich identisch
  und es gibt keine Umbenennung mehr.

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
| `CheckBox`, `RadioButton`, `ToggleButton` | `CheckBox`, `RadioButton`, `ToggleButton` | `Toggleable`, `HasToggleState` |
| `Edit`, `Text`, `PasswordBox` | `Edit`, `Text` | `EditableText`, `HasValue`, `Clearable` |
| `ComboBox` | `ComboBox` | `Expandable`, `Selectable`, `EditableText` (editierbar) |
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
  ein `@pattern_proxy_for(role="Label", properties={...})` registriert,
  der im Match höheres Gewicht bekommt.
- **Drei-Ebenen-Fallback pro Pattern** (von spezifisch nach generisch):
  1. App/Framework-spezifischer Proxy (User-Registrierung)
  2. Default-Proxy für die Standard-Rolle (PlatynUI)
  3. Adapter-Pattern (Provider-nativ) oder generische
     Pattern-Defaults in `core/patterns/defaults.py`
     (Click-basiertes `Activatable`, Tastatur-basiertes `EditableText`,
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

> **Attribut-Namenskonvention:** Locator-Kwargs (`AutomationId=`,
> `ClassName=`, `Name=`, …) verwenden **PascalCase** und entsprechen
> 1:1 den Attributnamen, die der Adapter exposed (siehe AGENTS.md:
> *„Attribute use PascalCase"*). Das ist konsistent mit der
> XPath-Schreibweise (`//control:Button[@AutomationId='num5Button']`)
> und mit dem `properties=`-Kwarg in `@context`/`@pattern_proxy_for`.
> Python-eigene Locator-Optionen (Strategie-Modifier, z.B. `name=` als
> Convenience für `@Name=`) bleiben snake_case und sind in der
> Locator-API als reservierte Schlüsselwörter dokumentiert.

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
| `Focus` | `Focusable` + `HasFocus` | Element hat Fokus |
| `Toggle` | `Toggleable` + `HasToggleState` | Toggle-State hat sich geändert |
| `Check` / `Uncheck` / `Set Check State` | `Toggleable` + `HasToggleState` | Ziel-State erreicht |
| `Select` / `Deselect` / `Select Item` | `Selectable` + `HasIsSelected` | Selektion verifiziert |
| `Expand` / `Collapse` | `Expandable` + `HasIsExpanded` | Expand-State erreicht |
| `Set Value` / `Clear` / `Append` | `EditableText` + `HasValue` | Wert verifiziert |
| `Scroll Into View` | `Scrollable` + `IsInView`-Check | Element im Viewport |
| `Activate Window` / `Maximize Window` / `Minimize Window` / `Close Window` | `WindowSurface`-Patterns | Window-State verifiziert |
| `Get Property` / `Get Attribute` | `Properties` / direkter Attribut-Read | Wert |
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
│   │   ├── toggle.py               # Toggleable, HasToggleState
│   │   ├── text.py                 # EditableText, HasValue, Clearable
│   │   ├── expand.py               # Expandable, HasIsExpanded
│   │   ├── selection.py            # Selectable, HasIsSelected, …
│   │   ├── focus.py                # Focusable, HasFocus
│   │   ├── window.py               # WindowSurface (Activate/Close/Min/Max als Methoden)
│   │   ├── defaults.py             # Default-Implementierungen (alt: strategyimpl.py)
│   │   └── …
│   ├── devices.py                  # MouseProxy/KeyboardProxy (Wrapper über Rust-Runtime)
│   └── adapters/                   # konkrete Adapter-Implementierungen
│       ├── __init__.py
│       ├── rust.py                 # RustAdapter (default), wraps platynui_native
│       └── (jsonrpc.py, mock.py, …) # zukünftig
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
    ├── properties.py               # Get Property, Get Attribute
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
           return self.adapter.get_pattern(patterns.HasIsEnabled).is_enabled
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
    """Was die Adapter-Schicht (Rust, JSON-RPC, Mock, …) liefert."""
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

    # Attribute (Properties + Native-Properties)
    @abstractmethod
    def property_names(self) -> set[str]: ...
    @abstractmethod
    def property_value(self, name: str) -> object: ...
    @abstractmethod
    def native_property_names(self) -> set[str]: ...
    @abstractmethod
    def native_property_value(self, name: str) -> object: ...

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
(`HasBounds`, `Visibility`, `Activatable.is_enabled`, …). `Element`
(§7.1) bietet `@cached_property`-Convenience-Wrapper, die intern
`adapter.get_pattern(HasBounds).bounds` etc. aufrufen. So bleibt das
Adapter-Interface schmal und Pattern-orientiert.

**Pattern-Resolution-Reihenfolge** (siehe auch §4.3):
1. `self` ist `isinstance(pattern_type)` → `self`.
2. Adapter-internes Mapping (`_pattern_impls: dict[str, PatternBase]`) →
   gecached.
3. Adapter-spezifischer Lookup (Rust: PyO3-Call zu `UiNode.get_pattern`;
   JSON-RPC: Wire-Call) → cachen.
4. Sonst: `PatternNotSupportedError` (oder `None` bei
   `raise_exception=False`).

**`AdapterProxy`** (siehe §4 / §5.1) ist *kein* Adapter-Subtyp, sondern
eine **Komposition**: er hält einen `adapter: Adapter` und überschreibt
nur `get_pattern` (eigene Patterns zuerst, dann `adapter.get_pattern`)
plus `supported_patterns` (Vereinigung). Alle anderen Adapter-Aufrufe
delegieren transparent.

### A.5 `ContextBase`-API (`core/context.py`)

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

**Generische Property-Reads** (für Locator-`properties=`-Match und
Inspector-Zwecke):

```python
def property_names(self) -> set[str]: ...
def property_value(self, name: str) -> object: ...
def native_property_names(self) -> set[str]: ...
def native_property_value(self, name: str) -> object: ...
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
    @property
    def bounding_rectangle(self) -> Rect:
        return self.adapter.get_pattern(HasBounds).bounds

    @property
    def is_visible(self) -> bool:
        v = self.adapter.get_pattern(Visibility, raise_exception=False)
        return v.is_visible if v is not None else True

    @property
    def is_in_view(self) -> bool:
        v = self.adapter.get_pattern(Visibility, raise_exception=False)
        return v.is_in_view if v is not None else self.is_visible

    @property
    def is_enabled(self) -> bool:
        a = self.adapter.get_pattern(Activatable, raise_exception=False)
        return a.is_enabled if a is not None else True

    @property
    def is_focused(self) -> bool:
        f = self.adapter.get_pattern(Focusable, raise_exception=False)
        return f.is_focused if f is not None else False
```

Diese Properties sind die Quelle für Default-Predicates
(`is_visible`, `is_enabled`, …; siehe §A.3) und für Devices
(`MouseProxy.click(element)` liest `bounding_rectangle`).

### A.6 `@locator`-Mechanik (`core/locator.py`)

`Locator` ist eine `@dataclass`-ähnliche Builder-Klasse, die intern
einen XPath-2.0-Ausdruck für die Rust-XPath-Engine baut.

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

    # Freie Attribute (PascalCase erwartet, siehe §7.1)
    attributes: dict[str, str | re.Pattern[str]] = field(default_factory=dict)
    custom_attributes: list[str] = field(default_factory=list)  # raw Prädikate
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
4. Prädikate aus `attributes` (`@k='v'` oder `matches(@k, 'v')` für
   Regex) und `custom_attributes` (raw), mit ` and ` verbunden.
5. Suffix `[N]` aus `index`, `[position()=N]` aus `position`.
6. Default-Scope-Regel: ohne Parent → `children`; mit Parent →
   `children` falls Parent ein `Application`/`Desktop`, sonst
   `descendants`. (1:1 aus Altcode.)

**`@locator`-Decorator** ist die `Locator`-Klasse selbst (`locator =
Locator`). Verwendung:

```python
# Klassen-Default (am Page-Object)
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    pass

# Property-Variante (typisierter Child-Locator)
class CalculatorWindow(Window, role="Window", name="Rechner"):
    @property
    @locator(AutomationId="num5Button")
    def n5(self) -> Button: ...
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
             patterns.HasToggleState: ElementDescriptor[patterns.HasToggleState].convert,
             patterns.EditableText: ElementDescriptor[patterns.EditableText].convert,
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
3. `core.adapters.rust`-Import lädt die `RustAdapter`-Implementierung
   und registriert sie als Default-Technology. Andere Adapter (Mock,
   JSON-RPC) werden nur geladen, wenn explizit referenziert.
4. Die Robot-Library ist nutzungsbereit. **Es gibt keinen
   `Runtime`-Singleton in der Python-Schicht** — jeder Adapter hält
   seinen eigenen (Rust-`Runtime`-Instanz im `RustAdapter`).

**Desktop-Root** ist konzeptionell das Wurzelelement aller Adapter-
Trees. Praktisch gibt es ihn als Klasse:

```python
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    default_prefix = "control"
    # Stellt MouseProxy mit base_point=(0,0) und base_rect=Bildschirm
    # bereit, damit absolute Mouse-Operationen ohne Element möglich sind.
```

**Adapter-Bootstrap.** Welcher Adapter den Desktop-Root liefert, ist
über `core/technology.py` konfigurierbar:

```python
# core/technology.py
@dataclass(frozen=True, slots=True)
class Technology:
    name: str           # z.B. "rust", "jsonrpc", "mock"
    factory: Callable[[], Adapter]   # baut den Root-Adapter

_REGISTERED: dict[str, Technology] = {}

def register_technology(t: Technology) -> None:
    _REGISTERED[t.name] = t

def get_default_technology() -> Technology:
    name = Settings.current().technology   # default "rust"
    return _REGISTERED[name]
```

`core/adapters/rust.py` registriert sich beim Import als
`Technology(name="rust", factory=lambda: RustAdapter.create_root())`.
Der Mock-Adapter (§A.11) registriert sich nur, wenn das Mock-Modul
explizit importiert wird (kein Side-Effect aus `core/__init__.py`).
Beim Bau des Desktop-Roots ruft der Konstruktor
`get_default_technology().factory()` auf, sofern kein expliziter
`adapter=`-Parameter übergeben wurde. **Das ist die einzige
prozessweite Adapter-Registry — sie hat genau einen Zweck: den
Default-Adapter für ein neu erzeugtes Desktop-Objekt zu wählen.**

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

Die Python-Devices sind dünne Wrapper über die Rust-Profile. Sie liefern
zwei Dinge: Coordinate-Resolving relativ zur Element-BoundingBox und
Pre/Post-Verifikation (Element in View, App ready).

```python
class MouseAction(StrEnum):
    MOVE = "move"; PRESS = "press"; RELEASE = "release"
    CLICK = "click"; DOUBLE_CLICK = "double_click"

class MouseProxy(ABC):
    """Element-relativer Maus-Wrapper. Berechnet absolute Koordinaten
    aus base_point + default_click_position + Override (Point/x/y)."""
    @property @abstractmethod
    def base_point(self) -> Point: ...
    @property @abstractmethod
    def base_rect(self) -> Rect: ...
    @property
    def default_click_position(self) -> Point: return Point(0, 0)

    def before_action(self, action: MouseAction) -> None: ...
    def after_action(self, action: MouseAction) -> None: ...

    def move_to(self, pos: Point | None = None, *,
                x: int | None = None, y: int | None = None) -> Point: ...
    def press(self, *, button: MouseButton = MouseButton.LEFT,
              pos=None, x=None, y=None) -> None: ...
    def release(self, *, button=MouseButton.LEFT, pos=None, x=None, y=None) -> None: ...
    def click(self, *, button=MouseButton.LEFT, times: int = 1,
              pos=None, x=None, y=None) -> None: ...
    def double_click(self, *, button=MouseButton.LEFT, pos=None, x=None, y=None) -> None: ...

class AdapterMouseProxy(MouseProxy):
    """Standard-Implementierung: holt BoundingRect aus dem Adapter."""
    def __init__(self, adapter: Adapter, *,
                 mouse_device: MouseDevice | None = None) -> None: ...
    @property
    def base_point(self) -> Point:
        return self._adapter.get_pattern(patterns.HasBounds).bounds.top_left
    @property
    def base_rect(self) -> Rect:
        return self._adapter.get_pattern(patterns.HasBounds).bounds
    @property
    def default_click_position(self) -> Point:
        return self._adapter.get_pattern(patterns.HasBounds).default_click_position
```

**Coord-Berechnung** (`_calc_mouse_point`, 1:1 aus Altcode
`ui/core/devices/mouseproxy.py:58`):

- Leerer `Point()` → `base_point + default_click_position` (mittiger
  Default-Klick), `x`/`y` als Offsets.
- Konkretes `Point(x, y)` → absolute Koordinate (nicht relativ).
- `VirtualPoint(rel_x, rel_y)` → `pos.calc_rect(base_rect)` (Prozent-/
  Anker-basiert).

**`MouseDevice`** und **`KeyboardDevice`** sind die Low-Level-Adapter
zu den Rust-`PointerProfile`/`KeyboardProfile`. Sie kapseln Multi-
Click-Schutz, Move-Interpolation (`mouse_move_time`), Press/Release-
Delays etc. — alles über `Settings`-Felder konfigurierbar.

```python
class KeyboardProxy(ABC):
    def type_keys(self, *keys: str | Key, delay: float | None = None) -> None: ...
    def press_keys(self, *keys: str | Key) -> None: ...
    def release_keys(self, *keys: str | Key) -> None: ...
    def escape_text(self, value: str) -> str: ...
```

**Element-Integration** (analog zu Altcode `ui/element.py:211`): Die
`Element`-Klasse exposed `self.mouse` und `self.keyboard` als
property-cached Instanzen, deren `before_action` `ensure_that(
self._toplevel_parent_is_active, self._element_is_in_view)` ruft —
damit ist die Pre-Condition automatisch da, sobald jemand
`button.mouse.click()` ausführt.

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

**`EditableText` (Default):** Fokus + Clear-Sequenz (Ctrl+A, Del) +
`type_keys`.

```python
class DefaultEditableText(patterns.EditableText):
    pattern_name = patterns.EditableText.pattern_name
    def set_text(self, value: str) -> None:
        self._adapter.get_pattern(patterns.Focusable).focus()
        kb = AdapterKeyboardProxy(self._adapter)
        kb.type_keys("<Control+A>", "<Delete>")
        kb.type_keys(value)
```

**`Clearable` (Default):** Fokus + Ctrl+A + Del.

**`HasToggleState` + `Toggleable` (Default):** kein generischer
Default — diese Patterns *müssen* vom Adapter oder Proxy kommen, weil
ohne State-Read keine Verifikation möglich ist.

`core/patterns/defaults.py` enthält diese Defaults und bietet sie
**nur auf explizite Anforderung** an: Der `AdapterProxy.get_pattern`-
Lookup (siehe §A.4) erweitert sich um Stufe 4: „falls keine spezifische
Implementierung gefunden, prüfe, ob ein
`DEFAULT_PATTERN_FACTORIES[pattern_name]` existiert und instanziiere
ihn lazy". Das Mapping ist eine reine Modul-Konstante, keine globale
Registry mit Side-Effects.

### A.11 Mock-Adapter (`core/adapters/mock.py`)

Im Altprojekt nicht vorhanden — neu für die Test-Suite. Erlaubt
Python-Unit-Tests ohne Rust/Provider-Abhängigkeit.

```python
@dataclass
class MockNode:
    role: str
    name: str = ""
    class_name: str = ""
    framework_id: str = "Mock"
    bounds: Rect = field(default_factory=lambda: Rect(0, 0, 100, 30))
    properties: dict[str, object] = field(default_factory=dict)
    patterns: dict[str, PatternBase] = field(default_factory=dict)
    children: list["MockNode"] = field(default_factory=list)
    parent: "MockNode | None" = None

class MockAdapter(Adapter):
    """Adapter über einen MockNode-Tree. Patterns werden direkt am
    Node-Dict registriert; Tests können Spy-/Stub-Patterns einsetzen
    und das Verhalten der Proxies/UI-Klassen verifizieren."""
    def __init__(self, node: MockNode, *, technology: Technology = MOCK_TECHNOLOGY) -> None: ...
    # … alle Adapter-Methoden delegieren an MockNode

# Convenience-Builder
def build_tree(spec: dict) -> MockNode: ...

def mock_desktop(root_node: MockNode) -> "Desktop":
    """Baut ein `Desktop`-Context-Objekt, dessen Adapter-Resolver den
    MockNode-Tree liefert (statt den Rust-Adapter). Nutzbar als
    `parent`-Parameter für `get(...)`-Aufrufe."""
    return Desktop(
        locator=Locator(path="/."),
        adapter=MockAdapter(root_node),
    )
```

**Verwendungsbeispiel:**

```python
def test_button_activate_uses_provider_pattern_when_available():
    activate_calls = []
    button_node = MockNode(
        role="Button", name="OK",
        patterns={
            patterns.Activatable.pattern_name:
                StubActivatable(on_activate=lambda: activate_calls.append("api")),
        },
    )
    desktop = mock_desktop(MockNode(role="Desktop", children=[button_node]))
    button = desktop.get(Button, name="OK")
    button.activate()
    assert activate_calls == ["api"]
```

Konsistent mit §A.8: **kein Python-`Runtime`-Objekt**. Der Mock-
Adapter wird dem `Desktop`-Context direkt per `adapter=`-Parameter
übergeben; Children erben ihn über `ContextBase.get(...)` aus der
Parent-Chain.

**Abgrenzung zum Rust-Mock-Provider:** Der Rust-`provider-mock` liefert
einen UiTree für Integrations-Tests (Rust + Python via Maturin). Der
Python-`MockAdapter` ersetzt dagegen die *gesamte* Adapter-Schicht und
ist für reine Python-Tests gedacht (Patterns, Proxies, UI-Klassen,
Keywords). Beide existieren parallel und konkurrieren nicht.

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
   `TechnologyName`, `FrameworkId`) als freie String-Aliases
2. `core/settings.py` — als `@dataclass(frozen=True, slots=True, kw_only=True)`,
   mit `with`-Block und RF-Variablen-Brücke (siehe §9a.1)
3. `core/exceptions.py` — Hierarchie nach §9a.2 (Typo-Fix
   `PlatyUiError` → `PlatynUIError`)
4. `core/wait.py` — `wait_for` nach §9a.3
5. `core/ensure.py` — `ensure_that` mit Stage-Memo und re-entrant
   Thread-Local-Stack (siehe §9a.3); Decorator mit `ParamSpec`/
   `Concatenate` typisiert, `match`/`case` für Outcome
6. `core/weight_calculator.py` — 1:1 aus Altprojekt, `MatchCriteria`
   als Dataclass
7. `core/technology.py` — Marker + AdapterFactory-Registry
8. `core/locator.py` — neu, XPath-basiert über Rust, API nach §9a.6
   (`@overload`, Builder-Methoden geben `Self` zurück, `copy_from`-
   Vererbung)

**Ergebnis:** ~550 LOC Core-Infrastruktur, unabhängig testbar.

### Phase 2 — Adapter-Schicht

9. `core/adapter.py` — Interface
10. `core/adapter_proxy.py` — `AdapterProxy`, `PatternProxyFactory`,
    `@pattern_proxy_for` (portiert aus altem `adapterproxy.py`, nur
    umbenannt)
11. `core/patterns/` — Port aus altem `core/strategies/` (Umbenennung
    Strategy → Pattern)
12. `core/adapters/rust.py` — `RustAdapter`, der `UiNode` aus
    `platynui_native` umhüllt; mappt Rust-Patterns auf Python-Patterns
    (`FocusablePattern` → `Focusable`, `WindowSurfacePattern` → diverse
    Window-Patterns)
13. `core/devices.py` — `MouseProxy`/`KeyboardProxy` über Rust-Runtime

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
  Mapping in `RustAdapter` auf ein Python-Pattern. UI-Klassen und Proxies
  bleiben unverändert.
- Weitere Adapter-Implementierungen (JSON-RPC, …) als parallele
  `core/adapters/*.py`-Module, sobald ein Use-Case da ist.

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

### 11.4 `WeightCalculator` (114 → 114 LOC)

**Keine Vereinfachung.** Der Mechanismus ist genau richtig, wie er ist —
ein gewichtetes Multi-Kriterien-Match, das Spezialfälle elegant über
generische Fälle hebt. Wir übernehmen ihn 1:1.

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
                    │  (Py-MockAdapter ODER       │  -Integration
                    │   Rust-Mock via Maturin)    │
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

Zwei Varianten, je nach Test-Ziel:

**2a. Reine Python-Tests mit `MockAdapter` (§A.11)**
- Kein Rust-Build nötig
- Für: Pattern-ABCs, `@pattern_proxy_for`-Match-Logik, `AdapterProxy`-
  Pattern-Resolution, `ContextBase.get`, `wait_for`/`ensure_that`,
  UI-Klassen-Hybrid-Form, Keyword-Outcome-Verträge
- Stub-/Spy-Patterns werden am `MockNode` registriert

**2b. Python-Tests gegen den Rust-Mock-Provider**
- Build: `uv run maturin develop -m packages/native/Cargo.toml --features mock-provider`
- Für: PyO3-Bindings, `RustAdapter`-Pfad, end-to-end Pattern-Lookup
- Geringer Umfang, fokussiert auf Bindings-Korrektheit

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

| Test-Gegenstand                    | nextest | pytest(Py-Mock) | pytest(Rust-Mock) | RF(Rust-Mock) | RF(echt) |
|------------------------------------|:-------:|:---------------:|:-----------------:|:-------------:|:--------:|
| XPath-Parser/Evaluator             |   ✅    |       —         |         —         |       —       |    —     |
| Runtime-Orchestrierung             |   ✅    |       —         |        ✅         |       —       |    —     |
| Provider-Adapter (Rust)            |   ✅    |       —         |         —         |       —       |   ✅     |
| PyO3-Bindings                      |    —    |       —         |        ✅         |       —       |    —     |
| Pattern-ABC-Verträge               |    —    |      ✅         |         —         |       —       |    —     |
| `@pattern_proxy_for` Match-Logik   |    —    |      ✅         |         —         |       —       |    —     |
| `wait_for`/`ensure_that` Re-entry  |    —    |      ✅         |         —         |       —       |    —     |
| `ContextBase.get` Parent-Chain     |    —    |      ✅         |         —         |       —       |    —     |
| UI-Klassen (Button, Window, …)     |    —    |      ✅         |         —         |       —       |    —     |
| Keyword-Outcome-Verträge           |    —    |      ✅         |         —         |      ✅       |   ✅     |
| Locator-Syntax in RF               |    —    |       —         |         —         |      ✅       |   ✅     |
| Plattform-Provider (UIA/AT-SPI/AX) |    —    |       —         |         —         |       —       |   ✅     |
| End-to-End Page-Object             |    —    |       —         |         —         |      ✅       |   ✅     |

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

**Phase 2 (Patterns/Adapter):** pytest mit `MockAdapter`. Erst hier
existiert genug, um Pattern-Proxy-Match-Logik zu testen.

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
  `control:SupportedPatterns=["Focusable","WindowSurface"]` funktionieren.
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
   `@pattern_proxy_for(role="Label", properties={...})`-Proxy korrekt als
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

Aktuell exposed PyO3 nur `get_pattern(Focusable)`. Für
`RustAdapter.supported_patterns` brauchen wir eine generische Liste der
verfügbaren Patterns am UiNode. **Action:** in
`packages/native/src/lib.rs` eine `supported_patterns -> list[str]`
Methode ergänzen.

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

### 13.6 Rust-PatternId-Umstellung auf Reverse-DNS

Rust-`PatternId` verwendet aktuell bare names (`"Focusable"`,
`"WindowSurface"`). Im Zuge der Python-Migration umstellen auf
`org.platynui.patterns.<Name>`, damit Rust-`PatternId.as_str()` und
Python-`pattern_name` wörtlich identisch sind. Betroffene Stellen:

- `crates/core/src/ui/pattern.rs` — `FocusableAction::static_id`,
  `WindowSurfaceActions::static_id`, Test-Patterns
- `crates/core/src/ui/node.rs:134` — `PatternId::from("WindowSurface")`
- Provider-Crates, die `PatternId::from(...)` benutzen
  (Hauptkandidat: `crates/provider-windows-uia/src/node.rs`)
- Bench/Test-Fixtures

Keine API-Änderung an `PatternId` selbst (bleibt validierungsfrei) —
nur die literalen Strings.

## 14. Zusammenfassung

- **Patterns leben in Python.** Der `WeightCalculator` plus
  `@context` und `@pattern_proxy_for` ist der Kern der Erweiterbarkeit
  und wird aus dem Altprojekt übernommen (dort hießen sie Strategies).
- **Batterien inklusive:** PlatynUI liefert für alle gängigen
  UI-Rollen fertige UI-Klassen und Default-Proxies mit (siehe §5a).
  Test-Projekte sind ohne eigenen Proxy-Code produktiv; App-Spezialfälle
  überschreiben gezielt über `framework_id`/`class_name`/`properties`.
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
