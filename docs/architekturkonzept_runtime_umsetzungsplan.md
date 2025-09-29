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
- Mock-Stack: `platynui-provider-mock`/`platynui-platform-mock` hängen am optionalen Feature `mock-provider`. Werkzeuge (z. B. `cargo run -p platynui-cli --features mock-provider -- watch`) und Tests nutzen dieses Feature gezielt; Standard-Builds bleiben ohne Mock.

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
- [x] XPath-Atomisierung auf `typed_value()` umstellen: `UiValue`-Varianten werden in XDM-Atomics überführt (Booleans, Integer, Double), komplexe Strukturen (`Rect`, `Point`, `Size`) bleiben als JSON-Strings; Regressionstests (`data(//@*:Bounds.Width)`, `data(//@*:IsVisible)`) absichern.

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
- [x] Provider-spezifische Snapshots: Runtime hält pro Provider einen eigenen Knoten-Snapshot und aktualisiert ihn nur, wenn `event_capabilities = None` (Polling) oder ein passendes Ereignis / Change-Hint eintrifft.
- [x] Inventory-basierte Registrierungsmakros (`register_provider!`, `register_platform_module!`), inkl. Tests für Registrierungsauflistung; weitere `cfg`-Szenarien folgen bei der Runtime-Einbindung.
- [x] Factory-Lifecycle: Entscheidung dokumentiert – Provider erhalten bewusst nur `Arc<dyn UiTreeProvider>` ohne zusätzliche Services; Geräte bleiben in der Runtime.
- [x] Provider-Checkliste (`docs/provider_checklist.md`) via Contract-Test-Suite abgedeckt (Mock-Provider nutzt `contract::testkit`; künftige Provider erhalten dieselben Prüfungen, Tests laufen unter `cargo test`).
- [x] `ProviderDescriptor` um `event_capabilities` erweitern (Bitset `None`/`ChangeHint`/`Structure`/`StructureWithProperties`), Runtime-Strategie dokumentieren und Tests vorbereiten, damit Voll-Refresh nur bei fehlender Event-Unterstützung nötig bleibt.

### 5. CLI `list-providers` – Mock-Basis schaffen
- [x] Minimalen Laufweg „Runtime + platynui-platform-mock + platynui-provider-mock“ herstellen (Provider-Registry initialisieren, Mock-Provider instanziieren).
- [x] `platynui-platform-mock`: Grundgerüst mit Stub-Geräten & Logging, liefert zumindest Technologie-/Versionsinformationen.
- [x] `platynui-provider-mock`: Registriert sich mit eindeutiger `ProviderDescriptor`, stellt einfache Baumdaten bereit.
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
- [x] CLI-Kommando `watch`: Streaming-Ausgabe (Text/JSON) mit Filtern (`--namespace`, `--pattern`, `--runtime-id`) sowie optionaler XPath-Nachabfrage (`--expression`) und `--limit` für Tests.
- [x] Tests: Simulierte Eventsequenzen (TreeInvalidated + NodeUpdated) prüfen Ausgabe in Text/JSON-Format.

### 9. CLI `highlight`
- [x] `HighlightProvider` in `platynui-core` finalisieren (`highlight(&[HighlightRequest])`, `clear()` inkl. optionaler Dauerangabe).
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

### 14. CLI `pointer`
- [x] `PointerDevice`-Trait in `platynui-core` definieren: elementare Aktionen (`position()`, `move_to(Point)`, `press(PointerButton)`, `release(PointerButton)`, `scroll(ScrollDelta)`), Double-Click-Metadaten (`double_click_time()`, `double_click_size()`), Rückgabewerte/Fehler via `PlatformError`. Alle Koordinaten bleiben `f64` in Desktop-Bezug.
- [x] Bewegungs-Engine im Runtime-Layer ergänzen (Schrittgeneratoren linear/Bezier/Overshoot/Jitter, konfigurierbare Delays `after_move_delay`, `press_release_delay`, `multi_click_delay`, `before_next_click_delay`). Hardware-Defaults bleiben in `PointerSettings` (z. B. `double_click_time`, `double_click_size`, Standard-Button), während Bewegungsparameter im `PointerProfile` liegen; `PointerOverrides` dient als einziger Options-Typ pro Aufruf (`move_to`, `click`, `drag`, `scroll`). Die Builder-API (`PointerOverrides::new().origin(...).after_move_delay(...)`) bildet ausschließlich die Abweichungen zu den aktiven Runtime-Defaults ab. CLI-Optionen können Defaults und Ad-hoc-Overrides setzen.
- [x] `platynui-platform-mock`: Pointer-Implementation bereitstellen (Move/Press/Release/Scroll) mit Logging-Hooks (`take_pointer_log`, `reset_pointer_state`) und deterministischen Ergebnissen; Double-Click-Zeit/-Größe über Mock-Konstanten simulieren.
- [x] CLI-Vorbereitung: Parser für Koordinaten (`parse_point`), Scroll-Deltas (`parse_scroll_delta`) und Tasten (`parse_pointer_button`) ergänzen.
- [x] CLI-Kommando `pointer`: Unterbefehle für `move`, `click`, `press`, `release`, `scroll`, `drag`, `position`, optional `--motion <mode>` sowie `--origin` (`desktop`, `bounds`, `absolute`) für relative Koordinaten; Koordinaten-/Button-Parsing, Ausgabe mit Erfolg/Fehlerinformation.
- [x] Tests (`rstest`): Motion-Engine ist durch Runtime-Unit-Tests abgedeckt, CLI-Integration (Move/Click/Scroll) läuft gegen den Mock-Provider und nutzt das Feature-Flag `mock-provider`.

