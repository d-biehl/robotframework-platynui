# Umsetzungsplan PlatynUI Runtime

> Lebendes Dokument: Wir pflegen diesen Plan fortlaufend und passen ihn bei neuen Erkenntnissen oder Prioritäten an.

## Ausgangspunkt & Zielbild
- Konsistenter Desktop-zentrierter UI-Baum mit den Namespaces `control` (Standard), `item`, `app` und `native`.
- Trait-basierte Pattern-Schicht gemäß `docs/patterns.md`, mit Runtime-Aktionen (`Focusable`, `WindowSurface`, `Application`) und Attribut-basierter Datenlage (z. B. `ActivationTarget`, Auswahl-/Textinformationen).
- Plattformen werden strikt in zwei Crate-Gruppen aufgeteilt: (1) Geräte-/Window-Manager-Schicht unter `crates/platform-<ziel>` mit Paketnamen `platynui-platform-<ziel>` und (2) UiTreeProvider unter `crates/provider-<technik>` mit Paketnamen `platynui-provider-<technik>`. Neue Technologien müssen genau diesem Schema folgen; Abweichungen sind nicht erlaubt. Out-of-process-Anbindungen ergänzen das Set ausschließlich über `crates/provider-jsonrpc` (`platynui-provider-jsonrpc`).
- Runtime verwaltet Provider, Devices, WindowManager, XPath-Pipeline, Highlighting und Screenshot-Funktionen.
- CLI und Inspector dienen als Referenzwerkzeuge über Runtime bzw. JSON-RPC-Server.
- Fokus der ersten Iterationen liegt auf Windows (UIA) und Linux/X11 (AT-SPI2); macOS-Implementierungen folgen, sobald diese beiden Plattformen stabil laufen.
- Aktuelle Entwicklungsumgebung: WSL2 (Linux) mit Möglichkeit zum Cross-Build für Windows-Binaries; Windows-spezifische Tests laufen soweit machbar direkt oder per CI.
- Rust-Toolchain: `rustc 1.90.0 (1159e78c4 2025-09-14)` ist die Basis. Wir verfolgen die zugehörigen Release Notes und nutzen neue Sprach-/Standardbibliotheksfeatures soweit sinnvoll (z. B. aktuelle `let-else`/`if-let`-Verbesserungen, stabilisierte Traits, `async`-Erweiterungen).

## Querschnittsrichtlinien
- Dokumentationssprache: Planungskern (Konzept, Plan, Patterns, Checklisten) bleibt deutsch; alle übrigen Repository-Dateien führen wir auf Englisch.
- Abhängigkeiten: Beim Hinzufügen oder Aktualisieren immer die aktuell stabil veröffentlichte Version verwenden. Dafür `cargo search`, crates.io-API oder `cargo outdated` einsetzen und Versionsstand explizit im Review erwähnen.
- Crate-Erstellung: In jeder neuen oder überarbeiteten `Cargo.toml` muss der Eintrag `package.name` mit `platynui-` beginnen. Verzeichnisnamen dürfen kürzer sein (`crates/runtime` → `platynui-runtime`), entscheidend ist der Paketname. Bei Reviews aktiv prüfen, dass keine Ausnahme entsteht.
- Tests: In Rust-Unit- und Integrationstests konsequent `rstest` einsetzen und dessen Features nutzen (Fixtures, `#[rstest]` mit `case`/`matrix`, parametrische Tests). Bestehende Tests bei Anpassungen entsprechend migrieren.
- Typbenennung: Neue Rust-Typen (Structs, Enums, Traits) folgen dem üblichen Namensraum über Module – kein zusätzlicher `Platynui`-Präfix nötig. Stattdessen aussagekräftige Namen im entsprechenden Modul wählen (`RuntimeState`, `WindowsDeviceBundle`, ...).
- Planpflege: Nach jedem Arbeitspaket den Plan aktualisieren, erledigte Aufgaben abhaken und ggf. neue Erkenntnisse ergänzen. Der Umsetzungsplan bleibt so synchron zur tatsächlichen Umsetzung.

## Arbeitsbereiche
Die folgenden Kapitel listen Aufgabenpakete; Reihenfolgen innerhalb eines Abschnitts sind Empfehlungen, keine starre Vorgabe.

