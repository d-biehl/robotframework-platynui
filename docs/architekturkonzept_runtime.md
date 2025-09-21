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
├─ core                      # Gemeinsame Traits/Typen (UiTreeProvider, DeviceProvider, WindowManager, Patterns)
├─ runtime                   # Runtime, Provider-Registry, XPath-Pipeline, Fokus/Fenster-Aktionen
├─ server                    # JSON-RPC-Server als Frontend zur Runtime
├─ platform-windows-core     # (optional) Utilities für Windows (Handles, COM-Helfer)
├─ platform-windows          # Geräte, Window-Manager und sonstige Windows-spezifische Ressourcen
├─ provider-windows-uia      # UiTreeProvider auf Basis von UI Automation
├─ platform-linux-x11        # Geräte/Window-Manager für Linux/X11
├─ platform-linux-wayland    # Geräte/Window-Manager für Linux/Wayland (optional)
├─ provider-atspi            # UiTreeProvider auf Basis von AT-SPI2 (X11)
├─ platform-macos            # Geräte/Window-Manager für macOS
├─ provider-macos-ax         # UiTreeProvider auf Basis der macOS Accessibility API
├─ platform-mock             # Mock-Geräte und Window-Manager
├─ provider-mock             # Mock-UiTreeProvider (statisch/skriptbar)
├─ provider-jsonrpc          # Referenz-Adapter für externe JSON-RPC-Provider (optional)
├─ cli                       # `platynui-cli` Kommandozeilenwerkzeug
└─ inspector                 # (optional als crate oder unter apps/) GUI-Inspector

apps/
└─ inspector                 # Alternative Ablage für GUI-Inspector falls nicht als crate

docs/
├─ architekturkonzept_runtime.md # Architekturkonzept (dieses Dokument)
├─ umsetzungsplan.md         # Aufgabenplan
└─ patterns.md               # Pattern-Spezifikation (Entwurf)
```
Plattform-Crates bündeln Geräte, Window-Manager und Hilfen je OS; Provider-Crates liefern den UiTreeProvider. Beide greifen auf die gemeinsamen Traits im `crates/core` zurück.

### 2.2 Registrierungs- und Erweiterungsmodell
- `crates/core` definiert Marker-Traits (z. B. `PlatformModule`, `UiTreeProviderFactory`, `DeviceProviderFactory`, `WindowManagerFactory`). Alle Erweiterungen implementieren genau diese Traits und exportieren sich über ein `inventory`-basiertes Registrierungs-Makro. Die Runtime instanziiert ausschließlich über diese Abstraktionen und kennt keine konkreten Typen.
- Die Runtime initialisiert zur Compile-Zeit bekannte Erweiterungen über Inventory-Registrierungen (`register_platform_module!`, `register_provider!`). Eine dynamische Nachladefunktion ist derzeit nicht vorgesehen; zukünftige Erweiterungen greifen direkt auf denselben Mechanismus zurück.
- Welche Module eingebunden werden, entscheidet der Build: Über `cfg`-Attribute (z. B. `#[cfg(target_os = "windows")]`) binden wir die passenden Plattform- und Provider-Crates ein. Die Runtime führt lediglich die bereits kompilierten Registrierungen zusammen; es findet keine Plattform-Auswahl zur Laufzeit statt.
- Provider laufen entweder **in-process** (Rust-Crate) oder **out-of-process** (JSON-RPC). Für externe Provider stellt `provider-jsonrpc` Transport- und Vertragsebene bereit: Eine schlanke JSON-RPC-Spezifikation beschreibt den Mindestumfang (`initialize`, `listApplications`, `getRoot`, `getNode`, `getChildren`, `getAttributes`, `getSupportedPatterns`, optional `resolveRuntimeId`, `ping`). Die Runtime hält dazu einen JSON-RPC-Client, der den Provider zunächst über `initialize` nach Basismetadaten (Version, Technologiekennung, RuntimeId-Schema, Heartbeat-Intervall, optionale vendor-spezifische Hinweise) abfragt und anschließend die genannten Knotenoperationen aufruft. Provider senden Baum-Events (`$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`) zur Synchronisation. Der eigentliche Provider-Prozess liefert ausschließlich die UI-Baum-Daten und bleibt unabhängig vom Runtime-Prozess. Sicherheitsschichten (Pipe-/Socket-Namen, ACLs/Tokens) werden auf Transportebene definiert.
- Tests können das gleiche Registrierungsmodell nutzen: Mock-Plattformen oder -Provider registrieren sich mit niedriger Priorität und werden in Test-Szenarien vorrangig geladen, ohne produktive Module manipulieren zu müssen.

