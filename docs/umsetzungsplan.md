# Umsetzungsplan PlatynUI Runtime

> Lebendes Dokument: Wir pflegen diesen Plan fortlaufend und passen ihn bei neuen Erkenntnissen oder Prioritäten an.

## Ausgangspunkt & Zielbild
- Konsistenter Desktop-zentrierter UI-Baum mit den Namespaces `control` (Standard), `item`, `app` und `native`.
- Trait-basierte Pattern-Schicht gemäß `docs/patterns.md`, inklusive `ActivationTarget`, `Application`, `Focusable`, `WindowSurface` und `AcceptsUserInput`-Hilfen.
- Plattformen werden durch klar getrennte Crates für Devices (`platform-*`) und UiTreeProvider (`provider-*`) abgebildet; JSON-RPC-Provider ergänzen die Architektur optional.
- Runtime verwaltet Provider, Devices, WindowManager, XPath-Pipeline, Highlighting und Screenshot-Funktionen.
- CLI und Inspector dienen als Referenzwerkzeuge über Runtime bzw. JSON-RPC-Server.

## Arbeitsbereiche
Die folgenden Kapitel listen Aufgabenpakete; Reihenfolgen innerhalb eines Abschnitts sind Empfehlungen, keine starre Vorgabe.

### 1. Fundament & Repository-Struktur
- [ ] Workspace aufsetzen/aufräumen: `crates/core`, `crates/runtime`, `crates/server`, `crates/platform-{windows,linux-x11,linux-wayland?,macos,mock}`, `crates/provider-{windows-uia,atspi,macos-ax,jsonrpc,mock}`, `crates/cli`, `apps/inspector` (oder eigenes Crate).
- [ ] Gemeinsame Cargo-Einstellungen (Edition, Lints, Features) und Rustfmt/Clippy-Konfiguration vereinheitlichen.
- [ ] README/CONTRIBUTING aktualisieren: Namenskonventionen (PascalCase-Attribute, Namespaces), Architekturüberblick, Hinweis auf lebende Konzeptdokumente.
- [ ] Dev-Tooling notieren (`uv`, `cargo`, Inspector-Abhängigkeiten) und Basis-Skripte (Format/Lint/Test).

### 2. Core-Datenmodell & XPath-Grundlagen
- [ ] `UiNode`/`UiSnapshot`/`UiValue` finalisieren: Zugriffsmethoden, Lebensdauern, Serialisierung für Debugging.
- [ ] Pflichtattribute (`Bounds`, `Role`, `Name`, `IsVisible`, optional `IsOffscreen`, `RuntimeId`, `Technology`, `SupportedPatterns`) als Konstanten/Enums hinterlegen.
- [ ] Namespace-Registry (`control` als Standard, `item`, `app`, `native`) und XPath-Helper (`local-name()`-Mapping auf Rollen) implementieren.
- [ ] Dokumentwurzel „Desktop“ modellieren (komplette Desktop-Bounds, Monitor-Infos über Pattern oder Attribute).
- [ ] XPath-Engine-Integration (Streaming-Auswertung, Attribute-/Namespace-Resolver, Tests mit synthetischem Baum).
- [ ] Abgleich zum „Knoten- und Attributmodell“ aus dem Architekturkonzept herstellen; Unit-Tests für Attributschlüssel.

### 3. Pattern-System
- [ ] Pattern-Traits als `struct`/`trait`-Kombination definieren (z. B. `TextContent`, `TextEditable`, `ActivationTarget`, `Application`, `WindowSurface`, `AcceptsUserInput`).
- [ ] `SupportedPatterns`-Enum oder Identifier-Registry plus Validierung (z. B. `TextEditable` → erfordert `TextContent`).
- [ ] Provider-facing Contract-Tests: prüfen, dass Pflichtattribute vorhanden sind und Coordinates Desktop-relativ bleiben.
- [ ] Mapping-Hilfen zwischen Patterns und Technologie-spezifischen APIs (UIA-ControlType, AT-SPI Rollen, AX Attribute) bereitstellen.
- [ ] Patterns-Dokument (`docs/patterns.md`) parallel synchron halten (Beispiele, offene Punkte, Erweiterungswünsche).

