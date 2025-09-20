# Umsetzungsplan PlatynUI XPath-Runtime

> Lebendes Dokument: Der Plan dient als Ideensammlung und Priorisierungshilfe. Reihenfolge, Umfang und Aufgaben können sich während der Umsetzung ändern.

Dieser Plan beschreibt die Umsetzungsschritte, Aufgabenpakete und Abhängigkeiten für die Entwicklung der neuen XPath-basierten Runtime sowie der zugehörigen Werkzeuge. Er baut auf dem Konzept in `docs/architekturkonzept_runtime.md` auf und folgt einer komponentenorientierten Struktur ohne feste Zeitplanung.

## Gesamtziele
- Einheitliches UI-Baum-Modell mit `ui`-, `app`- und `native`-Namespaces.
- Plattformunabhängige XPath-Auswertung mit normalisierten Rollen und stabilen `RuntimeId`s.
- Modularisierte Provider- und Device-Schichten inklusive Mock-Implementierungen.
- Runtime mit JSON-RPC-Anbindung, CLI-Tooling und UI-Inspector.

## 1. Repository- und Projektstruktur
- [ ] Verzeichnis-Layout erweitern: `crates/core`, `crates/runtime`, `crates/server`, `crates/platform-windows-core` (optional), `crates/platform-windows`, `crates/platform-linux-x11`, `crates/platform-linux-wayland` (optional), `crates/platform-macos`, `crates/platform-mock`, `crates/provider-windows-uia`, `crates/provider-atspi`, `crates/provider-macos-ax`, `crates/provider-jsonrpc`, `crates/provider-mock`, `crates/cli`, `apps/inspector` (Platzhalter), `docs/` aktualisieren.
- [ ] Optional: Code-Owners/Team-Zuständigkeiten pro Plattformcrate notieren.
- [ ] Gemeinsame Cargo-Workspace-Konfiguration prüfen/aktualisieren; neue Crates mit `cargo new --lib`/`--bin` anlegen.
- [ ] Coding-Guidelines im README oder CONTRIBUTING ergänzen (Namespaces, Rollen, Attributkonventionen).

## 2. Core-Datenmodell & XPath
- [ ] Trait `UiNode` um Zugriffshilfen (`attribute`, `as_handle`) erweitern, `Attribute`-Interface auf PascalCase-Keys ausrichten.
- [ ] `UiValue`-Enum definieren (String, Number, Bool, Rect, Json, Native) inkl. Utility-Funktionen und Tests.
- [ ] `UiSnapshot` implementieren (lebensdauergebundene, teilbare Ansicht); Serde-Unterstützung für Debugging prüfen.
- [ ] `UiXdmNode`-Adapter schreiben und in `crates/xpath` integrieren; Namespaces (`ui`, `app`, `native`) registrieren.
- [ ] Automatisierte Tests mit synthetischem Baum (Dokument → App → Window → Button) und XPath-Abfragen (Elementnamen + Attribute + `local-name()`).
- [ ] Attributkatalog als Rust-Struktur/Konstante pflegen, Lint oder Test, der Provider-Ausgaben prüft.

## 3. Provider-Schicht
### 3.1 Basismodul `crates/core`
- [ ] Traits und Typen für `UiTreeProvider`, `DeviceProvider`, `WindowManager`, Pattern-Registry; Metadaten (Name, Plattform, Fähigkeiten) und Baum-Ereignistypen (`NodeAdded`, `NodeUpdated`, `NodeRemoved`) zur Synchronisation mit der Runtime.
- [ ] Gemeinsame Typen für Registrierungsinfo (`ProviderDescriptor`), Fehler (`ProviderError`) und Lifecycle (`ProviderHandle`).
- [ ] Helper für Normalisierung von Rollen → Elementnamen, Mapping-Utilities, Pattern-Zuordnung.

