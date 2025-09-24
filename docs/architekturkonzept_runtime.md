# Architekturkonzept PlatynUI Runtime

> Lebendes Dokument: Dieses Konzept sammelt aktuelle Ideen und Annahmen. Während der Implementierung passen wir Inhalte fortlaufend an, ergänzen Erkenntnisse und korrigieren Irrtümer.

## 1. Einleitung & Ziele
- PlatynUI soll eine plattformübergreifende UI-Automationsbibliothek bereitstellen, deren Kern eine konsistente Sicht auf native UI-Bäume (UIA, AT-SPI2, macOS AX, …) bildet.
- Die Runtime abstrahiert Plattform-APIs zu einem normalisierten Knotenbaum, der per XPath durchsucht wird und über Patterns beschreibende Fähigkeiten (keine direkten Aktionen) bereitstellt. Fokuswechsel und Fenstersteuerung bleiben die einzigen Laufzeitaktionen.
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
├─ architekturkonzept_runtime.md # Architekturkonzept (dieses Dokument)
├─ umsetzungsplan.md         # Aufgabenplan
└─ patterns.md               # Pattern-Spezifikation (Entwurf)
```
Plattform-Crates bündeln Geräte und Hilfen je OS; Provider-Crates liefern den UiTreeProvider. Beide greifen auf die gemeinsamen Traits im `crates/core` zurück.

### 2.2 Registrierungs- und Erweiterungsmodell
- `crates/core` definiert aktuell Marker-Traits wie `PlatformModule` und `UiTreeProviderFactory`. Weitere Erweiterungspunkte (`DeviceProviderFactory`) sind vorgesehen, aber noch nicht umgesetzt; solange diese Traits fehlen, dokumentieren wir sie hier ausdrücklich als geplante Ergänzungen. Alle Erweiterungen exportieren sich über ein `inventory`-basiertes Registrierungs-Makro. Die Runtime instanziiert ausschließlich über diese Abstraktionen und kennt keine konkreten Typen.
- Die Runtime initialisiert zur Compile-Zeit bekannte Erweiterungen über Inventory-Registrierungen (`register_platform_module!`, `register_provider!`). Eine dynamische Nachladefunktion ist derzeit nicht vorgesehen; zukünftige Erweiterungen greifen direkt auf denselben Mechanismus zurück.
- Welche Module eingebunden werden, entscheidet der Build: Über `cfg`-Attribute (z. B. `#[cfg(target_os = "windows")]`) binden wir die passenden Plattform- und Provider-Crates ein. Die Runtime führt lediglich die bereits kompilierten Registrierungen zusammen; es findet keine Plattform-Auswahl zur Laufzeit statt.
- Provider laufen entweder **in-process** (Rust-Crate) oder **out-of-process** (JSON-RPC). Für externe Provider stellt `platynui-provider-jsonrpc` Transport- und Vertragsebene bereit: Eine schlanke JSON-RPC-Spezifikation beschreibt den Mindestumfang (`initialize`, `getNodes`, `getAttributes`, `getSupportedPatterns`, optional `ping`). Die Runtime hält dazu einen JSON-RPC-Client, der den Provider zunächst über `initialize` nach Basismetadaten (Version, Technologiekennung, RuntimeId-Schema, Heartbeat-Intervalle, optionale vendor-spezifische Hinweise) abfragt und anschließend `getNodes(parentRuntimeId)` nutzt, um Kinder eines Parents (Desktop, App, Container) zu laden. Provider senden Baum-Events (`$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`) zur Synchronisation. Der eigentliche Provider-Prozess liefert ausschließlich die UI-Baum-Daten und bleibt unabhängig vom Runtime-Prozess. Sicherheitsschichten (Pipe-/Socket-Namen, ACLs/Tokens) werden auf Transportebene definiert. Komfortfunktionen wie Kontext-Abfragen (`evaluate(node, xpath, options)`) liegen vollständig bei der Runtime; Provider liefern ausschließlich Rohdaten.
- Tests können das gleiche Registrierungsmodell nutzen: Mock-Plattformen oder -Provider registrieren sich über `inventory` und lassen sich in Test-Szenarien gezielt auswählen, ohne produktive Module manipulieren zu müssen.

