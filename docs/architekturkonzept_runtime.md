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
├─ platform-windows          # Geräte, Window-Manager und sonstige Windows-spezifische Ressourcen – Crate `platynui-platform-windows`
├─ provider-windows-uia      # UiTreeProvider auf Basis von UI Automation – Crate `platynui-provider-windows-uia`
├─ platform-linux-x11        # Geräte/Window-Manager für Linux/X11 – Crate `platynui-platform-linux-x11`
├─ provider-atspi            # UiTreeProvider auf Basis von AT-SPI2 (X11) – Crate `platynui-provider-atspi`
├─ platform-macos            # Geräte/Window-Manager für macOS – Crate `platynui-platform-macos`
├─ provider-macos-ax         # UiTreeProvider auf Basis der macOS Accessibility API – Crate `platynui-provider-macos-ax`
├─ platform-mock             # Mock-Geräte und Window-Manager – Crate `platynui-platform-mock`
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
Plattform-Crates bündeln Geräte, Window-Manager und Hilfen je OS; Provider-Crates liefern den UiTreeProvider. Beide greifen auf die gemeinsamen Traits im `crates/core` zurück.

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
      fn supported_patterns(&self) -> &[PatternId];
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
- **Attribute statt Methoden:** Informationen wie `Technology`, Sichtbarkeits- oder Geometriedaten werden ausschließlich als Attribute bereitgestellt. Welche Felder vorhanden sind, ergibt sich aus den gemeldeten Patterns und der jeweiligen Plattform. Das Trait liefert nur Struktur- und Navigationsinformationen; Clients greifen über `UiNode::attribute(...)` oder die XPath-Ausgabe darauf zu. Für konsistente Benennungen stellt `platynui-core::ui::attribute_names::<pattern>::*` Konstanten bereit.
- **Pattern-Zugriff:** `UiPattern` ist das gemeinsame Basistrait für Runtime-Aktionen (`Any + Send + Sync`). Provider hinterlegen ihre Instanzen in einer Registry (z. B. `PatternRegistry` aus `platynui-core`, basierend auf `HashMap<PatternId, Arc<dyn UiPattern>>` plus Erfassungsreihenfolge) und liefern sie über `UiNode::pattern::<FocusablePattern>()`. `supported_patterns()` und `pattern::<T>()` müssen konsistent sein: Ein Pattern taucht nur in der Liste auf, wenn auch eine Instanz bereitsteht. Aktionen wie `FocusablePattern::focus()` oder `WindowSurfacePattern::maximize()` geben `Result<_, PatternError>` zurück, sodass Fehler sauber an Clients propagiert werden. Reine Lese-Informationen bleiben Attribute ohne zusätzliche Runtime-Traits.
- **Lazy Modell:** Die Runtime fordert Attribute/Kinder immer on-demand an. Provider können intern cachen, aber die Schnittstelle zwingt keine Vorab-Materialisierung.
- **Vertragsprüfung:** `platynui-core` stellt mit `validate_control_or_item(node)` einen Hilfsprüfer bereit, der lediglich prüft, ob `SupportedPatterns` keine Duplikate enthält. Weitere Attribut- oder Pattern-Prüfungen verbleiben bei Provider- oder Pattern-spezifischen Tests.
- **`UiValue`:** Typisiert (String, Bool, Integer, Float, strukturierte Werte wie `Rect`, `Point`, `Size`). Für strukturierte Werte erzeugt der XPath-Wrapper zusätzliche Alias-Attribute (`Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`), damit Abfragen simpel bleiben.
- **Namespaces:**
  - `control` (Standard) – Steuerelemente.
  - `item` – Elemente innerhalb von Containern (ListItem, TreeItem, TabItem).
  - `app` – Applikations-/Prozessknoten.
  - `native` – Technologie-spezifische Rohattribute.