### 3.2 Plattform-Adapter
- [ ] `platform-windows` (ggf. ergänzt durch `platform-windows-core`): Desktop-Erkennung, Prozess → App-Zuordnung (UIA), Window/Control-Hierarchie, Attribute-Füllung, Baum-Events (StructureChanged) für Runtime-Sync.
- [ ] `platform-linux-x11`: AT-SPI2-Anbindung via D-Bus (X11), Mapping Application → Frame → Children, Zusatzattribute (Workspace, Window-ID).
- [ ] `platform-linux-wayland` (optional): Vorbereitende Infrastruktur für Wayland (Portal-/XWayland-Interaktion).
- [ ] `platform-macos`: macOS AX API (NSWorkspace + AXUIElement), Fensterauflistung, Properties.
- [ ] Gemeinsame Tests: App- und Window-Knoten mit erwarteten `ui:*`/`native:*`-Attributen.

### 3.3 JSON-RPC-Provider
- [ ] OpenRPC/JSON-Schema erstellen (`initialize`, `listApplications`, `getNode`, `shutdown`, Baum-Event-Notifications, `nodeName`).
- [ ] Registrierungsmechanismus (named pipe/unix socket/loopback) inkl. Token-/ACL-Validierung.
- [ ] Client in `runtime` zur Verwaltung mehrerer Instanzen (Heartbeat, Reconnect).
- [ ] Referenzadapter `crates/provider-jsonrpc` implementieren (Routing zu Runtime, Fehlerbehandlung, Beispielskripte).
- [ ] Dokumentation für externe Implementierer (FAQ, Sicherheits- und Deployment-Hinweise).

### 3.4 Mock-Provider
- [ ] `provider-mock`: `StaticMockTree`, `ScriptedMockTree`, Builder-API, deterministische `RuntimeId`s (nutzt Hilfen aus `platform-mock`).
- [ ] Automatisierter Test-Rahmen (Integrationstest) für Mock-Provider + Mock-Devices.
- [ ] Test-Hilfen (`assert_xpath_result`, `assert_namespace`) und Fixtures für Integrationstests.

## 4. Geräte-Schicht
### 4.1 Traits & Utilities (`crates/core`)
- [ ] `DeviceProvider`-Trait samt Capability-Typen (`DeviceCapability`, `DeviceError`, `Unsupported`).
- [ ] Abstraktionen für Koordinatensysteme, DPI/Scaling und Rechteck-Serialisierung.
- [ ] Hilfsfunktionen zur Konvertierung zwischen nativen Handles und XPath-relevanten Attributen.

### 4.2 Plattform-Implementierungen
- [ ] `platform-windows`: `SendInput`/`InjectTouchInput`, Desktop Duplication/BitBlt, Overlay-Fenster.
- [ ] `platform-linux-x11`: `x11rb`/`xtest`, `GetImage`/Pipewire-Screenshot, Overlay-Fenster.
- [ ] `platform-linux-wayland` (optional): Wayland-Schnittstellen (Seat, Virtual Keyboard, Screencopy, Portal-Fallbacks).
- [ ] `platform-macos`: `CGEvent`, `CGDisplayCreateImage`, transparente `NSWindow`/CoreAnimation.

### 4.3 Mock-Devices (`platform-mock`)
- [ ] In-Memory-Implementierungen für Devices (optional Eventlogging), Erfolgs-/Fehlerpfade und Window-Manager-Simulation.
- [ ] Tests, die `highlight`, `capture`, Window-Aktionen und Input-Sequenzen prüfen.
- [ ] JSON-RPC-kompatibler Mock-Server (optional) für externe Provider-Tests.
- [ ] Tooling für deterministische Playback-Szenarien (Aktionen/Aktionen).

## 5. Window-Management
### 5.1 Trait & Utilities (`crates/core`)
- [ ] `WindowManager`-Trait definieren (Operationen `list_windows`, `activate`, `minimize`, `maximize`, `restore`, `move/resize`, `close`).
- [ ] Gemeinsame Helper für Handle-Konvertierung, Fokusverwaltung und Fehlercodes.