### 2.3 Laufzeitkontext
- Runtime läuft lokal, verwaltet Provider-Instanzen (nativ oder JSON-RPC) und agiert als Backend für CLI/Inspector.
- `crates/server` (Crate `platynui-server`) exponiert optional eine JSON-RPC-2.0-Schnittstelle (Language-Server-ähnlich) für Remote-Clients.
- Build-Targets und `cfg`-Attribute legen fest, welche Plattform-/Providerkombinationen in einem Artefakt enthalten sind.

## 3. Datenmodell & Namespaces
### 3.1 Knoten- & Attributmodell
- **`UiNode`-Trait:** Provider stellen ihren UI-Baum als `Arc<dyn UiNode>` bereit. Das Trait kapselt ausschließlich Strukturinformationen, alles weitere erfolgt über Attribute bzw. Patterns:
  ```rust
  use std::any::Any;
  use std::sync::{Arc, Weak};
  pub trait UiNode: Send + Sync {
      fn namespace(&self) -> Namespace;
      fn role(&self) -> &str;                // z. B. "Window", "Button", "ListItem"
      fn name(&self) -> &str;
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
      fn name(&self) -> &str;        // PascalCase
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
- **`UiValue`:** Typisiert (String, Bool, Integer, Float, strukturierte Werte wie `Rect`, `Point`, `Size`). Für strukturierte Werte erzeugt der XPath-Wrapper zusätzliche Alias-Attribute (`Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`), damit Abfragen simpel bleiben.
- **Namespaces:**
  - `control` (Standard) – Steuerelemente.
  - `item` – Elemente innerhalb von Containern (ListItem, TreeItem, TabItem).
  - `app` – Applikations-/Prozessknoten.
  - `native` – Technologie-spezifische Rohattribute.
- **Standardpräfix:** `control` wird als Default registriert. Ausdrücke ohne Präfix beziehen sich nur auf Steuerelemente; `item:` oder ein Wildcard-Namespace erweitern den Suchraum.
- **Desktop-Zusammensetzung:** Plattform-Crates liefern eine `DesktopInfo` (über einen `DesktopInfoProvider`), die Auflösung, Monitorliste, Betriebssystemdaten usw. enthält. Die Registrierung erfolgt – analog zu den UiTreeProvidern – per `inventory`; die Runtime verwendet den zuerst registrierten Provider, baut daraus den Desktop-Dokumentknoten (XPath-Wurzel) und stellt die Metadaten über `Runtime::desktop_info()` bereit. UiTreeProvider liefern lediglich ihren technologischen Baum (Anwendungen, Fenster, Controls) und geben mit `UiTreeProvider::get_nodes(parent)` jene Knoten zurück, die unterhalb eines vom Runtime-Host bereitgestellten Parents eingehängt werden sollen. Idealerweise stellen Provider zwei Sichten bereit:
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
- Registrierungen erfolgen über die neuen Makros `register_provider!(&FACTORY)` bzw. `register_platform_module!(&MODULE)`. Beide Makros hängen statische Einträge an eine `inventory`-Sammlung; Hilfsfunktionen (`provider_factories()`, `platform_modules()`) erlauben es der Runtime, zur Laufzeit alle registrierten Erweiterungen zu enumerieren. Tests können denselben Mechanismus nutzen, um Mocks temporär zu registrieren. Die Runtime nutzt anschließend den `ProviderRegistry`, um die erzeugten Factories je Technologie zu gruppieren.
- Plattform-spezifische Helfer implementieren das Trait `PlatformModule` (Methoden `name()` und `initialize() -> Result<(), PlatformError>`). Darüber stellen Plattform-Crates ihre Geräte-Bündel bereit und können beim Programmstart deterministisch initialisiert werden.
- Plattform-Crates liefern OS-spezifische Infrastruktur (Handles, D-Bus/COM-Brücken, Geräte): `crates/platform-windows` (Crate `platynui-platform-windows`, optional `platynui-platform-windows-core`), `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`), optional `platynui-platform-linux-wayland`, `crates/platform-macos` (Crate `platynui-platform-macos`), `crates/platform-mock` (Crate `platynui-platform-mock`).
- Provider-Crates bauen darauf auf: `crates/provider-windows-uia` (Crate `platynui-provider-windows-uia`), `crates/provider-atspi` (Crate `platynui-provider-atspi`), `crates/provider-macos-ax` (Crate `platynui-provider-macos-ax`), `crates/provider-mock` (Crate `platynui-provider-mock`), `crates/provider-jsonrpc` (Crate `platynui-provider-jsonrpc`).
- Das Mock-Provider-Crate stellt zusätzlich einen skriptbaren `StaticMockTree` sowie Hilfsfunktionen wie `install_mock_tree`/`TreeGuard` bereit. Tests und Werkzeuge können damit deterministische Bäume aufbauen, ohne den produktiven Code zu verändern; nach dem Guard-Drop wird der Standardbaum automatisch wiederhergestellt. Fokusfähige Knoten beziehen ihr `IsFocused`-Attribut direkt aus einem gemeinsamen Mock-Fokuszustand und senden nach einem Fokuswechsel automatisch `ProviderEventKind::NodeUpdated` für die alte und neue Auswahl.
- Tests prüfen, ob Pflichtattribute und Patterns eingehalten werden; der Buildumfang wird über `cfg`-Attribute bzw. Ziel-Tripel gesteuert.

### 5.1 Provider-Richtlinien
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
- `DeviceProvider`-Trait + Capability-Typen leben in `crates/core` (Pointer, Keyboard, DesktopInfoProvider, ScreenshotProvider, HighlightProvider); Touch-Unterstützung wird später ergänzt.
  - `HighlightProvider` zeichnet Hervorhebungen über `highlight(&[HighlightRequest])` und entfernt sie via `clear()`.
    * `HighlightRequest` enthält die Desktop-Koordinaten (`Rect`). Optional kann eine gewünschte Sichtbarkeitsdauer (`Duration`) mitgegeben werden.
    * Fehlt die Dauer, entscheidet die Plattform über einen sinnvollen Default (z. B. Overlay bleibt sichtbar, bis `clear()` aufgerufen wird).
    * Es existiert immer nur ein aktives Highlight. Erneute Aufrufe ersetzen das bestehende Overlay: Der Rahmen wandert zur neuen Position, die Dauer beginnt von vorne.
  - `PointerDevice` kapselt elementare Zeigereingaben vollständig in Desktop-Koordinaten (`f64`). Das Trait umfasst mindestens `position() -> Point`, `move_to(Point)`, `press(PointerButton)`, `release(PointerButton)` sowie `scroll(ScrollDelta)`; optional liefern Provider Double-Click-Metadaten (`double_click_time()`, `double_click_size()`), soweit die Plattform sie bereitstellt. Notwendige Umrechnungen in native Koordinatensysteme (Win32-Absolute, X11-Integer, macOS-CGFloat) erfolgen providerseitig.
    * Oberhalb des Traits implementiert die Runtime eine Bewegungs-Engine, die Zielkoordinaten in Schrittfolgen übersetzt (linear, Bezier/Overshoot, zufällige Jitter) und konfigurierbare Verzögerungen (`after_move_delay`, `press_release_delay`, `before_next_click_delay`, `multi_click_delay`) berücksichtigt. CLI-Kommandos greifen standardmäßig auf diese Engine zurück, Provider müssen lediglich die atomaren Operationen zuverlässig bereitstellen.
    * Vor jedem Aufruf klemmt die Runtime Koordinaten anhand der Desktop-Bounds (`DesktopInfo`). Provider dürfen zusätzliche Sicherheitsprüfungen durchführen (z. B. Fokusfenster-Abgleich), liefern aber stets normalisierte `f64`-Koordinaten zurück oder signalisieren Fehler, falls die OS-API das Bewegen verhindert.
    * Allgemeine Zeiger-Settings liegen in einer separaten Struktur `PointerSettings`. Sie decken grundlegende Betriebswerte ab und können global (z. B. über CLI/Config-Datei) angepasst werden:
      - `double_click_time`, `double_click_size`
      - Standard-Button (`default_button`)
      - Basis-Delays (`press_release_delay`, `after_input_delay`, `after_click_delay`, `before_next_click_delay`)
      - Multi-/Double-Click-Fenster (`multi_click_delay`, `multi_click_threshold`)
      - Nachlauf nach Bewegungen (`after_move_delay`, `ensure_move_timeout`, `ensure_move_threshold`)
      - Scroll-Grundwerte (`scroll_step`, `scroll_delay`)
    * Temporäre Überschreibungen laufen über eine einheitliche `PointerOverrides`-Struktur. Jede API (`move_to`, `click`, `drag`, `scroll`, …) akzeptiert optional `Option<PointerOverrides>`; gesetzte Felder überschreiben nur für den jeweiligen Aufruf die Defaults (z. B. anderes Bewegungsprofil, alternative Delays), alle übrigen Werte bleiben bei `PointerSettings`/`PointerProfile`. Die `PointerOverrides::new()`-Builder-API bildet ausschließlich die Deltas ab – keine Duplikation der Plattformdefaults nötig.
    * `PointerOverrides` enthält neben Profil-/Delay-Feldern auch einen optionalen Ursprungsbezug (`origin`). Standard ist `PointOrigin::Desktop`, womit Zielkoordinaten bereits in Desktop-Bezug erwartet werden. Wird `origin` z. B. auf `PointOrigin::Bounds(Rect)` gesetzt, konvertiert die Runtime eingehende Koordinaten (z. B. `Point::new(1.0, 5.0)`) automatisch relativ zum angegebenen Referenzrechteck in Desktop-Koordinaten. Mit `PointOrigin::Absolute(Point)` lässt sich eine beliebige Basisposition (z. B. Fenster-Offset) angeben. Damit lassen sich Klicks innerhalb eines Controls präzise versetzen, ohne dass Aufrufer selbst Bounds addieren müssen.
    * Die Motion-Engine ist über ein `PointerProfile` konfigurierbar. Wichtige Parameter:
      - **Bewegungsmodus** (`mode`): `direct`, `linear`, `bezier`, `overshoot`, `jitter`.
      - **Schrittauflösung** (`steps_per_pixel`): bestimmt die Anzahl interpolierter Punkte pro Distanz.
      - **Geschwindigkeitsbudget** (`max_move_duration`, optional `speed_factor`): verteilt Delays auf die Schrittfolge.
      - **Beschleunigungsprofil** (`acceleration_profile`): konstant, langsam→schnell, schnell→langsam, sanfte S-Kurve.
      - **Overshoot-Regler** (`overshoot_ratio`, `overshoot_settle_steps`): nur für Overshoot-Modi aktiv.
      - **Kurven-/Jitter-Amplitude** (`curve_amplitude`, `jitter_amplitude`): steuert seitliche Abweichungen in geschwungenen Pfaden.
      - **Follow-up-Delays** (`after_move_delay`, `after_input_delay`): kurze Pausen nach Bewegung bzw. Eingaben.
      - **Klick-Zeitfenster** (`press_release_delay`, `after_click_delay`, `before_next_click_delay`, `multi_click_delay`): beeinflusst Single-/Multi-Klick-Sequenzen.
      - **Positionssicherung** (`ensure_move_position`, `ensure_move_threshold`, `ensure_move_timeout`): optionaler Check, ob der Cursor das Ziel erreicht, mit Nachjustierung oder Fehler.
      - **Scroll-Konfiguration** (`scroll_step`, `scroll_delay`, optional `scroll_axis`): legt diskrete Scrollschritte fest.
      Profile werden als benannte Presets gespeichert (z. B. `default`, `fast`, `human-like`) und lassen sich über CLI oder API auswählen/überschreiben.
    * Die Runtime stellt darauf aufbauend Methoden wie `pointer_move_to`, `pointer_click`, `pointer_press`, `pointer_release`, `pointer_drag` und `pointer_scroll` bereit. Alle akzeptieren optional `PointerOverrides` und übernehmen die koordinatensichere Umsetzung inklusive Verzögerungen, Pfadinterpolation und Positionsprüfung. Fehler (z. B. verpasste Ziele) werden als `PointerError` gemeldet.
  - `ScreenshotProvider` liefert Bildschirmaufnahmen. `ScreenshotRequest` beschreibt optional eine Teilfläche, ansonsten wird der komplette Desktop aufgenommen. Das Resultat (`Screenshot`) enthält Breite, Höhe, Rohdaten (`Vec<u8>`) und das Pixelformat (`PixelFormat::Rgba8` oder `PixelFormat::Bgra8`). Aufrufende Komponenten (Runtime, CLI, Inspector) sind dafür verantwortlich, die Daten in gewünschte Containerformate (PNG, JPEG, …) umzuwandeln.
- Implementierungen:
  - `crates/platform-windows` (Crate `platynui-platform-windows`): `SendInput`, Desktop Duplication/BitBlt, Overlays.
  - `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`): `x11rb` + XTEST, Screenshots via X11 `GetImage`/Pipewire, Overlays.
  - `platynui-platform-linux-wayland` (optional): Wayland-APIs (Virtuelles Keyboard, Screencopy, Portal-Fallbacks).
  - `crates/platform-macos` (Crate `platynui-platform-macos`): `CGEvent`, `CGDisplayCreateImage`, transparente `NSWindow`/CoreAnimation.
- `crates/platform-mock` (Crate `platynui-platform-mock`) stellt In-Memory-Devices, Event-Logging sowie Highlight-, Pointer- und Screenshot-Simulation bereit (`take_highlight_log`, `take_pointer_log`, `take_screenshot_log`, entsprechende `reset_*`-Helfer); unterstützt JSON-RPC-Tests. Das Mock-Setup spiegelt ein dreiteiliges Monitor-Arrangement wider: links ein hochkant ausgerichtetes 2160×3840-Display, in der Mitte ein primärer UHD-Monitor (3840×2160) und rechts ein FHD-Monitor (1920×1080), dessen Oberkante vertikal zum Primärmonitor zentriert ist. Der Desktop-Bereich vereinigt alle Monitore, sodass XPath/Screenshot-Beispiele auch übergreifende Bounding-Boxen prüfen können.

## 7. Hinweise zur WindowSurface-Umsetzung
- Ziel des Runtime-Patterns `WindowSurface` ist es, Fensteraktionen (Aktivieren, Minimieren, Maximieren, Wiederherstellen, Verschieben/Größenänderung) und den Eingabestatus (`accepts_user_input()`) konsistent bereitzustellen. Provider entscheiden je Technologie, welche Informationen direkt aus dem UiTree stammen und wo ergänzende Betriebssystem-APIs nötig sind.
- **Direkte Technologie-Nutzung:** Einige APIs liefern bereits Fenster-Metadaten und Aktionen über eigene Patterns (z. B. UIA `IUIAutomationWindowPattern`, AT-SPI2 `org.a11y.atspi.Window`). Wo diese Schnittstellen stabil funktionieren, dürfen Provider sie eins-zu-eins einbinden und nur fehlende Felder ergänzen.
- **Zusätzliche OS-Hilfen:** Reicht die Accessibility-API nicht aus (z. B. fehlende `WaitForInputIdle`-Information oder eingeschränkte Move/Resize-Unterstützung), greifen Provider auf die jeweiligen Windowing-APIs zurück: Windows (Win32 `HWND`, `SetForegroundWindow`, `ShowWindow`, `MoveWindow`), X11 (EWMH/NetWM über Xlib/x11rb, ggf. XWayland-Erkennung), macOS (AppKit/CoreGraphics `NSWindow`/`AXUIElement`). Die Provider-Schicht bleibt dabei zuständig; die Runtime stellt keine zusätzliche abstrakte „Window-Manager“-Ebene mehr bereit.
- **Fokus-Kopplung:** `WindowSurface::activate()` bzw. `restore()` sollen den Fokus über das `Focusable`-Pattern setzen; `minimize()` und `close()` geben ihn frei. So bleibt das Verhalten deckungsgleich mit nativen Foreground-Wechseln (Alt+Tab, Klick) und das `IsFocused`-Attribut der Fenster bleibt konsistent.
- **Zuordnung zum UiTree:** Jeder Fensterknoten, der `WindowSurface` meldet, muss sich eindeutig einem Applikations- oder Control-Knoten zuordnen lassen. Alias-Sichten (flach vs. gruppiert) verwenden dieselbe `RuntimeId`, ergänzen aber Ordnungsschlüssel, damit Dokumentsortierung und Aktionen reproduzierbar bleiben.
- **Mock & Tests:** `platynui-platform-mock` und `platynui-provider-mock` liefern einfache Referenzimplementierungen für die Pattern-Aktionen. Sie dienen als Blaupause, bevor echte Plattformen angebunden werden, und stellen sicher, dass CLI-Befehle wie `window` früh testbar bleiben.
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
- **Runtime-Helfer:** `Runtime::evaluate_options()` liefert vorbereitete `EvaluateOptions` inklusive des aktuell bekannten Desktop-Dokumentknotens. Ergänzend stellt die Runtime `desktop_node()` und `desktop_info()` bereit, um den gebauten Knoten beziehungsweise die Metadaten (`DesktopInfo`) erneut zu verwenden. Weitere Resolver-Mechanismen werden erst evaluiert, sobald konkrete Anforderungen entstehen.
- **Fokus-Aktion:** `Runtime::focus(&Arc<dyn UiNode>)` ruft das registrierte `FocusableAction` des Knotens auf. Fehlt das Pattern, liefert die Methode `FocusError::PatternMissing`; schlägt die Plattformaktion fehl, wird `FocusError::ActionFailed` mit der originalen `PatternError`-Nachricht weitergereicht. Provider sollten im Erfolgsfall `ProviderEventKind::NodeUpdated` für den alten und neuen Fokus emittieren, damit nachfolgende XPath-Abfragen den aktualisierten Zustand sehen.
- **Namespaces & Präfixe:** Der `StaticContext` registriert die festen Präfixe `control`, `item`, `app`, `native`. Provider können zusätzliche Präfixe ergänzen (z. B. `uia`, `ax`).
- **Typed Values zuerst:** `XdmNode::typed_value()` ist verpflichtend und liefert ausschließlich XDM-konforme Atomics (`xs:boolean`, `xs:integer`, `xs:double`, `xs:string`, `xs:dateTime`, …). `string_value()` wird automatisch aus der typisierten Sequenz abgeleitet. Die Runtime mappt dafür alle `UiValue`-Varianten: numerische Felder landen als `xs:double` bzw. `xs:integer`, Booleans als `xs:boolean`. Komplexere Strukturen wie `Rect`, `Point` oder `Size` bleiben als JSON-kodierte Strings verfügbar – ihre abgeleiteten Komponenten (`Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`, …) werden hingegen als numerische Atomics exponiert.
- **Strukturierte Attribute:** Die Wrapper erzeugen on-the-fly abgeleitete Attribute (`Bounds.X`, `ActivationPoint.Y`), damit XPath keine Sonderfunktionen benötigt. `UiAttribute`-Instanzen werden ebenfalls gewrappt, sodass der XPath-Layer direkt auf `UiValue`- und `typed_value()`-Ergebnisse zugreifen kann, ohne Provider-Objekte zu duplizieren.
- **Ergebnisformat:** Die Abfrage liefert eine Sequenz aus `EvaluationItem`. Neben `Node` (Dokument-, Element-, Attribut-Knoten) und `Value` (`UiValue`) existiert die Variante `Attribute`, die Besitzer, Namen und Wert eines Attributs gebündelt bereitstellt. Kommentar- oder Namespace-Knoten sowie Funktions-/Map-/Array-Items aus XPath 3.x sind vorerst nicht vorgesehen und würden als Fehler gemeldet.
- **Ausblick:** Ein dediziertes Caching (inklusive Wiederverwendung von Wrappern und Rehydratisierung nach `invalidate`) bleibt ein späteres Performance-Thema. Die aktuelle Implementierung priorisiert Korrektheit und Einfachheit.

## 10. Runtime-Pipeline & Komposition
1. **Runtime (`crates/runtime`, Crate `platynui-runtime`)** – verwaltet `PlatformRegistry`/`PlatformBundle`, lädt Desktop (`UiXdmDocument`), evaluiert XPath (Streaming), triggert Highlight/Screenshot.
2. **Server (`crates/server`, Crate `platynui-server`)** – JSON-RPC-2.0-Frontend (Language-Server-ähnlich) für Remote-Clients.
3. **Pipelines** – Mischbetrieb (z. B. AT-SPI2 + XTEST) möglich; Plattform-Erkennung wählt Implementierungen zur Laufzeit.
4. **Desktop-Aktualisierung** – Vor jeder XPath-Auswertung erstellt die Runtime den aktuellen Desktop-Snapshot neu, indem sie alle `UiTreeProvider`-Wurzeln unterhalb des Desktop-Dokumentknotens einhängt (`Runtime::evaluate`). Damit steht dem Evaluator immer der aktuelle Zustand zur Verfügung, ohne dass ein globales Cache-Layer benötigt wird.
5. **Fensterbereitschaft** – Über das `WindowSurface`-Pattern kann die Runtime per `accepts_user_input()` prüfen, ob ein Fenster Eingaben annimmt (Windows nutzt `WaitForInputIdle`; andere Plattformen liefern bestmögliche Heuristiken oder `None`). Die Werte werden on-demand abgefragt.

> Hinweis: Die Runtime lädt und bewertet nur die aktuell vorliegenden Knoten. Wenn Elemente erst durch Benutzerinteraktion erscheinen (z. B. Scrollen, Paging, Kontextmenüs), müssen Clients dieselben Eingaben auslösen wie ein Mensch vor dem Bildschirm. So behalten Automationen identische Freiheitsgrade wie interaktive Anwender.

## 11. Werkzeuge auf Basis der Runtime
1. **CLI (`crates/cli`, Crate `platynui-cli`)** – modularer Satz an Befehlen, die wir schrittweise ausbauen:
   - `list-providers`: registrierte Provider/Technologien anzeigen (Name, Version, Aktiv-Status; Mock → reale Plattformen).
   - `info`: Desktop-/Plattformmetadaten (OS, Auflösung, Monitore) über `DesktopInfoProvider` ausgeben.
   - `query`: XPath-Auswertung mit Ausgabe als Tabelle oder JSON; optional lassen sich Ergebnisse nach Namespace (`--namespace`) und Patterns (`--pattern`) filtern.
   - Referenzstruktur des Mock-Baums: siehe `crates/provider-mock/assets/mock_tree.xml`; für Tests stellt `platynui-provider-mock` Hilfsfunktionen wie `emit_event(...)` und `emit_node_updated(...)` bereit, um gezielt Ereignisse zu erzeugen. Der Mock wird nur eingebunden, wenn das Cargo-Feature `mock-provider` aktiviert ist (z. B. `cargo run -p platynui-cli --features mock-provider -- watch --limit 1`).
   - `watch`: Provider-Ereignisse streamen (Text oder JSON), Filter auf Namespace/Pattern/RuntimeId anwenden und optional per `--expression` nach jedem Event eine XPath-Abfrage nachschieben; `--limit` erleichtert automatisierte Tests.
   - `highlight`: Bounding-Boxen hervorheben; nutzt `HighlightProvider` (Mock, später nativ) und akzeptiert XPath-Ausdrücke, eine optionale Dauer (`--duration-ms`), sowie `--clear`, um bestehende Hervorhebungen zu entfernen oder neu zu positionieren.
  - `screenshot`: Bildschirm-/Bereichsaufnahmen über `ScreenshotProvider` erzeugen, `--bbox` (optional, `x,y,width,height`) und `--output` (Pfad) akzeptieren und die Daten aktuell als PNG ablegen. Ohne Bounding-Box wird automatisch der vollständige Desktop (vereinigt über alle Monitore laut `DesktopInfo`) aufgenommen. Übergebene Bereiche dürfen sich über mehrere Monitore erstrecken; die Runtime reicht die Werte unverändert an den Provider durch.
   - `focus`: XPath-Ausdruck evaluieren, gefundene Knoten nach `RuntimeId` deduplizieren und über `Runtime::focus()` den Fokus setzen. Die Ausgabe listet erfolgreiche Fokuswechsel sowie übersprungene Knoten (fehlendes Pattern oder Pattern-Fehler) getrennt auf.
- `window`: Fensterlisten (`--list`) sowie Aktionen auf `WindowSurface` (`--activate`, `--minimize`, `--maximize`, `--restore`, `--close`, `--move x y`, `--resize w h`). Ausgabe fasst Zustände (Bounds, Topmost, AcceptsUserInput) zusammen; basiert aktuell auf dem Mock-Provider (`--features mock-provider`).
- `pointer`: Zeigeraktionen (Move/Click/Press/Release/Scroll/Drag) über `PointerDevice` ausführen; unterstützt `--origin`, `--motion` sowie Delay-Overrides.
- `keyboard`: Tastatureingaben (Text, Keycodes) via `KeyboardDevice` simulieren.
  Weitere Kommandos (z. B. `dump-node`, `watch --script`) folgen nach Stabilisierung der Basisfunktionen.

> **Hinweis zur XPath-Suche:** Alias-Sichten (Anwendungsstruktur) erhalten eigene Präfixe (`app:*`, `appitem:*`). Eine Abfrage wie `/control:*/descendant-or-self::control:*[@IsFocused=true()]` traversiert ausschließlich die flache `control:`-Sicht und liefert damit jeden Knoten höchstens einmal, während `//app:*` gezielt die Anwendungssicht adressiert.
2. **Inspector (GUI)** – Tree-Ansicht, Property-Panel (`control:*`, `item:*`, `native:*`), XPath-Editor (Autocompletion), Ergebnisliste, Highlighting, Element-Picker, Export/Logging; arbeitet eingebettet oder über `crates/server` (Crate `platynui-server`).

