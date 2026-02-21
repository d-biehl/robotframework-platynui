# Architekturkonzept PlatynUI Runtime

> Leb- Tests nutzen explizite Factory-Zugriffe für Mock-Provider: Mock-Provider registrieren sich **nicht** automatisch im Inventory. In Rust-Tests wird die Factory direkt verwendet (z. B. `MOCK_PROVIDER_FACTORY.create()`), in Python-Tests werden explizite Handles genutzt (`Runtime.new_with_providers([MOCK_PROVIDER])`). OS-spezifische Provider (AT-SPI, Windows UIA, macOS AX) registrieren sich dagegen automatisch, wenn sie gelinkt werden.ndes Dokument: Dieses Konzept sammelt aktuelle Ideen und Annahmen. Während der Implementierung passen wir Inhalte fortlaufend an, ergänzen Erkenntnisse und korrigieren Irrtümer.

## 1. Einleitung & Ziele
- PlatynUI soll eine plattformübergreifende UI-Automationsbibliothek bereitstellen, deren Kern eine konsistente Sicht auf native UI-Bäume (UIA, AT-SPI2, macOS AX, …) bildet.
- Die Runtime abstrahiert Plattform-APIs zu einem normalisierten Knotenbaum, der per XPath durchsucht wird und über Patterns beschreibende Fähigkeiten (keine direkten Aktionen) bereitstellt. Fokuswechsel und Fenstersteuerung bleiben die einzigen Laufzeitaktionen.
 - Jede UiNode kann zusätzlich eine vom Anwendungsentwickler gesetzte, stabile Kennung `Id` tragen. Diese ist inhalt‑ und sprachunabhängig, optional und dient als dauerhafte Selektor‑Basis über Prozessneustarts hinweg (im Gegensatz zu `RuntimeId`, die nur für die Lebensdauer des nativen Elements stabil ist).
- Dieses Dokument beschreibt die Architektur, zentrale Module und offene Fragen als Grundlage für Implementierung und Diskussion.

## 2. Architekturüberblick
### 2.1 Crate-Landschaft
```
crates/
├─ core                      # Gemeinsame Traits/Typen (UiTreeProvider, DeviceProvider, Patterns) – Crate `platynui-core`
├─ xpath                     # XPath-Evaluator und Parser-Hilfen – Crate `platynui-xpath`
├─ runtime                   # Runtime, Provider-Registry, XPath-Pipeline, Fokus/Fenster-Aktionen – Crate `platynui-runtime`
├─ server                    # JSON-RPC-Server als Frontend zur Runtime – Crate `platynui-server`
├─ platform-windows          # Geräte und sonstige Windows-spezifische Ressourcen – Crate `platynui-platform-windows`
├─ provider-windows-uia      # UiTreeProvider auf Basis von UI Automation – Crate `platynui-provider-windows-uia`
├─ platform-linux-x11        # Geräte für Linux/X11 – Crate `platynui-platform-linux-x11`
├─ provider-atspi            # UiTreeProvider auf Basis von AT-SPI2 (X11) – Crate `platynui-provider-atspi`
├─ platform-macos            # Geräte für macOS – Crate `platynui-platform-macos`
├─ provider-macos-ax         # UiTreeProvider auf Basis der macOS Accessibility API – Crate `platynui-provider-macos-ax`
├─ platform-mock             # Mock-Geräte und Infrastruktur – Crate `platynui-platform-mock`
├─ provider-mock             # Mock-UiTreeProvider (statisch/skriptbar) – Crate `platynui-provider-mock`
├─ provider-jsonrpc          # Referenz-Adapter für externe JSON-RPC-Provider (optional) – Crate `platynui-provider-jsonrpc`
└─ cli                       # Kommandozeilenwerkzeug – Crate `platynui-cli`

apps/
└─ inspector                # GUI-Inspector (falls als App ausgelagert) – Crate `platynui-inspector`

docs/
├─ architekturkonzept_runtime.md                # Architekturkonzept (dieses Dokument)
├─ architekturkonzept_runtime_umsetzungsplan.md # Aufgabenplan
├─ patterns.md                                   # Pattern-Spezifikation (Entwurf)
└─ provider_checklist.md                         # Provider-Checkliste (Entwurf)
```
Plattform-Crates bündeln Geräte und Hilfen je OS; Provider-Crates liefern den UiTreeProvider. Beide greifen auf die gemeinsamen Traits im `crates/core` zurück. Jede Plattform implementiert `PlatformModule::initialize()`; die Runtime ruft diese Methode beim Start genau einmal auf (z. B. richtet Windows hier Per-Monitor-V2-DPI-Awareness ein), bevor Geräte oder Provider registriert werden.

### 2.2 Registrierungs- und Erweiterungsmodell
- `crates/core` definiert aktuell Marker-Traits wie `PlatformModule` und `UiTreeProviderFactory`. Weitere Erweiterungspunkte (`DeviceProviderFactory`) sind vorgesehen, aber noch nicht umgesetzt; solange diese Traits fehlen, dokumentieren wir sie hier ausdrücklich als geplante Ergänzungen. Alle Erweiterungen exportieren sich über ein `inventory`-basiertes Registrierungs-Makro. Die Runtime instanziiert ausschließlich über diese Abstraktionen und kennt keine konkreten Typen.
- Die Runtime nutzt Inventory-Registrierungen (`register_platform_module!`, `register_provider!`) als Mechanismus, lädt produktive Plattform-/Provider-Crates jedoch nicht mehr selbst. Stattdessen binden konsumierende Anwendungen (CLI, Python‑Extension) die gewünschten Plattform-/Provider‑Crates per `cfg(target_os = …)` ein. So bleiben Unit‑Tests unabhängig und können explizit den Mock verlinken. Eine dynamische Nachladefunktion ist derzeit nicht vorgesehen; zukünftige Erweiterungen greifen direkt auf denselben Mechanismus zurück. Eine Laufzeitauswahl zwischen mehreren Plattformen findet nicht statt. Perspektivisch für Linux: Sobald neben X11 auch Wayland unterstützt wird, bündelt ein Vermittlungs‑Crate `platynui-platform-linux` beide Untervarianten und entscheidet anhand der Session‑Umgebung (`$XDG_SESSION_TYPE`, Availability‑Checks) zur Laufzeit, welches der bereits kompilierten Module (`platynui-platform-linux-x11` bzw. `platynui-platform-linux-wayland`) aktiv genutzt wird.
- Welche Module eingebunden werden, entscheidet der Build: Über `cfg`-Attribute (z. B. `#[cfg(target_os = "windows")]`) binden wir die passenden Plattform- und Provider-Crates ein. Die Runtime führt lediglich die bereits kompilierten Registrierungen zusammen; es findet keine Plattform-Auswahl zur Laufzeit statt.
- Provider laufen entweder **in-process** (Rust-Crate) oder **out-of-process** (JSON-RPC). Für externe Provider stellt `platynui-provider-jsonrpc` Transport- und Vertragsebene bereit: Eine schlanke JSON-RPC-Spezifikation beschreibt den Mindestumfang (`initialize`, `getNodes`, `getAttributes`, `getSupportedPatterns`, optional `ping`). Die Runtime hält dazu einen JSON-RPC-Client, der den Provider zunächst über `initialize` nach Basismetadaten (Version, Technologiekennung, RuntimeId-Schema, Heartbeat-Intervalle, optionale vendor-spezifische Hinweise) abfragt und anschließend `getNodes(parentRuntimeId)` nutzt, um Kinder eines Parents (Desktop, App, Container) zu laden. Provider senden Baum-Events (`$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`) zur Synchronisation. Der eigentliche Provider-Prozess liefert ausschließlich die UI-Baum-Daten und bleibt unabhängig vom Runtime-Prozess. Sicherheitsschichten (Pipe-/Socket-Namen, ACLs/Tokens) werden auf Transportebene definiert. Komfortfunktionen wie Kontext-Abfragen (`evaluate(node, xpath, options)`) liegen vollständig bei der Runtime; Provider liefern ausschließlich Rohdaten.
- Tests nutzen das gleiche Registrierungsmodell: In Testmodulen werden die Mock‑Crates explizit gelinkt (z. B. `const _: () = { use platynui_platform_mock as _; use platynui_provider_mock as _; };`), sodass die Inventory‑Registrierungen garantiert im Test‑Binary vorhanden sind.

- Zusätzliche Verlinkungshilfe: Das Hilfscrate `platynui-link` stellt Makros zur Verlinkung bereit: `platynui_link_providers!()` (Feature‑gesteuert Mock vs. OS) und `platynui_link_os_providers!()` (explizit OS) vereinheitlichen die Einbindung in Anwendungen (CLI, Python‑Native) und halten die Runtime frei von Auto‑Verlinkung.
### 2.3 Laufzeitkontext
- Runtime läuft lokal, verwaltet Provider-Instanzen (nativ oder JSON-RPC) und agiert als Backend für CLI/Inspector.
- `crates/server` (Crate `platynui-server`) exponiert optional eine JSON-RPC-2.0-Schnittstelle (Language-Server-ähnlich) für Remote-Clients.
- Build-Targets und `cfg`-Attribute legen fest, welche Plattform-/Providerkombinationen in einem Artefakt enthalten sind.

### 2.4 Tests: Provider‑Injektion & Fixtures
English summary: Tests construct the Runtime explicitly via `Runtime::new_with_factories_and_platforms(...)` and inject mock platform devices. This avoids global inventory discovery and keeps tests deterministic. We provide small `rstest` fixtures per scenario (mock, UIA on Windows, etc.).

Lifecycle note (Drop): The Runtime calls `shutdown()` automatically in its `Drop` implementation. Explicit `shutdown()` remains available and is idempotent; calling it more than once (including via `Drop`) has no side effects. Providers must implement `UiTreeProvider::shutdown()` to release resources; the Runtime guarantees to invoke it during shutdown.

### 2.4 XPath‑Auswertung (Streaming & Normalisierung)

Die XPath‑Engine arbeitet grundsätzlich streaming, d. h. Teilergebnisse werden sofort weitergereicht und Prädikate früh ausgewertet. Die (laut XPath 2.0) vorgeschriebene Normalisierung „Dokumentreihenfolge + Duplikate entfernen“ ist explizit in zwei IR‑Operationen aufgeteilt:

- `EnsureDistinct`: entfernt Duplikate, bewahrt die Reihenfolge; als Cursor implementiert und damit vollständig streaming.
- `EnsureOrder`: stellt die Dokumentreihenfolge her. Der Cursor reicht monotone Eingaben direkt durch, repariert einfache Inversionen lokal und fällt nur bei echter Unordnung auf Puffern+Sortieren zurück.

Emissionsregeln im Compiler (konservativ, spezifikationskonform):

- Forward‑Achsen `child`, `self`, `attribute`, `namespace`: keine Normalisierung.
- Forward‑Achsen `descendant`, `descendant-or-self`, `following`, `following-sibling`: `EnsureDistinct`.
- Reverse‑Achsen `parent`, `ancestor*`, `preceding*`: `EnsureDistinct` + `EnsureOrder`.
- Beliebige Teilausdrücke (`PathExprStep`) und Vereinigungs-/Schnitt-/Differenzmengen werden vor dem nächsten Schritt normalisiert.

Zur Duplikatvermeidung an der Quelle minimieren wir Kontexte vor bestimmten Achsen (z. B. `descendant*`, `following*`) konservativ: überlappende Kontexte werden entfernt, ohne unsortierte Eingaben fälschlich zu verwerfen. Dadurch entfällt in vielen Fällen die Notwendigkeit nachträglicher Normalisierung und erste Ergebnisse erscheinen sofort.