- **Standardpräfix:** `control` wird als Default registriert. Ausdrücke ohne Präfix beziehen sich nur auf Steuerelemente; `item:` oder ein Wildcard-Namespace erweitern den Suchraum.
- **Desktop-Zusammensetzung:** Plattform-Crates liefern eine `DesktopInfo` (über einen `DesktopInfoProvider`), die Auflösung, Monitorliste, Betriebssystemdaten usw. enthält. Die Runtime baut daraus den tatsächlichen `control:Desktop`-Knoten. UiTreeProvider liefern lediglich ihren technologischen Baum (Anwendungen, Fenster, Controls) und geben mit `UiTreeProvider::get_nodes(parent)` jene Knoten zurück, die unterhalb eines vom Runtime-Host bereitgestellten Parents (z. B. Desktop, Anwendung) eingehängt werden sollen.
- **Fehlerbehandlung:** Provider dürfen Backend-Fehler in Attributewerten reflektieren (z. B. `UiValue::Null`). Die Runtime konvertiert Fehler nicht in Panics, sondern propagiert sie an den Client.

### 3.2 Pflichtattribute & Normalisierung
- **Attribute & Normalisierung:** Provider liefern Attribute entsprechend der eigenen Technologie und den gemeldeten Patterns. Übliche Felder wie `Role`, `RuntimeId`, `Bounds`, `Technology` oder `Name` sollten weiterhin verfügbar sein, damit XPath-Abfragen und Tools damit arbeiten können. `SupportedPatterns` dient als deklarative Liste und darf keine Duplikate enthalten.
- **Rollen & PascalCase:** Provider übersetzen native Rollen (`UIA_ButtonControlTypeId`, `ATSPI_ROLE_PUSH_BUTTON`, `kAXButtonRole`) in PascalCase (`Button`). Dieser Wert erscheint sowohl als lokaler Name (`control:Button`) als auch im Attribut `Role`. Die Originalrolle wird zusätzlich als `native:Role` abgelegt.
- **ActivationTarget:** Wird dieses Pattern gemeldet, muss `ActivationPoint` (absoluter Desktop-Koordinatenwert) vorhanden sein. Native APIs (`GetClickablePoint`, `Component::get_extents`, `AXPosition`) haben Vorrang; gibt es keine dedizierte Funktion, dient das Zentrum von `Bounds` als Fallback. Optional kann `ActivationArea` ein erweitertes Zielrechteck liefern. `ActivationPoint`/`ActivationArea` liegen im Namespace des Elements (`control` oder `item`).
- **Anwendungsbereitschaft:** Der Status `AcceptsUserInput` wird über das `WindowSurface`-Pattern ermittelt (z. B. Windows `WaitForInputIdle`, andernorts best effort). Provider können zusätzlich ein Attribut `window:AcceptsUserInput` bereitstellen; bei Unkenntnis bleibt es leer.
- **RuntimeIds:** Provider geben native IDs weiter (UIA RuntimeId, AT-SPI D-Bus-Pfad, AX ElementRef). Fehlen diese, generiert ein deterministischer Hash (z. B. Kombination aus Prozess, Pfad, Index) eine Laufzeit-ID, die stabil bleibt, solange das Element existiert.


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
- Ebenfalls neu ist der `ProviderEventDispatcher`: eine Fan-Out-Komponente, die Provider-Ereignisse synchron an registrierte Sinks weiterleitet. Provider registrieren den Dispatcher aktiv über `UiTreeProvider::subscribe_events(listener)`; externe Provider senden analoge JSON-RPC-Notifications, die der Adapter in `ProviderEvent`-Strukturen übersetzt.
- `ProviderEventKind` bildet die Synchronisationsereignisse ab (`NodeAdded`, `NodeUpdated`, `NodeRemoved`, `TreeInvalidated`). Die Runtime führt die Events in einer zentralen Pipeline zusammen; Provider melden neue Knoten immer inklusive vollständiger `UiNode`-Instanz. Weitere Konsumenten können sich über `Runtime::register_event_sink` einklinken.
- Registrierungen erfolgen über die neuen Makros `register_provider!(&FACTORY)` bzw. `register_platform_module!(&MODULE)`. Beide Makros hängen statische Einträge an eine `inventory`-Sammlung; Hilfsfunktionen (`provider_factories()`, `platform_modules()`) erlauben es der Runtime, zur Laufzeit alle registrierten Erweiterungen zu enumerieren. Tests können denselben Mechanismus nutzen, um Mocks temporär zu registrieren. Die Runtime nutzt anschließend den `ProviderRegistry`, um die erzeugten Factories je Technologie zu gruppieren.
- Plattform-spezifische Helfer implementieren das Trait `PlatformModule` (Methoden `name()` und `initialize() -> Result<(), PlatformError>`). Darüber stellen Plattform-Crates ihre Geräte-/Window-Manager-Bündel bereit und können beim Programmstart deterministisch initialisiert werden.
- Plattform-Crates liefern OS-spezifische Infrastruktur (Handles, D-Bus/COM-Brücken, Geräte, Window-Manager): `crates/platform-windows` (Crate `platynui-platform-windows`, optional `platynui-platform-windows-core`), `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`), optional `platynui-platform-linux-wayland`, `crates/platform-macos` (Crate `platynui-platform-macos`), `crates/platform-mock` (Crate `platynui-platform-mock`).
- Provider-Crates bauen darauf auf: `crates/provider-windows-uia` (Crate `platynui-provider-windows-uia`), `crates/provider-atspi` (Crate `platynui-provider-atspi`), `crates/provider-macos-ax` (Crate `platynui-provider-macos-ax`), `crates/provider-mock` (Crate `platynui-provider-mock`), `crates/provider-jsonrpc` (Crate `platynui-provider-jsonrpc`).
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

