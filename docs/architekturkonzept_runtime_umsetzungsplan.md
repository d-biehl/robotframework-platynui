# Umsetzungsplan PlatynUI Runtime

English summary: Implementation plan for the PlatynUI runtime with status checklists per area.

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
- Crate-Erstellung: Für Workspace‑Crates (unter `crates/*`, `apps/*`) muss der Eintrag `package.name` mit `platynui-` beginnen. Verzeichnisnamen dürfen kürzer sein (`crates/runtime` → `platynui-runtime`), entscheidend ist der Paketname. Packaging/FFI‑Crates außerhalb des Cargo‑Workspaces (z. B. `packages/native` → `platynui_native`) sind von der Regel ausgenommen und folgen den Konventionen des Ziel‑Ökosystems. Ausnahmen sind im Review explizit zu benennen.
- Tests: In Rust-Unit- und Integrationstests konsequent `rstest` einsetzen und dessen Features nutzen (Fixtures, `#[rstest]` mit `case`/`matrix`, parametrische Tests). Bestehende Tests bei Anpassungen entsprechend migrieren.
- Typbenennung: Neue Rust-Typen (Structs, Enums, Traits) folgen dem üblichen Namensraum über Module – kein zusätzlicher `Platynui`-Präfix nötig. Stattdessen aussagekräftige Namen im entsprechenden Modul wählen (`RuntimeState`, `WindowsDeviceBundle`, ...).
- Planpflege: Nach jedem Arbeitspaket den Plan aktualisieren, erledigte Aufgaben abhaken und ggf. neue Erkenntnisse ergänzen. Der Umsetzungsplan bleibt so synchron zur tatsächlichen Umsetzung.
- Mock-Stack: `platynui-provider-mock`/`platynui-platform-mock` hängen am optionalen Feature `mock-provider`. Werkzeuge (z. B. `cargo run -p platynui-cli --features mock-provider -- watch`) und Tests nutzen dieses Feature gezielt; Standard‑Builds bleiben ohne Mock. Produktive Plattform-/Provider‑Crates werden nicht mehr durch die Runtime, sondern durch Anwendungen (CLI, Python‑Extension) per `cfg(target_os = …)` verlinkt.
 - Verlinkung produktiver Provider/Plattformen: Anwendungen verlinken OS‑spezifische Crates explizit über das Hilfscrate `platynui-link`. Die Makros `platynui_link_providers!()` (Feature‑gesteuert Mock vs. OS) und `platynui_link_os_providers!()` (explizit OS) stellen sicher, dass keine Auto‑Verlinkung in der Runtime erfolgt und Tests den Mock gezielt einbinden können.

### Status-Update (2025-10-09 – Release 0.1.0)
- Erste Preview-Version `0.1.0` veröffentlicht (09.10.2025) mit lauffähiger Runtime, CLI, XPath-Evaluator und Python-Bindings.
- Windows-Fokus (UIA + Plattformgeräte) stabilisiert; Pointer- und Keyboard-Stacks produktionsreif inklusive Mock-Parität.
- CLI erweitert um strukturierte Fenster-, Pointer- und Keyboard-Kommandos; `snapshot`-Export basiert auf Streaming-Writer.
- Python-Integration deckt Highlight, Pointer, Keyboard und Fenstersteuerung ab und stellt Iterator-APIs bereit.

### Status-Update (2025-11-05 – Dev 0.12.0)
- Workspace-Version auf `0.12.0-dev` angehoben; laufende Arbeiten erfolgen in inkrementellen Dev‑Releases.
- CLI `snapshot` ist vollständig verfügbar (Text/XML, Depth/Include/Exclude/Alias/Split) inkl. Doku und Tests; siehe auch `docs/cli_snapshot_spec.md`.
- Fenstersteuerung: Runtime bietet `bring_to_front_and_wait(timeout)`; CLI `window --bring-to-front --wait-ms N` nutzt dies für „activate & input-ready“.
- Python/Robot: BareMetal‑Library mit umfangreichen Keywords (Pointer/Keyboard/Window/Query/Screenshot) einsatzfähig; native Python‑Bindings (Iteratoren, Typen, Overrides) stabilisiert.
- Inspector: TreeView Phasen 1–4 abgeschlossen (echte UiNode‑Quelle), Properties‑Sync in Arbeit; siehe `docs/ui/treeview_plan.md`.
- CI-Pipeline (Linux/Windows/macOS) aktiv: Build + Tests (nextest), Format/Lint (rustfmt, clippy -D warnings, ruff, mypy), Wheel‑Builds (maturin/uv). Siehe `.github/workflows/ci.yml`.
- Ergänzende Analyse/Leitfäden: `docs/xpath_streaming_analysis.md`, `docs/provider_windows_uia_design.md`.

### Ergänzung: UiNode `Id` (entwicklerseitige Kennung)
- Neues, optionales Attribut `control:Id` als stabile, sprach‑ und inhaltsunabhängige Kennung eines Elements. Abzugrenzen von `RuntimeId` (nur laufzeitstabil).
- Plattform‑Mapping (Ziel): Windows → `AutomationId`, Linux/AT‑SPI2 → `accessible_id` (falls vorhanden), macOS/AX → `AXIdentifier` (falls vorhanden). Leere oder fehlende Werte gelten als „nicht gesetzt“.
- Anwendungsknoten: Nach Plattform wählen (z. B. Bundle Identifier auf macOS); ansonsten konservativer Fallback wie `ProcessName` – dokumentationspflichtig.

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
- [x] Dokumentwurzel „Desktop“ beschreiben (Monitore, Bounds als `Rect`); Alias‑Attribute wie `Bounds.X` werden nicht providerseitig geliefert, sondern zur Abfragefreundlichkeit in der Runtime/XPath‑Ebene abgeleitet. Tests entsprechend anpassen.
- [x] XPath-Atomisierung auf `typed_value()` umstellen: `UiValue`-Varianten werden in XDM-Atomics überführt (Booleans, Integer, Double), komplexe Strukturen (`Rect`, `Point`, `Size`) bleiben als JSON-Strings; Regressionstests (`data(//@*:Bounds.Width)`, `data(//@*:IsVisible)`) absichern.

### 3. Pattern-System
- [x] Runtime-Pattern-Traits in `platynui-core` anlegen (`FocusablePattern`, `WindowSurfacePattern`) inkl. `PatternError`-Fehlertyp; rein lesende Patterns verbleiben bei Attributen.
- [x] Runtime-Aktionsschnittstellen der Patterns (z. B. `FocusablePattern::focus()`, `WindowSurfacePattern::maximize()`) präzisieren und Beispiel-Implementierungen samt Tests dokumentieren; nur diese Pattern dürfen Laufzeitaktionen anbieten. (Umgesetzt via `FocusableAction`, `WindowSurfaceActions` + rstest-Coverage.)
- [x] Leitfaden für `SupportedPatterns`-Verwendung (Dokumentation erledigt; providerseitige Tests folgen in den konkreten Provider-Szenarien), damit Pattern-Kombinationen nachvollziehbar bleiben.
- [x] Provider-facing Contract-Tests: Core-Testkit (`platynui_core::ui::contract::testkit`) prüft Basisattribute (z. B. `Bounds`, `ActivationPoint`) und Pattern‑Pflichten. Alias‑Attribute (`Bounds.*`, `ActivationPoint.*`) sind kein Provider‑Contract mehr, da sie von der Runtime/XPath‑Ebene abgeleitet werden.
- [x] Mapping-Hilfen zwischen Patterns und Technologie-spezifischen APIs (UIA-ControlType, AT-SPI Rollen, AX Attribute) ergänzt. `docs/patterns.md` enthält eine Orientierungstabelle; Provider dokumentieren Abweichungen individuell.
- [x] Patterns-Dokument (`docs/patterns.md`) aktualisiert: Klarstellung, dass Alias‑Attribute (z. B. `Bounds.X`) von Runtime/XPath erzeugt werden; Rollenkatalog und Mapping‑Tabelle für UIA/AT‑SPI/AX ergänzt; bleibt ein lebendes Dokument für weitere Erweiterungen.