- Konstruktoren für Tests:
  - `Runtime::new_with_factories(factories)`: baut eine Runtime ausschließlich aus den übergebenen Provider-Factories (keine Inventory-Suche).
  - `Runtime::new_with_factories_and_platforms(factories, PlatformOverrides)`: wie oben, zusätzlich mit expliziten Plattform-Overrides (`HighlightProvider`, `ScreenshotProvider`, `PointerDevice`, `KeyboardDevice`).
- Zentrale Testhilfe (`crates/runtime/src/test_support.rs`):
  - `runtime_with_factories_and_mock_platform(&[&FACTORY, ...]) -> Runtime` injiziert immer die Mock-Geräte und nimmt die Provider-Factories entgegen.
- `rstest`-Fixtures im `platynui-runtime`-Crate:
  - `#[fixture] fn rt_runtime_platform() -> Runtime { return rt_with_pf(&[]); }` – nur Mock-Geräte (ohne Provider), für reine Plattformtests.
  - `#[fixture] fn rt_runtime_stub() -> Runtime { return rt_with_pf(&[&RUNTIME_FACTORY]); }` – Laufzeit-Stub-Provider.
- `#[fixture] fn rt_runtime_focus() -> Runtime { return rt_with_pf(&[&FOCUS_FACTORY]); }` – Fokus-spezifischer Stub.
  - Plattform-/Provider-spezifische Fixtures (z. B. UIA auf Windows) werden bei Bedarf in konsumierenden Crates (CLI, Integrationstests) definiert, nicht in `platynui-runtime`.
  - Motivation: Keine stillen Nebenwirkungen durch Inventory, kürzere und stabilere Tests, klarer Arrange-Block in den Tests (Fixture-Namen einheitlich: `rt_runtime_*`).

### 2.5 Host‑Resolver & FFI (Ergänzung 2025‑10‑21)
EN: This section documents the host‑side NodeResolver and the owned evaluation stream designed for FFI bindings.

- NodeResolver (Runtime): Über `EvaluateOptions::with_node_resolver(...)` kann die Runtime Kontextknoten anhand ihrer `RuntimeId` vor der Auswertung re‑resolven. Bei fehlendem Knoten wird ein spezifischer Fehler (`ContextNodeUnknown`) gemeldet; Providerfehler werden durchgereicht.
- Owned Evaluation Streams: `Runtime::evaluate_iter_owned(...)` liefert einen owneden Iterator (`EvaluationStream`), der keine geliehenen Slices/Strings referenziert. Er eignet sich für FFI‑Bindings (z. B. Python), die Ergebnisse über Iterator‑Protokolle konsumieren möchten.
- Python‑Binding: Der `EvaluationIterator` im Paket `platynui_native` baut auf dem owned Stream auf und liefert `UiNode`/`EvaluatedAttribute`/native Werte (None/bool/int/float/str/Point/Size/Rect/Array/Dict).
- Desktop‑Fallback: Falls kein `DesktopInfoProvider` registriert ist, erzeugt die Runtime einen Fallback‑Desktop (Bounds/OS‑Infos), um Diagnose‑Kommandos (CLI/Python) weiterhin zu ermöglichen.

## 3. Datenmodell & Namespaces
### 3.1 Knoten- & Attributmodell
- **`UiNode`-Trait:** Provider stellen ihren UI-Baum als `Arc<dyn UiNode>` bereit. Das Trait kapselt ausschließlich Strukturinformationen, alles weitere erfolgt über Attribute bzw. Patterns:
  ```rust
  use std::any::Any;
  use std::sync::{Arc, Weak};
  pub trait UiNode: Send + Sync {
      fn namespace(&self) -> Namespace;
      fn role(&self) -> &str;                // z. B. "Window", "Button", "ListItem"
      fn name(&self) -> String;  //
      fn runtime_id(&self) -> &RuntimeId;    // stabil pro Lebensdauer
      fn parent(&self) -> Option<Weak<dyn UiNode>>;
      fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_>;
      fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_>;
      fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>>;
      fn supported_patterns(&self) -> Vec<PatternId>;
      fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>>;
      fn invalidate(&self);
  }

  impl dyn UiNode {
      pub fn pattern<P>(&self) -> Option<Arc<P>>
      where
          P: UiPattern + 'static,
      {
          // delegiert an pattern_by_id + Downcast (siehe platynui-core)
      }
  }

  pub trait UiAttribute: Send + Sync {
      fn namespace(&self) -> Namespace;
  fn name(&self) -> String;        // PascalCase
      fn value(&self) -> UiValue;    // lazily ermittelter Wert
  }

  pub trait UiPattern: Any + Send + Sync {
      fn id(&self) -> PatternId;
      fn static_id() -> PatternId
      where
          Self: Sized;
      fn as_any(&self) -> &dyn Any;
  }

  pub struct PatternError { /* message, Display + Error */ }

  pub trait FocusablePattern: UiPattern {
      fn focus(&self) -> Result<(), PatternError>;
  }

  pub trait WindowSurfacePattern: UiPattern {
      fn activate(&self) -> Result<(), PatternError>;
      fn minimize(&self) -> Result<(), PatternError>;
      fn maximize(&self) -> Result<(), PatternError>;
      fn restore(&self) -> Result<(), PatternError>;
      fn close(&self) -> Result<(), PatternError>;
      fn move_to(&self, position: Point) -> Result<(), PatternError>;
      fn resize(&self, size: Size) -> Result<(), PatternError>;
  }

  ```
  Kinder- und Attributlisten werden als `Box<dyn Iterator<...> + Send + '_>` zurückgegeben. Provider können eigene Iterator-Typen verwenden, solange sie das Trait erfüllen. Die Laufzeit übernimmt keine Vorab-Materialisierung, sondern ruft `UiAttribute::value()` nur bei Bedarf auf.
- **Navigation:** Das Extension-Trait `UiNodeExt` stellt Komfortfunktionen wie `parent_arc()`, `ancestors()` oder `top_level_or_self()` bereit. Damit lassen sich Vorfahren traversieren oder Patterns gezielt finden, z. B. `node.ancestor_pattern::<WindowSurfacePattern>()`. `parent_arc()` blendet den Desktop-Knoten automatisch aus – erkannt wird er über feste Eigenschaften (`RuntimeId = "platynui:Desktop"`, Rolle `Desktop`, kein Parent).
- **Attribute statt Methoden:** Informationen wie `Technology`, Sichtbarkeits- oder Geometriedaten werden ausschließlich als Attribute bereitgestellt. Welche Felder vorhanden sind, ergibt sich aus den gemeldeten Patterns und der jeweiligen Plattform. Das Trait liefert nur Struktur- und Navigationsinformationen; Clients greifen über `UiNode::attribute(...)` oder die XPath-Ausgabe darauf zu. Für konsistente Benennungen stellt `platynui-core::ui::attribute_names::<pattern>::*` Konstanten bereit.
- **Pattern-Zugriff:** `UiPattern` ist das gemeinsame Basistrait für Runtime-Aktionen (`Any + Send + Sync`). Provider hinterlegen ihre Instanzen in einer Registry (z. B. `PatternRegistry` aus `platynui-core`, basierend auf `HashMap<PatternId, Arc<dyn UiPattern>>` plus Erfassungsreihenfolge) und liefern sie über `UiNode::pattern::<FocusablePattern>()`. `supported_patterns()` und `pattern::<T>()` müssen konsistent sein: Ein Pattern taucht nur in der Liste auf, wenn auch eine Instanz bereitsteht. Aktionen wie `FocusablePattern::focus()` oder `WindowSurfacePattern::maximize()` geben `Result<_, PatternError>` zurück, sodass Fehler sauber an Clients propagiert werden. Reine Lese-Informationen bleiben Attribute ohne zusätzliche Runtime-Traits.
- **Lazy-Erkennung:** `PatternRegistry::register_lazy` erlaubt es, teure Plattform-Checks (z. B. `GetCurrentPattern` unter UIAutomation) erst beim ersten Zugriff auszuführen. Die Registry cached das Ergebnis und ergänzt `supported_patterns()` sowie das `SupportedPatterns`-Attribut automatisch nur dann, wenn die Probe erfolgreich war. Der Mock-Provider demonstriert dieses Verhalten am `Focusable`-Pattern.
- **Lazy Modell:** Die Runtime fordert Attribute/Kinder immer on-demand an. Provider können intern cachen, aber die Schnittstelle zwingt keine Vorab-Materialisierung.
- **Vertragsprüfung:** `platynui-core` stellt mit `validate_control_or_item(node)` einen Hilfsprüfer bereit, der lediglich prüft, ob `SupportedPatterns` keine Duplikate enthält. Weitere Attribut- oder Pattern-Prüfungen verbleiben bei Provider- oder Pattern-spezifischen Tests.
- **`UiValue`:** Typisiert (String, Bool, Integer, Float, strukturierte Werte wie `Rect`, `Point`, `Size`). Für strukturierte Werte erzeugt die Runtime/XPath‑Ebene zusätzliche Alias‑Attribute (`Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`), damit Abfragen simpel bleiben. Provider sollen diese Aliase nicht selbst liefern.
- **Namespaces:**
  - `control` (Standard) – Steuerelemente.
  - `item` – Elemente innerhalb von Containern (ListItem, TreeItem, TabItem).
  - `app` – Applikations-/Prozessknoten.
  - `native` – Technologie-spezifische Rohattribute.
- **Standardpräfix:** `control` wird als Default registriert. Ausdrücke ohne Präfix beziehen sich nur auf Steuerelemente; `item:` oder ein Wildcard-Namespace erweitern den Suchraum.
- **Desktop-Zusammensetzung:** Plattform-Crates liefern eine `DesktopInfo` (über einen `DesktopInfoProvider`), die Auflösung, Monitorliste, Betriebssystemdaten usw. enthält. Die Registrierung erfolgt – analog zu den UiTreeProvidern – per `inventory`; die Runtime verwendet den zuerst registrierten Provider, baut daraus den Desktop-Dokumentknoten (XPath-Wurzel) und stellt die Metadaten über `Runtime::desktop_info()` bereit. Existiert noch kein Provider (z. B. in frühen Portierungsphasen), erzeugt die Runtime einen Fallback-Datensatz mit generischen Werten („Fallback“-Technologie, Bounds 1920×1080, leere Monitorliste), sodass Werkzeuge wie `platynui-cli info` lauffähig bleiben. UiTreeProvider liefern lediglich ihren technologischen Baum (Anwendungen, Fenster, Controls) und geben mit `UiTreeProvider::get_nodes(parent)` jene Knoten zurück, die unterhalb eines vom Runtime-Host bereitgestellten Parents eingehängt werden sollen. Idealerweise stellen Provider zwei Sichten bereit:
  1. **Flache Sicht:** Unterhalb des Desktop-Dokumentknotens hängen alle Top-Level-Controls (z. B. Fenster, Dialoge, Container) direkt im Standard-Namespace `control`. XPath-Ausdrücke wie `/*` oder `//control:Window` erfassen damit den vollständigen Bestand, unabhängig von Applikationsgrenzen.
  2. **Gruppierte Sicht:** Dieselben Controls erscheinen zusätzlich als Kinder zugehöriger `app:Application`-Knoten, sodass Abfragen wie `app:Application[@Name='Studio']//control:Window` gezielt nach Anwendungszuordnung filtern können. Provider sorgen dafür, dass jedes Fenster/Control genau den Anwendungen zugeordnet wird, denen es nativ angehört (z. B. über Prozess- oder Accessibility-Handles).
  Falls eine Technologie nur eine der beiden Sichten sinnvoll abbilden kann, darf sie sich auf diese Variante beschränken. Ein kurzer Hinweis, wie Anwendungen alternativ identifiziert werden (z. B. über Attribute oder Filter), genügt. Langfristig soll eine Provider-Konfiguration erlauben, einzelne Sichten zu aktivieren bzw. abzuwählen (z. B. für ressourcenschonende Minimal-Setups). Alias-Knoten behalten dieselbe `RuntimeId` und liefern eindeutige Ordnungsschlüssel, damit XPath-Dokumentordnung stabil bleibt.