### 1. Fundament & Repository-Struktur
- [x] Workspace aufsetzen/aufräumen: `crates/core`, `crates/runtime` (Crate `platynui-runtime`), `crates/server` (Crate `platynui-server`), `crates/platform-{windows,linux-x11,linux-wayland?,macos,mock}` (Crates `platynui-platform-*`), `crates/provider-{windows-uia,atspi,macos-ax,jsonrpc,mock}` (Crates `platynui-provider-*`), `crates/cli` (Crate `platynui-cli`), `apps/inspector` (Crate `platynui-inspector`; alternativ eigenes Crate).
- [x] Gemeinsame Cargo-Einstellungen (Edition, Lints, Features) und Rustfmt/Clippy-Konfiguration vereinheitlichen.
- [x] README/CONTRIBUTING aktualisieren: Namenskonventionen (PascalCase-Attribute, Namespaces), Architekturüberblick, Hinweis auf lebende Konzeptdokumente.
- [x] Dev-Tooling notieren (`uv`, `cargo`, Inspector-Abhängigkeiten) und Basis-Skripte (Format/Lint/Test).

### 2. Core-Datenmodell & XPath-Grundlagen
- [x] `UiNode`-/`UiAttribute`-Traits einführen und den alten Struct-/Builder-Ansatz vollständig entfernen (kein Übergangs-Mockbaum im Core).
- [x] Runtime-Wrapper für `UiNode`/`UiAttribute` implementieren (direkte `Arc<dyn UiNode>`-Adapter ohne Snapshot, kontextabhängige Invalidierung optional).
- [x] `UiPattern`-Basistrait plus `UiNode::pattern::<T>()`-Lookup implementieren; `PatternRegistry` in `platynui-core` stellt konsistente Speicherung (`PatternId` → `Arc<dyn UiPattern>`) sicher. Contract-Tests folgen, um Diskrepanzen zwischen `supported_patterns()` und abrufbaren Instanzen aufzudecken.
- [x] `UiValue` definieren: strukturierte Werte (`Rect`, `Point`, `Size`, `Integer`) und JSON-kompatible Konvertierungen.
- [x] Namespace-Registry (`control`, `item`, `app`, `native`) und Hilfsfunktionen implementieren.
- [x] Evaluation-API auf `EvaluationItem` (Node/Attribute/Value) umstellen und Konsumenten/Tests anpassen (Kontext per `Option<Arc<dyn UiNode>>`).
- [ ] Kontextknoten außerhalb des aktuellen Wurzelknotens über `RuntimeId` neu auflösen. (Backlog) – derzeit liefert die Runtime bei ungültigen Kontextknoten einen kontrollierten Fehler; ein Resolver folgt bei Bedarf.
- [x] Basis-Validator implementieren (`validate_control_or_item`), der aktuell nur doppelte `SupportedPatterns` meldet; weitere Attribut-Prüfungen erfolgen pattern- bzw. provider-spezifisch. `UiAttribute`-Trait + XPath-Wrapper bleiben bestehen.
- [x] Dokumentwurzel „Desktop“ samt Monitor-Alias-Attributen (Bounds.X usw.) beschreiben und Tests erstellen.

### 3. Pattern-System
- [x] Runtime-Pattern-Traits in `platynui-core` anlegen (`FocusablePattern`, `WindowSurfacePattern`, `ApplicationPattern`) inkl. `PatternError`-Fehlertyp; rein lesende Patterns verbleiben bei Attributen.
- [x] Runtime-Aktionsschnittstellen der Patterns (z. B. `FocusablePattern::focus()`, `WindowSurfacePattern::maximize()`) präzisieren und Beispiel-Implementierungen samt Tests dokumentieren; nur diese Pattern dürfen Laufzeitaktionen anbieten. (Umgesetzt via `FocusableAction`, `WindowSurfaceActions`, `ApplicationStatus` + rstest-Coverage.)
- [x] Leitfaden für `SupportedPatterns`-Verwendung (Dokumentation erledigt; providerseitige Tests folgen in den konkreten Provider-Szenarien), damit Pattern-Kombinationen nachvollziehbar bleiben.
- [x] Provider-facing Contract-Tests: Core-Testkit (`platynui_core::ui::contract::testkit`) meldet fehlende/abweichende Alias-Werte (`Bounds.*`, `ActivationPoint.*`) und stellt Hilfen für Pattern-Erwartungen bereit.
- [x] Mapping-Hilfen zwischen Patterns und Technologie-spezifischen APIs (UIA-ControlType, AT-SPI Rollen, AX Attribute) ergänzt. `docs/patterns.md` enthält eine Orientierungstabelle; Provider dokumentieren Abweichungen individuell.
- [x] Patterns-Dokument (`docs/patterns.md`) aktualisiert: Alias-Hinweise, Rollenkatalog und Mapping-Tabelle für UIA/AT-SPI/AX ergänzt; bleibt ein lebendes Dokument für weitere Erweiterungen.