### 4. Provider-Infrastruktur (Core)
- [x] Traits `UiTreeProvider`, `UiTreeProviderFactory` plus Basistypen (`ProviderDescriptor`, `ProviderEvent`, Fehler) definiert; Lifecycle-Erweiterungen (Events weiterreichen, Shutdown) folgen beim Runtime-Wiring.
- [x] `ProviderRegistry` im Runtime-Crate sammelt registrierte Factories via `inventory`, gruppiert sie je Technologie und erzeugt Instanzen. In Unit‑Tests werden Provider/Plattformen explizit injiziert (`Runtime::new_with_factories[_and_platforms]`), um deterministische Testläufe ohne globale Discovery zu gewährleisten (siehe Fixtures in `runtime`/`cli`).
- [x] Event-Pipeline auf Runtime-Seite: Dispatcher verteilt Ereignisse an registrierte Sinks, Shutdown leert Abonnenten, Runtime ruft `UiTreeProvider::subscribe_events(...)` für alle Provider auf und stellt über `register_event_sink` eine einfache Erweiterungsstelle bereit.
- [x] Provider-spezifische Snapshots: Runtime hält pro Provider einen eigenen Knoten-Snapshot und aktualisiert ihn nur, wenn `event_capabilities = None` (Polling) oder ein passendes Ereignis / Change-Hint eintrifft.
- [x] Inventory-basierte Registrierungsmakros (`register_provider!`, `register_platform_module!`), inkl. Tests für Registrierungsauflistung; weitere `cfg`-Szenarien folgen bei der Runtime-Einbindung.
- [x] Factory-Lifecycle: Entscheidung dokumentiert – Provider erhalten bewusst nur `Arc<dyn UiTreeProvider>` ohne zusätzliche Services; Geräte bleiben in der Runtime.
- [x] Provider-Checkliste (`docs/provider_checklist.md`) via Contract-Test-Suite abgedeckt (Mock-Provider nutzt `contract::testkit`; künftige Provider erhalten dieselben Prüfungen, Tests laufen unter `cargo test`).
- [x] `ProviderDescriptor` um `event_capabilities` erweitern (Bitset `None`/`ChangeHint`/`Structure`/`StructureWithProperties`), Runtime-Strategie dokumentieren und Tests vorbereiten, damit Voll-Refresh nur bei fehlender Event-Unterstützung nötig bleibt.

### 5. CLI `list-providers` – Mock-Basis schaffen
- [x] Minimalen Laufweg „Runtime + platynui-platform-mock + platynui-provider-mock“ herstellen (Provider-Registry initialisieren, Mock-Provider instanziieren).
- [x] `platynui-platform-mock`: Grundgerüst mit Stub-Geräten & Logging, liefert zumindest Technologie-/Versionsinformationen.
- [x] `platynui-provider-mock`: Wird über Factory-Handle bereitgestellt (keine Auto-Registrierung), stellt einfache Baumdaten bereit.
- [x] CLI-Kommando `list-providers`: Gibt registrierte Provider/Technologien aus (Name, Version, Aktiv-Status), unterstützt Text und JSON.
- [x] Tests: Provider-Registry + CLI-Output gegen Mock-Setup; `rstest` verwenden.

### 6. CLI `info` – Desktop-/Plattform-Metadaten
- [x] `DesktopInfoProvider`-Trait in `platynui-core` definieren (OS-/Monitor-Metadaten, Bounds) und in Runtime verankern.
- [x] `platynui-platform-mock`: Liefert DesktopInfo-Daten (OS, Monitorliste, Auflösung) zum Testen.
- [x] Runtime baut den Desktop-Dokumentknoten aus `DesktopInfoProvider` und stellt Daten für CLI bereit.
- [x] CLI-Kommando `info`: Zeigt Plattform, Desktop-Bounds, Monitore, verfügbare Provider. Ausgabe als Text/JSON.
- [x] Tests: `info`-Kommando mit Mock-Daten (Mehrmonitor, OS-Varianten) validieren.

### 7. CLI `query` – XPath-Abfragen
- [x] `platynui-provider-mock`: Erzeugt einen skriptbaren Baum (`StaticMockTree`) mit deterministischen `RuntimeId`s.
- [x] API-Variante `evaluate(node: Option<Arc<dyn UiNode>>, xpath, options)` fertigstellen; Kontextsteuerung ohne Cache.
- [x] CLI-Kommando `query`: Führt XPath aus, unterstützt Formatoptionen (Text, JSON) und Filter (`--namespace`, `--pattern`).
- [x] Tests: Beispiel-XPath-Abfragen gegen Mock-Baum (control/item/app/native Namen, Attribute, Patterns).
- [x] Referenzbaum in `crates/provider-mock/assets/mock_tree.xml` dokumentiert (Struktur, RuntimeIds, Beispiele).
- [x] Baumdefinition aus `crates/provider-mock/assets/mock_tree.xml` laden und in NodeSpecs konvertieren.
- [x] Textausgabe der `query`-Nodes auf XML-ähnliches Format umgestellt; JSON liefert nun vollständige Attributlisten.
- [x] Provider-Doppelsicht (flach + gruppiert) umgesetzt: Mock-Provider erzeugt Desktop-Alias-Knoten mit eindeutigen Ordnungsschlüsseln, Dokumentation angepasst. Konfigurationsoptionen können später ergänzt werden.

### 8. CLI `watch` – Ereignisse beobachten
- [x] Event-Pipeline der Runtime an CLI anbinden (`watch` lauscht auf `ProviderEventKind`, kann optional Folgeabfragen auslösen).
- [x] Mock-Provider erweitert Szenarien um Event-Simulation (`emit_event`, `emit_node_updated`, initiales `TreeInvalidated`).
- [x] Runtime respektiert `event_capabilities`: Event-fähige Provider markieren Snapshots als „dirty“, ereignislose Provider lösen weiterhin Vollabfragen aus.
- [x] CLI-Kommando `watch`: Streaming-Ausgabe (Text/JSON) mit optionaler XPath‑Nachabfrage (`--expression`) und `--limit` für Tests.
- [ ] Watch‑Filter nach Namespace/Pattern/RuntimeId (`--namespace`, `--pattern`, `--runtime-id`).
- [x] Tests: Simulierte Eventsequenzen (TreeInvalidated + NodeUpdated) prüfen Ausgabe in Text/JSON-Format.

### 9. CLI `highlight`
- [x] `HighlightProvider` in `platynui-core` finalisiert (`highlight(&HighlightRequest)`, `clear()` inkl. optionaler Dauerangabe für eine gesamte Anfrage).
- [x] `platynui-platform-mock`: Stellt Highlight-Attrappe (Logging) bereit (`take_highlight_log`, `reset_highlight_state`).
- [x] CLI-Kommando `highlight`: Markiert Bounding-Boxen über XPath-Ergebnisse (`Bounds`), unterstützt `--duration-ms` und `--clear`.
- [x] Tests: Highlight-Aufrufe protokollieren und überprüfen (`rstest`, Mock-Log-Auswertung).

### 10. CLI `screenshot`
- [x] `ScreenshotProvider`-Trait in `platynui-core` festgelegt (`ScreenshotRequest`, `Screenshot`, `PixelFormat`).
- [x] `platynui-platform-mock`: Liefert deterministischen RGBA-Gradienten und Logging-Helfer (`take_screenshot_log`, `reset_screenshot_state`).
- [x] CLI-Kommando `screenshot`: Einheitlich `--rect x,y,width,height` (statt `--bbox`), Zielpfad als Positionsargument (ohne `--output`). Ohne Pfad generieren wir einen Default-Dateinamen mit Zeitstempel und stellen Eindeutigkeit über Suffixe sicher. PNG-Encoding via `png` 0.18.0.
- [x] Tests: Screenshot-Aufrufe verifizieren Ausgabe, Logging und erzeugte Dateien (Temp-Verzeichnis über `tempfile` 3.23.0).