- **Fehlerbehandlung:** Provider dürfen Backend-Fehler in Attributewerten reflektieren (z. B. `UiValue::Null`). Die Runtime konvertiert Fehler nicht in Panics, sondern propagiert sie an den Client.

### 3.2 Pflichtattribute & Normalisierung
- **Attribute & Normalisierung:** Provider liefern Attribute entsprechend der eigenen Technologie und den gemeldeten Patterns. Übliche Felder wie `Role`, `RuntimeId`, `Bounds`, `Technology` oder `Name` sollten weiterhin verfügbar sein, damit XPath-Abfragen und Tools damit arbeiten können. `SupportedPatterns` dient als deklarative Liste und darf keine Duplikate enthalten.
- **Rollen & PascalCase:** Provider übersetzen native Rollen (`UIA_ButtonControlTypeId`, `ATSPI_ROLE_PUSH_BUTTON`, `kAXButtonRole`) in PascalCase (`Button`). Dieser Wert erscheint sowohl als lokaler Name (`control:Button`) als auch im Attribut `Role`. Die Originalrolle wird zusätzlich als `native:Role` abgelegt.
- **ActivationTarget:** Wird dieses Pattern gemeldet, muss `ActivationPoint` (absoluter Desktop-Koordinatenwert) vorhanden sein. Native APIs (`GetClickablePoint`, `Component::get_extents`, `AXPosition`) haben Vorrang; gibt es keine dedizierte Funktion, dient das Zentrum von `Bounds` als Fallback. Optional kann `ActivationArea` ein erweitertes Zielrechteck liefern. `ActivationPoint`/`ActivationArea` liegen im Namespace des Elements (`control` oder `item`).
- **Anwendungsbereitschaft:** Der Status `AcceptsUserInput` wird über das `WindowSurface`-Pattern ermittelt (z. B. Windows `WaitForInputIdle`, andernorts best effort). Provider können zusätzlich ein Attribut `window:AcceptsUserInput` bereitstellen; bei Unkenntnis bleibt es leer.
- **RuntimeIds:** Jede ID besteht aus einem Präfix und dem eigentlichen Wert, getrennt durch einen Doppelpunkt (`prefix:value`). Das Präfix kennzeichnet eindeutig den Provider bzw. die Technologie (`uia`, `atspi`, `ax`, `mock`, ...); der nachgestellte Teil bleibt dem Provider überlassen (z. B. native Handles, Hashes). Fehlt eine native ID, erzeugt der Provider einen deterministischen Wert, der während der Lebensdauer des Elements stabil bleibt. Der Desktop-Knoten reserviert das Präfix `platynui` und nutzt die feste ID `platynui:Desktop`.

### 3.3 Ereignis-Fähigkeiten & Invalidierung
- **Descriptor-Fähigkeiten:** `ProviderDescriptor` erhält ein Feld `event_capabilities`, das beschreibt, welchen Umfang an Ereignissen ein Provider liefern kann. Derzeit planen wir vier Stufen: `None` (keine Events verfügbar, Runtime muss pollend neu materialisieren), `ChangeHint` (Provider signalisiert "irgendetwas hat sich verändert" – Runtime löst daraufhin einen gezielten Refresh für den betroffenen Parent bzw. eine Vollabfrage aus), `Structure` (Strukturereignisse mit Parent/RuntimeId, Runtime kann betroffene Zweige selektiv behandeln) und `StructureWithProperties` (zusätzlich Property-Änderungen, z. B. Zustände oder Attribute). Das Feld ist ein Bitset, damit Provider mehrere Stufen kombinieren können; fehlende Fähigkeiten dokumentieren wir explizit.
- **Runtime-Strategie:** Die Runtime entscheidet anhand der gemeldeten Fähigkeit, ob sie weiterhin neu materialisieren muss oder gezielt `UiNode::invalidate()` aufruft. Bei `ChangeHint` invalidiert sie mindestens den Parent-Knoten und fragt bei Bedarf dessen Kinder neu ab. Bei `Structure`/`StructureWithProperties` werden exakt die betroffenen Knoten invalide gesetzt (bzw. entfernt oder hinzugefügt) und erst bei der nächsten Abfrage lazily erneut geladen. `TreeInvalidated` bleibt der Fallback für drastische Änderungen (z. B. Provider-Neustart) und führt zu einem vollständigen Reload des Provider-Baums.
- **Runtime-Strategie:** Die Runtime hält pro Provider einen eigenen Snapshot der zuletzt abgefragten Knoten. Anhand der gemeldeten Fähigkeit entscheidet sie, ob sie diesen Snapshot pollend erneuert (`None`), nur auf einen allgemeinen Änderungs-Hinweis reagiert (`ChangeHint`) oder gezielt auf Strukturevents wartet (`Structure`/`StructureWithProperties`). Bei `ChangeHint` invalidiert sie mindestens den Parent-Knoten und fragt bei Bedarf dessen Kinder neu ab. Bei `Structure`/`StructureWithProperties` werden exakt die betroffenen Knoten invalide gesetzt (bzw. entfernt oder hinzugefügt) und erst bei der nächsten Abfrage lazily erneut geladen. `TreeInvalidated` bleibt der Fallback für drastische Änderungen (z. B. Provider-Neustart) und führt zu einem vollständigen Reload des Provider-Baums.
- **Implementierungsverantwortung:** Provider müssen `UiNode::invalidate()` so implementieren, dass gecachte Daten (Kindlisten, Attribute, Pattern-Objekte) verworfen werden und beim nächsten Zugriff frisch aus der nativen API kommen. Ist eine Invalidation technisch nicht möglich, muss der Provider die entsprechende Fähigkeit im Descriptor deaktivieren; die Runtime fällt dann automatisch auf Vollabfragen zurück.


## 4. Pattern-System & Fähigkeiten
- Elemente deklarieren reine Capability-Patterns über `SupportedPatterns` im jeweiligen Namespace (`control:SupportedPatterns` oder `item:SupportedPatterns`). Der ausführliche Entwurf liegt in `docs/patterns.md` und bleibt diskutierbar.
- Patterns verhalten sich wie Traits: Sie beschreiben zusätzliche Attribute (z. B. `TextContent`, `Selectable`, `StatefulValue`, `ActivationTarget`) und können beliebig kombiniert werden. Die Runtime stellt keine generischen Aktions-APIs mehr bereit.
- Ausnahmen: Fokuswechsel (`Focusable` → `focus()`) und Fenstersteuerung (`WindowSurface` → `activate()`, `maximize()`, …) laufen über das `WindowSurface`-Pattern und greifen intern auf die jeweiligen Plattform-APIs zu.
- Hilfstypen im Core (`FocusableAction`, `WindowSurfaceActions`) kapseln die Laufzeitaktionen als Closure-basierte Implementierungen und dienen sowohl Tests als auch späteren Runtime-Registrierungen.
- `ActivationTarget` liefert absolute Desktop-Koordinaten für Standard-Klickpositionen; Provider müssen Koordinaten und Flächen immer im Desktop-Bezugssystem melden, damit Geräte-/Highlight-Komponenten ohne zusätzliche Transformation arbeiten können.
- Die aktuelle Mapping-Tabelle zwischen Patterns und nativen APIs (UIA, AT-SPI2, AX) liegt in `docs/patterns.md` und wird gemeinsam mit den Providerteams gepflegt.
- Hinweis zur Terminologie: Patterns definieren keinerlei Event-Mechanik; Änderungen werden ausschließlich über aktualisierte Attribute sichtbar. Tree- oder Provider-Ereignisse (z. B. `NodeAdded`) existieren weiterhin zur Synchronisation der Runtime, sind aber von den Pattern-Spezifikationen getrennt.
- Clients entscheiden, wie sie mit den gelieferten Informationen interagieren (z. B. Maus-/Tastatursimulation, Gesten). Dadurch bleiben dieselben Möglichkeiten erhalten, die ein Mensch vor dem Bildschirm hat.

## 5. UiTreeProvider & Plattformlayer
- `crates/core` stellt Traits und Caching-Hilfen (`UiTreeProvider`, `ProviderDescriptor`, `ProviderEvent`, `UiTreeProviderFactory`) bereit. `ProviderDescriptor` beschreibt eine Implementierung (`id`, Anzeigename, `TechnologyId`, `ProviderKind` = `Native` oder `External`). `UiTreeProviderFactory::create()` liefert eine `Arc<dyn UiTreeProvider>`-Instanz; zusätzliche Ressourcen werden nicht übergeben. Der Provider verantwortet ausschließlich die Anwendungsebene (`app:`) sowie `control:`/`item:`-Knoten und liefert diese über `UiTreeProvider::get_nodes(parent)` jeweils für den angegebenen Parent zurück; die Runtime kombiniert sie mit der Plattform-Desktop-Node.
- `crates/runtime` ergänzt einen `ProviderRegistry`, der alle via `inventory` registrierten Factories einsammelt, nach Technologie gruppiert und Instanzen erzeugt. Die Registry bietet APIs, um passende Provider je Technologie zu ermitteln (z. B. erster passender Provider oder alle registrierten Varianten).
- Ebenfalls neu ist der `ProviderEventDispatcher`: eine Fan-Out-Komponente, die Provider-Ereignisse synchron an registrierte Sinks weiterleitet. Die Runtime hängt sich nicht mehr direkt an den Dispatcher, sondern setzt pro Provider einen kleinen `RuntimeEventListener`, der (a) das jeweilige Snapshot-Fragment als „dirty“ markiert, (b) ggf. betroffene `UiNode`-Instanzen invalidiert und (c) das Ereignis anschließend an den Dispatcher weiterreicht. Provider registrieren diesen Listener über `UiTreeProvider::subscribe_events(listener)`; externe Provider senden analoge JSON-RPC-Notifications, die der Adapter in `ProviderEvent`-Strukturen übersetzt.
- `ProviderEventKind` bildet die Synchronisationsereignisse ab (`NodeAdded`, `NodeUpdated`, `NodeRemoved`, `TreeInvalidated`). Die Runtime führt die Events in einer zentralen Pipeline zusammen; Provider melden neue Knoten immer inklusive vollständiger `UiNode`-Instanz. Weitere Konsumenten (z. B. CLI, Inspector) können sich über `Runtime::register_event_sink` einklinken und erhalten identische Events.
- Event-fähige Provider lösen nur noch gezielte Aktualisierungen aus: Beim nächsten XPath-Aufruf werden ausschließlich „dirty“ Snapshots neu aufgebaut. Provider ohne Events bleiben bei der bisherigen Strategie („Full Refresh vor Abfrage“).
- Registrierungen erfolgen über die neuen Makros `register_provider!(&FACTORY)` bzw. `register_platform_module!(&MODULE)`. Beide Makros hängen statische Einträge an eine `inventory`-Sammlung; Hilfsfunktionen (`provider_factories()`, `platform_modules()`) erlauben es der Runtime, zur Laufzeit alle registrierten Erweiterungen zu enumerieren. **Wichtig**: OS-spezifische Provider (AT-SPI, Windows UIA, macOS AX) registrieren sich automatisch; Mock-Provider (`platynui-provider-mock`, `platynui-platform-mock`) registrieren sich **nicht** und sind nur über explizite Factory-Handles verfügbar (siehe Testrichtlinien). Die Runtime nutzt den `ProviderRegistry`, um die registrierten Factories je Technologie zu gruppieren.
- Plattform-spezifische Helfer implementieren das Trait `PlatformModule` (Methoden `name()` und `initialize() -> Result<(), PlatformError>`). Darüber stellen Plattform-Crates ihre Geräte-Bündel bereit und können beim Programmstart deterministisch initialisiert werden.
- Plattform-Crates liefern OS-spezifische Infrastruktur (Handles, D-Bus/COM-Brücken, Geräte): `crates/platform-windows` (Crate `platynui-platform-windows`, optional `platynui-platform-windows-core`), `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`), optional `platynui-platform-linux-wayland`, `crates/platform-macos` (Crate `platynui-platform-macos`), `crates/platform-mock` (Crate `platynui-platform-mock`).
- Provider-Crates bauen darauf auf: `crates/provider-windows-uia` (Crate `platynui-provider-windows-uia`), `crates/provider-atspi` (Crate `platynui-provider-atspi`), `crates/provider-macos-ax` (Crate `platynui-provider-macos-ax`), `crates/provider-mock` (Crate `platynui-provider-mock`), `crates/provider-jsonrpc` (Crate `platynui-provider-jsonrpc`).
- Das Mock-Provider-Crate stellt zusätzlich einen skriptbaren `StaticMockTree` sowie Hilfsfunktionen wie `install_mock_tree`/`TreeGuard` bereit. Tests und Werkzeuge können damit deterministische Bäume aufbauen, ohne den produktiven Code zu verändern; nach dem Guard-Drop wird der Standardbaum automatisch wiederhergestellt. Fokusfähige Knoten beziehen ihr `IsFocused`-Attribut direkt aus einem gemeinsamen Mock-Fokuszustand und senden nach einem Fokuswechsel automatisch `ProviderEventKind::NodeUpdated` für die alte und neue Auswahl.
- Tests prüfen, ob Pflichtattribute und Patterns eingehalten werden; der Buildumfang wird über `cfg`-Attribute bzw. Ziel-Tripel gesteuert.