### 4. Provider-Infrastruktur (Core)
- [x] Traits `UiTreeProvider`, `UiTreeProviderFactory` plus Basistypen (`ProviderDescriptor`, `ProviderEvent`, Fehler) definiert; Lifecycle-Erweiterungen (Events weiterreichen, Shutdown) folgen beim Runtime-Wiring.
- [x] `ProviderRegistry` im Runtime-Crate sammelt registrierte Factories via `inventory`, sortiert nach Technologie/Priorität und erzeugt Instanzen.
- [x] Event-Pipeline auf Runtime-Seite: Dispatcher verteilt Ereignisse an registrierte Sinks, Shutdown leert Abonnenten, Runtime ruft `UiTreeProvider::subscribe_events(...)` für alle Provider auf und stellt über `register_event_sink` eine einfache Erweiterungsstelle bereit.
- [x] Inventory-basierte Registrierungsmakros (`register_provider!`, `register_platform_module!`), inkl. Tests für Registrierungsauflistung; weitere `cfg`-Szenarien folgen bei der Runtime-Einbindung.
- [x] Factory-Lifecycle: Entscheidung dokumentiert – Provider erhalten bewusst nur `Arc<dyn UiTreeProvider>` ohne zusätzliche Services; Geräte/Window-Manager bleiben in der Runtime. Optionales Erweiterungs-Interface bleibt ein Backlog-Thema.
- [x] Provider-Checkliste (`docs/provider_checklist.md`) via Contract-Test-Suite abgedeckt (Mock-Provider nutzt `contract::testkit`; künftige Provider erhalten dieselben Prüfungen, Tests laufen unter `cargo test`).

### 5. Native Provider (UiTree)
#### Phase 1 – Windows (UIA)
- [ ] `platynui-provider-windows-uia`: UIA-Wrapper (COM-Helfer ggf. in `platynui-platform-windows`), Rollennormalisierung (ControlType → lokale Namen), `RuntimeId`-Weitergabe, `AcceptsUserInput` via `WaitForInputIdle`/Fallback. Mapping-/Namensentscheidungen dokumentieren (Abgleich mit `docs/patterns.md`).
- [ ] Gemeinsame Tests (Provider vs. Mock) mit bereitgestelltem UI-Baum & XPath-Abfragen; Dokumentation von Abweichungen der UIA-API.

#### Phase 2 – Linux/X11 (AT-SPI2)
- [ ] `platynui-provider-atspi`: D-Bus-Integration, Baumaufbau (Application → Window → Control/Item), RuntimeId aus Objektpfad, Fokus- und Sichtbarkeitsflags. Plattform-spezifische Mapping-Entscheidungen (AT-SPI Rollen) im Provider festhalten.
- [ ] Ergänzende Tests (AT-SPI2) auf Basis des gleichen Testsets wie Windows, inklusive Namespaces `item`/`control`.

#### Backlog – macOS (AX)
- [ ] `platynui-provider-macos-ax`: AXUIElement-Brücke, Fenster-/App-Auflistung, RuntimeId aus AXIdentifier, Bound-Konvertierung (Core Graphics). AX-Rollen/Subrollen-Mapping dokumentieren und mit `docs/patterns.md` abgleichen.
- [ ] Plattformübergreifende Regressionstests um macOS-spezifische Unterschiede erweitern.