### 11. CLI `focus`
- [x] `FocusablePattern` im Mock-Baum aktiviert: `IsFocused` wird dynamisch aus dem globalen Fokusstatus berechnet, Fokuswechsel senden `ProviderEventKind::NodeUpdated` für alte und neue Ziele.
- [x] Runtime-API `Runtime::focus(&Arc<dyn UiNode>)` greift auf das registrierte `FocusableAction` zu und liefert differenzierte Fehler (`PatternMissing`, `ActionFailed`).
- [x] CLI-Kommando `focus`: XPath-Auswahl fokusfähiger Knoten, Ausgabe mit Erfolgs-/Skip-Liste, Tests verifizieren Zustandswechsel und Fehlermeldungen.

### 12. Runtime-Pattern-Integration (Mock)
- [x] Mock-UiTreeProvider liefert `FocusableAction`-Instanzen inkl. dynamischem `IsFocused`-Attribut; `WindowSurface` folgt separat.
- [x] PatternRegistry-/Lookup-Mechanismen über Tests abgesichert (`UiNode::pattern::<T>()`, Runtime `focus()`); verbleibende Szenarien für `WindowSurface` stehen aus.
- [x] `PatternRegistry::register_lazy` ermöglicht es, Pattern-Probes erst bei Bedarf auszuführen (Mock demonstriert dies für `Focusable`); der Unit-Test `register_lazy_resolves_on_demand` sichert das Caching-Verhalten ab.

### 13. CLI `window` – Fensteraktionen (Mock)
- [x] `WindowSurface`-Pattern im Mock vollständig befüllen (`activate`, `minimize`, `maximize`, `restore`, `move_to`, `resize`, `close`, `accepts_user_input`) inkl. dynamischer Attribute (`IsMinimized`, `Bounds.*`, `SupportsMove/Resize`) und `Focusable`-Pattern samt `IsFocused` für Top-Level-Fenster.
- [x] CLI-Kommando `window`: Aktionen (`--activate`, `--minimize`, `--maximize`, `--restore`, `--close`, `--move x y`, `--resize w h`) sowie `--list` für eine strukturierte Fensterübersicht implementiert; Ausgabe nutzt die neuen Mock-Daten (Dual-View, aktualisierte Bounds).
- [x] Tests: `platynui-provider-mock` validiert das Pattern (rstest, Lazy-Probing) und CLI-Tests decken Listen- und Aktionspfad ab (`window_actions_apply_sequence`, `window_actions_require_match`).
- [x] CLI-Befehl `window` an Runtime-Pattern angebunden; Fokuspfad bleibt unabhängig (`focus`).
- [x] Dokumentiert: Die flache `control:`-Sicht bleibt Default (Abfragen wie `/control:*/descendant-or-self::control:*[...]` erfassen nur echte Kontrollen), aliasierte Anwendungssicht liegt in `app:` / `appitem:`.
  - [x] XPath‑Normalisierung aufgesplittet: `EnsureDistinct`/`EnsureOrder` (keine Normalisierung für `attribute::`/`namespace::`; Forward‑Pfade streamen).

### 14. CLI `pointer`
- [x] `PointerDevice`-Trait in `platynui-core` definieren: elementare Aktionen (`position()`, `move_to(Point)`, `press(PointerButton)`, `release(PointerButton)`, `scroll(ScrollDelta)`), Double-Click-Metadaten (`double_click_time()`, `double_click_size()`), Rückgabewerte/Fehler via `PlatformError`. Alle Koordinaten bleiben `f64` in Desktop-Bezug.
- [x] Bewegungs-Engine im Runtime-Layer ergänzen (Schrittgeneratoren linear/Bezier/Overshoot/Jitter, konfigurierbare Delays `after_move_delay`, `press_release_delay`, `multi_click_delay`, `before_next_click_delay`). Hardware-Defaults bleiben in `PointerSettings` (z. B. `double_click_time`, `double_click_size`, Standard-Button), während Bewegungsparameter im `PointerProfile` liegen; `PointerOverrides` dient als einziger Options-Typ pro Aufruf (`move_to`, `click`, `drag`, `scroll`). Die Builder-API (`PointerOverrides::new().origin(...).after_move_delay(...)`) bildet ausschließlich die Abweichungen zu den aktiven Runtime-Defaults ab. CLI-Optionen können Defaults und Ad-hoc-Overrides setzen.
- [x] `platynui-platform-mock`: Pointer-Implementation bereitstellen (Move/Press/Release/Scroll) mit Logging-Hooks (`take_pointer_log`, `reset_pointer_state`) und deterministischen Ergebnissen; Double-Click-Zeit/-Größe über Mock-Konstanten simulieren.
- [x] CLI-Vorbereitung: Parser für Koordinaten (`parse_point`), Scroll-Deltas (`parse_scroll_delta`) und Tasten (`parse_pointer_button`) ergänzen.
- [x] CLI-Kommando `pointer`: Unterbefehle für `move`, `click`, `press`, `release`, `scroll`, `drag`, `position`, optional `--motion <mode>` sowie `--origin` (`desktop`, `bounds`, `absolute`) für relative Koordinaten; Koordinaten-/Button-Parsing, Ausgabe mit Erfolg/Fehlerinformation.
- [x] Tests (`rstest`): Motion-Engine ist durch Runtime-Unit-Tests abgedeckt, CLI-Integration (Move/Click/Scroll) läuft gegen den Mock-Provider und nutzt das Feature-Flag `mock-provider`.

### 15. Keyboard – Trait & Settings
- [x] `KeyboardDevice`-Trait in `platynui-core` fixieren (`key_to_code`, `send_key_event(KeyboardEvent)`, `start_input`/`end_input` nur für tastaturspezifische Vor-/Nachbereitung, `known_key_names()`) inkl. Fehler-Typen (`KeyboardError`, `KeyCodeError`).
- [x] Provider dokumentieren ihre unterstützten Tastennamen konsistent (`Control`, `Shift`, `Alt`, `Enter`, `Escape`, …) und halten sich an etablierte Plattformbezeichnungen (`Command`, `Option`, `Windows`, `Super`, ...).
- [x] `KeyboardEvent` als schlankes Struct (Felder `KeyCode`, `KeyState`) implementieren; `start_input()` ist optional und trägt keinen zusätzlichen Phasen-Typ mehr.
- [x] `KeyboardSettings` + `KeyboardOverrides` (Builder) analog zum Pointer-Stack definieren; Defaults aus Legacy-Werten übernehmen.
- [x] Dokumentation (Architektur, Plan, Provider-Checkliste) auf finalen Trait-/Event-Vertrag aktualisieren; Runtime stellt `keyboard_settings()`/`set_keyboard_settings()` bereit und hält das erste registrierte `KeyboardDevice` vor.

### 16. Keyboard – Sequenzparser & Runtime-API
- [x] Sequenz-Parser (`KeyboardSequence`) in `platynui-runtime` bereitstellen: unterstützt Strings, `<Ctrl+Alt+T>`-Notation, Backslash-Escapes (`\<`, `\>`, `\\`, `\xNN`, `\uNNNN`), Iterator-Eingaben und liefert `KeyboardEvent`-Iterationen.
- [x] Event-Auflösung: Parser mappt Token strikt gegen `key_to_code`; unbekannte Bezeichner führen zu einem Parserfehler (`KeyboardError::UnsupportedKey`).
- [x] Runtime-API (`keyboard_type`, `keyboard_press`, `keyboard_release`) implementieren: Fokus-/Sichtbarkeits-Prüfung via `Focusable`/`WindowSurface`, Lazy-Pattern-Abruf, Cleanup für gedrückte Tasten, Fehlerabbildung (`KeyboardActionError` kapselt Sequenz- und Providerfehler).
- [x] Unit-Tests im Runtime-Crate (rstest) für Sequenzaufbereitung, Fehlerpfade, Cleanup-Logik.
- [x] Dokumentation: Kapitel zur Sequenzsyntax/Runtime-API im Architekturkonzept erweitern.