### 5.1 Provider-Richtlinien
#### Knoten‑Identität: `control:Id` (optional, stabil)
- Zweck: Entwicklerseitig vergebene, applikationsinterne Kennung eines Elements. Sie ist unabhängig von sichtbaren Beschriftungen/Sprachen und vom Laufzeitlebenszyklus des nativen UI‑Elements.
- Sichtbarkeit: Attribut `Id` im `control`‑Namespace (XPath: `@control:Id`). Kann fehlen (`null`) oder leer sein, wenn die Plattform keine solche Kennung bereitstellt oder die Anwendung keine vergibt.
- Abgrenzung: `RuntimeId` bleibt weiterhin die laufzeitstabile, provider‑spezifische Identifikation (UIA/AT‑SPI/AX). `Id` ergänzt diese um eine (idealerweise) pro Anwendung lebensdauer‑stabile Kennung.
- Plattform‑Mapping (Richtlinie):
  - Windows (UIAutomation): `Id ← AutomationId` (`UIA_AutomationIdPropertyId`). Leere Werte werden als „nicht gesetzt“ behandelt.
  - Linux (AT‑SPI2): falls vorhanden `Id ← accessible_id` (Toolkit‑abhängig). Wo nicht verfügbar, bleibt `Id` leer.
  - macOS (AX): `Id ← AXIdentifier` (sofern unterstützt). Fallback: nicht gesetzt.
  - Application‑Knoten (`app:Application`): empfohlen ist eine plattformtypische, stabile Kennung (z. B. Bundle Identifier auf macOS). Wenn nicht verfügbar, kann ein heuristischer Fallback (z. B. `ProcessName`) verwendet und entsprechend dokumentiert werden. Unter Windows wäre als Alternative zurzeit optional die AppUserModelID (AUMID) nutzbar, die über `SHGetPropertyStoreForWindow(hwnd)`/`PKEY_AppUserModel_ID` ermittelt werden kann; aktuell verwenden wir den Prozessnamen.
- Verwendung: Für persistente Selektoren sollte – wenn vorhanden – `@control:Id='…'` bevorzugt werden, ggf. in Kombination mit Rolle/Struktur (`//control:*[@Id='login-button']`).

#### UiNode‑Trait – neue Methode `id()`
- Die Runtime stellt auf `UiNode` eine optionale Methode `id(&self) -> Option<String>` bereit. Standard‑Implementierung liefert `None`.
- Provider mit nativem Pendant setzen diese Methode (z. B. UIA → `CurrentAutomationId()`; ApplicationNode unter Windows → Prozessname als Fallback).
- Attribut‑Emission: Das Attribut `@control:Id` wird nur erzeugt, wenn `UiNode::id()` einen Wert liefert (keine Null‑/Leer‑Attribute).


- Liefere sämtliche Positionsangaben (`Bounds`, `ActivationPoint`, `ActivationArea`, Fensterrahmen) im Desktop-Koordinatensystem (linke obere Ecke des Primärmonitors = Ursprung). Etwaige DPI-/Scaling-Anpassungen erfolgen providerseitig; die Runtime erwartet normalisierte Desktop-Koordinaten.
- Spiegle native Rollennamen in `control:Role` bzw. `item:Role` (lokaler Name für XPath) und hinterlege die Originalrollen zusätzlich unter `native:*`, um Technologie-spezifische Detailabfragen zu erlauben.
- Pflege `SupportedPatterns` konsistent: Ein Pattern darf nur gemeldet werden, wenn alle Pflichtattribute verfügbar sind. Optionale Attribute werden als `null` oder ausgelassen, nicht mit Platzhalterwerten gefüllt.
- Ergänze `app:`-Attribute (z. B. `ProcessId`, `ProcessName`) für Wurzel- und Applikationsknoten, damit Clients Prozesse eindeutig zuordnen können.
- Liefere, wenn möglich, den `accepts_user_input()`-Zustand über das `WindowSurface`-Pattern (unter Windows via `WaitForInputIdle`, auf anderen Plattformen best effort); bei Nichtverfügbarkeit `Ok(None)` zurückgeben und eventuelle Attribute leer lassen.
- Stelle sicher, dass `RuntimeId` pro Provider stabil bleibt, solange das zugrunde liegende Element existiert; bei Re-Creation darf sich die ID ändern.
- Typische Quellen für `RuntimeId`: UI Automation `RuntimeId`, AT-SPI D-Bus-Objektpfad auf dem Accessibility-Bus, macOS `AXUIElement` Identifier (kombiniert mit Prozessinformationen). Fehlt eine native Kennung, generiere eine deterministische ID, die während der Lebensdauer des Elements stabil bleibt.
- Dokumentiere Mapping-Entscheidungen in `docs/patterns.md`, wenn native APIs mehrere Möglichkeiten bieten (z. B. AX-Subrole vs. Role).
- Nutze die in `docs/provider_checklist.md` gepflegten Prüfschritte, bevor Provider-Änderungen gemergt werden (manuelle Review + automatisierte Tests).
- UI-Bäume dürfen keine Fenster oder Overlays des eigenen Prozesses enthalten. Highlight-Overlays oder andere Hilfsfenster werden ausschließlich von der Plattformebene verwaltet und niemals als reguläre `UiNode`-Elemente exponiert.

