# Linux X11 – Umsetzungsplan (AT‑SPI2 + Geräte)

English summary: This plan scopes and phases the Linux/X11 enablement for PlatynUI. We split work into platform devices (pointer, keyboard, screenshot, highlight, desktop info via XRandR) and the AT‑SPI2 UiTree provider. The first milestone targets a fully usable CLI path (query/snapshot/highlight/screenshot/input) on common X11 desktops; window actions follow via EWMH once Atspi → XID mapping is in place. CI builds compile on Linux; runtime/integration tests remain limited to mocks or opt‑in jobs.

Status: Draft (2025‑11‑05)
Update (2026‑02‑03): Phase‑1 Geräte (DesktopInfo/Pointer/Screenshot/Highlight) implementiert; Keyboard + `PlatformModule::initialize()` offen; AT‑SPI Provider weiterhin Stub; CLI `query`/`snapshot` warten auf Phase 2.
Update (2026‑02‑04): AT‑SPI Provider‑Grundgerüst umgesetzt (atspi‑connection, Rollen‑/Namespace‑Mapping inkl. `app`, Component‑gated Standard‑Attribute, Streaming‑Attribute, umfangreiche Native‑Interface‑Attribute). Events/WindowSurface/Tests bleiben offen.

Owner: Runtime/Providers Team

---

## Ziele

- Bereitstellen einer produktionsfähigen Linux/X11‑Schiene bestehend aus:
  - Plattform‑Geräten (`platynui-platform-linux-x11`): Pointer, Keyboard, Screenshot, Highlight, DesktopInfo
  - UiTree-Provider (`platynui-provider-atspi`): AT‑SPI2 Baum + Attribute, optional Patterns (Focusable, später WindowSurface)
- Nahtlose Einbindung über `platynui_link_os_providers!()` (bereits verdrahtet für `target_os = "linux"`).
- CLI‑Fähigkeiten: `list-providers`, `info`, `query`, `snapshot`, `highlight`, `screenshot`, `pointer`, `keyboard` auf X11 benutzen.
- Stabile Fallbacks und sinnvolle Defaults (Double‑Click‑Zeit/‑Größe, kein Wayland‑Hard‑Dependency).

Nicht‑Ziele (für den ersten Wurf)
- Wayland‑Backends (kann später ergänzt werden; Plan sieht Weg vor)
- Vollständige WindowSurface‑Aktionen (minimize/maximize/move/resize) – folgen nach EWMH/XID‑Brücke
- Vollständige AT‑SPI2‑Eventabdeckung – Start mit Basis‑Events, Ausbau später

## Architektur & Abhängigkeiten

- Plattform‑Crate `platynui-platform-linux-x11` (X11/XTest/XRandR):
  - Pointer: XTest (`FakeMotion/ButtonEvent`), QueryPointer
  - Keyboard: XTest (`FakeKeyEvent`), KeyName/Unicode‑Auflösung mit `xkbcommon-rs` (Safe‑Rust‑Port, keine System‑lib)
  - Screenshot: XGetImage (später optional XShm), RGBA/BGRA Konvertierung
  - Highlight: Override‑Redirect Overlay aus Segment‑Fenstern (solid rot, gestrichelte Kanten bei Clipping)
  - DesktopInfo: XRandR Monitore (ID/Name/Bounds/Primary)
  - Initialisierung: `PlatformModule::initialize()` (XInitThreads, Display öffnen), Fehler zu `PlatformError`