## 6. Geräte- und Interaktionsdienste
- `DeviceProvider`-Trait + Capability-Typen leben in `crates/core` (Pointer, Keyboard, DesktopInfoProvider, ScreenshotProvider, HighlightProvider); Touch-Unterstützung wird später ergänzt.
- Implementierungen:
  - `crates/platform-windows` (Crate `platynui-platform-windows`): `SendInput`, Desktop Duplication/BitBlt, Overlays.
  - `crates/platform-linux-x11` (Crate `platynui-platform-linux-x11`): `x11rb` + XTEST, Screenshots via X11 `GetImage`/Pipewire, Overlays.
  - `platynui-platform-linux-wayland` (optional): Wayland-APIs (Virtuelles Keyboard, Screencopy, Portal-Fallbacks).
  - `crates/platform-macos` (Crate `platynui-platform-macos`): `CGEvent`, `CGDisplayCreateImage`, transparente `NSWindow`/CoreAnimation.
- `crates/platform-mock` (Crate `platynui-platform-mock`) stellt In-Memory-Devices, Event-Logging und Highlight/Capture-Simulation bereit; unterstützt JSON-RPC-Tests.

## 7. Window-Management-Schicht
- Funktionen: Fensterlisten, Aktivieren/Minimieren/Maximieren/Restore, `move`/`resize`, Fokus setzen; Zugriff auf native Windowing-APIs (`HWND`, X11 Window IDs, `NSWindow`).
- Linux: Runtime entscheidet anhand `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, XWayland-Anwesenheit zwischen X11- und Wayland-Pfaden.
- `crates/platform-mock` (Crate `platynui-platform-mock`) liefert deterministische Window-Manager-Mocks für Tests.

-## 8. JSON-RPC Provider & Adapter
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
- **Wrapper statt Kopien:** Für jede XPath-Abfrage erstellt die Runtime zurzeit flüchtige Wrapper, die die vorhandenen `Arc<dyn UiNode>` in `XdmNode`-fähige Objekte übersetzen. Die Wrapper existieren nur während der Auswertung und delegieren sämtliche Aufrufe (`children()`, `attributes()`, `string_value()`) direkt an das Provider-Objekt.
- **Dokument & Ordnung:** Der virtuelle Dokumentknoten (Desktop) wird pro Evaluation erzeugt und als `NodeKind::Document` deklariert; alle `control:`-/`item:`-Knoten erscheinen als `NodeKind::Element`. Eine separate Snapshot-/Caching-Schicht verschieben wir, bis konkrete Performance-Anforderungen vorliegen.
- **Abfrage-API:** `evaluate(node: Option<Arc<dyn UiNode>>, xpath: &str, options)` ist die zentrale Schnittstelle. Ohne Kontext (`None`) startet die Auswertung beim Desktop. Mit Kontext arbeitet die Runtime direkt mit dem übergebenen `Arc<dyn UiNode>`; falls der Knoten nicht mehr existiert, liefert die Auswertung kontrolliert einen Fehler. Eine automatische Re-Auflösung über `RuntimeId` ist aktuell nicht vorgesehen. Die Options-Struktur erlaubt lediglich leichte Steuerung (z. B. spätere Invalidation-Hooks); ein Cache bleibt explizit außen vor.
- **Runtime-Helfer:** `Runtime::evaluate_options(desktop_node)` liefert vorbereitete `EvaluateOptions`, die aktuell lediglich den Desktop-Knoten referenzieren. Weitere Resolver-Mechanismen werden erst evaluiert, sobald konkrete Anforderungen entstehen.
- **Namespaces & Präfixe:** Der `StaticContext` registriert die festen Präfixe `control`, `item`, `app`, `native`. Provider können zusätzliche Präfixe ergänzen (z. B. `uia`, `ax`).
- **Strukturierte Attribute:** Die Wrapper erzeugen on-the-fly abgeleitete Attribute (`Bounds.X`, `ActivationPoint.Y`), damit XPath keine Sonderfunktionen benötigt. `UiAttribute`-Instanzen werden ebenfalls gewrappt, sodass der XPath-Layer direkt auf `UiValue`-Ergebnisse zugreifen kann, ohne Provider-Objekte zu duplizieren.
- **Ergebnisformat:** Die Abfrage liefert eine Sequenz aus `EvaluationItem`. Neben `Node` (Dokument-, Element-, Attribut-Knoten) und `Value` (`UiValue`) existiert die Variante `Attribute`, die Besitzer, Namen und Wert eines Attributs gebündelt bereitstellt. Kommentar- oder Namespace-Knoten sowie Funktions-/Map-/Array-Items aus XPath 3.x sind vorerst nicht vorgesehen und würden als Fehler gemeldet.
- **Ausblick:** Ein dediziertes Caching (inklusive Wiederverwendung von Wrappern und Rehydratisierung nach `invalidate`) bleibt ein späteres Performance-Thema. Die aktuelle Implementierung priorisiert Korrektheit und Einfachheit.

## 10. Runtime-Pipeline & Komposition
1. **Runtime (`crates/runtime`, Crate `platynui-runtime`)** – verwaltet `PlatformRegistry`/`PlatformBundle`, lädt Desktop (`UiXdmDocument`), evaluiert XPath (Streaming), triggert Highlight/Screenshot.
2. **Server (`crates/server`, Crate `platynui-server`)** – JSON-RPC-2.0-Frontend (Language-Server-ähnlich) für Remote-Clients.
3. **Pipelines** – Mischbetrieb (z. B. AT-SPI2 + XTEST) möglich; Plattform-Erkennung wählt Implementierungen zur Laufzeit.
4. **Fensterbereitschaft** – Über das `WindowSurface`-Pattern kann die Runtime per `accepts_user_input()` prüfen, ob ein Fenster Eingaben annimmt (Windows nutzt `WaitForInputIdle`; andere Plattformen liefern bestmögliche Heuristiken oder `None`). Die Werte werden on-demand abgefragt.

> Hinweis: Die Runtime lädt und bewertet nur die aktuell vorliegenden Knoten. Wenn Elemente erst durch Benutzerinteraktion erscheinen (z. B. Scrollen, Paging, Kontextmenüs), müssen Clients dieselben Eingaben auslösen wie ein Mensch vor dem Bildschirm. So behalten Automationen identische Freiheitsgrade wie interaktive Anwender.

## 11. Werkzeuge auf Basis der Runtime
1. **CLI (`crates/cli`, Crate `platynui-cli`)** – modularer Satz an Befehlen, die wir schrittweise ausbauen:
   - `list-providers`: registrierte Provider/Technologien anzeigen (Name, Version, Aktiv-Status; Mock → reale Plattformen).
   - `info`: Desktop-/Plattformmetadaten (OS, Auflösung, Monitore) über `DesktopInfoProvider` ausgeben.
   - `query`: XPath-Auswertung mit Text/JSON-Ausgabe und Namespace-/Pattern-Filtern.
   - `watch`: Provider-Ereignisse streamen und optional Folgeabfragen auslösen.
   - `highlight`: Bounding-Boxen hervorheben; nutzt `HighlightProvider` (Mock, später nativ).
   - `screenshot`: Bildschirm-/Bereichsaufnahmen über `ScreenshotProvider` erzeugen.
   - `focus`: Fokuswechsel über `FocusablePattern` orchestrieren.
   - `window`: Fensteraktionen (aktivieren, minimieren, maximieren, verschieben) und Eingabestatus (`accepts_user_input`) über das `WindowSurface`-Pattern abfragen.
   - `pointer`: Zeigeraktionen (Move/Click/Scroll) über `PointerDevice` ausführen.
   - `keyboard`: Tastatureingaben (Text, Keycodes) via `KeyboardDevice` simulieren.
   Weitere Kommandos (z. B. `dump-node`, `watch --script`) folgen nach Stabilisierung der Basisfunktionen.
2. **Inspector (GUI)** – Tree-Ansicht, Property-Panel (`control:*`, `item:*`, `native:*`), XPath-Editor (Autocompletion), Ergebnisliste, Highlighting, Element-Picker, Export/Logging; arbeitet eingebettet oder über `crates/server` (Crate `platynui-server`).

## 12. Nächste Schritte
> Kurzfristiger Fokus: Windows (UIA) und Linux/X11 (AT-SPI2) werden zuerst umgesetzt; macOS folgt, sobald beide Plattformen stabil laufen.

1. **CLI + Mock-Stack** – Runtime mit `platynui-platform-mock`/`platynui-provider-mock` verdrahten; Befehle `list-providers`, `info`, `query`, `watch`, `highlight`, `screenshot`, `focus`, `pointer`, `keyboard` iterativ implementieren und Tests mit `rstest` etablieren.
2. **Runtime-Patterns** – Fokus-/WindowSurface-Pattern finalisieren, Mock-Provider/-Tests ergänzen und CLI `window` grundlegend funktionsfähig machen.
3. **Runtime-Basis** – Plattformunabhängige Mechanismen (`PlatformRegistry`/`PlatformBundle`) fertigstellen.
4. **Plattform Windows** – Geräte (`platynui-platform-windows`) und UiTree (`platynui-provider-windows-uia`) produktionsreif machen; Fokus-/Highlight-/Screenshot-/Window-Flows mit Windows-spezifischen APIs absichern.
5. **Plattform Linux/X11** – Geräte (`platynui-platform-linux-x11`) und AT-SPI2-Provider (`platynui-provider-atspi`) umsetzen; X11-spezifische Tests spiegeln.
6. **Werkzeuge** – CLI um weiterführende Befehle (`dump-node`, `watch`-Skripting) erweitern, Inspector-Prototyp aufsetzen.
7. **Optionale Erweiterungen** – macOS-Stack, JSON-RPC-Anbindung, Wayland-Support, Performance-/Caching-Themen und Community-Dokumentation.