## 6. Geräte- und Interaktionsdienste
- `DeviceProvider`-Trait + Capability-Typen leben in `crates/core` (Pointer, Keyboard, DesktopInfoProvider, ScreenshotProvider, HighlightProvider, WindowManager); Touch-Unterstützung wird später ergänzt.
  - `HighlightProvider` zeichnet Hervorhebungen über `highlight(&HighlightRequest)` und entfernt sie via `clear()`.
    * `HighlightRequest` enthält eine oder mehrere Desktop-Bounding-Boxen (`Vec<Rect>`) und optional eine gewünschte Sichtbarkeitsdauer (`Duration`). Eine Anfrage umfasst stets genau eine gemeinsame Dauer für alle Rechtecke.
    * Der Highlight-Effekt wird nach der angeforderten Dauer automatisch vom Provider entfernt (Timer im Provider/Overlay). Es existiert kein API‑Konzept „mehrerer Requests mit unterschiedlichen Dauern“ mehr.
    * Plattform (Windows): nicht‑aktivierendes, klick‑durchlässiges Layered‑Window mit rotem Rahmen (3 px) und 1 px Abstand um die Ziel‑BBox. Rahmen werden an Desktop‑Bounds beschnitten; abgeschnittene Seiten erscheinen gestrichelt.
    * Es existiert immer nur ein aktives Highlight. Erneute Aufrufe ersetzen das bestehende Overlay: Die Rahmen werden auf die neuen Ziel‑Rects aktualisiert.
  - `PointerDevice` kapselt elementare Zeigereingaben vollständig in Desktop-Koordinaten (`f64`). Das Trait umfasst mindestens `position() -> Point`, `move_to(Point)`, `press(PointerButton)`, `release(PointerButton)` sowie `scroll(ScrollDelta)`; optional liefern Provider Double-Click-Metadaten (`double_click_time()`, `double_click_size()`), soweit die Plattform sie bereitstellt. Notwendige Umrechnungen in native Koordinatensysteme (Win32-Absolute, X11-Integer, macOS-CGFloat) erfolgen providerseitig.
    * Oberhalb des Traits implementiert die Runtime eine Bewegungs-Engine, die Zielkoordinaten in Schrittfolgen übersetzt (linear, Bezier/Overshoot, zufällige Jitter) und konfigurierbare Verzögerungen (`after_move_delay`, `press_release_delay`, `before_next_click_delay`, `multi_click_delay`) berücksichtigt. CLI-Kommandos greifen standardmäßig auf diese Engine zurück, Provider müssen lediglich die atomaren Operationen zuverlässig bereitstellen.
    * Vor jedem Aufruf klemmt die Runtime Koordinaten anhand der Desktop-Bounds (`DesktopInfo`). Provider dürfen zusätzliche Sicherheitsprüfungen durchführen (z. B. Fokusfenster-Abgleich), liefern aber stets normalisierte `f64`-Koordinaten zurück oder signalisieren Fehler, falls die OS-API das Bewegen verhindert.
    * Allgemeine Zeiger-Settings liegen in einer separaten Struktur `PointerSettings`. Sie enthält ausschließlich hardware- bzw. providerabhängige Basiswerte, die beim Runtime-Start aus dem `PointerDevice` übernommen werden und sich global (z. B. via CLI/Config-Datei) anpassen lassen:
      - `double_click_time`, `double_click_size`
      - Standard-Button (`default_button`)
    * Bewegungs- und Timingparameter bündelt das `PointerProfile`. Es fungiert als aktives Preset der Runtime und lässt sich über `Runtime::pointer_profile()`/`set_pointer_profile()` austauschen.
    * Temporäre Überschreibungen laufen über eine einheitliche `PointerOverrides`-Struktur. Jede API (`move_to`, `click`, `drag`, `scroll`, …) akzeptiert optional `Option<PointerOverrides>`; gesetzte Felder überschreiben nur für den jeweiligen Aufruf die Defaults (z. B. anderes Bewegungsprofil, alternative Delays), alle übrigen Werte bleiben beim aktuellen `PointerProfile`. Die `PointerOverrides::new()`-Builder-API bildet ausschließlich die Deltas ab – keine Duplikation der Plattformdefaults nötig.
    * `PointerOverrides` enthält neben Profil-/Delay-Feldern auch einen optionalen Ursprungsbezug (`origin`). Standard ist `PointOrigin::Desktop`, womit Zielkoordinaten bereits in Desktop-Bezug erwartet werden. Wird `origin` z. B. auf `PointOrigin::Bounds(Rect)` gesetzt, konvertiert die Runtime eingehende Koordinaten (z. B. `Point::new(1.0, 5.0)`) automatisch relativ zum angegebenen Referenzrechteck in Desktop-Koordinaten. Mit `PointOrigin::Absolute(Point)` lässt sich eine beliebige Basisposition (z. B. Fenster-Offset) angeben. Damit lassen sich Klicks innerhalb eines Controls präzise versetzen, ohne dass Aufrufer selbst Bounds addieren müssen.
    * Die Motion-Engine ist über ein `PointerProfile` konfigurierbar. Wichtige Parameter:
      - **Bewegungsmodus** (`mode`): `direct`, `linear`, `bezier`, `overshoot`, `jitter`.
      - **Schrittauflösung** (`steps_per_pixel`): bestimmt die Anzahl interpolierter Punkte pro Distanz.
      - **Geschwindigkeitsbudget** (`max_move_duration`, optional `speed_factor`): verteilt Delays auf die Schrittfolge. `speed_factor > 1.0` verkürzt die Laufzeit proportional, Werte < 1.0 verlangsamen Bewegungen.
      - **Beschleunigungsprofil** (`acceleration_profile`): konstant, langsam→schnell, schnell→langsam, sanfte S-Kurve; die Runtime verteilt die Zwischenstopps entsprechend der gewählten Ease-Kurve.
      - **Overshoot-Regler** (`overshoot_ratio`, `overshoot_settle_steps`): nur für Overshoot-Modi aktiv.
      - **Kurven-/Jitter-Amplitude** (`curve_amplitude`, `jitter_amplitude`): steuert seitliche Abweichungen in geschwungenen Pfaden.
      - **Follow-up-Delays** (`after_move_delay`, `after_input_delay`): kurze Pausen nach Bewegung bzw. Eingaben.
      - **Klick-Zeitfenster** (`press_release_delay`, `after_click_delay`, `before_next_click_delay`, `multi_click_delay`): beeinflusst Single-/Multi-Klick-Sequenzen. `before_next_click_delay` wird vor Folgeklicks innerhalb des `multi_click_delay`-Fensters aktiv enforced.
      - **Positionssicherung** (`ensure_move_position`, `ensure_move_threshold`, `ensure_move_timeout`): optionaler Check, ob der Cursor das Ziel erreicht, mit Nachjustierung oder Fehler.
      - **Scroll-Konfiguration** (`scroll_step`, `scroll_delay`, optional `scroll_axis`): legt diskrete Scrollschritte fest.
      Profile werden als benannte Presets gespeichert (z. B. `default`, `fast`, `human-like`) und lassen sich über CLI oder API auswählen/überschreiben.
    * Die Runtime stellt darauf aufbauend Methoden wie `pointer_move_to`, `pointer_click`, `pointer_multi_click`, `pointer_press`, `pointer_release`, `pointer_drag` und `pointer_scroll` bereit. Alle akzeptieren optional `PointerOverrides` und übernehmen die koordinatensichere Umsetzung inklusive Verzögerungen, Pfadinterpolation und Positionsprüfung. Fehler (z. B. verpasste Ziele) werden als `PointerError` gemeldet.
  - `KeyboardDevice` abstrahiert Tastatureingaben. Das Trait liefert mindestens `key_to_code(&str) -> Result<KeyCode, KeyboardError>`, `send_key_event(KeyboardEvent) -> Result<(), KeyboardError>` sowie `end_input()`. Optionale Hooks in `start_input()`/`end_input()` erlauben tastaturspezifische Vor- und Nachbereitung (z. B. IME-Umschaltung, Puffer leeren) – Fokusverwaltung oder Fensteraktivierung bleibt bei Runtime/Patterns. Verzögerungen zwischen Events steuert ausschließlich die Runtime (`KeyboardSettings`, `KeyboardOverrides`). Der Provider dokumentiert eigenständig, welche Tastennamen er unterstützt; `key_to_code` löst Namen/Aliasse in einen provider-spezifischen `KeyCode` auf. Welche Bedeutung dieser Wert hat (Virtual-Key, Scan-Code, Usage-ID, eigener Wert) liegt beim Provider, muss aber in seiner Dokumentation nachvollziehbar sein. Namen werden plattformübergreifend abgestimmt: Tasten mit direkter Entsprechung tragen den gleichen Namen auf allen Betriebssystemen (z. B. `Enter`, `Escape`, `Shift`), plattformspezifische Belegungen verwenden etablierte OS-Bezeichnungen (`Command`, `Option`, `Globe` auf macOS; `Windows`-Taste auf Windows; `Super` oder `Meta` auf Linux-Desktopumgebungen). Provider orientieren sich bei der Benennung an den jeweiligen Plattform-Konstanten (Win32 `VK_*`, X11 `XK_*`/`XF86XK_*`, macOS `kVK_*`, etc.) und dokumentieren Abweichungen. Falls eine Plattform eine Taste nicht besitzt, taucht sie in der Liste schlicht nicht auf.
    * `KeyboardEvent` ist ein schlankes Struct mit zwei Feldern: `KeyCode` (vom Provider via `key_to_code` geliefert) und `KeyState` (`Press`/`Release`). Kombinierte Kurzbefehle entstehen durch Sequenzen mehrerer Events (z. B. `Control`-Press, `A`-Press, anschließend die passenden Releases).
    * Die Runtime bietet darauf aufbauend `KeyboardSequence` als zentrale Repräsentation. Sie parst gemischte Eingaben wie `"eins zwei<Ctrl+a><Ctrl+Delete>Hallo"`, Backslash-Escapes (`\\<`, `\\>`, `\\`, `\\xNN`, `\\uNNNN`) und Mehrfachshortcuts (`<Ctrl+K Ctrl+C>`) in eine lazy Folge von Tastenvorgängen. Während des Parsens werden Tastenbezeichner strikt gegen `key_to_code()` gematcht – unbekannte Namen führen unmittelbar zu `KeyboardError::UnsupportedKey`.
    * Globale Standardwerte (Press-/Release-Verzögerungen, Nachlauf) werden in `KeyboardSettings` gehalten (`press_delay`, `release_delay`, `between_keys_delay`, `chord_press_delay`, `chord_release_delay`, `after_sequence_delay`, `after_text_delay`). Temporäre Abweichungen pro Aufruf laufen über `KeyboardOverrides::builder()`, das ausschließlich Deltas zu den aktiven Settings beschreibt (z. B. anderes Delay, alternative Nachlaufzeit).
    * Die Runtime entdeckt das erste registrierte `KeyboardDevice` und erlaubt Konfigurationsänderungen via `Runtime::keyboard_settings()`/`set_keyboard_settings()`. Sequenzlogik (Parser, Press/Release/Type) folgt in den nächsten Arbeitsbereichen.
    * Die Runtime stellt drei APIs bereit:
      - `keyboard_press(sequence, overrides)`: sendet ausschließlich Press-Events (Modifier gedrückt halten).
      - `keyboard_release(sequence, overrides)`: sendet ausschließlich Release-Events (Modifier loslassen).
      - `keyboard_type(sequence, overrides)`: führt press→release für jeden Schritt aus und deckt damit Text- wie Shortcut-Eingaben gleichermaßen ab.
      Jede dieser Funktionen arbeitet gegen das aktuell fokussierte Element. Aufrufer – CLI, Tests oder spätere Clients – sind dafür verantwortlich, das Ziel vorab über `Runtime::focus()` oder plattformspezifische Wege zu aktivieren. Die Runtime protokolliert gedrückte Tasten intern und sendet im Fehlerfall Best-Effort-Releases, um hängende Modifier zu vermeiden.
      Fehler werden als `KeyboardActionError` gemeldet. Das Enum kapselt Parserfehler (`KeyboardSequenceError`) und Providerfehler (`KeyboardError`), sodass Aufrufer zwischen syntaktischen Problemen und Plattformfehlern unterscheiden können.
  - `ScreenshotProvider` liefert Bildschirmaufnahmen. `ScreenshotRequest` beschreibt optional eine Teilfläche, ansonsten wird der komplette Desktop aufgenommen. Das Resultat (`Screenshot`) enthält Breite, Höhe, Rohdaten (`Vec<u8>`) und das Pixelformat (`PixelFormat::Rgba8` oder `PixelFormat::Bgra8`). Aufrufende Komponenten (Runtime, CLI, Inspector) sind dafür verantwortlich, die Daten in gewünschte Containerformate (PNG, JPEG, …) umzuwandeln.
- Implementierungen:
  - `crates/platform-windows` (Crate `platynui-platform-windows`): `SendInput`, Desktop Duplication/BitBlt, Overlays (Highlight: layered, non‑activating, clamped, dashed clipping edges).
  - `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`): `x11rb` + XTEST, Screenshots via X11 `GetImage`/Pipewire, Overlays.
  - `platynui-platform-linux-wayland` (optional): Wayland-APIs (Virtuelles Keyboard, Screencopy, Portal-Fallbacks).
  - `crates/platform-macos` (Crate `platynui-platform-macos`): `CGEvent`, `CGDisplayCreateImage`, transparente `NSWindow`/CoreAnimation.
- `crates/platform-mock` (Crate `platynui-platform-mock`) stellt In-Memory-Devices, Event-Logging sowie Highlight-, Pointer-, Screenshot- **und Keyboard**-Simulation bereit (`take_highlight_log`, `take_pointer_log`, `take_screenshot_log`, `take_keyboard_log` plus passende `reset_*`-Helfer); unterstützt JSON-RPC-Tests. Das Mock-Setup spiegelt ein dreiteiliges Monitor-Arrangement wider: links ein hochkant ausgerichtetes 2160×3840-Display, in der Mitte ein primärer UHD-Monitor (3840×2160) und rechts ein FHD-Monitor (1920×1080), dessen Oberkante vertikal zum Primärmonitor zentriert ist. Der Desktop-Bereich vereinigt alle Monitore, sodass XPath/Screenshot-Beispiele auch übergreifende Bounding-Boxen prüfen können.

### 6.1 Windows: Screenshot- und Highlight-Details
- Screenshot
  - Capture via GDI: `CreateDIBSection` (top‑down, 32 bpp) + `BitBlt` aus dem Screen‑HDC, Rückgabeformat `BGRA8`.
  - Region: Vor dem Capture wird gegen die Virtual‑Screen‑Bounds (SM_*VIRTUALSCREEN) geschnitten; vollständig außerhalb → Fehler. Teilweise außerhalb → effektive Breite/Höhe werden reduziert.
  - CLI: `platynui-cli screenshot [--rect X,Y,W,H] [DATEI]`. Ohne `DATEI` wird ein Default‑Name erzeugt; negative Koordinaten werden unterstützt (Clap `allow_hyphen_values`).
- Highlight
  - Overlay-Fenster: `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE`; Anzeige via `SW_SHOWNOACTIVATE`; `WM_MOUSEACTIVATE → MA_NOACTIVATE` (nicht‑aktivierend, klick‑durchlässig, nicht in Alt‑Tab/Taskbar).
  - Darstellung: Roter Rahmen (3 px) mit 1 px Abstand um das Zielrechteck; abgeschnittene Seiten gestrichelt (6 an / 4 aus), andere Seiten durchgezogen.
  - Dauer: Die Overlays planen `clear()` mittels internem Timer nach der angeforderten Dauer (generation‑aware). Die Runtime schedult keinen Fallback‑Timer; die CLI kann optional lokal für die gewünschte Dauer blockieren.

— DPI/Scaling
- Die Plattforminitialisierung setzt Per‑Monitor‑V2‑DPI‑Awareness (`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`). Koordinaten in Runtime/CLI beziehen sich auf Desktop‑Pixel (Virtual Screen). GDI‑Capture (BitBlt) und `UpdateLayeredWindow` arbeiten in denselben Gerätepixeln; zusätzliche Skalierungen sind nicht erforderlich.