### 17. Keyboard – Mock & CLI
- [x] `platynui-platform-mock`: Logging-Keyboard mit Mapping für Buchstaben, Sonderzeichen, Modifier; Utilities `take_keyboard_log`, `reset_keyboard_state` ergänzen.
- [x] `platynui-provider-mock`: Beispiel-Key-Mapping (`key_to_code`) und Text-Handhabung (z. B. Emojis, IME-Strings) implementieren.
- [x] CLI-Kommando `keyboard`: Unterbefehle `type`, `press`, `release`; alle nehmen eine komplette Sequenz (z. B. `<Ctrl+A>Hallo`). Bei aktiviertem Mock-Feature schreibt der Plattform-Mock Press/Release-Ereignisse auf stdout; `--delay-ms`/spezifische Override-Flags spiegeln den Pointer-Stil.
- [x] Zusätzlich: `keyboard list [--format text|json]` gibt die vom aktiven Keyboard‑Device unterstützten Tastennamen zurück (Quelle: `KeyboardDevice::known_key_names()`).
- [x] Tests (`rstest`): Parser-Unit-Tests, Runtime-Tests sowie CLI-Integration gegen den Mock (Fokus-Pflicht, Fehlerformat). Feature-Flag `mock-provider` berücksichtigen.
- [x] README/CLI-Hilfe (`--help`) um Keyboard-Beispiele ergänzen.

### 18. Mock-Fallback & Build-Zuordnung
- [x] `mock-provider`-Feature dokumentieren und sicherstellen, dass es sämtliche produktiven Plattform- und Provider-Crates ausschließt (nur Mock bleibt übrig).
- [x] Überprüfen, dass reale Plattformmodule ausschließlich über `cfg(target_os = …)` eingebunden werden und niemals parallel aktiv sind.

### 18a. ScrollIntoView Runtime-Aktion – Design & Mock-Implementation
- [ ] **Runtime-Aktion definieren**: `scroll_into_view(node: &Arc<dyn UiNode>)` als Runtime-Funktion für **alle Elemente**. Versucht das gesamte Element sichtbar zu machen; bei großen Elementen wird der Bereich um den ActivationPoint priorisiert.
- [ ] **ScrollIntoViewPattern**: Provider implementieren `ScrollIntoViewPattern` für jedes Element, nicht nur für scrollbare. Fallback-Verhalten bei fehlenden scrollbaren Containern (No-Op).
- [ ] **Dynamische Container-Erkennung**: Runtime traversiert zur Laufzeit die Ancestor-Kette und findet scrollbare Container mit `Scrollable`-Pattern.
- [ ] **Runtime-Implementierung**: Fokussierte Scroll-Logik die scrollbare Container zur Laufzeit identifiziert und Element in sichtbaren Bereich scrollt.
- [ ] **Mock-Implementation**: `platynui-provider-mock` erweitern um scrollbare Container-Hierarchien und Runtime-Logging für `scroll_into_view()`-Aufrufe. Implementierung für alle Elemente mit intelligenter Scroll-Strategie (ganzes Element vs. ActivationPoint-Bereich).
- [ ] **CLI-Integration**: Optionaler `--scroll-into-view`-Flag für `pointer click` Kommando sowie separates `scroll-into-view` Subkommando.
- [ ] **Tests**: Umfassende Tests für Runtime-Container-Suche, Options-Parsing, Mock-Logging und CLI-Integration (rstest, Feature-Flag `mock-provider`).
- [ ] **Dokumentation vervollständigen**: Technologie-Mapping und Implementierungsdetails für verschiedene Plattformen dokumentieren.

### 19. Plattform Windows – Devices & UiTree

#### 19.1 Pointer (`platynui-platform-windows`)
- [x] Bewegungs-/Klick-/Drag-/Scroll-Pipeline auf Basis der Win32-Input-APIs fertigstellen, inklusive Double-Click-Metriken und Fehlerbehandlung (Logging folgt separat).
- [x] Integration in Runtime-Registry (`pointer_devices`) sicherstellen und PointerOverrides/Profile validieren.
- [x] Pointer-Gerät nutzt zentral gesetzte Per-Monitor-V2-DPI-Awareness und kommentiert das Verhalten (`SetCursorPos` + Virtual-Screen-Bounds).
- [x] Tests (Mock): CLI `pointer move` deckt negative Koordinaten ab; Architektur-/Plan-Doku verlinkt DPI-Awareness.

#### 19.2 Highlight (`platynui-platform-windows`)
- [x] Overlay-Lifecycle (Erstellen/Aktualisieren/Clear) mit Z-Order- und Farbensteuerung implementieren, Ressourcen sauber freigeben.
- [x] Runtime-Anbindung (`highlight_providers`) herstellen und Fallback-Verhalten definieren.

Implementierungsstand (2025-09-29)
- Overlay: Layered-Window (UpdateLayeredWindow) mit `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE`; Anzeige mit `SW_SHOWNOACTIVATE`; `WM_MOUSEACTIVATE → MA_NOACTIVATE` (kein Fokus-/Aktivierungswechsel, klick‑durchlässig, nicht in Alt‑Tab/Taskbar).
- Darstellung: Roter Rahmen (RGBA 255,0,0,230), 3 px Rahmenstärke, 1 px Abstand um Ziel‑Rects (keine Überdeckung des Inhalts).
- Clamping: Rahmengeometrie wird gegen Desktop‑Bounds geschnitten; komplett außerhalb → kein Overlay. Abgeschnittene Seiten werden gestrichelt (Muster 6 an/4 aus) gezeichnet; ungeschnittene Seiten bleiben durchgezogen.
- Dauer/Timeout: Das Overlay im Highlight‑Provider plant ein `clear()` nach der minimal angeforderten Dauer (generation‑aware). Die Runtime selbst schedult keinen Fallback‑Timer. Die CLI blockiert für die Dauer (sichtbare Haltbarkeit in Ein‑Shot‑Szenarien).
- CLI: `platynui-cli highlight --rect X,Y,WIDTH,HEIGHT [--duration-ms N]` (Default 1500 ms) alternativ zu XPATH; `--clear` zum Entfernen aktiver Highlights; Prozess hält für die angegebene Dauer.

#### 19.3 Screenshot (`platynui-platform-windows`)
- [x] Capture via GDI (BitBlt) umsetzen, Cropping/Format-Wandlung und Fehlerpfade behandeln.
- [x] Runtime (`screenshot_providers`) verdrahten, Parameter dokumentieren.
- [x] Architektur-/CLI-Doku aktualisieren.

Implementierungsstand (2025-09-29)
- Provider: GDI‑basierter Capture-Pfad (`CreateDIBSection` top‑down 32 bpp + `BitBlt` aus Screen‑HDC). Rückgabeformat `BGRA8`.
- Region: Desktop‑Clamping (Virtual‑Screen‑Bounds). Vollständig außerhalb → Fehler; teilweise außerhalb → gekappte Größe (Beispiel: `--rect -10,-10,200,2000` ergibt 200×1990, wenn Desktop bei (0,0) beginnt).
- CLI: `platynui-cli screenshot [--rect X,Y,W,H] [FILE]`. Ohne `FILE` wird `screenshot-<epoch_ms>.png` im CWD erzeugt; Existenz → numerische Suffixe. Negative Koordinaten werden korrekt geparst (Clap `allow_hyphen_values`).
- PNG: CLI konvertiert BGRA→RGBA, schreibt PNG (`png` 0.18.0).

Ergänzungen (2025-09-29 später am Tag)
- Ressourcen-Cleanup umgesetzt: `ReleaseDC(NULL, screen_dc)` im Screenshot‑Provider (alle Pfade), `ReleaseDC` im Highlight‑Overlay, `DestroyWindow` bei `clear()` (Overlay wird vollständig entsorgt).
- DPI-Hinweis ergänzt: Per‑Monitor‑V2‑DPI‑Awareness aktiv; Koordinaten = Desktop‑Pixel; GDI/LayeredWindow arbeiten in denselben Gerätepixeln.

