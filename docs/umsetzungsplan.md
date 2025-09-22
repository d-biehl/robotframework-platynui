# Umsetzungsplan PlatynUI Runtime

> Lebendes Dokument: Wir pflegen diesen Plan fortlaufend und passen ihn bei neuen Erkenntnissen oder Prioritäten an.

## Ausgangspunkt & Zielbild
- Konsistenter Desktop-zentrierter UI-Baum mit den Namespaces `control` (Standard), `item`, `app` und `native`.
- Trait-basierte Pattern-Schicht gemäß `docs/patterns.md`, mit Runtime-Aktionen (`Focusable`, `WindowSurface`) und Attribut-basierter Datenlage (z. B. `ActivationTarget`, Auswahl-/Textinformationen).
- Plattformen werden strikt in zwei Crate-Gruppen aufgeteilt: (1) Geräte-/Infrastruktur-Schicht unter `crates/platform-<ziel>` mit Paketnamen `platynui-platform-<ziel>` und (2) UiTreeProvider unter `crates/provider-<technik>` mit Paketnamen `platynui-provider-<technik>`. Neue Technologien müssen genau diesem Schema folgen; Abweichungen sind nicht erlaubt. Out-of-process-Anbindungen ergänzen das Set ausschließlich über `crates/provider-jsonrpc` (`platynui-provider-jsonrpc`).
- Runtime verwaltet Provider, Devices, XPath-Pipeline, Highlighting und Screenshot-Funktionen.
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
- [x] Basis-Validator implementieren (`validate_control_or_item`), der aktuell nur doppelte `SupportedPatterns` meldet; weitere Attribut-Prüfungen erfolgen pattern- bzw. provider-spezifisch. `UiAttribute`-Trait + XPath-Wrapper bleiben bestehen.
- [x] Dokumentwurzel „Desktop“ samt Monitor-Alias-Attributen (Bounds.X usw.) beschreiben und Tests erstellen.

### 3. Pattern-System
- [x] Runtime-Pattern-Traits in `platynui-core` anlegen (`FocusablePattern`, `WindowSurfacePattern`) inkl. `PatternError`-Fehlertyp; rein lesende Patterns verbleiben bei Attributen.
- [x] Runtime-Aktionsschnittstellen der Patterns (z. B. `FocusablePattern::focus()`, `WindowSurfacePattern::maximize()`) präzisieren und Beispiel-Implementierungen samt Tests dokumentieren; nur diese Pattern dürfen Laufzeitaktionen anbieten. (Umgesetzt via `FocusableAction`, `WindowSurfaceActions` + rstest-Coverage.)
- [x] Leitfaden für `SupportedPatterns`-Verwendung (Dokumentation erledigt; providerseitige Tests folgen in den konkreten Provider-Szenarien), damit Pattern-Kombinationen nachvollziehbar bleiben.
- [x] Provider-facing Contract-Tests: Core-Testkit (`platynui_core::ui::contract::testkit`) meldet fehlende/abweichende Alias-Werte (`Bounds.*`, `ActivationPoint.*`) und stellt Hilfen für Pattern-Erwartungen bereit.
- [x] Mapping-Hilfen zwischen Patterns und Technologie-spezifischen APIs (UIA-ControlType, AT-SPI Rollen, AX Attribute) ergänzt. `docs/patterns.md` enthält eine Orientierungstabelle; Provider dokumentieren Abweichungen individuell.
- [x] Patterns-Dokument (`docs/patterns.md`) aktualisiert: Alias-Hinweise, Rollenkatalog und Mapping-Tabelle für UIA/AT-SPI/AX ergänzt; bleibt ein lebendes Dokument für weitere Erweiterungen.