### 4. Provider-Infrastruktur (Core)
- [ ] Traits `UiTreeProvider`, `UiTreeProviderFactory`, Lifecycle (`initialize`, `shutdown`, Events), Fehler-/Result-Typen.
- [ ] Baum-Event-Typen (`NodeAdded`, `NodeUpdated`, `NodeRemoved`) und Event-Verteiler in der Runtime.
- [ ] Inventory-basierte Registrierungsmakros (`register_provider!`, `register_platform_module!`), inkl. Tests für Mehrfach-Registrierung und `cfg`-gesteuerte Aktivierung.
- [ ] Provider-Checkliste (`docs/provider_checklist.md`) automatisiert verknüpfen (CI-Lints oder Contract-Test-Suite).

### 5. Native Provider (UiTree)
- [ ] `provider-windows-uia`: UIA-Wrapper (COM-Helfer ggf. in `platform-windows-core`), Rollennormalisierung (ControlType → lokale Namen), `RuntimeId`-Weitergabe, `AcceptsUserInput` via `WaitForInputIdle`/Fallback.
- [ ] `provider-atspi`: D-Bus-Integration, Baumaufbau (Application → Window → Control/Item), RuntimeId aus Objektpfad, Fokus- und Sichtbarkeitsflags.
- [ ] `provider-macos-ax`: AXUIElement-Brücke, Fenster-/App-Auflistung, RuntimeId aus AXIdentifier, Bound-Konvertierung (Core Graphics).
- [ ] Gemeinsame Tests (pro Provider) mit Snapshot-Baum & XPath-Abfragen; Dokumentation von Abweichungen je API.

### 6. JSON-RPC-Provider & Out-of-Process Integration
- [ ] JSON-RPC 2.0 Vertrag dokumentieren (Markdown + JSON-Schema): Mindestumfang `initialize`, `listApplications`, `getRoot`, `getNode`, `getChildren`, `getAttributes`, `getSupportedPatterns`, optional `resolveRuntimeId`, `ping`; Events `$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`.
- [ ] `provider-jsonrpc` implementieren: Verbindung über Named Pipe/Unix Socket/localhost, Registrierung bei Runtime, Heartbeat/Timeout-Handling, Sicherheit (Namenskonvention „PlatynUI+PID+User+…“).
- [ ] Beispiel-Provider (Mock oder UIA-Proxy) dokumentieren; Leitfaden für Drittanbieter.
- [ ] Runtime-Client-Schicht (Multiplexing, Fehler-Mapping, Provider-Restart-Strategien).
- [ ] Handshake- und Capability-Design an LSP/MCP-Prinzipien ausrichten (Versionsangaben, optionale Fähigkeiten, klare Rollen), Dokumentation entsprechend ergänzen.

### 7. Mocking & Tests
- [ ] `platform-mock`: Geräte (Pointer, Keyboard, Display, Highlight, Screenshot) als In-Memory-Implementierungen, Logging für Assertions.
- [ ] `provider-mock`: `StaticMockTree`, Skriptbares Verhalten (z. B. über JSON/Szenario-Dateien), deterministische `RuntimeId`s.
- [ ] Integrationstest-Suite (Runtime + Mock-Provider + Mock-Devices) mit Beispiel-XPath-Abfragen, Pattern-Verifikation, Highlight-Simulation.

### 8. Devices & Window-Management
- [ ] Traits `PointerDevice`, `KeyboardDevice`, `TouchDevice`, `DisplayInfo`, `ScreenshotDevice`, `HighlightOverlay` (Desktop-Koordinaten, DPI-Korrektur).
- [ ] WindowManager-Trait definieren (`activate`, `minimize`, `maximize`, `move`, `restore`, `bring_to_front`, Status-Abfragen) + Mapping in Patterns (`WindowSurface`).
- [ ] Plattform-Implementierungen: Windows (Win32 + UIA), Linux/X11 (X11 + XTest, Fenstermanager-APIs), MacOS (AppKit/CoreGraphics). Wayland vorbereiten (Portal/XDG-Activation) als Option.
- [ ] Fokus-Helper (`focus_control` Standardweg + Fallbacks, Integration mit `Focusable` Pattern).
- [ ] Tests: Desktop-Bounds, Default-Click-Point aus `ActivationTarget`, Sichtbarkeit (`IsVisible`, `IsOffscreen`).