### 6. JSON-RPC-Provider & Out-of-Process Integration
- [ ] JSON-RPC 2.0 Vertrag dokumentieren (Markdown + JSON-Schema): Mindestumfang `initialize`, `getNodes(parentRuntimeId|null)`, `getAttributes(nodeRuntimeId)`, `getSupportedPatterns(nodeRuntimeId)`, optional `ping`; Events `$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`.
- [ ] `platynui-provider-jsonrpc` implementieren: Verbindung über Named Pipe/Unix Socket/localhost, Registrierung bei Runtime, Heartbeat/Timeout-Handling, Sicherheit (Namenskonvention „PlatynUI+PID+User+…“).
- [ ] Beispiel-Provider (Mock oder UIA-Proxy) dokumentieren; Leitfaden für Drittanbieter.
- [ ] Runtime-Client-Schicht (Multiplexing, Fehler-Mapping, Provider-Restart-Strategien).
- [ ] Handshake- und Capability-Design an LSP/MCP-Prinzipien ausrichten (Versionsangaben, optionale Fähigkeiten, klare Rollen), Dokumentation entsprechend ergänzen.

### 7. Mocking & Tests
- [ ] `platynui-platform-mock`: Geräte (Pointer, Keyboard, Display, Highlight, Screenshot) als In-Memory-Implementierungen, Logging für Assertions.
- [ ] `platynui-provider-mock`: `StaticMockTree`, Skriptbares Verhalten (z. B. über JSON/Szenario-Dateien), deterministische `RuntimeId`s.
- [ ] Integrationstest-Suite (Runtime + Mock-Provider + Mock-Devices) mit Beispiel-XPath-Abfragen, Pattern-Verifikation, Highlight-Simulation.
- [ ] Sämtliche neuen Unit- und Integrationstests mit `rstest`-Makros (Fixtures, `#[rstest]`, `#[case]`, `#[matrix]`) modellieren; bestehende Tests bei Gelegenheit migrieren.

### 8. Devices & Window-Management
- [ ] Traits `PointerDevice`, `KeyboardDevice`, `TouchDevice`, `DisplayInfo`, `ScreenshotDevice`, `HighlightOverlay` (Desktop-Koordinaten, DPI-Korrektur).
- [ ] WindowManager-Trait definieren (`activate`, `minimize`, `maximize`, `move`, `restore`, `bring_to_front`, Status-Abfragen) + Mapping in Patterns (`WindowSurface`).

#### Phase 1 – Windows
- [ ] `platynui-platform-windows`: Pointer/Keyboard via Win32 & UIAutomation Hilfen, Screenshot/Highlight (DComposition/GDI), Window-Manager-Bridge (SetForegroundWindow, Modal-Handling).
- [ ] Fokus-Helper (`focus_control`) mit UIA-Fallbacks und Integration in `Focusable`.

#### Phase 2 – Linux/X11
- [ ] `platynui-platform-linux-x11`: Pointer/Keyboard via XTest oder äquivalente APIs, Screenshot (XShm), Highlight (XComposite), Window-Manager-Integration (EWMH/NetWM).
- [ ] Fokus-Helper für AT-SPI2 + Window-Manager-Fallbacks.

#### Backlog – macOS
- [ ] `platynui-platform-macos`: Devices via Quartz/Event-Taps, Screenshot/Highlight mit CoreGraphics, Window-Manager via AppKit.

- [ ] Tests: Desktop-Bounds, Default-Click-Point aus `ActivationTarget`, Basiseigenschaften (`IsVisible`, `IsEnabled`, `IsOffscreen`).