### 4. Provider-Infrastruktur (Core)
- [x] Traits `UiTreeProvider`, `UiTreeProviderFactory` plus Basistypen (`ProviderDescriptor`, `ProviderEvent`, Fehler) definiert; Lifecycle-Erweiterungen (Events weiterreichen, Shutdown) folgen beim Runtime-Wiring.
- [x] `ProviderRegistry` im Runtime-Crate sammelt registrierte Factories via `inventory`, gruppiert sie je Technologie und erzeugt Instanzen.
- [x] Event-Pipeline auf Runtime-Seite: Dispatcher verteilt Ereignisse an registrierte Sinks, Shutdown leert Abonnenten, Runtime ruft `UiTreeProvider::subscribe_events(...)` für alle Provider auf und stellt über `register_event_sink` eine einfache Erweiterungsstelle bereit.
- [x] Inventory-basierte Registrierungsmakros (`register_provider!`, `register_platform_module!`), inkl. Tests für Registrierungsauflistung; weitere `cfg`-Szenarien folgen bei der Runtime-Einbindung.
- [x] Factory-Lifecycle: Entscheidung dokumentiert – Provider erhalten bewusst nur `Arc<dyn UiTreeProvider>` ohne zusätzliche Services; Geräte/Window-Manager bleiben in der Runtime.
- [x] Provider-Checkliste (`docs/provider_checklist.md`) via Contract-Test-Suite abgedeckt (Mock-Provider nutzt `contract::testkit`; künftige Provider erhalten dieselben Prüfungen, Tests laufen unter `cargo test`).

### 5. CLI `list-providers` – Mock-Basis schaffen
- [ ] Minimalen Laufweg „Runtime + platynui-platform-mock + platynui-provider-mock“ herstellen (Provider-Registry initialisieren, Mock-Provider instanziieren).
- [ ] `platynui-platform-mock`: Grundgerüst mit Stub-Geräten & Logging, liefert zumindest Technologie-/Versionsinformationen.
- [ ] `platynui-provider-mock`: Registriert sich mit eindeutiger `ProviderDescriptor`, stellt einfache Baumdaten bereit.
- [ ] CLI-Kommando `list-providers`: Gibt registrierte Provider/Technologien aus (Name, Version, Aktiv-Status), unterstützt Text und JSON.
- [ ] Tests: Provider-Registry + CLI-Output gegen Mock-Setup; `rstest` verwenden.

### 6. CLI `info` – Desktop-/Plattform-Metadaten
- [ ] `DesktopInfoProvider`-Trait in `platynui-core` definieren (OS-/Monitor-Metadaten, Bounds) und in Runtime verankern.
- [ ] `platynui-platform-mock`: Liefert DesktopInfo-Daten (OS, Monitorliste, Auflösung) zum Testen.
- [ ] Runtime baut `control:Desktop`-Knoten aus `DesktopInfoProvider` und stellt Daten für CLI bereit.
- [ ] CLI-Kommando `info`: Zeigt Plattform, Desktop-Bounds, Monitore, verfügbare Provider. Ausgabe als Text/JSON.
- [ ] Tests: `info`-Kommando mit Mock-Daten (Mehrmonitor, OS-Varianten) validieren.

### 7. CLI `query` – XPath-Abfragen
- [ ] `platynui-provider-mock`: Erzeugt einen skriptbaren Baum (`StaticMockTree`) mit deterministischen `RuntimeId`s.
- [ ] API-Variante `evaluate(node: Option<Arc<dyn UiNode>>, xpath, options)` fertigstellen; Kontextsteuerung ohne Cache.
- [ ] CLI-Kommando `query`: Führt XPath aus, unterstützt Formatoptionen (Text, JSON) und Filter (`--namespace`, `--pattern`).
- [ ] Tests: Beispiel-XPath-Abfragen gegen Mock-Baum (control/item/app/native Namen, Attribute, Patterns).