#### 19.4 Platform-Initialisierung (`platynui-platform-windows`)
- [x] `PlatformModule::initialize()` verwendet, um den Prozess einmalig auf `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` zu setzen, bevor Geräte/Provider registriert werden.
- [x] Tests/Doku: Init-Reihenfolge abgesichert und dokumentiert. Ein Runtime-Test registriert ein Test-`PlatformModule`, das einen Flag setzt, und einen Provider, der in `create()` den Flag prüft. `Runtime::new()` ruft `initialize_platform_modules()` vor dem Provider-Instanziieren auf (Kontrakt getestet). DPI-/Koordinatenhinweis in `architekturkonzept_runtime.md` ergänzt.

#### 19.5 Desktop Provider (`platynui-platform-windows`)
- [x] DesktopInfoProvider für Windows implementieren (Monitor-Auflistung, Bounds, RuntimeId `windows://desktop`).
- [x] Desktop-Attribute (OS-Version, Monitor-Metadaten) gemäß Runtime-Vertrag: Monitore via `EnumDisplayMonitors`/`GetMonitorInfoW(MONITORINFOEXW)` plus Friendly‑Names über `DisplayConfigGetDeviceInfo(DISPLAYCONFIG_TARGET_DEVICE_NAME)` (Fallbacks vorhanden); OS‑Version als `<major>.<minor>[.<build>]` (Fallback vorhanden).
 - [x] Tests: Smoke‑Tests unter Windows (≥1 Monitor, genau ein Primary, Union(Monitore) ⊆ Virtual‑Screen, IDs/Namen ≠ leer); Helper‑Tests (`trim_wstr`, `os_version_string`).
 - [x] DPI‑Scale pro Monitor: `GetDpiForMonitor(MDT_EFFECTIVE_DPI)` → `scale_factor = dpi/96.0`; CLI `info` zeigt den Faktor als `@ 1.25x`.

#### 19.6 UIAutomation Provider (`platynui-provider-windows-uia`)
- [x] COM-Init (MTA) und Wiederverwendung: `IUIAutomation` + RawView‑`IUIAutomationTreeWalker` als thread‑lokale Singletons.
- [x] Baumaufbau (Application → Window → Control/Item) via Raw View `TreeWalker` (Parent/FirstChild/NextSibling); kein `FindAll`, keine UIA‑`CacheRequest` (eigenes Caching optional später).
- [x] Iteratoren statt Vektoren: gemeinsamer `ElementChildrenIter` mit Lazy‑Erstaufruf (`first`‑Flag) und Sibling‑Traversal.
- [x] Rollen-/Namespace‑Mapping sowie RuntimeId‑Übernahme; Attribute als lazy `UiAttribute.value()`.
- [x] Patterns (Slice 1): `Focusable` (SetFocus); `WindowSurface` nur bei Verfügbarkeit (`WindowPattern`/`TransformPattern`).
- [x] Gruppierte Sicht (Application → Window): Synthetische `app:Application`‑Knoten erzeugen und Top‑Level‑Elemente per `CurrentProcessId` gruppieren; stabile RuntimeId (`uia-app://<pid>`) und sinnvoller `doc_order_key` definiert.
 - [x] Application‑Attribute befüllt: `application::PROCESS_ID`, `NAME` (ohne Dateierweiterung; aus `EXECUTABLE_PATH` abgeleitet), `EXECUTABLE_PATH`, `COMMAND_LINE` (Roh‑String), `USER_NAME` (DOMAIN\User), `START_TIME` (ISO‑8601), `ARCHITECTURE`. Kinder der Application sind die zugehörigen `control:`‑Top‑Level‑Knoten.
- [x] WindowSurface‑Status/Capabilities als Attribute bereitstellen: `window_surface::IS_MINIMIZED`, `IS_MAXIMIZED`, `IS_TOPMOST`, `SUPPORTS_MOVE`, `SUPPORTS_RESIZE` (über `WindowPattern`/`TransformPattern`).
- [x] `WindowSurface.accepts_user_input()` implementieren (Heuristik: `IsEnabled && !IsOffscreen`; perspektivisch `WaitForInputIdle`). Optional gleichnamiges Attribut bereitstellen.
 - [x] Virtualisierte Elemente: Best‑effort `VirtualizedItemPattern::Realize()` vor Kind‑Traversal/Focus; `UiNode::is_valid()` nutzt eine leichte Live‑Abfrage.
  - [x] Native UIA‑Properties: Unterstützung und Werte ermitteln
  - [x] Properties ermitteln über Programmatic‑Name‑Katalog (IDs im typischen UIA‑Bereich via `IUIAutomation::GetPropertyProgrammaticName()`); nur Werte übernehmen, die per `GetCurrentPropertyValueEx(propertyId, true)` einen sinnvollen Wert liefern (Sentinels/Empty filtern).
  - [x] Werte abrufen über `IUIAutomationElement::GetCurrentPropertyValueEx(propertyId, /*ignoreDefault*/ true)`; Rückgabewerte konvertieren (VARIANT → `UiValue`).
  - [x] Sentinels korrekt behandeln: `ReservedNotSupportedValue` → „nicht unterstützt“ (gefiltert); `ReservedMixedAttributeValue` → „gemischt“ (gefiltert).
  - [x] Exponierung als Attribute im Namespace `native:`: Für jedes unterstützte Property ein `UiAttribute` mit Programmatic Name (z. B. `native:ClassName`). Optionales Aggregatobjekt kann später ergänzt werden.
  - [x] Typumsetzung definiert: `VT_BOOL`→Bool, `VT_I2/VT_UI2/VT_I4/VT_UI4/VT_I8/VT_UI8`→Integer, `VT_R8/VT_R4/VT_DECIMAL/VT_DATE`→Number, `BSTR`→String, `SAFEARRAY(1D)`→Array (inkl. obiger Elementtypen), `IUnknown`‑Sentinels wie oben; unbekannte Typen auslassen.
  - [x] Details: `VT_DECIMAL` wird via `VarR8FromDec` nach `f64` konvertiert; SAFEARRAY‑Elemente analog je Elementtyp abgebildet; UIA‑Sentinels (`ReservedNotSupportedValue`, `ReservedMixedAttributeValue`) werden gefiltert; VARENUM‑Konstanten aus dem `windows`‑Crate anstelle magischer Zahlen verwendet; Werteabruf über `GetCurrentPropertyValueEx(id, /*ignoreDefault*/ true)`.

Aktualisierungen/Status (2025‑10‑05)
- [x] App‑Knoten streamt Attribute per Iterator (lazy); zusätzliche `app:RuntimeId` als Attribut verfügbar.
- [x] `app:Name` leitet sich aus dem Prozess‑Image (Dateiname ohne `.exe`) ab — nicht mehr PSAPI/K32 BaseModuleName.
- [x] Root‑Streaming: Erst `control:`‑Desktop‑Kinder (eigenen Prozess ausfiltern, PIDs sammeln), anschließend pro gesehener PID genau ein `app:Application` in stabiler Reihenfolge.
- [ ] Tests: Windows‑Smoke für Iterator‑Reihenfolge (Elements → Apps), Eigener‑PID‑Filter, App‑Attribute, Native‑Properties.
- [ ] Tests: Struktur-/Attribut‑Abdeckung, Pattern‑Liste, Desktop‑Top‑Level (Windows‑only smoke). Optional: Root‑Geschwister‑Iteration.

Aktualisierung (2025‑10‑10)
- Watch‑Filter sind noch offen; die aktuelle CLI unterstützt `--expression` und `--limit`.
- Highlight‑Timer‑Clear liegt im Provider‑Overlay; die Runtime schedult keinen Fallback‑Timer (siehe 19.2). Die CLI blockiert optional für die gewünschte Dauer.

Aktualisierung (2025‑10‑17)
- Python Bindings: `Runtime.highlight(rects: Rect | Iterable[Rect], duration_ms: float | None)` akzeptiert einen einzelnen `Rect` oder beliebige Iterables von `Rect`‑Objekten; intern werden Iterables materialisiert und als einzelner `HighlightRequest` an die Runtime/Provider übergeben.
- Python Core‑Typen: `Point`, `Size`, `Rect`, `PatternId`, `RuntimeId`, `TechnologyId`, `Namespace` implementieren `__eq__`/`__ne__` sowie `__hash__` und sind damit als Dict‑Keys/Set‑Elemente nutzbar. Stubs wurden synchronisiert.