### 5.2 Plattform-Implementierungen
- [ ] `platform-windows`: Win32 APIs (`SetForegroundWindow`, `ShowWindow`, `MoveWindow`, `GetWindowPlacement`).
- [ ] `platform-linux-x11`: EWMH via `x11rb`/`xcb`, optionale Portal-Fallbacks.
- [ ] `platform-linux-wayland` (optional): Wayland-Protokolle (Seat, Window Management, Screencopy) inkl. XWayland-Fallback.
- [ ] `platform-macos`: `NSWindow`, Accessibility, AppleScript.
- [ ] Integration in Provider-/Runtime-Schicht: Window-Handles über `native:BackendId` bereitstellen, Mapping App ↔ Fenster synchronisieren.

### 5.3 Mock & Infrastruktur (`platform-mock`)
- [ ] Mock-Implementierung (deterministische Fensterliste, Aktionsergebnisse, Fehlerpfade).
- [ ] JSON-RPC-Erweiterung für fensterbezogene Befehle (`window.activate`, `window.move`, ...).
- [ ] Plattform-Erkennung unter Linux implementieren (X11 vs. Wayland, XWayland-Fallback) und dynamisches Laden des passenden Plattform-Crates in der Runtime.

## 6. Pattern-System & Fähigkeiten
- [ ] Pattern-Katalog in `docs/patterns.md` finalisieren (Trait-basierte Capability-Patterns wie `Application`, `TextContent`, `TextEditable`, `Selectable`, `StatefulValue`, `Activatable`, `ActivationTarget`, `WindowSurface`, `Highlightable` etc.).
- [ ] Mapping-Tabelle (UIA ↔ AT-SPI2 ↔ AX) pflegen und bei neuen Patterns erweitern; Dokumentation der Mapping-Entscheidungen je Provider.
- [ ] Rust-Typen (`SupportedPattern`, Registry/Resolver) implementieren, String-IDs ↔ Enum/Bitflags abbilden und Erweiterbarkeit gewährleisten.
- [ ] `UiNode`-Datenmodell um `ui:SupportedPatterns` und patternbezogene Attribute erweitern; Provider verpflichten, Pflichtattribute je Pattern zu liefern und alle Koordinaten (`Bounds`, `ActivationPoint`, `ActivationArea`) im Desktop-Koordinatensystem bereitzustellen.
- [ ] Plattformübergreifende Abfrage für `Application.AcceptsUserInput` definieren (Windows: `WaitForInputIdle`; Linux/macOS: dokumentierte Heuristiken oder `null`), Runtime-Hilfsfunktion implementieren.
- [ ] Runtime-API auf Fokus- und Fensteraktionen beschränken (`focus()`, `activate()`, `maximize()`, …); übrige Interaktionen bleiben Client-Aufgabe.
- [ ] Provider-Guidelines (Koordinatensystem, Rollenabbildung, Pflichtattribute) konsolidieren und in Architekturkonzept aktuell halten.
- [ ] Tests: Mock-Provider/Devices mit Pattern-Kombinationen (z. B. Textfeld = `TextContent` + `TextEditable` + `TextSelection` + `Focusable`; Button = `TextContent` + `Activatable` + `ActivationTarget`), Abgleich der Pflichtattribute, Validierung der Koordinaten und Fensteraktionen.

## 7. Runtime (`crates/runtime`)
- [ ] `PlatformRegistry` + `PlatformBundle` (Tree-, Device-, Capture-, Highlight-Handles).
- [ ] Provider-Registrierung (Native & JSON-RPC), Capability-Prüfung, Heartbeat-Handling.
- [ ] Query-Pipeline: Dokument-Lade/Cache-Mechanismus, `evaluate_expr`, Streaming/Limit, Fehlerpropagation.
- [ ] Highlight-/Capture-Trigger in Verbindung mit Query-Ergebnissen.
- [ ] Logging/Tracing (z. B. `tracing`-Crate) für Abfragen, Providerereignisse, Geräteoperationen.
- [ ] Integrationstests: Mock-Provider + Mock-Devices, Abfrage + Highlight, Rekontextualisierung via `RuntimeId`.