### 2.3 Laufzeitkontext
- Runtime läuft lokal, verwaltet Provider-Instanzen (nativ oder JSON-RPC) und agiert als Backend für CLI/Inspector.
- `crates/server` exponiert optional eine JSON-RPC-2.0-Schnittstelle (Language-Server-ähnlich) für Remote-Clients.
- Build-Targets und `cfg`-Attribute legen fest, welche Plattform-/Providerkombinationen in einem Artefakt enthalten sind.

## 3. Datenmodell & Namespaces
### 3.1 Knoten- & Attributmodell
- **`UiNode` & `UiValue`:** Attribute liefern lazily typisierte Werte (`UiValue`). Provider hinterlegen Getter, die erst beim Zugriff ausgeführt werden (Pull-Modell); der XPath-Layer ruft `attribute(&ExpandedName)` bzw. `attributes()` on-demand auf. `UiSnapshot` friert eine konsistente Sicht pro Abfrage ein und verhindert seiteneffektreiche Zweifachabfragen.
- **Namespaces:**
  - `control` (Standard) – Oberflächen-Steuerelemente (Fenster, Buttons, Textfelder, Listen, TreeViews). Elementnamen entsprechen der normalisierten Rolle (`control:Button`); Attribute sind PascalCase (`Name`, `Bounds`, `RuntimeId`, …).
  - `item` – Inhaltselemente innerhalb von Containern (`item:ListItem`, `item:TreeItem`, `item:TabItem`). Attribute folgen denselben Konventionen wie bei `control`-Elementen. Durch den separaten Namespace lassen sich umfangreiche Item-Bäume gezielt filtern.
  - `app` – Applikationsknoten (`AppId`, `ProcessId`, `Binary`, `AcceptsUserInput`), gespeist vom `Application`-Pattern.
  - `native` – unveränderte Backend-Daten (`BackendId`, native Rollen-IDs, Pattern-Token, Framework-spezifische Flags).
- **Standardpräfix:** Der Namespace `control` wird als Default-Präfix in XPath registriert. Abfragen ohne Präfix (`//Window//Button`) beziehen sich somit ausschließlich auf Steuerelemente; Item-Sammlungen werden erst durch explizite Verwendung von `item:` oder Wildcard-Namespace berücksichtigt.
- **Lazy Attribute & Fehlerbehandlung:** Attribute dürfen Exceptions/Fehler des Backends kapseln; Laufzeit stellt Hilfen bereit (`UiAttributeValue::from_result`), um Fehlermeldungen in eindeutigen Strings zu serialisieren statt Panics auszulösen.

### 3.2 Pflichtattribute & Normalisierung
- **Pflichtattribute:** `Name`, `Role`, `RuntimeId`, `Bounds`, `IsVisible`, `Technology`, `SupportedPatterns`. Desktop ergänzt `OsName`, `OsVersion`, `DisplayCount`, `Monitors` und nutzt `Bounds` als Gesamtfläche in Desktop-Koordinaten. `IsOffscreen` bleibt optional, wenn die Plattform es bereitstellt.
- **Rollen & PascalCase:** Provider übersetzen native Rollen (`UIA_ButtonControlTypeId`, `ATSPI_ROLE_PUSH_BUTTON`, `kAXButtonRole`) in PascalCase (`Button`). Dieser Wert erscheint sowohl als lokaler Name (`control:Button`) als auch im Attribut `Role`. Die Originalrolle wird zusätzlich als `native:Role` abgelegt.
- **ActivationTarget:** Wird dieses Pattern gemeldet, muss `ActivationPoint` (absoluter Desktop-Koordinatenwert) vorhanden sein. Native APIs (`GetClickablePoint`, `Component::get_extents`, `AXPosition`) haben Vorrang; gibt es keine dedizierte Funktion, dient das Zentrum von `Bounds` als Fallback. Optional kann `ActivationArea` ein erweitertes Zielrechteck liefern. `ActivationPoint`/`ActivationArea` liegen im Namespace des Elements (`control` oder `item`).
- **Anwendungsbereitschaft:** Das Feld `app:AcceptsUserInput` spiegelt, ob die Anwendung Eingaben akzeptiert (`WaitForInputIdle` auf Windows; best effort Heuristiken auf anderen Plattformen). Bei Nichtverfügbarkeit wird das Attribut ausgelassen oder als `null` geliefert.
- **RuntimeIds:** Provider geben native IDs weiter (UIA RuntimeId, AT-SPI D-Bus-Pfad, AX ElementRef). Fehlen diese, generiert ein deterministischer Hash (z. B. Kombination aus Prozess, Pfad, Index) eine Laufzeit-ID, die stabil bleibt, solange das Element existiert.