Aktualisierung (2025‑10‑21)
- FFI & Host‑Resolver: Owned‑Iterator (`EvaluationStream`, `Runtime::evaluate_iter_owned`) und `NodeResolver` in der Runtime sind implementiert und in den Python‑Bindings als `EvaluationIterator` verfügbar. Der frühere Backlog‑Punkt wurde geschlossen.
- Linking‑Makros: Dokumentation ergänzt; CLI und Python‑Native verwenden `platynui_link_providers!()` zur OS‑spezifischen Verlinkung.
- Pointer‑Overrides: CLI/Runtime unterstützen detaillierte Overrides (Bewegungsmodus, Beschleunigungsprofile, Geschwindigkeitsfaktor sowie Schritt-/Zeit‑Parameter für Move/Scroll/Click). Hinweise im Plan ergänzt.
- UIA‑Details: Best‑effort‑Realize für virtualisierte Items und eine leichte `UiNode::is_valid()`‑Liveness‑Prüfung sind aktiv.
- Desktop‑Fallback: Bei fehlenden Desktop‑Providern liefert die Runtime einen generischen Fallback‑Desktop (Diagnosezwecke).

Aktualisierung (2025‑11‑05)
- Watch‑Filter bleiben offen (CLI `watch` unterstützt weiterhin `--expression`/`--limit`; Feingranularität für Event‑Typ/IDs ist Backlog).
- Distribution & Packaging: Rust‑CLI wird als Python‑Wheel (`packages/cli`) gebaut; native Bindings `platynui_native` liefern `core`/`runtime` Submodule (maturin/uv). Details unten.

Aktuelle Design-Notizen (2025‑09‑30)
- Keine Actor‑Schicht, kein NodeStore: `UiaNode` wrappt direkt `IUIAutomationElement`.
- Provider liefert Desktop‑Kinder über denselben Iterator; Root‑Geschwister werden derzeit nicht zusammengeführt (kann ergänzt werden).
- `invalidate()` in `UiaNode` bewusst No‑Op (Trait‑Signaturen geben Referenzen zurück; Attributwerte bleiben lazy und können unabhängig neu gelesen werden).

Weitere Details siehe: `docs/provider_windows_uia_design.md`.

#### 19.7 Keyboard (`platynui-platform-windows`)
- [x] Key-Code-Auflösung und Event-Injektion via Win32 `SendInput`/`MapVirtualKeyW` implementiert; Press/Release/Type werden über die Runtime-Sequenzpipeline gespeist.
- [x] VK‑Namensauflösung: Globale `VK_MAP` mit allen von Windows definierten `VK_*`-Konstanten (ohne Präfix `VK_`, z. B. `ESCAPE`, `RETURN`, `F24`, `LCTRL`, `RMENU`). Buchstaben/Ziffern werden bewusst nicht in die Map aufgenommen, sondern als Einzelzeichen über die Zeichenpfade behandelt.
  Inklusive neuerer Konstanten: `NAVIGATION_*` und `GAMEPAD_*` (Windows 10+). Aliasse für reservierte Zeichen: `PLUS`, `MINUS`, `LESS`/`LT`, `GREATER`/`GT`.
- [x] Zeichenpfad: Einzelne Zeichen werden mit `VkKeyScanW` auf `(vk, shift, ctrl, alt)` gemappt; für Buchstaben wird bei aktivem CapsLock das Shift‑Bit invertiert. Fallback auf Unicode‑Injection (`KEYEVENTF_UNICODE`) für Zeichen ohne Mapping.
- [x] AltGr: Wenn `VkKeyScanW` `Ctrl+Alt` signalisiert, injiziert der Provider stattdessen `VK_RMENU` (Right Alt) anstelle eines getrennten `Ctrl+Alt`‑Chords (entspricht gängiger Windows‑Semantik für AltGr).
- [x] Extended Keys: Für bekannte Extended‑Keys wird `KEYEVENTF_EXTENDEDKEY` gesetzt (u. a. Right Ctrl/Alt, Insert/Delete/Home/End/PgUp/PgDn, Pfeile, NumLock, Divide, Windows/Menu).
- [x] Links/Rechts‑Modifier‑Aliasse: `LSHIFT/LEFTSHIFT`, `RSHIFT/RIGHTSHIFT`, `LCTRL/LEFTCTRL/LEFTCONTROL`, `RCTRL/RIGHTCTRL/RIGHTCONTROL`, `LALT/LEFTALT`, `ALTGR/RALT/RIGHTALT`, `LEFTWIN/RIGHTWIN` sind zusätzlich zu den offiziellen Namen verfügbar. Präfix `VK_` wird nicht benötigt und nicht akzeptiert.
- [x] Symbol‑Aliasse für reservierte Zeichen: `PLUS` (`+`), `MINUS` (`-`), `LESS`/`LT` (`<`), `GREATER`/`GT` (`>`). Implementiert in Mock und Windows; Linux/macOS folgen.
- [x] Abdeckung weiterer VK‑Gruppen: neben NAVIGATION_* und GAMEPAD_* sind auch regionale/IME‑bezogene Tasten wie `VK_ABNT_C1`, `VK_ABNT_C2` (brasilianisches Layout) und japanische DBE‑Tasten (`VK_DBE_*`) sowie die OEM‑Gruppe (`VK_OEM_*`, inkl. `VK_OEM_102`) aufgenommen. Namen werden ohne `VK_`‑Präfix exponiert.
- [x] L/R‑Modifier und Synonyme: neben den generischen Modifiers (`Shift`, `Control`, `Alt`, `Windows`) existieren synonyme Links/Rechts‑Bezeichner (`LSHIFT`/`LEFTSHIFT`, `RSHIFT`/`RIGHTSHIFT`, `LCTRL`/`LEFTCTRL`/`LEFTCONTROL`, `RCTRL`/`RIGHTCTRL`/`RIGHTCONTROL`, `LALT`/`LEFTALT`, `RALT`/`RIGHTALT`/`ALTGR`, `LEFTWIN`/`RIGHTWIN`). Diese lösen auf die jeweiligen VK‑Codes (z. B. `VK_LSHIFT`, `VK_RMENU`).
- [x] Bekannte Namen listen: `KeyboardDevice::known_key_names()` liefert die unterstützten Namen (Union aus `VK_MAP`‑Keys plus `A..Z`/`0..9`). CLI‑Unterbefehl `keyboard list` gibt diese Namen in Text/JSON aus.
- [ ] Fehlerabbildung (`KeyboardError`) noch verfeinern und, wo sinnvoll, auf Win32‑Fehler (LastError) abstützen.
- [ ] Tests (Windows‑Host): AltGr‑Szenarien (z. B. `@` via DE‑Layout), Groß-/Kleinschreibung mit/ohne CapsLock, Extended‑Keys und Shortcuts. Ergänzend: Stabilität der Namensliste (CLI/Python).
- [ ] Tests (VK‑Sondergruppen): OEM‑Tasten (`OEM_*` inkl. `OEM_102`), ABNT‑Tasten (`ABNT_C1/ABNT_C2`) und DBE‑Tasten (`DBE_*`) mindestens „does not crash“ verifizieren; Verhalten ist layout‑/IME‑abhängig.
- [ ] Option bewerten: `VkKeyScanExW` mit Thread‑Layout (HKL) einsetzen, falls die Layout‑Auflösung im Multi‑Layout‑Szenario unzureichend ist (Feature‑Gate, dokumentierte Abhängigkeit zum aktiven Layout).