- Provider‑Crate `platynui-provider-atspi` (AT‑SPI2 über D‑Bus):
  - Abhängigkeiten: `atspi-connection`, `atspi-common`, `atspi-proxies` (zbus nur für Address‑Parsing)
  - Knotenmodell: `AtspiNode` mit lazy `children()` und Streaming‑`attributes()`
  - Attribute (Standard): `Role`, `Name`, `RuntimeId`, `Technology="AT-SPI2"`, optional `Id` (via `accessible_id`/`GetAttributes`‑Fallback)
  - Component‑gated Attribute: `Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsOffscreen`, `IsFocused`, `SupportedPatterns`
  - Namespace‑Mapping: `control`/`item` über AT‑SPI `Role`; `app:Application` wenn `Application`‑Interface vorhanden
  - Pattern: `Focusable` via `Component::grab_focus` + State‑Flags
  - Native‑Attribute: `Native/<Interface>.<Property>` (z. B. `Accessible.Name`, `Component.Extents`, `Text.CharacterCount`); `GetAttributes` → `Native/Accessible.Attributes` + `Native/Accessible.Attribute.<key>`
  - (Später) `WindowSurface`: via EWMH (_NET_ACTIVE_WINDOW, _NET_WM_STATE, _NET_MOVERESIZE_WINDOW) nachdem XID‑Mapping steht
  - Events: Basisweiterleitung (`NodeAdded/Removed/Updated`, `TreeInvalidated`) an Runtime‑Dispatcher

- Linking: Bereits durch `platynui_link_os_providers!()` abgedeckt (Linux → `platform-linux-x11` + `provider-atspi`).

## Phasen & Arbeitspakete

### Phase 0 – Vorbereitung
- [x] Crate‑Abhängigkeiten evaluieren/festlegen
  - `x11rb` für X11‑Protokoll (inkl. XKB/XTst/RandR)
  - `xkbcommon-rs` für Keymap/Keysym/Modifier‑Auflösung (reiner Rust‑Port von libxkbcommon)
  - `atspi-connection`/`atspi-proxies` (typisierte AT‑SPI2‑Proxies) und `zbus` (Address‑Parsing) für Bus‑Discovery/Calls
- [x] Feature‑Flags skizzieren (nur dokumentiert; werden mit Implementierungen eingeführt)
  - `xshm` (Screenshot Fast‑Path), `debug-log`, `events`
- [x] Sicherheits-/Sandboxhinweise dokumentieren (XTest benötigt Zugriff auf lokalen X‑Server)
- [x] PlatformModule Skeleton registriert (Initialize = No‑Op, keine Nebenwirkungen)
- [x] Plan in README verlinkt

### AT‑SPI Bus Discovery & Session Handling (atspi-connection)

- Ziel: Stabile, synchrone Verbindung zum Accessibility‑Bus (A11y‑Bus) herstellen.
- Discovery‑Reihenfolge:
  1) `AT_SPI_BUS_ADDRESS` prüfen und verwenden, falls gesetzt.
  2) Falls nicht gesetzt: `AccessibilityConnection::new()` nutzt intern `org.a11y.Bus/GetAddress`.
- Betrieb:
  - Blocking‑API in eigenem Thread für Event‑Streams; Weiterleitung an Runtime‑Dispatcher.
  - Reconnect‑Backoff bei Verbindungsabbruch.

### Phase 1 – Plattform Linux/X11 Minimal (Geräte)
- [ ] `PlatformModule::initialize()`
  - XInitThreads, Display öffnen, Atoms/Extensions (XTest/RandR) prüfen
- [x] `DesktopInfoProvider` (XRandR)
  - Monitore inklusive Primary und Bounds (ScaleFactor default 1.0)
- [x] `PointerDevice`
  - `position()` via `QueryPointer`
  - `move_to()` via XTest `FakeMotion`
  - `press/release()` via XTest Buttons (1,2,3,8,9), Scroll via Buttons (4/5 vertikal, 6/7 horizontal)
  - `double_click_time/size()` Defaults (z. B. 400 ms, 4×4 px) – später konfigurierbar
- [ ] `KeyboardDevice`
  - KeyName/Unicode → Keysym/KeyCode/Modifier via `xkbcommon-rs`, Injection via XTest
  - `known_key_names()` konsistent mit Windows‑Aliasen (mindestens: AlphaNum, F‑Tasten, Pfeile, Modifiers)