### 9. Runtime-Kern
- [ ] Provider-Registry initialisieren (Inventory lesen, `cfg` prüfen, Prioritäten setzen), Provider-Lifecycle steuern.
- [ ] Provider-Auswahl nach `ProviderPriority` (Fallback-Strategie) und Routing mehrerer Provider derselben Technologie implementieren.
- [ ] Dokumentaufbau: Desktop-Wurzel laden, App- und Control-Nodes verknüpfen, `item`-Namespace an Container-Knoten hängen.
- [ ] `AcceptsUserInput`-Hilfsmethode (Windows `WaitForInputIdle`, Linux/macOS heuristische Implementierung), Rückfallverhalten dokumentieren.
- [ ] XPath-Auswertung → Iterator über `Arc<dyn UiNode>`, Filterung nach Patterns, Attribute-Lazy-Loading.
- [ ] API-Variante `evaluate(node: Option<Arc<dyn UiNode>>, xpath, options)` implementieren; `None` verwendet automatisch das Desktop-Dokument, ansonsten dient der übergebene Knoten als Kontext (`.//item:*`). Die Options-Struktur steuert nur, ob vor der Auswertung ein Provider-Refresh ausgelöst wird – persistentes Caching bleibt ein Backlog-Thema.
- [ ] Highlighting/Screenshot orchestrieren: Koordination zwischen Runtime, Devices, WindowManager.
- [ ] Fehler- & Telemetrieschnittstelle (Tracing, Logging, Metriken) entwerfen.

### 10. JSON-RPC-Server & APIs
- [ ] `crates/platynui-server`: JSON-RPC-Endpunkte für XPath-Abfragen, Fokus- und Fensteraktionen (über `Focusable`/`WindowSurface`), Highlighting, Screenshot, Provider/Device-Status sowie Heartbeat – keine generischen UI-Aktions-APIs.
- [ ] Security-Guidelines (lokaler Zugriff, Authentication-Optionen) definieren.
- [ ] Versionierung & Capability-Negotiation (Server ↔ Client) dokumentieren – Orientierung an LSP/MCP-Konzepten festhalten.

### 11. Werkzeuge
- [ ] CLI (`crates/platynui-cli`): Befehle `query`, `highlight`, `watch`, `dump-node`, `focus`, optional `--json`/`--yaml` Ausgabe.
- [ ] Inspector (GUI): Tree-Ansicht mit Namespaces, Property-Panel (Patterns), XPath-Editor, Element-Picker, Highlight; arbeitet wahlweise Embedded oder via JSON-RPC.
- [ ] Beispiel-Workflows dokumentieren (Readme/Docs): XPath → Highlight, Fokus setzen, AcceptsUserInput prüfen.

### 12. Qualitätssicherung & Prozesse
- [ ] CI-Pipeline: `cargo fmt --all`, `cargo clippy --all`, `cargo test --workspace`, `uv run ruff check .`, `uv run mypy src/PlatynUI packages/core/src` (sofern Python-Anteile relevant).
- [ ] Contract-Tests für Provider & Devices (pattern-spezifische Attribute, Desktop-Koordinaten, RuntimeId-Quellen).
- [ ] Dokumentation pflegen: Architekturkonzept, Patterns, Provider-Checkliste, Legacy-Analyse; Hinweis auf lebende Dokumente beibehalten.
- [ ] Release-/Versionierungsstrategie festlegen (SemVer pro Crate? Workspace-Version?).

### 13. Backlog & Explorations
- Optionaler Wayland-Support (Runtime-Erkennung Wayland/X11, Provider-Auswahl, Devices).
- Weitere Patterns (z. B. Tabellen-Navigation, Drag&Drop) nach Bedarf evaluieren.
- Erweiterte Eingabegeräte (Gamepad, Stift), Barrierefreiheits-Funktionen.
- Performance-Optimierungen (Delta-Updates, Snapshot-/Caching-Layer für XPath, Binary Transport).
- Community-Guides, Beispiel-Provider, Trainingsmaterial.

## Empfohlene Reihenfolge (High-Level)
1. Fundament (Abschnitt 1) + Core-Datenmodell (2) als Basis.
2. Pattern-System (3) und Provider-Infrastruktur-Core (4) definieren.
3. Mocking (7) und Devices/Window-Management (8) – zunächst Windows, danach Linux/X11 – für End-to-End-Prototyp sichern.
4. Runtime-Kern (9) mit XPath & Highlighting; parallel JSON-RPC-Provider (6) vorbereiten.
5. Native Provider (5) schrittweise: zuerst Windows, anschließend Linux/X11; macOS in den Backlog verschieben.
6. Server & Tools (10–11) aufsetzen, sobald Runtime stabil ist.
7. Qualitätssicherung (12) verankern, Backlog (13) nach Stabilisierung adressieren; macOS-Implementierungen folgen nach Windows/Linux.