### 8. CLI `watch` – Ereignisse beobachten
- [ ] Event-Pipeline der Runtime an CLI anbinden (`watch` lauscht auf `ProviderEventKind` und optional wiederholt Abfragen).
- [ ] Mock-Provider erweitert Szenarien um Event-Simulation (NodeAdded/Removed/Updated, TreeInvalidated).
- [ ] CLI-Kommando `watch`: Ausgabe im Streaming-Modus; Optionen für Filter (Namespace, Pattern, RuntimeId).
- [ ] Tests: Simulierte Eventsequenzen prüfen (z. B. NodeAdded → Query-Ergebnis).

### 9. CLI `highlight`
- [ ] `HighlightProvider` in `platynui-core` finalisieren.
- [ ] `platynui-platform-mock`: Stellt Highlight-Attrappe (Logging) bereit.
- [ ] CLI-Kommando `highlight`: Markiert Bounding-Boxen basierend auf `ActivationTarget`/`Bounds` aus `query`-Ergebnissen.
- [ ] Tests: Highlight-Aufrufe protokollieren und überprüfen (`rstest`).

### 10. CLI `screenshot`
- [ ] `ScreenshotProvider`-Trait in `platynui-core` festlegen (Pixel-Format, Pfad/Ziel).
- [ ] `platynui-platform-mock`: Liefert Testbilder/Screenshot-Attrappe.
- [ ] CLI-Kommando `screenshot`: Fertigt Bildschirm-/Bereichsaufnahmen an (`--bbox`, `--output`), nutzt Mock zum Speichern/Logging.
- [ ] Tests: Screenshot-Aufrufe prüfen (Datei/Logging).

### 11. CLI `focus`
- [ ] `FocusablePattern`-Nutzung in Runtime/CLI freischalten; Mock-Knoten unterstützen Fokuswechsel.
- [ ] CLI-Kommando `focus`: Setzt Fokus auf Knoten (z. B. via RuntimeId oder XPath-Resultatindex).
- [ ] Tests: Fokuswechsel protokollieren (Mock-Highlight/Pointer optional nicht nötig).

### 12. CLI `pointer`
- [ ] `PointerDevice`-Trait in `platynui-core` fixieren (Desktop-Koordinaten, Buttons, Scroll).
- [ ] `platynui-platform-mock`: Simuliert Zeigeraktionen (Move/Click/Scroll) inkl. Logging.
- [ ] CLI-Kommando `pointer`: Führt Bewegungen und Klicks aus (z. B. `--move x y`, `--click left`).
- [ ] Tests: Pointer-Aufrufe gegen Mock protokollieren und verifizieren (`rstest`).

### 13. CLI `keyboard`
- [ ] `KeyboardDevice`-Trait in `platynui-core` fixieren (Keycodes, Texteingabe, Modifiers).
- [ ] `platynui-platform-mock`: Simuliert Tastatureingaben (Sequenzen, Sondertasten) inkl. Logging.
- [ ] CLI-Kommando `keyboard`: Sendet Sequenzen (`--text`, `--key ENTER`) an das fokussierte Element.
- [ ] Tests: Keyboard-Aufrufe gegen Mock prüfen (`rstest`).

### 14. CLI `window` – Fensteraktionen (Mock)
- [ ] `WindowSurface`-Pattern im Mock vollständig befüllen (`activate`, `minimize`, `maximize`, `restore`, `move`, `resize`, `accepts_user_input`).
- [ ] CLI-Kommando `window`: Unterstützt Aktionen wie `--activate`, `--minimize`, `--maximize`, `--move x y`, `--list` (Fenster/Mappings).
- [ ] Tests: Window-CLI-Aufrufe gegen Mock verifizieren (`rstest`).

### 15. Runtime-Pattern-Integration (Mock)
- [ ] Mock-UiTreeProvider reichert stabile Testknoten mit `supported_patterns()` und `pattern::<T>()`-Instanzen (Focusable, WindowSurface) an.
- [ ] PatternRegistry-/Lookup-Mechanismen in Runtime-Wrappern verifizieren und Tests ergänzen (`rstest`).
- [ ] CLI-Befehle `focus` und `window` (Mock) auf die vorhandenen Pattern-Actions aufsetzen; Fehlerpfade dokumentieren.