Init-Reihenfolge (19.4)
- Die Runtime ruft beim Start zuerst alle registrierten `PlatformModule::initialize()`‑Hooks auf und erst danach die Provider‑Fabriken. Ein Runtime‑Test stellt sicher, dass diese Reihenfolge eingehalten wird (ein Test‑PlatformModule setzt einen Flag, ein Test‑Provider prüft den Flag in `create()`). Damit sind DPI‑Einstellungen aktiv, bevor Geräte/Provider Koordinaten abfragen oder Fenster/Monitore ermitteln.

Windows Desktop & Monitore (19.5)
- Desktop‑Bounds: aus Virtual‑Screen (`SM_*VIRTUALSCREEN`);
- Monitorliste: `EnumDisplayMonitors` + `GetMonitorInfoW(MONITORINFOEXW)` liefert pro Display `id`/`name` (Device‑Name), `bounds` und `is_primary`.
- OS‑Version: best‑effort als `<major>.<minor>[.<build>]` (Fallback vorhanden). Genauigkeit ist für die Runtime zweitrangig; wichtig ist die Stabilität der Desktop‑Koordinaten.
- DPI/Scaling: pro‑Monitor Effektiv‑DPI via `GetDpiForMonitor(MDT_EFFECTIVE_DPI)` → `scale_factor = dpi/96.0`. Die CLI zeigt den Faktor als Suffix `@ <x.xx>x`.
- Beispielausgabe (CLI `info`, Textformat):
  ```
  Monitors:
    [1]* DELL U2720Q [\\.\\DISPLAY2] 3840×2160 at (0, 0) @ 1.25x
    [2]  HP Z27      [\\.\\DISPLAY3] 2560×1600 at (3840, 0) @ 1.00x
    [3]  HP Z27      [\\.\\DISPLAY4] 2560×1600 at (3840, 1600) @ 1.00x
    [4]  BenQ EW32   [\\.\\DISPLAY1] 3840×2160 at (-3840, 0) @ 1.50x
  ```
- Negative Koordinaten entstehen bei Anordnungen mit links/oben liegenden Displays; CLI‑Beispiele funktionieren über die Vereinigungsfläche des Virtual‑Screens.

Hinweise & offene Punkte
- Ressourcenfreigabe (Windows): HDCs nach `GetDC(HWND(0))` freigeben (`ReleaseDC`); Overlay‑Fenster bei `clear()` ggf. zerstören (Klasse deregistrieren, falls nötig).
- DPI/Scaling: Verhalten unter Per‑Monitor‑V2 prüfen und dokumentieren.

## 7. Hinweise zur WindowSurface-Umsetzung
- Ziel des Runtime-Patterns `WindowSurface` ist es, Fensteraktionen (Aktivieren, Minimieren, Maximieren, Wiederherstellen, Verschieben/Größenänderung) und den Eingabestatus (`accepts_user_input()`) konsistent bereitzustellen. Provider entscheiden je Technologie, welche Informationen direkt aus dem UiTree stammen und wo ergänzende Betriebssystem-APIs nötig sind.
- **Direkte Technologie-Nutzung:** Einige APIs liefern bereits Fenster-Metadaten und Aktionen über eigene Patterns (z. B. UIA `IUIAutomationWindowPattern`, AT-SPI2 `org.a11y.atspi.Window`). Wo diese Schnittstellen stabil funktionieren, dürfen Provider sie eins-zu-eins einbinden und nur fehlende Felder ergänzen.
- **WindowManager – plattformnative Fenstersteuerung:** Nicht jedes Accessibility-Framework deckt alle Fensteroperationen vollständig und zuverlässig ab. Zum Beispiel implementiert unter Windows nicht jedes UIA-Element mit `ControlType.Window` das `WindowPattern` oder `TransformPattern` — das native HWND ist aber immer vorhanden und erlaubt über Win32 (`SetForegroundWindow`, `MoveWindow`, `ShowWindow`, `GetWindowRect`) volle Kontrolle. Unter X11 liefert AT-SPI2 keine Fenstersteuerung; GTK4 gibt über `Component.GetExtents(Screen)` zudem ungültige Koordinaten zurück — EWMH über das native XID ist der einzige zuverlässige Weg. Unter Wayland gibt es kein globales Koordinatensystem; Fenstermanagement läuft über Compositor-Protokolle.

  Um diese Plattform-Abhängigkeiten sauber zu kapseln, stellt `platynui-core` den Trait **`WindowManager`** bereit. Er abstrahiert den Zugriff auf native Fenster-Handles und -Operationen und wird über das Inventory-System registriert (analog zu `PointerDevice`, `KeyboardDevice` etc.):

  ```rust
  /// Opaque native window handle (HWND, XID, Wayland surface ID).
  pub struct WindowId(u64);

  pub trait WindowManager: Send + Sync {
      fn name(&self) -> &'static str;

      /// Resolve the native window handle from a UI node.
      /// Each implementation inspects the node's native attributes as needed.
      fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;

      fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError>;
      fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;

      fn activate(&self, id: WindowId) -> Result<(), PlatformError>;
      fn close(&self, id: WindowId) -> Result<(), PlatformError>;
      fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;
      fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;
      fn restore(&self, id: WindowId) -> Result<(), PlatformError>;
      fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;
      fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;

      /// Ensure the window is accessible on the current virtual desktop/workspace.
      /// Default: no-op (returns Ok(())).
      fn ensure_window_accessible(&self, id: WindowId) -> Result<(), PlatformError> { Ok(()) }
  }
  ```

  **Design-Entscheidung: `resolve_window` akzeptiert `&dyn UiNode`** statt z. B. PID + Koordinaten. Jede Plattform-Implementierung extrahiert selbst, was sie braucht:
  - **Windows:** liest `native:NativeWindowHandle` → HWND.
  - **X11 (EWMH):** ermittelt PID aus D-Bus-Credentials, nutzt `native:Component.Extents.Screen` als Geometry-Hint, matcht über `_NET_CLIENT_LIST` + `_NET_WM_PID`.
  - **Wayland (zukünftig):** nutzt App-ID + PID und matcht über `wlr-foreign-toplevel-list` o. ä.

  **Registrierung:** `register_window_manager!(&PROVIDER)` + Iterator `window_managers()`.

  **Schichtenmodell:**
  ```
  provider-atspi / provider-windows-uia  ──uses──►  core::WindowManager (trait)
                                                              ▲          ▲
                                                              │          │
                                            platform-linux-x11   platform-windows
                                              (EWMH/NetWM)       (Win32 HWND)
                                                              ▲
                                                              │
                                            platform-linux-wayland (zukünftig)
  ```

  Damit bleibt `provider-atspi` frei von `x11rb`-Abhängigkeiten und funktioniert identisch auf X11, Wayland und jeder zukünftigen Display-Server-Variante. Der Windows-UIA-Provider kann den `WindowManager` als zuverlässigen Fallback nutzen, wenn Elemente kein `WindowPattern`/`TransformPattern` implementieren.

  **EWMH-Support-Prüfung:** Die X11-Implementierung prüft beim `PlatformModule::initialize()` zusätzlich, ob ein EWMH-kompatibler Fenstermanager läuft (`_NET_SUPPORTING_WM_CHECK`) und welche Atoms unterstützt werden (`_NET_SUPPORTED`). Neben den bisherigen Atoms werden auch `_NET_CURRENT_DESKTOP`, `_NET_WM_DESKTOP` und `_NET_NUMBER_OF_DESKTOPS` geprüft — sie sind Voraussetzung für `ensure_window_accessible()` (siehe unten). Fehlende EWMH-Unterstützung wird als `warn` geloggt (kein harter Fehler), da AT-SPI-Basisfunktionen auch ohne WM funktionieren. Der Name des WM wird aus `_NET_WM_NAME` gelesen und per `info!` protokolliert.
- **Fokus-Kopplung:** `WindowSurface::activate()` bzw. `restore()` sollen den Fokus über das `Focusable`-Pattern setzen; `minimize()` und `close()` geben ihn frei. So bleibt das Verhalten deckungsgleich mit nativen Foreground-Wechseln (Alt+Tab, Klick) und das `IsFocused`-Attribut der Fenster bleibt konsistent.
- **Virtuelle Desktops / Workspaces:** Falls sich ein Zielfenster auf einem anderen virtuellen Desktop befindet, muss es vor der Aktivierung erreichbar gemacht werden. Dafür stellt `WindowManager` die Methode `ensure_window_accessible(WindowId)` bereit (Default-Implementierung: No-Op). Die Runtime ruft sie in `bring_to_front()` *vor* `activate()` auf — best-effort, d. h. ein Fehler wird geloggt, bricht aber den Fokussierungsversuch nicht ab. Plattformspezifische Strategien:
  - **X11:** Desktop des Fensters per `_NET_WM_DESKTOP` lesen, bei Abweichung via `_NET_CURRENT_DESKTOP`-ClientMessage zum Desktop wechseln.
  - **Windows:** `IVirtualDesktopManager::GetWindowDesktopId` liefert die GUID des Fenster-Desktops; bei Abweichung vom aktuellen Desktop wird das Fenster per `MoveWindowToDesktop` auf den aktiven Desktop verschoben.
  - **macOS:** No-Op — `kAXRaiseAction` in `activate()` löst implizit einen Space-Wechsel aus.
  Details und vollständige Implementierungsskizzen: → `docs/virtual_desktop_switching.md`.
- **Zuordnung zum UiTree:** Jeder Fensterknoten, der `WindowSurface` meldet, muss sich eindeutig einem Applikations- oder Control-Knoten zuordnen lassen. Alias-Sichten (flach vs. gruppiert) verwenden dieselbe `RuntimeId`, ergänzen aber Ordnungsschlüssel, damit Dokumentsortierung und Aktionen reproduzierbar bleiben.
- **Mock & Tests:** `platynui-platform-mock` und `platynui-provider-mock` liefern einfache Referenzimplementierungen für die Pattern-Aktionen. Sie dienen als Blaupause, bevor echte Plattformen angebunden werden, und stellen sicher, dass CLI-Befehle wie `window`, `pointer` und `keyboard` früh testbar bleiben. Der Provider hält dynamische Textpuffer (`append_text`, `replace_text`, `apply_keyboard_events`), so dass Tests Tastatureingaben inklusive Emojis oder IME-Strings simulieren und anschließend via XPath prüfen können.
- **Offene Punkte:** Während der Implementierung prüfen wir je Plattform, ob die Accessibility-Schnittstelle alleine genügt oder ob ergänzende System-APIs zwingend sind. Erkenntnisse fließen in die Provider-Dokumentation und `docs/patterns.md` ein.

## 8. JSON-RPC Provider & Adapter
- `platynui-provider-jsonrpc` stellt einen klar definierten JSON-RPC 2.0-Vertrag für externe Sprachen bereit. Kernkomponenten:
  - **Transport:** Named Pipes unter Windows (`\\.\pipe\PlatynUI+<pid>+<user>+<id>`), Unix Domain Sockets (`/tmp/platynui.<pid>.<user>.<id>`) oder Loopback TCP (per Konfiguration). Die Runtime stellt keine Transportinstanzen bereit, sondern verbindet sich mit dem vom Provider bereitgestellten Endpunkt. Sicherheitsanforderungen (ACLs/Tokens) liegen beim Provider.
  - **Handshake (`initialize`):** Provider melden Version, Technologiekennung, RuntimeId-Schema, Heartbeat-Intervalle/Zeitouts sowie optionale Zusatzinformationen (z. B. eigene Namensräume). Welche Rollen/Pattern letztlich verfügbar sind, ergibt sich aus den gelieferten Baumdaten.