## 12. Nächste Schritte
> Kurzfristiger Fokus: Windows (UIA) und Linux/X11 (AT-SPI2) werden zuerst umgesetzt; macOS folgt, sobald beide Plattformen stabil laufen.

1. **CLI + Mock-Stack** – Runtime mit `platynui-platform-mock`/`platynui-provider-mock` verdrahten; Befehle `list-providers`, `info`, `query`, `watch`, `highlight`, `screenshot`, `focus`, `window`, `pointer` sind umgesetzt (Mock-basiert, `rstest`-abgedeckt). Nächste Ausbaustufen betreffen insbesondere den Keyboard-Befehl sowie erweiterte Ausgabeformate (`--json`, `--yaml`).
2. **Runtime-Patterns** – Fokus-/WindowSurface-Pattern finalisieren, Mock-Provider/-Tests ergänzen und CLI `window` grundlegend funktionsfähig machen.
3. **Runtime-Basis** – Plattformunabhängige Mechanismen (`PlatformRegistry`/`PlatformBundle`) fertigstellen.
4. **Plattform Windows** – Geräte (`platynui-platform-windows`) und UiTree (`platynui-provider-windows-uia`) produktionsreif machen; Fokus-/Highlight-/Screenshot-/Window-Flows mit Windows-spezifischen APIs absichern.
5. **Plattform Linux/X11** – Geräte (`platynui-platform-linux-x11`) und AT-SPI2-Provider (`platynui-provider-atspi`) umsetzen; X11-spezifische Tests spiegeln.
6. **Werkzeuge** – CLI um weiterführende Befehle (`dump-node`, `watch`-Skripting) erweitern, Inspector-Prototyp aufsetzen.
7. **Optionale Erweiterungen** – macOS-Stack, JSON-RPC-Anbindung, Wayland-Support, Performance-/Caching-Themen und Community-Dokumentation.