### 16. Runtime-Ausbau – Plattformunabhängige Basis
- [ ] `PlatformRegistry`/`PlatformBundle` implementieren: Plattformmodule registrieren Devices, Runtime bündelt sie je Technologie.
- [ ] `WindowSurface`-Pattern-Schnittstelle final durchgehen (Methoden klar dokumentieren, keine zusätzlichen Wrapper nötig).

### 17. Plattform Windows – Devices & UiTree
- [ ] `platynui-platform-windows`: Pointer/Keyboard via Win32 & UIAutomation-Hilfen, Screenshot/Highlight (DComposition/GDI).
- [ ] Fokus-Helper (`focus_control`) mit UIA-Fallbacks und Integration in `Focusable`.
- [ ] Tests: Desktop-Bounds, ActivationPoint, Sichtbarkeits-/Enabled-Flags unter Windows.
- [ ] `platynui-provider-windows-uia`: UIA-Wrapper (COM-Helfer ggf. in `platynui-platform-windows`), Rollennormalisierung, `RuntimeId`-Weitergabe.
- [ ] `WindowSurface`-Pattern implementieren: Aktionen (aktivieren/minimieren/maximieren/verschieben) und `accepts_user_input()` via Windows-spezifische APIs (`SetForegroundWindow`, `ShowWindow`, `WaitForInputIdle`).
- [ ] Gemeinsame Tests (Provider vs. Mock) mit bereitgestelltem UI-Baum & XPath-Abfragen; Dokumentation von Abweichungen der UIA-API.

### 18. CLI `window` – Windows-Integration
- [ ] CLI-Kommandos erweitern, um Windows-spezifische Optionen (z. B. Fensterliste mit Prozessinfos) zu nutzen.
- [ ] Tests: CLI `window` gegen reale Windows-Fenstersteuerung (soweit automatisierbar) bzw. Mock-Abdeckung.

### 19. Plattform Linux/X11 – Devices & UiTree
- [ ] `platynui-platform-linux-x11`: Pointer/Keyboard via XTest oder äquivalente APIs, Screenshot (XShm), Highlight (XComposite), Fenstersteuerung über EWMH/NetWM.
- [ ] Fokus-Helper für AT-SPI2 + plattformspezifische Fallbacks.
- [ ] Tests: Desktop-Bounds, ActivationPoint, Sichtbarkeits- und Enable-Flags unter X11.
- [ ] `platynui-provider-atspi`: D-Bus-Integration, Baumaufbau (Application → Window → Control/Item), RuntimeId aus Objektpfad, Fokus-/Sichtbarkeitsflags.
- [ ] Ergänzende Tests (AT-SPI2) auf Basis des Windows-Testsets inkl. Namespaces `item`/`control`.

### 20. CLI `window` – Linux/X11-Integration
- [ ] CLI `window` nutzt X11-spezifische Funktionen (EWMH/NetWM) für Fensterlisten, Move/Resize etc.
- [ ] Tests: CLI `window` gegen Mock/X11-spezifische Szenarien (soweit automatisierbar).

- ### 21. Werkzeuge
- [ ] CLI (`crates/platynui-cli`): Erweiterungen für `watch`, `dump-node`, strukturierte Ausgabe (`--json`, `--yaml`), Skript-Integration; ergänzt die MVP-Kommandos (`query`/`highlight`).
- [ ] Inspector (GUI): Tree-Ansicht mit Namespaces, Property-Panel (Patterns), XPath-Editor, Element-Picker, Highlight; arbeitet wahlweise Embedded oder via JSON-RPC.
- [ ] Beispiel-Workflows dokumentieren (Readme/Docs): XPath → Highlight, Fokus setzen, Fensterstatus (`accepts_user_input`) ermitteln.