- **Knoten-API:** `getNodes(parentRuntimeId | null)`, `getAttributes(nodeRuntimeId)`, `getSupportedPatterns(nodeRuntimeId)`, optional `ping`. Alle Antworten liefern normalisierte Attribute (`control:*`, `item:*`, `app:*`, `native:*`). Für den Einstiegsaufruf übergibt die Runtime ein vereinbartes Parent-Token (z. B. `null` oder eine spezielle Desktop-ID), das der Provider als Wurzel interpretiert.
  - **Events:** `$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`. Diese halten den Runtime-Baum synchron, enthalten aber keine Pattern-spezifischen Aktionen.
  - **Heartbeat/Recovery:** Runtime sendet periodisch `ping`; bleibt eine Antwort aus (`timeout`), wird der Provider als offline markiert und Abfragen schlagen mit definiertem Fehlercode fehl.
  - **Design-Vorbild:** Handshake, Capability-Negotiation und Nachrichtenfluss lehnen sich an etablierte Standards wie das Language Server Protocol (LSP) sowie das Model Context Protocol (MCP) an. Wir übernehmen deren Prinzipien (Versionskennzeichnung, optionale Erweiterungen, klar getrennte Rollen von Host/Client/Server) und passen sie auf UI-Automation zu, ohne die Protokolle 1:1 zu replizieren.
- JSON-RPC-Provider decken ausschließlich den UI-Baum ab; Eingabe- und Window-Management-Funktionen bleiben den Plattform-Crates der Runtime vorbehalten.
- Der Adapter kapselt sämtliche JSON-RPC-spezifischen Typen (Requests, Notifications, Fehler) und mappt sie auf die internen Provider-Traits.

## 9. XPath-Integration
- **Wrapper statt Kopien:** Für jede XPath-Abfrage erstellt die Runtime zurzeit flüchtige Wrapper, die die vorhandenen `Arc<dyn UiNode>` in `XdmNode`-fähige Objekte übersetzen. Die Wrapper existieren nur während der Auswertung und delegieren sämtliche Aufrufe (`children()`, `attributes()`, `typed_value()`) direkt an das Provider-Objekt.
- **Dokument & Ordnung:** Der virtuelle Dokumentknoten entspricht direkt dem Desktop und wird pro Evaluation als `NodeKind::Document` erzeugt; alle `control:`-/`item:`-Knoten erscheinen als `NodeKind::Element`. Absolute Pfade wie `/*` listen damit unmittelbar die obersten UI-Kinder (z. B. Anwendungen). Desktop-Attribute bleiben über den Kontextknoten (`.` bzw. `document-node()`) via `./@…` verfügbar. Eine separate Snapshot-/Caching-Schicht verschieben wir, bis konkrete Performance-Anforderungen vorliegen.
- **Abfrage-API:** `evaluate(node: Option<Arc<dyn UiNode>>, xpath: &str, options)` ist die zentrale Schnittstelle. Ohne Kontext (`None`) startet die Auswertung beim Desktop. Mit Kontext arbeitet die Runtime direkt mit dem übergebenen `Arc<dyn UiNode>`; falls der Knoten nicht mehr existiert, liefert die Auswertung kontrolliert einen Fehler. Eine automatische Re-Auflösung über `RuntimeId` ist aktuell nicht vorgesehen. Die Options-Struktur erlaubt lediglich leichte Steuerung (z. B. spätere Invalidation-Hooks); ein Cache bleibt explizit außen vor.
- **Aktueller Refresh-Fallback:** Solange kein Provider Ereignisse melden kann (oder `event_capabilities = None` anzeigt), ruft die Runtime vor jeder Auswertung einen Refresh der Provider-Knoten (aktuell über `Runtime::refresh_desktop_nodes()`) auf. Das stellt konsistente Ergebnisse sicher, kostet aber zusätzliche Aufrufe. Sobald Provider mindestens `ChangeHint` oder `Structure` liefern, entfällt dieser Fallback: Ereignisse invalidieren dann zielgerichtet die betreffenden Knoten, und `evaluate` greift auf die bereits aktualisierte Struktur zu.
- **Runtime-Helfer:** `Runtime::evaluate_options()` liefert vorbereitete `EvaluateOptions` (ohne Cache) inklusive des aktuell bekannten Desktop-Dokumentknotens. `Runtime::create_cache()` erzeugt einen neuen `XdmCache`; für wiederholte Abfragen stehen Convenience-Methoden bereit (`evaluate_cached()`, `evaluate_iter_cached()`, `evaluate_single_cached()`). Ergänzend stellt die Runtime `desktop_node()` und `desktop_info()` bereit, um den gebauten Knoten beziehungsweise die Metadaten (`DesktopInfo`) erneut zu verwenden.
- **Fokus-Aktion:** `Runtime::focus(&Arc<dyn UiNode>)` ruft das registrierte `FocusableAction` des Knotens auf. Fehlt das Pattern, liefert die Methode `FocusError::PatternMissing`; schlägt die Plattformaktion fehl, wird `FocusError::ActionFailed` mit der originalen `PatternError`-Nachricht weitergereicht. Provider sollten im Erfolgsfall `ProviderEventKind::NodeUpdated` für den alten und neuen Fokus emittieren, damit nachfolgende XPath-Abfragen den aktualisierten Zustand sehen.
- **Namespaces & Präfixe:** Der `StaticContext` registriert die festen Präfixe `control`, `item`, `app`, `native`. Provider können zusätzliche Präfixe ergänzen (z. B. `uia`, `ax`).
- **Typed Values zuerst:** `XdmNode::typed_value()` ist verpflichtend und liefert ausschließlich XDM-konforme Atomics (`xs:boolean`, `xs:integer`, `xs:double`, `xs:string`, `xs:dateTime`, …). `string_value()` wird automatisch aus der typisierten Sequenz abgeleitet. Die Runtime mappt dafür alle `UiValue`-Varianten: numerische Felder landen als `xs:double` bzw. `xs:integer`, Booleans als `xs:boolean`. Komplexere Strukturen wie `Rect`, `Point` oder `Size` bleiben als JSON-kodierte Strings verfügbar – ihre abgeleiteten Komponenten (`Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`, …) werden hingegen als numerische Atomics exponiert.
- **Strukturierte Attribute:** Die Wrapper erzeugen on-the-fly abgeleitete Attribute (`Bounds.X`, `ActivationPoint.Y`), damit XPath keine Sonderfunktionen benötigt. `UiAttribute`-Instanzen werden ebenfalls gewrappt, sodass der XPath-Layer direkt auf `UiValue`- und `typed_value()`-Ergebnisse zugreifen kann, ohne Provider-Objekte zu duplizieren.
- **Ergebnisformat:** Die Abfrage liefert eine Sequenz aus `EvaluationItem`. Neben `Node` (Dokument-, Element-, Attribut-Knoten) und `Value` (`UiValue`) existiert die Variante `Attribute`, die Besitzer, Namen und Wert eines Attributs gebündelt bereitstellt. Kommentar- oder Namespace-Knoten sowie Funktions-/Map-/Array-Items aus XPath 3.x sind vorerst nicht vorgesehen und würden als Fehler gemeldet.
- **Ausblick:** Die event-gesteuerte Cache-Invalidierung (Option B, siehe `docs/event_driven_cache_invalidation.md`) ist der nächste Schritt: Ein atomares Dirty-Flag in der Runtime wird von Provider-Events gesetzt und vor jeder gecachten Auswertung geprüft, sodass der `XdmCache` automatisch bei Strukturänderungen geleert wird.

## 10. Runtime-Pipeline & Komposition
1. **Runtime (`crates/runtime`, Crate `platynui-runtime`)** – verwaltet `PlatformRegistry`/`PlatformBundle`, lädt Desktop (`UiXdmDocument`), evaluiert XPath (Streaming), triggert Highlight/Screenshot.
2. **Server (`crates/server`, Crate `platynui-server`)** – JSON-RPC-2.0-Frontend (Language-Server-ähnlich) für Remote-Clients.
3. **Pipelines** – Mischbetrieb (z. B. AT-SPI2 + XTEST) möglich; Plattform-Erkennung wählt Implementierungen zur Laufzeit.
4. **Desktop-Aktualisierung** – Ohne Cache erstellt die Runtime vor jeder XPath-Auswertung den Desktop-Snapshot neu, indem sie alle `UiTreeProvider`-Wurzeln unterhalb des Desktop-Dokumentknotens einhängt. Mit `XdmCache` wird der Baum zwischen Auswertungen wiederverwendet und lazy revalidiert (`is_valid()` + `prepare_for_evaluation()`); nur bei ungültigen Knoten erfolgt ein partieller Neuaufbau.
5. **Fensterbereitschaft** – Über das `WindowSurface`-Pattern kann die Runtime per `accepts_user_input()` prüfen, ob ein Fenster Eingaben annimmt (Windows nutzt `WaitForInputIdle`; andere Plattformen liefern bestmögliche Heuristiken oder `None`). Die Werte werden on-demand abgefragt.

> Hinweis: Die Runtime lädt und bewertet nur die aktuell vorliegenden Knoten. Wenn Elemente erst durch Benutzerinteraktion erscheinen (z. B. Scrollen, Paging, Kontextmenüs), müssen Clients dieselben Eingaben auslösen wie ein Mensch vor dem Bildschirm. So behalten Automationen identische Freiheitsgrade wie interaktive Anwender.

## 11. Werkzeuge auf Basis der Runtime
1. **CLI (`crates/cli`, Crate `platynui-cli`)** – modularer Satz an Befehlen, die wir schrittweise ausbauen:
   - `list-providers`: registrierte Provider/Technologien anzeigen (Name, Version, Aktiv-Status; Mock → reale Plattformen).
   - `info`: Desktop-/Plattformmetadaten (OS, Auflösung, Monitore) über `DesktopInfoProvider` ausgeben.

## Ergänzung: Keyboard-Device und CLI (Stand 2025-10-22)

Diese Ergänzung fasst den aktuellen Stand der Tastatur-Schnittstellen zusammen und präzisiert Benennungen, Mapping und CLI/Python-APIs.

- Trait und Benennung
  - `KeyboardDevice` stellt `key_to_code(&str)`, `send_key_event(KeyboardEvent)`, optionale `start_input`/`end_input` sowie `known_key_names()` bereit.
  - `known_key_names()` liefert die von der jeweiligen Plattform unterstützten Tastennamen (case‑insensitiv zu vergleichen). Zeichen‑Eingaben (z. B. Buchstaben/Ziffern) können von `key_to_code()` akzeptiert werden, ohne in dieser Liste zu erscheinen.
  - Benennungsregeln: Plattform‑offizielle Namen ohne Präfixe (z. B. Windows ohne `VK_`). Gemeinsame Tasten tragen plattformübergreifend denselben Namen (z. B. `Enter`, `Escape`, `Shift`). Plattformspezifische Tasten nutzen etablierte OS‑Begriffe (`Command`, `Option`, `Windows`, `Super`/`Meta`).
  - Shortcut‑Syntax: In Shortcut‑Blöcken (`<…>`) kann ein Key entweder ein Name (alphanumerisch/`_`/Escapes) oder ein einzelnes, nicht reserviertes Zeichen sein (z. B. `<Ctrl+#>`, `<Ctrl+Shift+.>`). Reserviert sind `+`, `<`, `>` und Whitespace; hierfür existieren Symbol‑Aliasse (siehe unten).