## 4. Pattern-System & Fähigkeiten
- Elemente deklarieren reine Capability-Patterns über `SupportedPatterns` im jeweiligen Namespace (`control:SupportedPatterns` oder `item:SupportedPatterns`). Der ausführliche Entwurf liegt in `docs/patterns.md` und bleibt diskutierbar.
- Patterns verhalten sich wie Traits: Sie beschreiben zusätzliche Attribute (z. B. `TextContent`, `Selectable`, `StatefulValue`, `ActivationTarget`) und können beliebig kombiniert werden. Die Runtime stellt keine generischen Aktions-APIs mehr bereit.
- Ausnahmen: Fokuswechsel (`Focusable` → `focus()`) und Fenstersteuerung (`WindowSurface` → `activate()`, `maximize()`, …) sind weiterhin Runtime-Funktionen, die über die Device-/Window-Manager abstrahiert werden.
- `ActivationTarget` liefert absolute Desktop-Koordinaten für Standard-Klickpositionen; Provider müssen Koordinaten und Flächen immer im Desktop-Bezugssystem melden, damit Geräte-/Highlight-Komponenten ohne zusätzliche Transformation arbeiten können.
- Die aktuelle Mapping-Tabelle zwischen Patterns und nativen APIs (UIA, AT-SPI2, AX) liegt in `docs/patterns.md` und wird gemeinsam mit den Providerteams gepflegt.
- Hinweis zur Terminologie: Patterns definieren keinerlei Event-Mechanik; Änderungen werden ausschließlich über aktualisierte Attribute sichtbar. Tree- oder Provider-Ereignisse (z. B. `NodeAdded`) existieren weiterhin zur Synchronisation der Runtime, sind aber von den Pattern-Spezifikationen getrennt.
- Clients entscheiden, wie sie mit den gelieferten Informationen interagieren (z. B. Maus-/Tastatursimulation, Gesten). Dadurch bleiben dieselben Möglichkeiten erhalten, die ein Mensch vor dem Bildschirm hat.

## 5. UiTreeProvider & Plattformlayer
- `crates/core` stellt Traits und Caching-Hilfen (`UiTreeProvider`, `ProviderDescriptor`, `ProviderHandle`) bereit.
- Plattform-Crates liefern OS-spezifische Infrastruktur (Handles, D-Bus/COM-Brücken, Geräte, Window-Manager): `platform-windows(-core)`, `platform-linux-x11`, optional `platform-linux-wayland`, `platform-macos`, `platform-mock`.
- Provider-Crates bauen darauf auf: `provider-windows-uia`, `provider-atspi`, `provider-macos-ax`, `provider-mock`, `provider-jsonrpc`.
- Tests prüfen, ob Pflichtattribute und Patterns eingehalten werden; der Buildumfang wird über `cfg`-Attribute bzw. Ziel-Tripel gesteuert.

### 5.1 Provider-Richtlinien
- Liefere sämtliche Positionsangaben (`Bounds`, `ActivationPoint`, `ActivationArea`, Fensterrahmen) im Desktop-Koordinatensystem (linke obere Ecke des Primärmonitors = Ursprung). Berücksichtige DPI-Skalierung und Multi-Monitor-Layouts.
- Spiegle native Rollennamen in `control:Role` bzw. `item:Role` (lokaler Name für XPath) und hinterlege die Originalrollen zusätzlich unter `native:*`, um Technologie-spezifische Detailabfragen zu erlauben.
- Pflege `SupportedPatterns` konsistent: Ein Pattern darf nur gemeldet werden, wenn alle Pflichtattribute verfügbar sind. Optionale Attribute werden als `null` oder ausgelassen, nicht mit Platzhalterwerten gefüllt.
- Ergänze `app:`-Attribute (z. B. `ProcessId`, `ProcessName`) für Wurzel- und Applikationsknoten, damit Clients Prozesse eindeutig zuordnen können.
- Ermittle nach Möglichkeit den `AcceptsUserInput`-Status für `Application`-Knoten (unter Windows via `WaitForInputIdle`, auf anderen Plattformen best effort über Toolkit/Accessibility-Daten); wenn nicht ermittelbar, Attribut weglassen oder `null` setzen.
- Stelle sicher, dass `RuntimeId` pro Provider stabil bleibt, solange das zugrunde liegende Element existiert; bei Re-Creation darf sich die ID ändern.
- Typische Quellen für `RuntimeId`: UI Automation `RuntimeId`, AT-SPI D-Bus-Objektpfad auf dem Accessibility-Bus, macOS `AXUIElement` Identifier (kombiniert mit Prozessinformationen). Fehlt eine native Kennung, generiere eine deterministische ID, die während der Lebensdauer des Elements stabil bleibt.
- Dokumentiere Mapping-Entscheidungen in `docs/patterns.md`, wenn native APIs mehrere Möglichkeiten bieten (z. B. AX-Subrole vs. Role).
- Nutze die in `docs/provider_checklist.md` gepflegten Prüfschritte, bevor Provider-Änderungen gemergt werden (manuelle Review + automatisierte Tests).