### 15. Keyboard – Trait & Settings
- [x] `KeyboardDevice`-Trait in `platynui-core` fixieren (`key_to_code`, `send_key_event(KeyboardEvent)`, `start_input`/`end_input` nur für tastaturspezifische Vor-/Nachbereitung) inkl. Fehler-Typen (`KeyboardError`, `KeyCodeError`).
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
- [x] Tests (`rstest`): Parser-Unit-Tests, Runtime-Tests sowie CLI-Integration gegen den Mock (Fokus-Pflicht, Fehlerformat). Feature-Flag `mock-provider` berücksichtigen.
- [x] README/CLI-Hilfe (`--help`) um Keyboard-Beispiele ergänzen.

### 18. Mock-Fallback & Build-Zuordnung
- [x] `mock-provider`-Feature dokumentieren und sicherstellen, dass es sämtliche produktiven Plattform- und Provider-Crates ausschließt (nur Mock bleibt übrig).
- [x] Überprüfen, dass reale Plattformmodule ausschließlich über `cfg(target_os = …)` eingebunden werden und niemals parallel aktiv sind.

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
- Dauer/Timeout: Runtime triggert einen `clear()`‑Fallback nach der minimalen angeforderten Dauer (portabel), zusätzlich blockiert die CLI für die Dauer (sichtbare Haltbarkeit in Ein‑Shot‑Szenarien).
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
- [ ] COM-Hosting, Session-Lebenszyklus und Baumaufbau (Application → Window → Control/Item) entwickeln.
- [ ] Rollen-/Namespace-Mapping sowie RuntimeId-Übernahme etablieren; Attribute und Client-/Runtime-Patterns (u. a. `Focusable`, `WindowSurface`) bereitstellen.
- [ ] Tests: Struktur-/Attribut-Abdeckung, Pattern-Liste, Dokumentation der UIA-Abbildungen.

#### 19.7 Keyboard (`platynui-platform-windows`)
- [ ] Key-Code-Tabellen und Sequenzauflösung (Press/Release/Type) implementieren, Modifier-Chords und Unicode/IME-Fälle abdecken.
- [ ] Fehlerabbildung (`KeyboardError`) auf UIA/Win32-Codes abstützen und in Runtime integrieren.
- [ ] Tests: Sequenzparser-Resolver plus Echtgeräte-Logging (Mock-Vergleich), Dokumentation der unterstützten Key-Namen.


#### 19.8 Fokus & WindowSurface via UIA
- [ ] Fokussteuerung (`focus_control`) und WindowSurface-Aktionen (aktivieren/minimieren/maximieren/verschieben) direkt über UIAutomation (`WindowPattern`, `InvokePattern`, `WaitForInputIdle`) kapseln.
- [ ] Fehler-Mapping in `FocusableAction`/`WindowSurface` konsolidieren, Integrationstests mit Provider-Nodes.
- [ ] Dokumentation: Ablaufdiagramme, Troubleshooting (Foreground-Locks, UAC), Abgleich mit Provider-Checklist.

#### 19.9 Tests & Mock-Abgleich
- [ ] Gemeinsame Tests (Provider vs. Mock) für Bounds, ActivationPoint, Sichtbarkeit/Enabled, Fokuswechsel und WindowSurface-Aktionen etablieren.
- [ ] Abweichungen der UIA-API dokumentieren und Regression-Playbooks festlegen.
- [ ] Test-Infrastruktur (z. B. Windows-spezifischer CI-Job) entwerfen oder vorhandene Plattformen anpassen.