- [x] `ScreenshotProvider`
  - Root‑Window `XGetImage` (RGBA) für Region; später optional XShm
- [x] `HighlightProvider`
  - Override‑Redirect Segment‑Fenster (solid rot, Dash bei Clipping), `clear()` schließt Overlay; Timer im Provider‑Thread
- [ ] Unit‑Tests (so weit headless möglich):
  - Registrierung via `inventory` (keine echte Server‑Verbindung)
  - Konvertierungsfunktionen (Keynames/Buttons)

Akzeptanz: `platynui-cli info/pointer/screenshot/highlight` laufen auf X11‑Desktopen ohne Provider; `keyboard` folgt nach Phase 1, `query`/`snapshot` nach Phase 2.

### Phase 2 – AT‑SPI2 Provider (Baum & Attribute)
- [x] Grundgerüst
  - Bus‑Discovery via `atspi-connection` (`AT_SPI_BUS_ADDRESS` oder `org.a11y.Bus/GetAddress`)
  - Verbindung zum A11y‑Bus, Registry‑Root ermitteln
  - `AtspiNode` mit lazy `children()`/Streaming‑`attributes()`/`parent()`
- [x] Rollen‑Mapping
  - AT‑SPI `Role` → `control:`/`item:` Rollen (PascalCase); `Application`‑Interface → `app:Application`
- [x] Attribute
  - Standard: `Name`, `Role`, `RuntimeId`, `Technology`, optional `control:Id`
  - Component‑gated: `Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsOffscreen`, `IsFocused`, `SupportedPatterns`
  - Native‑Interface‑Attribute (`Accessible.*`, `Component.*`, `Text.*`, `Table.*`, …)
- [x] Pattern `Focusable`
  - Fokus setzen via `Component::grab_focus` (State‑Flags als Gate)
- [ ] Events
  - Subscribe: Struktur/Property‑Events → `ProviderEventDispatcher`
- [ ] Smoke‑Tests (so weit möglich ohne echten Desktop):
  - Offliner‑Fixtures/Adapter, Contract‑Checks (keine doppelten Patterns), Rollen‑Mapping

Akzeptanz: `platynui-cli list-providers` zeigt AT‑SPI; `query/snapshot` liefern realistische Knoten/Attribute; `focus` funktioniert auf fokussierbaren Elementen.

### Phase 3 – WindowSurface (EWMH) & Integration
- [x] XID‑Auflösung
  - AT‑SPI → X11 Window‑ID (XID) ermitteln via `_NET_CLIENT_LIST` + `_NET_WM_PID` Matching, mit Geometrie‑Fallback bei mehreren Fenstern pro PID
- [x] EWMH‑Aktionen (über X11)
  - `_NET_ACTIVE_WINDOW` (activate), `_NET_CLOSE_WINDOW` (close), `_NET_ACTIVE_WINDOW` Property‑Abfrage (is‑active)
- [x] `WindowSurface` Pattern an AtspiNode exponieren (für Frame/Window/Dialog Rollen)
  - `activate()`, `close()`, `accepts_user_input()` (= is‑active via EWMH)
  - Attribute: `IsTopmost` (via EWMH `_NET_ACTIVE_WINDOW`), `AcceptsUserInput` (via AT‑SPI State)
- [ ] EWMH‑Aktionen erweitern: `_NET_WM_STATE` (minimize/maximize), `_NET_MOVERESIZE_WINDOW` (move/resize)
- [ ] CLI `window` End‑to‑End: `--bring-to-front`/`--minimize`/`--maximize`/`--move`/`--resize`

Akzeptanz: Fensteraktionen funktionieren in gängigen X11 WMs (KDE, Xfce, Openbox); Logs/Fehler klar.