## 6. Geräte- und Interaktionsdienste
- `DeviceProvider`-Trait + Capability-Typen leben in `crates/core` (Pointer, Keyboard, Touch, Display, Capture, Highlight).
- Implementierungen:
  - `platform-windows`: `SendInput`/`InjectTouchInput`, Desktop Duplication/BitBlt, Overlays.
  - `platform-linux-x11`: `x11rb` + XTEST, Screenshots via X11 `GetImage`/Pipewire, Overlays.
  - `platform-linux-wayland` (optional): Wayland-APIs (Virtuelles Keyboard, Screencopy, Portal-Fallbacks).
  - `platform-macos`: `CGEvent`, `CGDisplayCreateImage`, transparente `NSWindow`/CoreAnimation.
- `platform-mock` stellt In-Memory-Devices, Event-Logging und Highlight/Capture-Simulation bereit; unterstützt JSON-RPC-Tests.

## 7. Window-Management-Schicht
- `WindowManager`-Trait im `core`-Crate; Implementierungen in den Plattform-Crates.
- Funktionen: Fensterlisten, Aktivieren/Minimieren/Maximieren/Restore, `move`/`resize`, Fokus setzen; Zugriff auf native Windowing-APIs (`HWND`, X11 Window IDs, `NSWindow`).
- Linux: Runtime entscheidet anhand `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, XWayland-Anwesenheit zwischen X11- und Wayland-Pfaden.
- `platform-mock` liefert deterministische Window-Manager-Mocks für Tests.

-## 8. JSON-RPC Provider & Adapter
- `provider-jsonrpc` stellt einen klar definierten JSON-RPC 2.0-Vertrag für externe Sprachen bereit. Kernkomponenten:
  - **Transport:** Named Pipes unter Windows (`\\.\pipe\PlatynUI+<pid>+<user>+<id>`), Unix Domain Sockets (`/tmp/platynui.<pid>.<user>.<id>`) oder Loopback TCP (per Konfiguration). Die Runtime stellt keine Transportinstanzen bereit, sondern verbindet sich mit dem vom Provider bereitgestellten Endpunkt. Sicherheitsanforderungen (ACLs/Tokens) liegen beim Provider.
  - **Handshake (`initialize`):** Provider melden Version, Technologiekennung, RuntimeId-Schema, Heartbeat-Intervalle/Zeitouts sowie optionale Zusatzinformationen (z. B. eigene Namensräume). Welche Rollen/Pattern letztlich verfügbar sind, ergibt sich aus den gelieferten Baumdaten.
  - **Knoten-API:** `listApplications`, `getRoot`, `getNode`, `getChildren`, `getAttributes`, `getSupportedPatterns`, optional `resolveRuntimeId`, `ping`. Alle Antworten liefern normalisierte Attribute (`control:*`, `item:*`, `app:*`, `native:*`).
  - **Events:** `$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`. Diese halten den Runtime-Baum synchron, enthalten aber keine Pattern-spezifischen Aktionen.
  - **Heartbeat/Recovery:** Runtime sendet periodisch `ping`; bleibt eine Antwort aus (`timeout`), wird der Provider als offline markiert und Abfragen schlagen mit definiertem Fehlercode fehl.
  - **Design-Vorbild:** Handshake, Capability-Negotiation und Nachrichtenfluss lehnen sich an etablierte Standards wie das Language Server Protocol (LSP) sowie das Model Context Protocol (MCP) an. Wir übernehmen deren Prinzipien (Versionskennzeichnung, optionale Erweiterungen, klar getrennte Rollen von Host/Client/Server) und passen sie auf UI-Automation zu, ohne die Protokolle 1:1 zu replizieren.
- JSON-RPC-Provider decken ausschließlich den UI-Baum ab; Eingabe- und Window-Management-Funktionen bleiben den Plattform-Crates der Runtime vorbehalten.
- Der Adapter kapselt sämtliche JSON-RPC-spezifischen Typen (Requests, Notifications, Fehler) und mappt sie auf die internen Provider-Traits.

## 9. XPath-Integration
- `UiXdmNode` adaptiert `UiNode` an die XPath-Engine (`NodeKind::Document`, `Element`, `Attribute`).
- Iteratoren liefern Kinder/Attribute lazy; `UiValue` → XPath-Atomics. `doc_order_key` beschleunigt Dokumentordnung.
- `UiStaticContext` registriert Präfixe (`control`, `item`, `app`, `native`) und optionale Funktionen (`control:nearby`, `control:contains_point`).
- Typische Abfrage: `app:Application[@Name='Foo']/control:Window//control:Button[@Name='Ok']`; Items lassen sich über Ausdrücke wie `control:List/item:ListItem` adressieren. Generische Tests können `*[local-name()='Button']` nutzen (Standardnamespace `control`).