Aktualisierung (2025‑10‑22)
- Implementierung des Windows‑Keyboard‑Devices abgeschlossen und in die Runtime integriert. Neue Runtime‑API `keyboard_known_key_names()` sowie Python‑Binding `Runtime.keyboard_known_key_names()` hinzugefügt. CLI erweitert um `platynui-cli keyboard list [--format text|json]`.
- Mapping‑Entscheidung: Radikale Trennung der „benannten“ VK‑Tasten (ohne `VK_`‑Präfix) und der zeichenbasierten Eingabe. Für Buchstaben/Ziffern wird nicht über `VK_*`‑Konstanten injiziert, sondern über `VkKeyScanW` bzw. Unicode, um Layout‑Korrektheit (AltGr, Dead‑Keys, CapsLock) sicherzustellen.
- Bekannte Einschränkungen: Einige NAVIGATION_*/GAMEPAD_*/OEM/DBE/ABNT‑VKeys erzeugen über `SendInput` keine sichtbare Wirkung in allen Apps/Windows‑Versionen. Wir behandeln diese als Best‑Effort‑Mapping und dokumentieren Abweichungen in Tests/README, sobald Beobachtungen vorliegen.
- Bekannte offene Punkte: Einsatz von `VkKeyScanExW` mit Thread‑Layout (HKL) evaluieren; optional L/R‑spezifische Modifier bei erzwungener Injektion; Clippy‑Hinweis im CLI (Sortierung) umgesetzt; Cross‑Build‑Hinweis siehe unten.

Kurzfassung (EN)
- Windows keyboard device implemented (SendInput, VkKeyScan). Complete VK name map (without `VK_` prefix), AltGr as `VK_RMENU`, extended‑key flagging, known key names exposed to CLI/Python. Remaining: refine error mapping, verify AltGr on DE layout, consider VkKeyScanExW.


#### 19.8 Fokus & WindowSurface via UIA
- [x] Fokussteuerung (`SetFocus`) und WindowSurface-Aktionen (aktivieren/minimieren/maximieren/verschieben) direkt über UIAutomation (`WindowPattern`, `TransformPattern`) implementiert.
- [x] Fehler-Mapping in `FocusableAction`/`WindowSurface` umgesetzt, Basic-Pattern-Integration funktional.
- [x] **WaitForInputIdle-Integration**: `WaitForInputIdle()` Win32 API in `accepts_user_input()` implementiert - prüft, ob Prozess bereit für Input ist (100ms Timeout). Kombiniert mit `IsEnabled` und `!IsOffscreen` für robuste Interaktionsbereitschaft.
- [ ] **Erweiterte Fehlerbehandlung**: Robuste Behandlung von Foreground-Locks, UAC-Dialogen, nicht-responsive Anwendungen.
- [ ] Integrationstests mit Provider-Nodes für verschiedene Anwendungstypen (WPF, WinForms, Win32, UWP).
- [ ] Dokumentation: Ablaufdiagramme, Troubleshooting (Foreground-Locks, UAC), Abgleich mit Provider-Checklist.

#### 19.9 ScrollIntoView Runtime-Aktion via UIA
- [ ] `scroll_into_view()` Runtime-Implementierung für UIA: `ScrollIntoViewPattern` für alle Elemente. Nutzt `ScrollItemPattern::ScrollIntoView()` und `VirtualizedItemPattern::Realize()` mit intelligenter Scroll-Strategie (ganzes Element vs. ActivationPoint-Fokus).
- [ ] Dynamische Container-Suche: Implementierung der Ancestor-Traversierung mit TreeWalker zur Laufzeit-Identifikation von scrollbaren Parent-Elementen mit `Scrollable`-Pattern.
- [ ] Fehlerbehandlung: UIA-spezifische Fehlerzustände (Element nicht realisierbar, Scroll nicht verfügbar, etc.) auf Runtime-Errors abbilden.
- [ ] Integration in bestehende CLI-Pointer-Logik: Automatisches ScrollIntoView vor Click-Aktionen als Option.
- [ ] Tests: UIA-Provider mit ScrollItemPattern/VirtualizedItemPattern, Koordination mit Windows-eigenen Scrollable Controls.

#### 19.9 Tests & Mock-Abgleich
- [ ] Gemeinsame Tests (Provider vs. Mock) für Bounds, ActivationPoint, Sichtbarkeit/Enabled, Fokuswechsel und WindowSurface-Aktionen etablieren.
- [ ] Abweichungen der UIA-API dokumentieren und Regression-Playbooks festlegen.
- [ ] Test-Infrastruktur (z. B. Windows-spezifischer CI-Job) entwerfen oder vorhandene Plattformen anpassen.

##### Cross-Build (Hinweis)
- Der komplette Workspace enthält das Python‑FFI‑Crate `platynui_native` (Maturin/PyO3). Ein Cross‑Build des gesamten Workspaces nach `x86_64-pc-windows-gnu` schlägt ohne passende Python‑Dev‑Umgebung i. d. R. fehl (fehlendes `-lpython3*`).
- Workarounds:
  - Nur die relevanten Crates bauen: z. B. `cargo build -p platynui-platform-windows -p platynui-provider-windows-uia --target x86_64-pc-windows-gnu`.
  - Alternativ den Workspace‑Build ohne Python‑Crate ausführen: `cargo build --workspace --exclude platynui_native --target x86_64-pc-windows-gnu`.
  - Für das Python‑Crate selbst Windows‑seitig mit Maturin bauen: `uv run maturin develop --release`.

### 20. CLI `window` – Windows-Integration
- [x] Implementiert: Fensterliste mit Status/Capabilities (minimized/maximized/topmost/accepts_user_input) und Bounds; Aktionen: activate/minimize/maximize/restore/close sowie move/resize/bring_to_front (inklusive `--wait-ms` für `accepts_user_input`). Deduplizierte Treffer pro RuntimeId, farbige Textausgabe und klare Fehlertexte bei leeren Treffern.
- [x] Runtime-/Python-Erweiterung `bring_to_front_and_wait` ergänzt (22.10.2025) und vom CLI `--wait-ms` genutzt.
- [x] Tests: Mock‑Abdeckung für Listing und Aktionssequenzen inkl. Fehlerpfade; E2E‑Tests auf echtem Windows bleiben optionaler Ausbau.

### 21. Plattform Linux/X11 – Devices & UiTree
- `platynui-platform-linux-x11`:
  - [x] DesktopInfoProvider (XRandR, Root-Fallback).
  - [x] Pointer via XTest (move/press/release/scroll).
  - [ ] Keyboard via xkbcommon-rs + XTest Injection.
  - [x] Screenshot via XGetImage (XShm optional).
  - [x] Highlight via override-redirect Segment-Fenster (kein XComposite-Pfad).
  - [ ] Fenstersteuerung über EWMH/NetWM.
  - [ ] `PlatformModule::initialize()` (XInitThreads, Extension-Checks).
- [ ] Fokus-Helper für AT-SPI2 + plattformspezifische Fallbacks.
- [ ] Tests: Desktop-Bounds, ActivationPoint, Sichtbarkeits- und Enable-Flags unter X11.
- [x] `platynui-provider-atspi`: D-Bus-Integration und Registry-Root, RuntimeId aus Objektpfad, Rollen-/Namespace-Mapping (inkl. `app:Application`), Streaming-Attribute.
- [x] `platynui-provider-atspi`: Component-gated Standard-Attribute (`Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsOffscreen`, `IsFocused`) und `Focusable` Pattern.
- [x] `platynui-provider-atspi`: Native Interface-Attribute (`Native/<Interface>.<Property>` inkl. `Accessible.GetAttributes` Mapping).
- [ ] `platynui-provider-atspi`: Baumstruktur verifizieren (Application → Window → Control/Item) und Window-Relationen dokumentieren.
- [ ] Ergänzende Tests (AT-SPI2) auf Basis des Windows-Testsets inkl. Namespaces `item`/`control`.
- [ ] Vorausplanen für Wayland: Vermittlungscrate `platynui-platform-linux` entwerfen, das zur Laufzeit anhand der Session-Umgebung (`$XDG_SESSION_TYPE`, heuristische Fallbacks) zwischen `platynui-platform-linux-x11` und `platynui-platform-linux-wayland` vermittelt, sobald letztere Implementierung verfügbar ist.

### 22. CLI `window` – Linux/X11-Integration
- [ ] CLI `window` nutzt X11-spezifische Funktionen (EWMH/NetWM) für Fensterlisten, Move/Resize etc.
- [ ] Tests: CLI `window` gegen Mock/X11-spezifische Szenarien (soweit automatisierbar).