### Phase 4 – Stabilisierung & DX
- [ ] Keyboard‑Namensraum erweitern (Alias‑Kompatibilität, `list` Ausgabe stabil sortiert)
- [ ] Screenshot‑Performance: optional XShm‑Pfad + Tests
- [ ] Highlight‑UX: randlos, zentrierte Labels optional, Mehrfach‑Rects performant
- [ ] Provider‑Events filtern und `cli watch` ggf. erweitern (Event‑Typ/Limit/Filter)

## Tests & CI

- CI baut Linux‑Artefakte (bereits vorhanden). Headless‑Tests beschränken sich auf Unit‑/Konvertierungs‑Logik.
- Optionale Integrationstests nur in speziellen CI‑Jobs (Xvfb + a11y‑Bus), initial „manuell“ in lokalen Dev‑Umgebungen.
- Weiterhin: Mock‑Provider für deterministische Runtime/CLI‑Tests; Linux‑Spezifika separat halten.

### Ergebnis Phase 0
- Architektur/Abhängigkeiten festgelegt und dokumentiert (x11rb, xkbcommon‑rs, atspi-connection/atspi-proxies + zbus).
- Linux‑Plattformmodul als No‑Op registriert; Skeleton‑Module für Geräte angelegt (ohne Registrierung/Deps).
- Bus‑Discovery als Kern‑Pfad dokumentiert (ohne Feature‑Gate; Implementierung folgt in Phase 2).
- Keine neuen Abhängigkeiten hinzugefügt; bestehende Builds bleiben unverändert.

## Sicherheit & Sandbox (X11/AT‑SPI2)

Grundlagen
- X11 Authentifizierung: Zugriff auf den Display‑Server erfolgt über `DISPLAY` + MIT‑MAGIC‑COOKIE‑1 (typisch `~/.Xauthority`) oder XDG Laufzeit‑Socket (`/tmp/.X11-unix`). XTest erlaubt Eingabeinjektion für alle Clients dieser Session.
- AT‑SPI2: eigener Accessibility‑Bus (A11y‑Bus). Adresse über `org.a11y.Bus/GetAddress` oder `AT_SPI_BUS_ADDRESS`. Kommunikation bleibt lokal (Unix Domain Socket).

Empfehlungen (lokal)
- Automation im normalen Benutzerkontext derselben Desktop‑Session ausführen (keine `root`‑Session, kein `xhost +`).
- Keine Display‑ oder Bus‑Adressen in Logs ausgeben; Debug‑Logs hinter Flag halten.
- `xauth` verwenden (keine globalen `xhost`‑Freigaben). Cookies niemals commiten.

Container/CI
- Headless: Xvfb starten (z. B. `Xvfb :99 -screen 0 1920x1080x24`), `DISPLAY=:99` setzen.
- D‑Bus Session pro Job starten (`dbus-daemon --session --print-address --fork`) und `DBUS_SESSION_BUS_ADDRESS` exportieren.
- AT‑SPI2‑Registry im Headless‑Setup ggf. explizit starten (`at-spi2-registryd &`) oder durch first‑use triggern; `AT_SPI_BUS_ADDRESS` wird dann bereitgestellt bzw. via `org.a11y.Bus` ermittelt.
- Für Container: `/tmp/.X11-unix` und Xauthority Cookie selektiv einbinden; keine Privileg‑Erhöhungen; User‑Namespace nutzen.

Wayland‑Hinweis
- Wayland ist nicht Teil dieses Plans. Unter reinen Wayland‑Sitzungen ist XTest nicht verfügbar; Injection erfordert andere Wege (z. B. Portale/Compositor‑APIs). Für X11‑Kompatibilität XWayland sicherstellen.

Overlay/Highlight
- Aktuell: mehrere kleine `override-redirect` Fenster pro Rahmen‑Segment, ohne SHAPE/XFixes; dadurch keine Input‑Region‑Manipulation nötig, Mitte bleibt frei.

Fehler‑/Timeout‑Strategie
- Defensive Timeouts für Bus‑Discovery und X11‑Calls; klare Fehlermeldungen (z. B. „XTest/XRandR nicht verfügbar“, „A11y‑Bus nicht erreichbar“).