## 8. JSON-RPC-Server (`crates/server`)
- [ ] Listener (Loopback + UNIX/Named Pipe) mit Authentisierung.
- [ ] Request-Routing zu Runtime (XPath-Abfragen, Device-Befehle, Highlight, Capture).
- [ ] Baum-Event-Streaming (z. B. `$/treeChanged`) an Clients zur Aktualisierung des UI-Baums.
- [ ] Fehler-Handling und Logging, Abschlussszenarien (`shutdown`).
- [ ] Systemtests: Server + Runtime + Mock-Provider, CLI-Client als Verbraucher.

## 9. Tooling
### 9.1 CLI (`crates/cli`)
- [ ] Basis-CLI (clap o. ä.) mit Befehlen `query`, `highlight`, `watch`.
- [ ] Ausgabeformate (JSON, YAML, Tabelle), Filteroptionen (`--provider`, `--window`, `--limit`).
- [ ] REPL-Modus (History, Autocomplete optional).
- [ ] Tests mit Mock-Runtime (`platform-mock` + Runtime) und Dokumentation/Beispiele im README.

### 9.2 UI-Inspector (`apps/inspector`)
- [ ] Technologie auswählen (Tauri, egui, wgpu etc.) und Boilerplate aufsetzen.
- [ ] Tree-Ansicht + Property-Panel (UI-Thread-Synchronisation, Baum-Event-Handling).
- [ ] XPath-Editor mit sofortiger Ausführung, Ergebnisliste, optionaler Highlight-Schalter.
- [ ] Element-Picker (Device-Highlight → Auswahl → XPath-Generierung).
- [ ] Export-/Logging-Funktionen; Settings für Provider-/Runtime-Verbindungen.
- [ ] Smoke-Tests mit Mock-Runtime und Integrationstests mit `platform-mock`/`provider-mock`.

## 10. Qualitätssicherung & Dokumentation
- [ ] Style/Lint: `cargo fmt`, `cargo clippy`, `ruff`, `mypy` in CI konfigurieren.
- [ ] Integrationstests für kritische Pfade (Provider ↔ Runtime ↔ Devices ↔ CLI/UI).
- [ ] Beispiel-XPath-Ausdrücke und Tutorials (CLI, Inspector) in `docs/` hinzufügen.
- [ ] Contribution-Guidelines für externe Provider/Devices/JSON-RPC-Implementierungen.
- [ ] Issue-Template für neue Plattformen/Funktionen.
- [ ] Provider-Checkliste (`docs/provider_checklist.md`) in Review-Template aufnehmen und automatisierte Checks (Koordinaten, Pflichtattribute, Pattern-Konformität) implementieren.

## 11. Erweiterungen & Backlog
- Wayland-Unterstützung (AT-SPI2 + Wayland-Devices).
- Zusätzliche Devices (Gamepad, Stift, Sprachsteuerung).
- Remote-Provider über gesicherte Tunnel.
- Performance-Tuning (Caching, Delta-Updates, Binary Encoding).
- Community-Beispiele, Plugins, Third-Party-Provider.

## Abhängigkeiten & Reihenfolge (Empfehlung)
1. Repository-Struktur + Core (Abschnitte 1–2) schaffen die Basis für alle weiteren Arbeiten.
2. Provider-Basis + Mock (3.1 & 3.4) zusammen mit Devices-Mock (4.3) ermöglichen End-to-End-Tests.
3. Window-Management-Trait (5) etablieren, damit Runtime/Patterns auf konsistente Fensteraktionen zurückgreifen können.
4. Pattern-System (6) integrieren, dann Runtime (7) aufbauen und JSON-RPC-Server (8) anflanschen.
5. CLI (9.1) als erster Konsument; Inspector (9.2) sobald Runtime+Server stabil sind.
6. Native Provider/Devices parallelisieren (3.2 & 4.2) unter Nutzung des Window-Managers; optionale Wayland-Komponenten nachgelagert.
7. Erweiterungen (Abschnitt 11) nach Stabilisierung des Basissystems priorisieren.