## 10. Runtime-Pipeline & Komposition
1. **Runtime (`crates/runtime`)** – verwaltet `PlatformRegistry`/`PlatformBundle`, lädt Desktop (`UiXdmDocument`), evaluiert XPath (Streaming), triggert Highlight/Screenshot.
2. **Server (`crates/server`)** – JSON-RPC-2.0-Frontend (Language-Server-ähnlich) für Remote-Clients.
3. **Pipelines** – Mischbetrieb (z. B. AT-SPI2 + XTEST) möglich; Plattform-Erkennung wählt Implementierungen zur Laufzeit.
4. **Application Readiness** – Runtime stellt Hilfsfunktionen bereit, um `Application`-Knoten auf `AcceptsUserInput` zu prüfen (Windows nutzt `WaitForInputIdle`; andere Plattformen liefern bestmöglich heuristische Werte oder `null`). Diese Informationen werden nicht gecacht, sondern bei Bedarf abgefragt.

> Hinweis: Die Runtime lädt und bewertet nur die aktuell vorliegenden Knoten. Wenn Elemente erst durch Benutzerinteraktion erscheinen (z. B. Scrollen, Paging, Kontextmenüs), müssen Clients dieselben Eingaben auslösen wie ein Mensch vor dem Bildschirm. So behalten Automationen identische Freiheitsgrade wie interaktive Anwender.

## 11. Werkzeuge auf Basis der Runtime
1. **CLI (`crates/cli`)** – Befehle `query`, `highlight`, `watch`, Ausgabe in JSON/YAML/Tabellen, Filteroptionen, optionaler REPL; nutzt Runtime direkt oder via JSON-RPC.
2. **Inspector (GUI)** – Tree-Ansicht, Property-Panel (`control:*`, `item:*`, `native:*`), XPath-Editor (Autocompletion), Ergebnisliste, Highlighting, Element-Picker, Export/Logging; arbeitet eingebettet oder über `crates/server`.

## 12. Nächste Schritte
1. **Core & XPath** – Attributkatalog finalisieren, `UiXdmNode`-Prototyp entwickeln, Tests schreiben.
2. **Provider & JSON-RPC** – Crates (`provider-windows-uia`, `provider-atspi`, `provider-macos-ax`, `provider-mock`, `provider-jsonrpc`) anlegen; JSON-RPC-Schema/Registrierung/Heartbeats implementieren.
3. **Devices & Interaktion** – Plattform-Devices fertigstellen, Screenshot/Highlight-PoCs, Fallback-Strategien definieren.
4. **Runtime & Server** – Runtime-API, Fehlerbehandlung, Provider-Registry und JSON-RPC-Server (Sicherheitsgrenzen) umsetzen.
5. **Tooling** – CLI-MVP, Inspector-Prototyp, Integrationstests mit `platform-mock`/`provider-mock`.
6. **Optionale Erweiterungen** – Wayland-spezifische Bausteine, Performance-Tuning, Community-Dokumentation.