- Windows‑Spezifika (Provider `platynui-platform-windows`)
  - Benannte VKs: vollständige Map aller `VK_*`‑Konstanten ohne Präfix (`ESCAPE`, `RETURN`, `F24`, `LCTRL`, `RMENU`, …). Links/Rechts‑Aliasse zusätzlich (`LSHIFT/LEFTSHIFT`, `RCTRL/RIGHTCTRL`, `ALTGR/RALT/RIGHTALT`, `LEFTWIN/RIGHTWIN`).
  - Zeichen: Einzelzeichen werden via `VkKeyScanW` auf `(vk, shift, ctrl, alt)` gemappt; für Buchstaben invertiert aktives CapsLock das Shift‑Bit. Fallback: Unicode‑Injection für unmappbare Zeichen.
  - AltGr: Wenn `Ctrl+Alt` gemeldet wird, injiziert der Provider `VK_RMENU` (Right Alt) statt eines separaten Ctrl‑/Alt‑Chords.
  - Extended Keys: bekannte Extended‑VKs setzen `KEYEVENTF_EXTENDEDKEY` (z. B. Right Ctrl/Alt, Insert/Delete/Home/End/PgUp/PgDn, Pfeile, NumLock, Divide, Windows/Menu).

### Symbol‑Aliasse für reservierte Zeichen
- Motivation: In Shortcut‑Blöcken sind `+`, `<`, `>` und Whitespace reserviert. Statt Escapes (`<Ctrl+\\+>`, `<Ctrl+\\>>`) können symbolische Aliasse verwendet werden.
- Aliasse: `PLUS` (`+`), `MINUS` (`-`), `LESS`/`LT` (`<`), `GREATER`/`GT` (`>`).
- Implementierungsstand: Im Mock‑Keyboard und im Windows‑Keyboard implementiert. Für Linux/macOS‑Provider ist die Umsetzung vorgesehen.

- CLI
  - `keyboard type <SEQUENCE>` – gemischte Eingabe (Text + `<Ctrl+A>` etc.).
  - `keyboard press <SEQUENCE>` / `keyboard release <SEQUENCE>` – reiner Press/Release‑Modus.
  - `keyboard list [--format text|json]` – gibt `known_key_names()` des aktiven Keyboard‑Geräts aus.
  - Alle Kommandos akzeptieren Timing‑Overrides (`--delay-ms`, `--press-delay`, `--release-delay`, `--between-keys-delay`, `--chord-press-delay`, `--chord-release-delay`, `--after-sequence-delay`, `--after-text-delay`).

- Python
  - `Runtime.keyboard_type(sequence, overrides=None)`
  - `Runtime.keyboard_press(sequence, overrides=None)` / `Runtime.keyboard_release(sequence, overrides=None)`
  - Neu: `Runtime.keyboard_known_key_names() -> list[str]` – liefert die vom aktiven Keyboard‑Gerät bekannten Tastennamen (gleiche Quelle wie `keyboard list`).

   - `query`: XPath-Auswertung mit Ausgabe als Tabelle oder JSON; optional lassen sich Ergebnisse nach Namespace (`--namespace`) und Patterns (`--pattern`) filtern.
   - Referenzstruktur des Mock-Baums: siehe `crates/provider-mock/assets/mock_tree.xml`; für Tests stellt `platynui-provider-mock` Hilfsfunktionen wie `emit_event(...)`, `emit_node_updated(...)`, `append_text(...)`, `replace_text(...)` und `apply_keyboard_events(...)` bereit. Damit lassen sich gezielt Ereignisse erzeugen oder Texte in Steuerelementen manipulieren, ohne native APIs zu berühren. Der Mock wird nur eingebunden, wenn das Cargo-Feature `mock-provider` aktiviert ist (z. B. `cargo run -p platynui-cli --features mock-provider -- watch --limit 1`).
   - `watch`: Provider-Ereignisse streamen (Text oder JSON), Filter auf Namespace/Pattern/RuntimeId anwenden und optional per `--expression` nach jedem Event eine XPath-Abfrage nachschieben; `--limit` erleichtert automatisierte Tests.
   - `highlight`: Bounding-Boxen hervorheben; nutzt `HighlightProvider` (Mock, später nativ) und akzeptiert XPath-Ausdrücke, eine optionale Dauer (`--duration-ms`), sowie `--clear`, um bestehende Hervorhebungen zu entfernen oder neu zu positionieren.
  - `screenshot`: Bildschirm-/Bereichsaufnahmen über `ScreenshotProvider` erzeugen. Optional `--rect x,y,width,height` für Teilbereiche; der Zielpfad kann als Positionsargument angegeben werden. Ohne Pfad wird ein Default-Dateiname im aktuellen Verzeichnis erstellt. Ohne `--rect` wird automatisch der vollständige Desktop (vereinigt über alle Monitore laut `DesktopInfo`) aufgenommen. Übergebene Bereiche dürfen sich über mehrere Monitore erstrecken; die Runtime reicht die Werte unverändert an den Provider durch.
  - `focus`: XPath-Ausdruck evaluieren, und über `Runtime::focus()` den Fokus setzen. Die Ausgabe listet erfolgreiche Fokuswechsel sowie übersprungene Knoten (fehlendes Pattern oder Pattern-Fehler) getrennt auf.
- `window`: Fensterlisten (`--list`) sowie Aktionen auf `WindowSurface` (`--activate`, `--minimize`, `--maximize`, `--restore`, `--close`, `--move x y`, `--resize w h`). Ausgabe fasst Zustände (Bounds, Topmost, AcceptsUserInput) zusammen; basiert aktuell auf dem Mock-Provider (`--features mock-provider`).
- `pointer`: Zeigeraktionen (Move/Click/Press/Release/Scroll/Drag) über `PointerDevice` ausführen; unterstützt `--origin`, `--motion`, `--acceleration` sowie Delay-Overrides und bietet `position`, um die aktuelle Desktop-Koordinate des Cursors auszugeben. Zusätzlich lassen sich mit `--move-duration` (absolute Obergrenze), `--move-time-per-pixel` (proportionaler Aufpreis je Distanz) und `--speed-factor` (Skalierung der Zielzeit) die Bewegungsprofile feintunen; die Runtime verteilt das Budget über die Zwischenpunkte (`PointerProfile::max_move_duration`, `PointerProfile::move_time_per_pixel`, `PointerProfile::speed_factor`). Auf Windows setzt die Plattform-Initialisierung die Anwendung vorab auf `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`, sodass `SetCursorPos` und `GetCursorInfo` hardwarebezogene Koordinaten liefern.
- `keyboard`: Tastatureingaben über `KeyboardDevice` ausführen; akzeptiert Sequenzen im selben Format wie die Runtime (`"foo<Ctrl+c>bar"`, Escapes über Backslash: `\\<`, `\\>`, `\\`, `\\xNN`, `\\uNNNN`). Die Runtime erwartet, dass der Client das Ziel zuvor fokussiert hat, und sendet press/release entsprechend der Sequenz. Im Mock-Szenario protokolliert der Plattform-Treiber die Press-/Release-Events auf stdout, sodass Tests den Ablauf nachvollziehen können.
  Weitere Kommandos (z. B. `dump-node`, `watch --script`) folgen nach Stabilisierung der Basisfunktionen.

> **Hinweis zur XPath-Suche:** Alias-Sichten (Anwendungsstruktur) erhalten eigene Präfixe (`app:*`, `appitem:*`). Eine Abfrage wie `/control:*/descendant-or-self::control:*[@IsFocused=true()]` traversiert ausschließlich die flache `control:`-Sicht und liefert damit jeden Knoten höchstens einmal, während `//app:*` gezielt die Anwendungssicht adressiert.
2. **Inspector (GUI)** – Tree-Ansicht, Property-Panel (`control:*`, `item:*`, `native:*`), XPath-Editor (Autocompletion), Ergebnisliste, Highlighting, Element-Picker, Export/Logging; arbeitet eingebettet oder über `crates/server` (Crate `platynui-server`).

## 12. Nächste Schritte
> Kurzfristiger Fokus: Windows (UIA) und Linux/X11 (AT-SPI2) werden zuerst umgesetzt; macOS folgt, sobald beide Plattformen stabil laufen.

1. **CLI + Mock-Stack** – Runtime mit `platynui-platform-mock`/`platynui-provider-mock` verdrahten; Befehle `list-providers`, `info`, `query`, `watch`, `highlight`, `screenshot`, `focus`, `window`, `pointer`, `keyboard` sind umgesetzt (Mock-basiert, `rstest`-abgedeckt). Nächste Ausbaustufen betreffen erweiterte Ausgabeformate (`--json`).
2. **Runtime-Patterns** – Fokus-/WindowSurface-Pattern finalisieren, Mock-Provider/-Tests ergänzen und CLI `window` grundlegend funktionsfähig machen.
3. **Runtime-Basis** – Plattformunabhängige Mechanismen (`PlatformRegistry`/`PlatformBundle`) fertigstellen.
4. **Plattform Windows** – Geräte (`platynui-platform-windows`) und UiTree (`platynui-provider-windows-uia`) produktionsreif machen; Fokus-/Highlight-/Screenshot-/Window-Flows mit Windows-spezifischen APIs absichern.
5. **Plattform Linux/X11** – Geräte (`platynui-platform-linux-x11`) und AT-SPI2-Provider (`platynui-provider-atspi`) umsetzen; X11-spezifische Tests spiegeln.
6. **Werkzeuge** – CLI um weiterführende Befehle (`dump-node`, `watch`-Skripting) erweitern, Inspector-Prototyp aufsetzen.
7. **Optionale Erweiterungen** – macOS-Stack, JSON-RPC-Anbindung, Wayland-Support, Performance-/Caching-Themen und Community-Dokumentation.
# 6. Werkzeuge – CLI Snapshot (Text/XML Export)

Ziel: UI‑Teilbäume exportieren – standardmäßig als lesbarer Text‑Baum, optional als XML (kompatibel zu externen XPath‑Parsern). Der Export ist streaming‑basiert und skaliert für große Bäume.

Kurzüberblick
- Befehl: `platynui-cli snapshot <XPATH>`
- Standardausgabe: Text‑Baum auf stdout oder in Datei (`--output`).
- XML nur mit `--format xml` (siehe unten).

XML‑Export
- Elementname = Rolle, Präfix = Knoten‑Namespace (`control`, `item`, `app`, `native`).
- Attribute als XML‑Attribute; komplexe Werte (Rect/Point/Size/Array/Object) als JSON‑String.
- Kinder in Dokumentreihenfolge als Kindelemente.

Namespaces (fest)
- `xmlns:control = "urn:platynui:control"`
- `xmlns:item    = "urn:platynui:item"`
- `xmlns:app     = "urn:platynui:app"`
- `xmlns:native  = "urn:platynui:native"`

Wichtige Optionen
- `--output FILE` (Text oder XML je nach `--format`; bei mehreren XML‑Wurzeln Wrapper `<snapshot>`)
- `--split PREFIX` (je Root eine Datei; `.txt` für Text, `.xml` für XML)
- `--max-depth N` (0=nur Wurzel, 1=+Kinder, …)
- `--attrs default|all|list`, `--include ns:Name[*]`, `--exclude ns:Name[*]`
- `--exclude-derived` (Alias‑Attribute wie `Bounds.X/Y/…` unterdrücken; Standard = erzeugen, wenn Basisattribut enthalten ist)
- `--include-runtime-id` (`control:RuntimeId` hinzufügen)
- `--pretty` (Einrückung/Zeilenumbrüche)
- `--format text|xml` (Default: `text`)
- `--no-attrs` (Text nur Struktur, keine Attributzeilen)
- `--no-color` (Farben aus)

Beispiel (XML, alle Fenster, alle Attribute)
```
platynui-cli snapshot "//control:Window" --attrs all --pretty --format xml --output windows.xml
```

Weitere Details: `docs/cli_snapshot_spec.md`, Umsetzungsplan §23.1.