## Highlight (X11) – aktueller Stand

Ziel: Windows‑ähnlicher Rahmen (3 px, 1 px Gap) über einem/mehreren Rechtecken. Aktuell realisiert durch mehrere kleine override‑redirect Fenster (pro Rahmen‑Segment), solid rot; bei Clipping (außerhalb Desktop) werden betroffene Kanten gestrichelt.

Umsetzung (Stand 2026‑02):
- Kein ARGB/Compositing‑Pfad mehr; stattdessen pro Rahmen‑Segment ein kleines Fenster mit `background_pixel` (rot) und `override_redirect`, gestapelt `ABOVE`.
- Kein SHAPE/XFixes‑Einsatz, dadurch weniger Black‑Fill auf XWayland/Xephyr; Fenster werden nur auf die Linienbreite gemalt, Mitte bleibt frei.
- Clamping an Desktop‑Bounds, Clipping zeigt gestrichelte Kanten (8 px on / 4 px off) analog Windows.
- Thread mit mpsc‑Channel (`Show/Clear`), Duration‑Timer per Deadline im Loop; Fenster werden reused/unmapped statt zerstört.

Einschränkungen / TODO:
- Xwayland/Wayland: Anzeige kann ausbleiben; ggf. Diagnose/Logging oder ein Single‑Window‑Fallback ergänzen.
- Keine Halbtransparenz; Farbe aktuell fix rot.
- Kein ARGB‑Buffer‑Pfad (früherer Plan verworfen); falls Compositing‑Pfad gebraucht wird, müsste er neu entworfen werden.

## Risiken & Gegenmaßnahmen

- X11/XTest Sicherheit/Verfügbarkeit: Auf manchen Distros deaktiviert → klare Fehlermeldungen, Fallbacks, Doku.
- AT‑SPI2 Liveness/Performance: D‑Bus Roundtrips; konsequent lazy iterieren, Attribute on‑demand.
- EWMH‑Abdeckung: WM‑Unterschiede; robustes Fehler‑Handling und best‑effort Aktionen mit Rückfall.
- Wayland: Nicht im Scope; Plan für spätere Erkennung (Wayland/X11) und Fallback dokumentieren.

## Deliverables & Milestones

1) M1 – Geräte funktionsfähig (Phase 1): CLI input/screenshot/highlight/info laufen auf X11 (1–2 Wochen)
2) M2 – AT‑SPI2 Baum/Focus (Phase 2): Query/Snapshot/Focus (2–3 Wochen)
3) M3 – WindowSurface via EWMH (Phase 3): Min/Max/Move/Resize/Activate (2 Wochen)
4) M4 – Feinheiten & Performance (Phase 4): XShm, Events, DX (laufend)

## Umsetzungsnotizen (mapping/konventionen)

- `TechnologyId = "AT-SPI2"` für Provider; Plattformgeräte melden keine eigene Technologie.
- `RuntimeId` Format Vorschlag: `atspi://<bus>/<path>#<id>` (stabil innerhalb Session)
- Buttons: 1=Left, 2=Middle, 3=Right, 4/5=Scroll V, 6/7=Scroll H; Delta‑Mapping 120er‑Raster wie Windows
- Keyboard: Keynames harmonisieren (z. B. `RETURN/ENTER`, `ESCAPE/ESC`, `SUPER/WINDOWS`), Großschreibung konsistent; Mapping/State via `xkbcommon-rs`

---

### Nächste direkte Schritte
- [x] Phase‑0 PR: Abhängigkeitsliste, Feature‑Flags, Skeletons für Geräte/Provider, Doku‑Abschnitt in README/Plan
- [ ] Phase‑1 Implementierung + manuelle Verifikation auf X11 Desktop (Keyboard/Init/Tests offen)
- [ ] Phase‑2 Grundgerüst AT‑SPI2 + erste Queries (Smoke)