### 23. Werkzeuge
- [x] CLI: `watch`‑Befehl mit Text/JSON‑Ausgabe und optionaler Query‑Auswertung pro Event (Fan‑out über `ProviderEventDispatcher`).
- [x] CLI: strukturierte Ausgabe `--json` für `query` umgesetzt.
- [x] CLI: Pointer-Kommandos liefern bei XPath-Zielen eine Kurzbeschreibung des angesprochenen Elements (seit 20.10.2025).
- [ ] CLI: `dump-node`.
- [ ] Skript‑Integration/weitere CLI‑Ergonomie.
- [ ] Inspector (GUI): Tree-Ansicht mit Namespaces, Property-Panel (Patterns), XPath-Editor, Element-Picker, Highlight; arbeitet wahlweise Embedded oder via JSON-RPC.
- [ ] Beispiel-Workflows dokumentieren (Readme/Docs): XPath → Highlight, Fokus setzen, Fensterstatus (`accepts_user_input`) ermitteln.

#### 23.1 CLI `snapshot` – XML‑Tree Export (neu)
- [x] Spezifikation erstellt: `docs/cli_snapshot_spec.md` (Aufruf, XML‑Modell, Filter, Multi‑Root, Beispiele).
- [x] CLI‑Scaffold: neues Kommando `snapshot` einhängen (Args‑Parser mit Validierung).
- [x] Streaming‑XML‑Writer: feste Namespaces (`urn:platynui:*`), Elementname=Rolle, Attribute namespace‑qualifiziert; komplexe Werte als JSON‑String.
- [x] Attribut‑Filter: `--attrs default|all|list`, `--include/--exclude` (Wildcards `*`), `--include-runtime-id`.
- [x] Alias‑Attribute: standardmäßig erzeugen; per `--exclude-derived` unterdrücken.
- [x] Tiefenbegrenzung: `--max-depth` (0=Wurzel, 1=+Kinder, …).
- [x] Multi‑Root: Wrapper `<snapshot>` bei `--output`; Dateisplitting via `--split PREFIX` (sichere Nummerierung, keine Überschreibung).
- [x] Pretty‑Modus: optionale Einrückung/Zeilenumbrüche (`--pretty`).
- [x] Tests (Mock): Golden‑Vergleiche für Default/All/Filter/Alias/Depth/Multi‑Root/Pretty; Fehlerfälle (leere Query, ungültige Patterns).
- [x] Doku: README‑Abschnitt/Beispiele verlinken; `docs/cli_snapshot_spec.md` vom Plan referenzieren.

### 24. Qualitätssicherung & Prozesse
- [x] CI-Pipeline: `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`, `cargo nextest run`, `uv run ruff check`, `uv run mypy`, Wheel‑Builds via maturin/uv (siehe `.github/workflows/ci.yml`).
- [ ] Contract-Tests für Provider & Devices (pattern-spezifische Attribute, Desktop-Koordinaten, RuntimeId-Quellen).
- [ ] Dokumentation pflegen: Architekturkonzept, Patterns, Provider-Checkliste, Legacy-Analyse; Hinweis auf lebende Dokumente beibehalten.
- [ ] Release-/Versionierungsstrategie festlegen (SemVer pro Crate? Workspace-Version?).

### 25. Backlog & Explorations
- [x] Kontextknoten-Resolver: `RuntimeId`-basierte Re-Resolution für Kontextknoten außerhalb des aktuellen Wurzelknotens (umgesetzt; siehe Aktualisierung 2025‑10‑21).
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
  - `platynui-platform-macos`: Devices via Quartz/Event-Taps, Screenshot/Highlight mit CoreGraphics, Fenstersteuerung via AppKit.
- Optionales Erweiterungs-Interface für Provider (z. B. optionale Runtime-Services).
- Persistentes XPath-Caching & Snapshot-Layer.
- Optionaler Wayland-Support (Runtime-Erkennung Wayland/X11, Provider-Auswahl, Devices).
- Weitere Patterns (z. B. Tabellen-Navigation, Drag&Drop) nach Bedarf evaluieren.
- Erweiterte Eingabegeräte (Gamepad, Stift), Barrierefreiheits-Funktionen.
- Touch-Device-Unterstützung (Traits, CLI-Befehle) nach erfolgreichem Pointer/Keyboard-Ausbau.
- Community-Guides, Beispiel-Provider, Trainingsmaterial.

### 26. UiNode `Id` – Umsetzungsschritte
- Core
  - [x] Attribut definieren: `control:Id` als optionales String‑Attribut (leer/fehlend = nicht gesetzt). Konstanten in `attribute_names` ergänzt.
  - [x] UiNode‑Trait um `fn id(&self) -> Option<String>` erweitert (Default `None`).
  - [x] Dokumentation: Architektur/Patterns/Checkliste um Semantik und Beispiele erweitert (XPath‑Nutzung, Stabilität, Abgrenzung zu `RuntimeId`) – siehe `docs/architekturkonzept_runtime.md`, `docs/patterns.md`, `docs/provider_checklist.md`.
- Runtime/XPath
  - [x] Keine Alias‑Ableitungen nötig (reiner String). Sicherstellen, dass `@control:Id` als `xs:string` atomisiert wird. (Verifiziert 2026-02-03: `UiValue::String` → `XdmAtomicValue::String` in `crates/runtime/src/xpath.rs`.)
- Provider
  - [x] Windows/UIA: `AutomationId` → `control:Id` übernommen (leere Strings = „nicht gesetzt“). `UiNode::id()` nutzt `CurrentAutomationId()`.
  - [x] Windows/ApplicationNode: `id()` liefert Prozessname (Executable‑Stem); `@control:Id` wird nur erzeugt, wenn gesetzt.
  - [ ] AT‑SPI2: falls verfügbar `accessible_id` mappen; ansonsten auslassen (kein Fallback generieren).
  - [ ] macOS/AX: `AXIdentifier` mappen (sofern vorhanden); ansonsten auslassen.
  - [ ] Application‑Knoten: Plattformangemessene, möglichst stabile Kennung (präferiert Bundle Identifier auf macOS). Fallbacks (z. B. `ProcessName`) dokumentieren.
  - [ ] Windows (Option): AUMID als Application‑Id prüfen und ggf. bevorzugen. Ermittlung über Main‑Window‑Handle → `SHGetPropertyStoreForWindow(hwnd)` → `PKEY_AppUserModel_ID`. Fallback bleibt Prozessname.
- Tests
  - [ ] Core‑Contracttests: `Id` darf fehlen/leer sein; wenn gesetzt, ist es `xs:string` und nicht sprachabhängig.
  - [ ] Provider‑Tests: Windows/UIA Smoke‑Test, der `AutomationId` → `Id` spiegelt (Mock/Fixture); AT‑SPI2/macOS optional, sobald verfügbar.
- [ ] CLI/Python: Beispiel‑Abfragen dokumentieren (`//*[@control:Id='login-button']`).

### 27. Distribution & Packaging
- CLI als Python‑Wheel `platynui-cli` (maturin bindings = `bin`), baut das Rust‑CLI und liefert das Binary plattformabhängig aus (`packages/cli`).
- Native Python‑Bindings `platynui_native` (PyO3/maturin) mit Submodulen `core` (Typen/IDs/Namensräume) und `runtime` (UiNode, Evaluate, Pointer/Keyboard/Highlight/Screenshot) unter `packages/native`.
- Lokale Entwicklung: `uv sync --dev`, danach `uv run maturin develop -m packages/native/Cargo.toml --release` für das Native‑Modul und `cargo build --workspace` für Rust‑Crates.
- Release‑Artefakte (Wheels) werden in CI für Linux/macOS/Windows gebaut und als Artefakte bereitgestellt.

### 28. Robot BareMetal (interim)
- Robot Framework‑Library `PlatynUI.BareMetal` mit Keywords für Query, Pointer, Keyboard, Window, Screenshot: `src/PlatynUI/BareMetal/__init__.py`.
- Akzeptanztests starten unter `tests/BareMetal/`; weitere Suites folgen. Bis zur finalen Runner‑Integration bitte temporäre Schritte im PR vermerken.