### 9. Runtime-Kern
- [ ] Provider-Registry initialisieren (Inventory lesen, `cfg` prüfen, Prioritäten setzen), Provider-Lifecycle steuern.
- [ ] Dokumentaufbau: Desktop-Wurzel laden, App- und Control-Nodes verknüpfen, `item`-Namespace an Container-Knoten hängen.
- [ ] `AcceptsUserInput`-Hilfsmethode (Windows `WaitForInputIdle`, Linux/macOS heuristische Implementierung), Rückfallverhalten dokumentieren.
- [ ] XPath-Auswertung → `UiNodeRef`-Iterator, Filterung nach Patterns, Attribute-Lazy-Loading.
- [ ] API-Variante `evaluate(node: Option<UiNodeRef>, xpath, cache_policy)` implementieren; `None` verwendet automatisch das Desktop-Dokument, ansonsten wird der Knoten als Kontext genutzt (`.//item:*`). `cache_policy` entscheidet, ob ein vorhandener Snapshot genutzt oder frische Provider-Daten geladen werden (Namensfindung noch offen).
- [ ] Highlighting/Screenshot orchestrieren: Koordination zwischen Runtime, Devices, WindowManager.
- [ ] Fehler- & Telemetrieschnittstelle (Tracing, Logging, Metriken) entwerfen.

### 10. JSON-RPC-Server & APIs
- [ ] `crates/server`: JSON-RPC-Endpunkte für XPath-Abfragen, Fokus- und Fensteraktionen (über `Focusable`/`WindowSurface`), Highlighting, Screenshot, Provider/Device-Status sowie Heartbeat – keine generischen UI-Aktions-APIs.
- [ ] Security-Guidelines (lokaler Zugriff, Authentication-Optionen) definieren.
- [ ] Versionierung & Capability-Negotiation (Server ↔ Client) dokumentieren – Orientierung an LSP/MCP-Konzepten festhalten.

### 11. Werkzeuge
- [ ] CLI (`crates/cli`): Befehle `query`, `highlight`, `watch`, `dump-node`, `focus`, optional `--json`/`--yaml` Ausgabe.
- [ ] Inspector (GUI): Tree-Ansicht mit Namespaces, Property-Panel (Patterns), XPath-Editor, Element-Picker, Highlight; arbeitet wahlweise Embedded oder via JSON-RPC.
- [ ] Beispiel-Workflows dokumentieren (Readme/Docs): XPath → Highlight, Fokus setzen, AcceptsUserInput prüfen.

### 12. Qualitätssicherung & Prozesse
- [ ] CI-Pipeline: `cargo fmt --all`, `cargo clippy --all`, `cargo test --workspace`, `uv run ruff check .`, `uv run mypy src/PlatynUI packages/core/src` (sofern Python-Anteile relevant).
- [ ] Contract-Tests für Provider & Devices (Pflichtattribute, Desktop-Koordinaten, Pattern-Abhängigkeiten, RuntimeId-Quellen).
- [ ] Dokumentation pflegen: Architekturkonzept, Patterns, Provider-Checkliste, Legacy-Analyse; Hinweis auf lebende Dokumente beibehalten.
- [ ] Release-/Versionierungsstrategie festlegen (SemVer pro Crate? Workspace-Version?).

### 13. Backlog & Explorations
- Optionaler Wayland-Support (Runtime-Erkennung Wayland/X11, Provider-Auswahl, Devices).
- Weitere Patterns (z. B. Tabellen-Navigation, Drag&Drop) nach Bedarf evaluieren.
- Erweiterte Eingabegeräte (Gamepad, Stift), Barrierefreiheits-Funktionen.
- Performance-Optimierungen (Delta-Updates, Caching, Binary Transport).
- Community-Guides, Beispiel-Provider, Trainingsmaterial.

## Empfohlene Reihenfolge (High-Level)
1. Fundament (Abschnitt 1) + Core-Datenmodell (2) als Basis.
2. Pattern-System (3) und Provider-Infrastruktur-Core (4) definieren.
3. Mocking (7) und Devices/Window-Management (8) für End-to-End-Prototyp sichern.
4. Runtime-Kern (9) mit XPath & Highlighting; parallel JSON-RPC-Provider (6) vorbereiten.
5. Native Provider (5) inkrementell je Plattform integrieren.
6. Server & Tools (10–11) aufsetzen, sobald Runtime stabil ist.
7. Qualitätssicherung (12) verankern, Backlog (13) nach Stabilisierung adressieren.