### 20. CLI `window` – Windows-Integration
- [ ] CLI-Kommandos erweitern, um Windows-spezifische Optionen (z. B. Fensterliste mit Prozessinfos) zu nutzen.
- [ ] Tests: CLI `window` gegen reale Windows-Fenstersteuerung (soweit automatisierbar) bzw. Mock-Abdeckung.

### 21. Plattform Linux/X11 – Devices & UiTree
- [ ] `platynui-platform-linux-x11`: Pointer/Keyboard via XTest oder äquivalente APIs, Screenshot (XShm), Highlight (XComposite), Fenstersteuerung über EWMH/NetWM.
- [ ] Fokus-Helper für AT-SPI2 + plattformspezifische Fallbacks.
- [ ] Tests: Desktop-Bounds, ActivationPoint, Sichtbarkeits- und Enable-Flags unter X11.
- [ ] `platynui-provider-atspi`: D-Bus-Integration, Baumaufbau (Application → Window → Control/Item), RuntimeId aus Objektpfad, Fokus-/Sichtbarkeitsflags.
- [ ] Ergänzende Tests (AT-SPI2) auf Basis des Windows-Testsets inkl. Namespaces `item`/`control`.
- [ ] Vorausplanen für Wayland: Vermittlungscrate `platynui-platform-linux` entwerfen, das zur Laufzeit anhand der Session-Umgebung (`$XDG_SESSION_TYPE`, heuristische Fallbacks) zwischen `platynui-platform-linux-x11` und `platynui-platform-linux-wayland` vermittelt, sobald letztere Implementierung verfügbar ist.

### 22. CLI `window` – Linux/X11-Integration
- [ ] CLI `window` nutzt X11-spezifische Funktionen (EWMH/NetWM) für Fensterlisten, Move/Resize etc.
- [ ] Tests: CLI `window` gegen Mock/X11-spezifische Szenarien (soweit automatisierbar).

- ### 23. Werkzeuge
- [ ] CLI (`crates/platynui-cli`): Erweiterungen für `watch`, `dump-node`, strukturierte Ausgabe (`--json`, `--yaml`), Skript-Integration; ergänzt die MVP-Kommandos (`query`/`highlight`).
- [ ] Inspector (GUI): Tree-Ansicht mit Namespaces, Property-Panel (Patterns), XPath-Editor, Element-Picker, Highlight; arbeitet wahlweise Embedded oder via JSON-RPC.
- [ ] Beispiel-Workflows dokumentieren (Readme/Docs): XPath → Highlight, Fokus setzen, Fensterstatus (`accepts_user_input`) ermitteln.

### 24. Qualitätssicherung & Prozesse
- [ ] CI-Pipeline: `cargo fmt --all`, `cargo clippy --all`, `cargo test --workspace`, `uv run ruff check .`, `uv run mypy src/PlatynUI packages/core/src` (sofern Python-Anteile relevant).
- [ ] Contract-Tests für Provider & Devices (pattern-spezifische Attribute, Desktop-Koordinaten, RuntimeId-Quellen).
- [ ] Dokumentation pflegen: Architekturkonzept, Patterns, Provider-Checkliste, Legacy-Analyse; Hinweis auf lebende Dokumente beibehalten.
- [ ] Release-/Versionierungsstrategie festlegen (SemVer pro Crate? Workspace-Version?).

### 25. Backlog & Explorations
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
  - `platynui-platform-macos`: Devices via Quartz/Event-Taps, Screenshot/Highlight mit CoreGraphics, Fenstersteuerung via AppKit.
- Optionales Erweiterungs-Interface für Provider (z. B. optionale Runtime-Services).
- Persistentes XPath-Caching & Snapshot-Layer.
- Optionaler Wayland-Support (Runtime-Erkennung Wayland/X11, Provider-Auswahl, Devices).
- Weitere Patterns (z. B. Tabellen-Navigation, Drag&Drop) nach Bedarf evaluieren.
- Erweiterte Eingabegeräte (Gamepad, Stift), Barrierefreiheits-Funktionen.
- Touch-Device-Unterstützung (Traits, CLI-Befehle) nach erfolgreichem Pointer/Keyboard-Ausbau.
- Community-Guides, Beispiel-Provider, Trainingsmaterial.