### 22. Qualitätssicherung & Prozesse
- [ ] CI-Pipeline: `cargo fmt --all`, `cargo clippy --all`, `cargo test --workspace`, `uv run ruff check .`, `uv run mypy src/PlatynUI packages/core/src` (sofern Python-Anteile relevant).
- [ ] Contract-Tests für Provider & Devices (pattern-spezifische Attribute, Desktop-Koordinaten, RuntimeId-Quellen).
- [ ] Dokumentation pflegen: Architekturkonzept, Patterns, Provider-Checkliste, Legacy-Analyse; Hinweis auf lebende Dokumente beibehalten.
- [ ] Release-/Versionierungsstrategie festlegen (SemVer pro Crate? Workspace-Version?).

### 23. Backlog & Explorations
- Kontextknoten-Resolver: `RuntimeId`-basierte Re-Resolution für Kontextknoten außerhalb des aktuellen Wurzelknotens.
- JSON-RPC-Provider & Out-of-Process Integration
  - JSON-RPC 2.0 Vertrag dokumentieren (Markdown + JSON-Schema): Mindestumfang `initialize`, `getNodes(parentRuntimeId|null)`, `getAttributes(nodeRuntimeId)`, `getSupportedPatterns(nodeRuntimeId)`, optional `ping`; Events `$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`.
  - `platynui-provider-jsonrpc` implementieren: Verbindung über Named Pipe/Unix Socket/localhost, Registrierung bei Runtime, Heartbeat/Timeout-Handling, Sicherheit (Namenskonvention „PlatynUI+PID+User+…“).
  - Beispiel-Provider (Mock oder UIA-Proxy) dokumentieren; Leitfaden für Drittanbieter.
  - Runtime-Client-Schicht (Multiplexing, Fehler-Mapping, Provider-Restart-Strategien).
  - Handshake- und Capability-Design an LSP/MCP-Prinzipien ausrichten (Versionsangaben, optionale Fähigkeiten, klare Rollen), Dokumentation entsprechend ergänzen.
- JSON-RPC-Server & APIs
  - `crates/platynui-server`: JSON-RPC-Endpunkte für XPath-Abfragen, Fokus- und Fensteraktionen (über `Focusable`/`WindowSurface`), Highlighting, Screenshot, Provider/Device-Status sowie Heartbeat – keine generischen UI-Aktions-APIs.
  - Security-Guidelines (lokaler Zugriff, Authentication-Optionen) definieren.
  - Versionierung & Capability-Negotiation (Server ↔ Client) dokumentieren – Orientierung an LSP/MCP-Konzepten festhalten.
- macOS (AX) Provider
  - `platynui-provider-macos-ax`: AXUIElement-Brücke, Fenster-/App-Auflistung, RuntimeId aus AXIdentifier, Bound-Konvertierung (Core Graphics). AX-Rollen/Subrollen-Mapping dokumentieren und mit `docs/patterns.md` abgleichen.
  - Plattformübergreifende Regressionstests um macOS-spezifische Unterschiede erweitern.
- macOS Plattformmodule
  - `platynui-platform-macos`: Devices via Quartz/Event-Taps, Screenshot/Highlight mit CoreGraphics, Window-Manager via AppKit.
- Optionales Erweiterungs-Interface für Provider (z. B. optionale Runtime-Services).
- Persistentes XPath-Caching & Snapshot-Layer.
- Optionaler Wayland-Support (Runtime-Erkennung Wayland/X11, Provider-Auswahl, Devices).
- Weitere Patterns (z. B. Tabellen-Navigation, Drag&Drop) nach Bedarf evaluieren.
- Erweiterte Eingabegeräte (Gamepad, Stift), Barrierefreiheits-Funktionen.
- Touch-Device-Unterstützung (Traits, CLI-Befehle) nach erfolgreichem Pointer/Keyboard-Ausbau.
- Community-Guides, Beispiel-Provider, Trainingsmaterial.
