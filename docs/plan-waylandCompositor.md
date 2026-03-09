## Plan: PlatynUI Wayland Compositor + Platform-Crate (Final, priorisiert)

**TL;DR:** Smithay-basierter Compositor (`apps/wayland-compositor/`, aktuell ~15.000 LoC, 1902 Tests, 43 Protokoll-Globals) + Wayland Platform-Crate (`crates/platform-linux-wayland/`) + PlatynUI GNOME Shell Extension. Die Implementierung folgt einer klaren Reihenfolge: erst smithay-fertige Core-Protokolle verdrahten (lauffähiger Compositor in Phase 1 ✅), dann SSD + XWayland + DRM + Test-Control (Phase 2 ✅), dann Automation-Protokolle für PlatynUI (Phase 3: Layer-Shell, Foreign-Toplevel, Virtual-Input, Screencopy — Kern abgeschlossen ✅), dann Härtung & Code-Qualität (Phase 3a ✅), dann Bugfixes & Window-Management-Verbesserungen (Phase 3a+ ✅), dann verbleibende Automation-Protokolle (Phase 3b ✅: Tier 1+2+3 + Stubs komplett, EIS-Server komplett mit Keyboard/Pointer/Touch/Regions/XKB-Keymap, EIS-Test-Client komplett mit `type-text` + Compose-Support + XKB-Reverse-Lookup, Erkenntnisse in `docs/eis-libei.md`), dann Desktop-Integration & Projekt-Tooling (Phase 3c ✅: Winit-Fenster, App-IDs, `.desktop`-Dateien, Justfile), dann Touch-Input & SSD-Touch-Interaktion (Phase 3d ✅: Touch-Handler, Touch-Grabs, deferred SSD-Buttons, Multi-Slot-Isolation), dann Platform-Linux-Mediator (Phase 3e ✅: `crates/platform-linux/` als delegierender Mediator mit Laufzeit-Session-Erkennung, einmaliger Backend-Auflösung in `initialize()` via `Resolved`-Struct, Wayland fällt vorerst auf X11 zurück, Sub-Platforms als Libraries ohne Selbstregistrierung), dann das Wayland-Platform-Crate (Phase 4: Compositor-Typ-basierte Backend-Selektion, Compositor-spezifische IPC-Backends für KWin/Mutter/PlatynUI (Sway/Hyprland IPC optional), PlatynUI GNOME Shell Extension für Mutter, WindowManager liefert Fenster-Positionen an AT-SPI-Provider), dann eingebauter VNC/RDP-Server für Headless-Debugging (Phase 5). Panel, Portal/PipeWire und Doku kommen danach bei Bedarf. Jede Phase endet mit einem testbaren Meilenstein.

---

**Schritte**

### Phase 1: Lauffähiger Minimal-Compositor (~1.500 LoC, ~1 Woche) ✅ ERLEDIGT

*Ziel: Fenster öffnen sich, Tastatur/Maus funktioniert, Clipboard geht — eine GTK/Qt-App kann darin laufen.*

> **Status (2026-02-27):** Komplett implementiert. 24 Protokolle, ~2.400 LoC.
> Getestet mit Kate (Qt6), gtk4-demo, gnome-text-editor.
> Build/Clippy/Fmt alle sauber.

1. **Crate `apps/wayland-compositor/` anlegen** — `Cargo.toml` mit smithay (Features: `wayland_frontend`, `desktop`, `renderer_glow`, `backend_winit`, `backend_drm`, `backend_gbm`, `backend_libinput`, `backend_session_libseat`, `backend_udev`, `xwayland`), `calloop`, `tracing` + `tracing-subscriber`, `clap`. Alle Backends und XWayland werden bedingungslos kompiliert (keine optionalen Cargo-Features). Dependency: `egui` für Compositor-UI (Titlebars, Panel, Launcher).

2. **Event-Loop + State** (`src/main.rs`, `src/state.rs`): Smithay `Display` + calloop `EventLoop`. `CompositorState`-Struct hält alle smithay-`*State`-Objekte. CLI via `clap`: `--backend <headless|winit|drm>`, `--width`, `--height`, `--socket-name`.

3. **Core-Protokolle verdrahten** (alles `delegate_*!()` + Handler):
   - `src/handlers/compositor.rs` — `wl_compositor` + `wl_subcompositor` (~30 LoC)
   - `src/handlers/shm.rs` — `wl_shm` Buffer (~20 LoC)
   - `src/handlers/dmabuf.rs` — `linux-dmabuf-v1` — GPU-Buffer-Sharing für Chromium, Firefox, Electron, Vulkan-Apps. Smithay hat `delegate_dmabuf!()`. Format-Negotiation + Import. (~40 LoC)
   - `src/handlers/output.rs` — `wl_output` + `xdg-output-manager-v1` — Logische Output-Infos (Name, Position, logische Größe). GTK4/Qt6 fragen das ab. (~50 LoC)
   - `src/handlers/seat.rs` — `wl_seat` mit Single-Seat (~80 LoC)
   - `src/handlers/xdg_shell.rs` — `xdg_shell` Toplevels + Popups (~300 LoC)
   - `src/handlers/decoration.rs` — `xdg_decoration` → CSD anfordern (~30 LoC)

3b. **App-Kompatibilitäts-Protokolle** (smithay stellt Building Blocks bereit, jeweils `delegate_*!()` + minimaler Handler):
   - `src/handlers/viewporter.rs` — `wp-viewporter` — Surface-Skalierung, von GTK4/Qt6/Chromium genutzt (~20 LoC)
   - `src/handlers/fractional_scale.rs` — `wp-fractional-scale-v1` — HiDPI-Rendering (~20 LoC)
   - `src/handlers/xdg_activation.rs` — `xdg-activation-v1` — Focus-Stealing-Prevention, `gtk_window_present()` (~40 LoC)
   - `src/handlers/pointer_constraints.rs` — `pointer-constraints-v1` + `relative-pointer-v1` — Pointer-Lock/Confine für Drag-Operationen (~50 LoC)
   - `src/handlers/single_pixel_buffer.rs` — `wp-single-pixel-buffer-v1` — effiziente einfarbige Surfaces (~15 LoC)
   - `src/handlers/presentation_time.rs` — `wp-presentation-time` — Frame-Timing für Video/Animation (~30 LoC)
   - `src/handlers/keyboard_shortcuts_inhibit.rs` — `keyboard-shortcuts-inhibit-v1` — VNC/RDP-Clients brauchen alle Keys statt Compositor-Shortcuts (~30 LoC)
   - `src/handlers/text_input.rs` — `text-input-v3` + `input-method-v2` — IME-Support für CJK/Compose/Emoji (~80 LoC)
   - `src/handlers/idle_notify.rs` — `ext-idle-notify-v1` — Idle-Detection für Screensaver/Power-Management (~30 LoC)
   - `src/handlers/session_lock.rs` — `ext-session-lock-v1` — Screen-Locking (swaylock etc.) (~50 LoC)
   - `src/handlers/xdg_foreign.rs` — `xdg-foreign-v2` — Cross-App Parent/Child Window-Beziehungen (z.B. Datei-Dialog einer App als Child einer anderen, Portal-Dialoge) (~40 LoC)
   - `src/handlers/security_context.rs` — `wp-security-context-v1` — Flatpak/Sandbox-Apps: eingeschränkter Protokoll-Zugang für sandboxed Clients (~40 LoC)
   - `src/handlers/cursor_shape.rs` — `wp-cursor-shape-v1` — Server-seitiges Cursor-Shape-Handling, plus Cursor-Theme-Loading aus `$XCURSOR_THEME`/`$XCURSOR_SIZE` via `wayland-cursor`. Ohne: Apps zeigen keinen oder falschen Cursor. (~50 LoC)

4. **Fenster-Management** (`src/workspace.rs`): Einfache Stacking-Policy mit `desktop::Space`. Neue Fenster kaskadiert platzieren. Fokus via Klick. Mapping von Surface → Position/Größe/Titel/App-ID/PID. (~200 LoC)

5. **Input-Verdrahtung** (`src/input.rs`): Keyboard (`KeyboardHandle` + XKB-Keymap + Repeat) und Pointer (`PointerHandle` + Hit-Testing via `Space::element_under()`). Cursor-Rendering. (~250 LoC)

6. **Clipboard + Selection** (`src/handlers/selection.rs`): `delegate_data_device!()` + `delegate_primary_selection!()` + `SelectionHandler` (~80 LoC)

7. **Popup-Management**: `PopupManager` an Render-Loop anbinden (~80 LoC)

8. **Rendering** (`src/render.rs`): `GlowRenderer` für alle Backends (Winit, DRM via EGL-on-GBM, Headless via EGL auf DRI-Render-Node). `Space::render_output()` mit Cursor-Element. Software-Fallback via `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe). (~200 LoC)

9. **Backend-Abstraktion** (`src/backend/mod.rs`, `headless.rs`, `winit.rs`): Headless = off-screen `GlowRenderer` (EGL auf DRI-Render-Node). Winit = Fenster in X11/Wayland. DRM = EGL-on-GBM. Alle Backends nutzen einheitlich `GlowRenderer`. (~200 LoC)

9b. **Signal-Handling** (`src/signals.rs`): SIGTERM/SIGINT/SIGHUP via calloop Signal-Source. Graceful Shutdown: Clients benachrichtigen (`wl_display.destroy_clients()`), Sockets aufräumen, EIS/Portal/PipeWire/VNC/RDP stoppen. Watchdog-Timer: `--timeout <secs>` CLI-Flag lässt Compositor nach Zeitlimit automatisch beenden (nützlich für CI, verhindert Endlos-Hänger). (~80 LoC)

9c. **Readiness-Notification** (`src/ready.rs`): CI-Scripts müssen wissen, wann der Compositor bereit ist (Wayland-Socket erstellt, Outputs initialisiert, alle Protokolle registriert). Mechanismen:
- `--ready-fd <N>` — schreibt `READY\n` auf den angegebenen File-Descriptor (systemd-notify-Stil)
- `--print-env` — gibt alle nötigen Environment-Variablen auf stdout aus (`WAYLAND_DISPLAY=wayland-1`, `DISPLAY=:1` für XWayland, etc.)
- Ohne Flag: `READY\n` auf stderr wenn bereit
Verhindert Race-Conditions in CI-Scripts die den Compositor starten und sofort Tests ausführen. (~60 LoC)

9d. **Environment-Setup + Socket-Cleanup** (`src/environment.rs`): Beim Start:
- `$XDG_RUNTIME_DIR` prüfen/erstellen
- Verwaiste Wayland-Sockets in `$XDG_RUNTIME_DIR` aufräumen (altes `wayland-0` etc.)
- Socket-Name auto-wählen falls `--socket-name` nicht gesetzt (nächster freier `wayland-N`)
- `WAYLAND_DISPLAY` setzen für Child-Prozesse
- `DISPLAY` setzen wenn XWayland aktiviert
- Beim Graceful Shutdown: Socket-Datei löschen (~50 LoC)

**Meilenstein 1:** ✅ `cargo run -p platynui-wayland-compositor -- --backend winit` → Fenster öffnet sich → `WAYLAND_DISPLAY=... gtk4-demo` startet und ist bedienbar (Klick, Tippen, Clipboard, HiDPI, Popups mit Pointer-Constraints).

---

### Phase 2: SSD + XWayland + DRM + Test-Control (~1.400 LoC, ~1.5 Wochen) ✅ ERLEDIGT

*Ziel: Fenster haben Titelleisten mit Schließen/Maximieren/Minimieren, X11-Apps laufen via XWayland, DRM-Backend für echte Hardware, Test-Control-IPC für CI, Multi-Monitor-Support.*

> **Status (2026-03-01):** Komplett implementiert. Alle 7 Steps (10–13e) fertig, ~6.700 LoC gesamt.
> SSD mit egui-Titlebars (Close/Maximize/Minimize mit Hover-Highlighting, GPU-resident auf GlowRenderer),
> XWayland mit Clipboard-Forwarding (Data-Device + Primary-Selection),
> DRM-Backend mit voller Rendering-Pipeline (Connector-Enumeration, Mode-Setting, GBM, Scanout,
> VT-Switching, VBlank-Handling mit `frame_submitted()`), Test-Control-IPC (Unix-Socket + JSON)
> inkl. Screenshot (GlowRenderer → PNG → Base64), Multi-Monitor, Client Permissions (Enforcement in Protocol-Handlern),
> Keyboard-Layout-Config (CLI > Config > Env > Default).
> SHM-Formate: Argb8888, Xrgb8888, Abgr8888, Xbgr8888, Abgr16161616f, Xbgr16161616f.
> DMA-BUF-Formate: Argb8888, Xrgb8888, Abgr8888, Xbgr8888 (Linear-Modifier).
> Build/Clippy/Fmt/1853 Tests sauber (alle Backends + XWayland bedingungslos kompiliert).
>
> **Hinweis Minimize:** Minimierte Fenster werden via `space.unmap_elem()` aus dem
> Space entfernt und in `state.minimized_windows` gespeichert. Restore erfolgt über
> externe Taskbar (ironbar via `wlr-foreign-toplevel-management activate`-Request)
> oder SSD-Kontextmenü. Focus-Handling ist in `minimize_window()` integriert (Fokus
> wird auf das nächste sichtbare Fenster verschoben). Der frühere Workaround „Klick auf
> leere Desktop-Fläche = Restore" wurde entfernt — Minimize/Restore läuft ausschließlich
> über Protokoll-Requests (Taskbar) oder SSD-Buttons.
>
> **Hinweis Maximize:** Maximize speichert die Fenster-Position **und -Größe** in `state.pre_maximize_positions`
> (Typ `PreMaximizeState = (Window, Point, Option<Size>)`) vor dem Maximieren. Unmaximize
> (erneuter Klick auf Maximize-Button) stellt Position und Größe wieder her. Für X11/XWayland
> sind `XwmHandler::maximize_request()` und `unmaximize_request()` implementiert, sodass
> X11-Apps über `_NET_WM_STATE` korrekt maximieren/wiederherstellen.
>
> **Hinweis Screenshot Multi-Output:** `take_screenshot()` in `control.rs` übergibt den Primary
> Output an `collect_render_elements()`. Da `collect_render_elements` alle Fenster im Space
> iteriert (unabhängig vom Output) und der kombinierte Buffer korrekt dimensioniert wird,
> funktioniert Multi-Output-Screenshot bereits korrekt. Für echtes per-Output-Rendering
> (z.B. unterschiedliche Scales pro Output) wäre eine Anpassung in späteren Phasen nötig.

10. **Server-Side Decorations** (`src/decorations.rs`, `src/render.rs`, `src/ui.rs`): Compositor-seitige Fensterdekorationen für Apps die SSD anfordern (z.B. Kate/Qt-Apps). Titelleiste mit Fenster-Titel, Schließen/Maximieren/Minimieren-Buttons mit Hover-Highlighting. Rendering via egui (GPU-resident `TextureRenderElement<GlesTexture>` auf `GlowRenderer`, einheitlich für alle Backends). Borders als `SolidColorRenderElement`. Maus-Interaktion: Klick auf Close → `toplevel.send_close()`, Klick auf Maximize → Toggle-Maximize, Klick auf Minimize → Minimize-State, Drag auf Titelleiste → Window-Move. Hit-Testing über unified `Focus`-Enum (cosmic-comp-inspiriert) mit `pointer_hit_test()` für Front-to-Back Z-Order. (~460 LoC decorations.rs, ~280 LoC ui.rs, ~200 LoC render.rs)

11. **XWayland** (`src/xwayland.rs`): Smithay's XWayland-Integration. `XwmHandler` für X11-Window-Mapping, ICCCM/EWMH-Basics. X11-Fenster in Toplevel-Tracking integrieren. (~400 LoC)

12. **DRM-Backend** (`src/backend/drm.rs`): `backend_drm` + `backend_libinput` + `LibSeatSession`. Volle Rendering-Pipeline: Connector-Enumeration, Mode-Setting, GBM-Allocator, Scanout via `DrmCompositor` pro Output. VT-Switching (Session-Pause/Resume: `session_active`-Flag wird in calloop Session-Handler gesetzt, DRM-Rendering nur bei aktiver Session). VBlank-Handling: `frame_submitted()` + `pending_frame`-Reset im DRM-Event-Handler. DRM-State im `State`-Struct (nicht lokal in `run()`), damit calloop-Handler darauf zugreifen können. Nur bei `--backend drm`. (~400 LoC)

13. **Test-Control-IPC** (`src/control.rs`): Unix-Socket + JSON — Fenster-Liste abfragen/setzen, Input-Verifikation, Compositor pausieren, direkter Screenshot (Off-Screen-`GlowRenderer` mit shared EGL-Context rendert Frame in Abgr8888 → PNG-Encoding → Base64-Response). Control-Socket ist standardmäßig aktiviert (`--no-control-socket` zum Deaktivieren), Pfad wird in `PLATYNUI_CONTROL_SOCKET` exportiert. (~590 LoC)

13b. **Multi-Monitor** (`src/multi_output.rs`): Unterstützung für mehrere virtuelle Outputs. CLI-Flag `--outputs <N>` erstellt N Monitore mit konfigurierbarer Auflösung und Anordnung (`--output-layout <horizontal|vertical|custom>`). Jeder Output ist ein eigener `wl_output` mit eigenem Mode/Scale. Headless: Alle Outputs off-screen. Winit: Ein großes Fenster mit allen Outputs nebeneinander (inkl. visueller Trennlinie). DRM: Echte physische Outputs. Wichtig für Multi-Monitor-Testszenarien. Individuelle Output-Geometrie (`--output-config`) wird in Phase 2b (Step 13j) nachgerüstet. (~200 LoC)

13d. **Client-Permissions** (`src/security.rs`): Konfigurierbare Berechtigungen für privilegierte Protokolle. Welche Clients dürfen `zwlr_virtual_pointer`, `wlr-foreign-toplevel`, `ext-image-copy-capture`, Layer-Shell nutzen? Default: alle erlaubt (Wayland-Compositor). CLI-Flag `--restrict-protocols` aktiviert Whitelist-basierte Filterung (App-ID). Enforcement in `SecurityContextHandler`, `SessionLockHandler` und `InputMethodHandler`: unbekannte App-IDs werden rejected wenn Whitelist aktiv. Relevant für Flatpak/Sandbox-Tests. (~100 LoC)

13e. **Keyboard-Layout-Konfiguration** (`src/state.rs`, `src/lib.rs`): Tastaturlayout konfigurierbar statt hartcodiert US-English (`XkbConfig::default()`). Einlesen der Standard-Linux-Umgebungsvariablen: `XKB_DEFAULT_LAYOUT`, `XKB_DEFAULT_VARIANT`, `XKB_DEFAULT_MODEL`, `XKB_DEFAULT_RULES`, `XKB_DEFAULT_OPTIONS`. CLI-Flags überschreiben die Umgebungsvariablen (Priorität: CLI-Flag > Umgebungsvariable > XKB-Default).
  - **Per-Layout (kommagetrennte Listen, positionell zugeordnet):**
    - `--keyboard-layout` — Layout-Liste (z.B. `de,us,de`)
    - `--keyboard-variant` — Variant-Liste, positionell zu Layouts (z.B. `nodeadkeys,,neo`). Leere Einträge = Default-Variante.
    - Beispiel: `--keyboard-layout de,us,de --keyboard-variant nodeadkeys,,neo` → `de(nodeadkeys)`, `us`, `de(neo)`
  - **Global (einzelne Werte, gelten für alle Layouts):**
    - `--keyboard-model` — physisches Keyboard-Modell (z.B. `pc105`, Default: automatisch)
    - `--keyboard-rules` — XKB Rules-Datei (z.B. `evdev`, Default: System-Default)
    - `--keyboard-options` — kommagetrennt, globale XKB-Optionen (z.B. `grp:alt_shift_toggle,compose:ralt`). Enthält u.a. Layout-Wechsel per Tastenkombination (`grp:alt_shift_toggle` = Alt+Shift, `grp:win_space_toggle` = Super+Space) und Compose-Key/Caps-Remapping.
  - Beim Start wird das erste Layout der Liste aktiv. Baut `XkbConfig { rules, model, layout, variant, options }` zusammen und übergibt es an `seat.add_keyboard()`. (~30 LoC)

**Meilenstein 2:** Fenster haben Titelleisten mit funktionierenden Buttons (Schließen, Maximieren, Minimieren). XWayland-Apps laufen. DRM-Modus auf TTY funktioniert (Connector-Enumeration, Mode-Setting, GBM-Scanout, VT-Switching). Test-IPC ermöglicht Screenshot via `GlowRenderer` und Fenster-Kontrolle. Security-Policy wird in Protocol-Handlern enforced. Multi-Monitor mit 2+ Outputs funktioniert. Tastaturlayout wird korrekt aus Umgebungsvariablen/CLI übernommen.

---

### Phase 2b: Härtung & Verfeinerung (~650 LoC, ~3–4 Tage) ✅ ERLEDIGT

*Ziel: Offene TODOs und bekannte Einschränkungen aus Phase 2 beheben, bevor neue Features hinzukommen.*

> **Status (2026-03-01):** Steps 13f–13i komplett implementiert. Verbleibend: 13j–13o.
> **Status (2026-07-06):** Step 13p (Fullscreen-Support) implementiert.
> **Status (2026-03-01 update):** Steps 13p–13s komplett implementiert (Fullscreen, Maximize, Unmaximize-on-Drag, Kontextmenü). Verbleibend: 13j–13o.
> **Status (2026-07-17):** Steps 13j–13o komplett implementiert. Phase 2b ist abgeschlossen. 1871 Tests grün.
> **Status (2026-03-01 update 2):** Steps 13t (egui Test-App) und 13o-Erweiterung (17 IPC-Tests inkl. Client-Window-Tests) implementiert.
> egui-Titlebars nutzen GPU-residenten `TextureRenderBuffer` (kein Pixel-Readback),
> inspiriert von smithay-egui. Einheitlicher Render-Pfad: `GlowRenderer` für alle Backends
> (Winit, DRM via EGL-on-GBM, Screenshots via Off-Screen EGL auf DRI-Render-Node).
> `PixmanRenderer` komplett entfernt — Software-Rendering via `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe).

13f. ✅ **Crate umbenennen** (`apps/wayland-compositor/`, `Cargo.toml`): Verzeichnis `apps/test-compositor/` → `apps/wayland-compositor/` umbenannt, Crate-Name `platynui-test-compositor` → `platynui-wayland-compositor`, Binary-Name `platynui-test-compositor` → `platynui-wayland-compositor`. Alle Referenzen im Workspace angepasst (Dokumentation, README, Quellcode).

13g. ✅ **Child-Programm starten** (`src/lib.rs`, `src/child.rs`): Wie bei Weston, Sway und anderen Compositors: Trailing-Argumente nach `--` werden als Programm mit Argumenten interpretiert und nach Compositor-Readiness als Child-Prozess gestartet. Umgebung: `WAYLAND_DISPLAY`, `DISPLAY` (bei XWayland), `XDG_RUNTIME_DIR` werden automatisch gesetzt. Bei Prozess-Ende: optional Compositor beenden (`--exit-with-child` Flag). Beispiele:
  - `platynui-wayland-compositor --backend winit -- gtk4-demo`
  - `platynui-wayland-compositor --backend headless --exit-with-child -- python -m pytest tests/`
  - `platynui-wayland-compositor -- bash` (interaktive Shell in der Session)
Essenziell für CI-Pipelines: Compositor startet → App startet → Tests laufen → Compositor beendet sich. (~60 LoC)

13h. ✅ **Konfigurationsdatei** (`src/config.rs`, `Cargo.toml`): TOML-basierte Config-Datei für persistente Einstellungen. Pfad-Discovery: `--config <path>` CLI-Flag > `$XDG_CONFIG_HOME/platynui/compositor.toml` > eingebaute Defaults. CLI-Flags überschreiben Config-Werte (wie bei Git). Deps: `toml` + `serde` (Deserialize). Sections:
  ```toml
  [font]
  family = "Noto Sans"       # Fallback: egui built-in
  size = 13.0

  [theme]
  titlebar-background = "#3c3c3c"
  titlebar-text = "#ffffff"
  button-close = "#e06c75"
  button-maximize = "#98c379"
  button-minimize = "#e5c07b"
  active-border = "#61afef"
  inactive-border = "#5c6370"

  [keyboard]
  model = "pc105"
  options = "grp:alt_shift_toggle,compose:ralt"

  [[keyboard.layout]]
  name = "de"
  variant = "nodeadkeys"

  [[keyboard.layout]]
  name = "us"

  [[output]]
  width = 1920
  height = 1080
  x = 0
  y = 0
  scale = 1.0

  [[output]]
  width = 2560
  height = 1440
  x = 1920
  y = 0
  scale = 1.5
  ```
  Default-Font: Noto Sans (breiteste Unicode-Abdeckung, auf allen modernen Linux-Distros vorinstalliert, OFL-Lizenz). Fallback auf egui's eingebauten Font wenn Noto Sans nicht gefunden wird. Beim Start: Config laden, mit CLI-Overrides mergen, als `CompositorConfig`-Struct im State verfügbar. egui-Integration (Step 13i), Output-Geometrie (Step 13j) und Keyboard-Config (Step 13e) lesen aus dieser Struct. (~80 LoC)

13i. ✅ **egui-Integration für Compositor-UI** (`src/ui.rs`, `src/decorations.rs`, `src/render.rs`, `src/backend/winit.rs`): `egui` 0.33 + `egui_glow` 0.33 als UI-Framework für Compositor-Titlebars. Einheitlicher `GlowRenderer`-Pfad für alle Backends:
  - **GPU-residentes Rendering:** Inspiriert von [smithay-egui](https://github.com/Smithay/smithay-egui). `TitlebarRenderer` initialisiert lazy beim ersten Frame einen `egui_glow::Painter`. egui tesselliert die Titelleiste → `paint_and_update_textures()` rendert direkt in eine GPU-residente `TextureRenderBuffer` (via Smithays `Offscreen` + `Bind` + `Frame` API) → `TextureRenderElement<GlesTexture>` compositet das Ergebnis. **Kein Pixel-Readback** — die Textur bleibt durchgehend auf der GPU. Funktioniert einheitlich auf Winit, DRM (EGL-on-GBM) und Headless (EGL auf DRI-Render-Node).
  - Borders sind `SolidColorRenderElement`.
  - `render.rs` definiert ein einziges `render_elements!`-Makro: `CompositorRenderElement` direkt an `GlowRenderer` gebunden (kein generischer Pfad, kein `MemoryRenderBuffer`).
  - Theme-Farben und Font-Family/Size aus `CompositorConfig` (Step 13h). Button-Hover-Highlighting bei Maus-Interaktion.
  - Smithay-Features: `renderer_glow` + `backend_egl` (kein `renderer_pixman`). (~280 LoC ui.rs, ~320 LoC decorations.rs, ~200 LoC render.rs)

13j. ✅ **Individuelle Output-Geometrie** (`src/state.rs`, `src/multi_output.rs`): `[[output]]`-Sections in der Config-Datei (Step 13h) werden in `State::new()` ausgelesen und an die bestehende `OutputConfig`-Struct übergeben. Config-Einträge haben Vorrang vor `--outputs`/`--width`/`--height` CLI-Flags. Felder: `width`, `height`, `x`, `y`, `scale`. (~30 LoC)

13k. ✅ **Client-Cursor-Surface Compositing** (`src/render.rs`, `src/backend/winit.rs`): Wenn ein Client einen eigenen Cursor via `wl_pointer.set_cursor` (Surface statt Named) setzt, wird die Cursor-Surface als zusätzliches Render-Element (via `render_elements_from_surface_tree` mit `Kind::Cursor`) in den Frame composited und der Host-Cursor versteckt. Cursor-Elements werden an Index 0 eingefügt (über allen anderen Elementen). (~60 LoC)

13l. ✅ **Screenshot per-Output-Scale** (`src/control.rs`): `take_screenshot()` berechnet den maximalen Scale über alle Outputs (`f64::max` fold) und übergibt ihn an `take_screenshot_impl()`. Buffer-Dimensionen werden auf physische Pixel skaliert (`(logical * scale).ceil()`), `OutputDamageTracker` wird mit dem tatsächlichen Scale initialisiert. Screenshot-Response enthält `scale`-Feld und physische Pixel-Dimensionen. (~30 LoC)

13m. ✅ **Compositor-Control CLI** (`apps/wayland-compositor-ctl/`): Eigenes Crate `platynui-wayland-compositor-ctl` — CLI-Tool analog zu `swaymsg`/`hyprctl`. Verbindet sich per Unix-Socket mit dem laufenden Compositor und sendet JSON-Kommandos. Subcommands:
  - `platynui-wayland-compositor-ctl list-windows` — JSON-Array aller Toplevels
  - `platynui-wayland-compositor-ctl screenshot [-o file.png]` — Screenshot als PNG (stdout oder Datei)
  - `platynui-wayland-compositor-ctl focus <id>` — Fenster fokussieren
  - `platynui-wayland-compositor-ctl close <id>` — Fenster schließen
  - `platynui-wayland-compositor-ctl ping` — Health-Check
  - `platynui-wayland-compositor-ctl shutdown` — Graceful Shutdown
  Socket-Pfad-Discovery: `--socket <path>` explizit, oder automatisch aus `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control`. Deps: `clap 4` (derive), `serde_json 1`. Eigener Base64-Decoder (RFC 4648) für Screenshot-Dateien. (~250 LoC)

13n. ✅ **IPC-Protokoll-Dokumentation** (`apps/wayland-compositor/docs/ipc-protocol.md`): Formale Spezifikation des Test-Control-IPC-Protokolls: Transport (Unix-Domain-Socket, newline-delimited JSON), Socket-Pfad-Convention (`$XDG_RUNTIME_DIR/<socket-name>.control`), alle 7 Kommandos (ping, shutdown, list_windows, get_window, close_window, focus_window, screenshot) mit Request/Response-Beispielen und Feld-Beschreibungen, Error-Handling-Tabelle, CLI-Tool-Nutzungsbeispiele.

13o. ✅ **IPC-Integration-Tests** (`apps/wayland-compositor/src/control.rs`, `apps/wayland-compositor/tests/ipc_tests.rs`): Zwei Ebenen: (1) Unit-Tests in `control.rs` für JSON-Parsing (`extract_json_string`, `extract_json_u64`) und Base64-Encoding (11 Tests). (2) Integration-Tests in `ipc_tests.rs` — 17 Tests in drei Kategorien:
  - **Basis-Tests (ohne Client-Fenster):** `ping` (Version + Backend), `status` (Uptime, Windows, Outputs), `list_windows` (leere Liste + minimized), `get_window` (not found), `screenshot` (Protokoll-Flow), `shutdown` (clean exit), `unknown command`, `invalid JSON`, `missing command field`.
  - **Error-Path-Tests (ohne Client-Fenster):** `close_window` (not found), `focus_window` (not found).
  - **Client-Window-Tests (mit egui Test-App, Step 13t):** `list_windows_with_client` (app_id + Titel verifizieren), `get_window_by_app_id`, `get_window_by_title` (case-insensitive Substring-Match), `focus_window_by_app_id`, `close_window_by_app_id` (Close + Verify gone), `screenshot_with_client` (Screenshot mit sichtbarem Client).
  Backend-Auswahl via `PLATYNUI_TEST_BACKEND` Umgebungsvariable (`headless` default, `winit` für sichtbare Ausführung). Compositor-Binary wird via `env!("CARGO_BIN_EXE_platynui-wayland-compositor")` referenziert (Cargo setzt den Pfad automatisch). Test-App-Binary wird aus demselben Target-Verzeichnis abgeleitet. Stdout/Stderr nur im Headless-Modus unterdrückt. Tests werden graceful übersprungen wenn kein GPU/EGL verfügbar oder die Test-App nicht startet. (~583 LoC)

13t. ✅ **egui Test-App** (`apps/test-app-egui/`): Eigenes Crate `platynui-test-app-egui` — Wayland-Client mit breiter Widget-Palette für IPC-Tests (Step 13o) und zukünftige PlatynUI-Funktionstests (AT-SPI/`AccessKit`-Accessibility). eframe 0.33 (native Wayland-Support via winit). CLI: `--app-id` (Default `com.platynui.test.egui`), `--title`, `--auto-close <secs>` (für CI-Timeout), `--log-level`. Widgets: Buttons (Click Me, Reset, Conditional), TextInput (Singleline + Multiline), Checkboxes, Toggle-Switch, Radio-Buttons, Slider, `DragValue`/Spinner, `ComboBox`, ProgressBar, `CollapsingHeader`, Tooltip, Hyperlink. Menüleiste (File/Edit/Help) und Statusleiste. AccessKit liefert automatisch einen AT-SPI-Accessibility-Tree — ermöglicht zukünftig `platynui-cli query` gegen die Test-App. Der Name enthält `egui`, da perspektivisch Test-Apps mit verschiedenen GUI-Frameworks (Qt, GTK) hinzukommen können. Deps: `eframe` 0.33, `clap` 4, `tracing`, `tracing-subscriber`. (~400 LoC)

13p. ✅ **Fullscreen-Support** (`src/handlers/xdg_shell.rs`, `src/xwayland.rs`, `src/decorations.rs`): `fullscreen_request()` und `unfullscreen_request()` im `XdgShellHandler` sowie `XwmHandler` implementiert. Fenster wird auf volle Output-Größe gesetzt (kein Titlebar-Offset), an Output-Origin positioniert, SSD automatisch unterdrückt (`window_has_ssd()` gibt `false` für Fullscreen-Fenster zurück). Pre-Fullscreen-State (Position + Size) wird gespeichert und beim Verlassen des Fullscreen-Modus wiederhergestellt. Cleanup in `toplevel_destroyed`, `unmapped_window` und `destroyed_window`. Wayland (`xdg_toplevel.set_fullscreen`) und X11 (`_NET_WM_STATE_FULLSCREEN`) werden einheitlich unterstützt. (~120 LoC)

13q. ✅ **Maximize via Protokoll + Doppelklick** (`src/handlers/xdg_shell.rs`, `src/input.rs`, `src/state.rs`): `maximize_request()` und `unmaximize_request()` im `XdgShellHandler` implementiert — GTK/GNOME-Apps mit CSD können jetzt per Doppelklick auf ihre eigene Titelleiste maximieren/wiederherstellen. CSD-Fenster (ohne SSD) werden an Output-Origin positioniert, SSD-Fenster unterhalb der Titelleiste. Zusätzlich: Doppelklick-Erkennung (< 400ms) auf SSD-Titlebars togglet Maximize — gleicher Maximize-Code wird wiederverwendet. `last_titlebar_click: Option<Instant>` im State für Timing-Tracking. (~80 LoC)

13r. ✅ **Unmaximize-on-Drag** (`src/grabs.rs`): Beim Starten eines Move-Grabs auf einem maximierten Fenster wird die Wiederherstellung auf das **erste Motion-Event** verzögert (inspiriert von cosmic-comp's `DelayGrab`-Architektur) — ein einfacher Klick auf die Titelleiste maximierter Fenster löst kein Restore aus. Erst wenn die Maus sich bewegt: Fenster wird unmaximiert, proportionale X-Positionierung (Cursor behält prozentuale Position auf der Titelleiste). Y-Positionierung abhängig vom Dekorationstyp via `unmaximize_y()`: **SSD-Fenster** — Titelleiste wird *oberhalb* des Element-Origins gezeichnet, daher `element_y = cursor_y + TITLEBAR_HEIGHT/2` → Cursor landet in der Mitte der Server-Titelleiste. **CSD-Fenster** — Titelleiste beginnt ab Element-Origin, daher `element_y = cursor_y − TITLEBAR_HEIGHT/2` → Cursor ~15px in der Client-Titelleiste. Grab-Anker (`start_data.location`, `initial_window_location`) werden nach dem Restore auf aktuelle Cursor-Position zurückgesetzt. Funktioniert einheitlich für SSD-Titelleisten-Drag, Client-initiierter `move_request` (CSD) und X11/XWayland. `MaximizedMoveState`-Struct im Grab speichert die maximierte Geometrie, `detect_maximized_state()` prüft Maximized-Status ohne sofort zu restoren. (~120 LoC)

13s. ✅ **Titelleisten-Kontextmenü** (`src/decorations.rs`, `src/ui.rs`, `src/input.rs`, `src/render.rs`): Rechtsklick auf SSD-Titelleiste öffnet ein GPU-gerendertes Kontextmenü mit den Einträgen **Minimize**, **Maximize/Restore** und **Close**. Menü wird über die gleiche egui-GPU-Pipeline wie die Titelleisten gerendert (eigener `CachedRenderBuffer` im `GlowState`). `TitlebarContextMenu`-Struct in `decorations.rs` mit Hit-Test-Methoden (`item_at()`, `contains()`) und fixer Layout-Geometrie (180×95 logische Pixel, 3 Items à 26px + 9px-Separator + 4px-Padding). Hover-Highlighting per Frame aus `pointer_location` berechnet. Klick auf Item → Aktion ausführen via `handle_decoration_click()`, Klick außerhalb → Menü schließen. Rechtsklick-Erkennung über `BTN_RIGHT` (0x111) im Input-Handler. (~200 LoC)

**Meilenstein 2b:** ✅ Crate heißt `platynui-wayland-compositor`, Binary und alle Referenzen konsistent umbenannt. ✅ Konfigurationsdatei (`compositor.toml`) mit Font-, Theme-, Keyboard- und Output-Einstellungen. ✅ Titelleisten zeigen echte Fonts (Noto Sans) mit Antialiasing und Unicode-Support via GPU-residentem egui-Rendering, Theme-Farben konfigurierbar, Button-Hover-Highlighting. ✅ `platynui-wayland-compositor -- gtk4-demo` startet die App automatisch in der Session. ✅ Konsolidierung auf `GlowRenderer` — `PixmanRenderer` komplett entfernt, alle Backends (Winit, DRM, Headless, Screenshots) nutzen einheitlich `GlowRenderer`. ✅ Fullscreen-Support für Wayland und X11 — SSD wird automatisch unterdrückt, Position/Size wird gespeichert und wiederhergestellt. ✅ Maximize via Protokoll (`maximize_request`/`unmaximize_request`) und Doppelklick auf SSD-Titelleiste. ✅ Unmaximize-on-Drag — maximierte Fenster werden beim Ziehen automatisch wiederhergestellt mit proportionaler Cursor-Positionierung. ✅ Titelleisten-Kontextmenü — Rechtsklick auf SSD-Titelleisten öffnet ein egui-gerendertes Menü mit Minimize/Maximize/Close-Aktionen. ✅ `[[output]]`-Config-Sections werden in `State::new()` verdrahtet — per-Output-Geometrie vor CLI-Flags. ✅ Client-Cursor-Surfaces werden im Render-Pfad composited (Hotspot, render_elements_from_surface_tree, Index 0). ✅ Screenshots nutzen max(output-scale) für physische Pixeldimensionen. ✅ `platynui-wayland-compositor-ctl` CLI-Tool (7 Subcommands, Socket-Discovery, Base64-Decoder). ✅ IPC-Protokoll formell dokumentiert. ✅ IPC-Tests: 11 Unit-Tests + 17 Integration-Tests (Basis, Error-Path, Client-Window-Tests mit egui Test-App). ✅ egui Test-App (`platynui-test-app-egui`) mit breiter Widget-Palette + `AccessKit`-Accessibility + `PLATYNUI_TEST_BACKEND`-Support.

---

### Phase 3: Automation-Protokolle (~2.100 LoC, ~2–3 Wochen) ✅ ERLEDIGT (Kern-Steps)

*Ziel: Alle Wayland-Protokolle, die PlatynUI und externe Tools (wayvnc, waybar, wl-clipboard, wlr-randr) brauchen, sind im Compositor verfügbar. Nach dieser Phase kann man mit `wayvnc` auf den Compositor zugreifen, mit `waybar` ein externes Panel nutzen, und Clipboard programmatisch lesen/schreiben.*

> **Status (2026-03-03):** Steps 14 (Multi-Monitor-Enhancements), 15, 16, 18, 19 (inkl. echte CursorSessions für wayvnc VNC Cursor Pseudo-Encoding), 19b, 19c, 19e (Content-Type) abgeschlossen. ~12.350 LoC gesamt, 21 Compositor-Tests (13 JSON-Parsing-Unit-Tests entfielen durch Serde-Migration in Phase 3a, Step 19f).
> Verbleibende Feature-Steps werden in Phase 3b fortgeführt; ein umfassendes Code-Review hat
> zahlreiche Code-Smells identifiziert, die in Phase 3a (Härtung) adressiert werden.
>
> **Status (2026-07-19):** Alle Core-Steps (14, 15, 16, 18, 19, 19b, 19c) und Phase 3a (19f–19z)
> abgeschlossen. Phase 3a+ (19aa–19ai, Bugfixes & Window-Management) ebenfalls abgeschlossen.
> ~14.500 LoC, 1874 Tests. Zusätzlich erledigt (nicht im Plan als Steps):
> - Linux-Only-Gating: Alle Dependencies und Entry-Points in wayland-compositor und
>   wayland-compositor-ctl sind `cfg(target_os = "linux")`-gated.
> - Lint-Zentralisierung: Per-Crate Lint-Overrides entfernt, workspace-weite Lints gelten.
> - README-Restrukturierung: Beide READMEs von technischer Referenz zu Projekt-Überblick
>   umgeschrieben. Technische Details nach `docs/usage.md` und `docs/configuration.md` verschoben.
> - compositor-ctl: Implementierung in eigenes `app.rs`-Modul extrahiert.
>
> **Hinweis Foreign-Toplevel + ironbar (2026-03-03):** Umfangreiche Überarbeitung von Step 16
> (foreign_toplevel.rs, seat.rs, input.rs). Korrekte Integration mit ironbar (Taskbar-Client):
> - **Focus-Tracking in `SeatHandler::focus_changed`**: `last_focused_window` im State, bei jedem
>   Fokuswechsel wird der alte Window deaktiviert und der neue aktiviert. Verwendet
>   `send_foreign_toplevel_state_activated()` mit explizitem `is_activated`-Flag, weil
>   XDG `current_state()` erst nach Client-Ack aktuell ist (Roundtrip-Latenz).
> - **X11-Surface-State in `build_wlr_state`**: X11-Fenster melden jetzt korrekt
>   maximized/activated/fullscreen via `x11.is_maximized()` etc. Vorher: leerer State `[]`.
> - **Minimised-State ohne `activated`**: `build_wlr_state_with_minimized` stripped den
>   `activated`-Flag wenn `is_minimized=true`, damit ironbar den korrekten Zustand sieht.
> - **Focus-Handling in `minimize_window`**: Fokus wird auf das nächste sichtbare Fenster
>   verschoben (oder `None`), bevor der State-Update an Taskbar-Clients gesendet wird.
> - **Stale-State-Prevention in `update_toplevel_metadata`**: Nutzt `last_focused_window`
>   als autoritative `activated`-Quelle statt den veralteten XDG-State.
> - **Popup-Constraining für Layer-Shell-Popups**: `WlrLayerShellHandler::new_popup`
>   übernimmt Constraining + Configure für Popups deren Parent erst nach `new_popup` gesetzt wird.
> - **Click-to-Restore entfernt**: Klick auf leere Desktop-Fläche stellt minimierte Fenster
>   nicht mehr her — das war ein Workaround vor ironbar. Minimize/Restore läuft jetzt
>   ausschließlich über Taskbar (ironbar) oder SSD-Buttons.

14. ✅ **Multi-Monitor-Enhancements** (`src/state.rs`, `src/render.rs`, `src/input.rs`, `src/grabs.rs`, `src/backend/winit.rs`, `src/handlers/xdg_shell.rs`): Umfangreiche Verbesserungen für Multi-Monitor-Betrieb im Winit-Backend:
    - ✅ `--scale <f64>` CLI-Flag — Per-Output-Scale für alle Outputs (z.B. `--scale 1.5` für HiDPI-Preview). Übergabe an `create_output_configs()` und TOML-Config `[[output]]`.
    - ✅ `--window-scale <f64>` CLI-Flag — Skaliert das Winit-Preview-Fenster herunter, ohne die interne Output-Auflösung zu ändern (z.B. `--window-scale 0.5` für 50%-Preview). Clients sehen weiterhin die volle Auflösung/Scale.
    - ✅ **Mixed-Scale-Rendering** (`max_output_scale()`): Bei Outputs mit unterschiedlichen Scales wird `max(scale)` für das gesamte Framebuffer verwendet. Alle Render-Elemente nutzen den maximalen Scale, damit physische Pixel-Positionen konsistent bleiben.
    - ✅ **Dead-Zone Pointer-Handling**: Bei nicht-rechteckigen Output-Layouts (z.B. L-förmig) werden Pointer-Positionen in toten Zonen per `clamp_to_outputs()` auf die nächste gültige Output-Grenze geclampt. Move-Grabs prüfen `point_in_any_output()` für Current- und Previous-Frame.
    - ✅ **Inkrementeller MoveSurfaceGrab**: Grab-Anker wird pro Frame aktualisiert (`start_data.location = event.location`), sodass absolute-to-incremental-Delta korrekt berechnet wird. Verhindert Fenster-Sprünge bei Multi-Monitor-Pointer-Mapping.
    - ✅ **Edge-Output-Resize** (`resize_edge_outputs()`): Wenn das Winit-Fenster vom Benutzer vergrößert/verkleinert wird, werden Outputs an der rechten/unteren Kante des Bounding-Box proportional angepasst, sodass das Layout das Fenster exakt ausfüllt. Interior-Outputs bleiben unverändert.
    - ✅ **Layer-Surface-Rendering für alle Outputs**: Alle vier Layer-Typen (Background, Bottom, Top, Overlay) werden für jeden Output gerendert — nicht nur für den primären. Essentiell für ironbar/waybar-Panels auf mehreren Monitoren.
    - ✅ **Layer-Map-Rearrange** (`rearrange_layer_maps()`): Nach Mode-Änderungen (Resize, Output-Management) werden alle Layer-Maps neu arrangiert und `send_pending_configure` für jede Layer-Surface aufgerufen, damit Panels korrekte Geometrie erhalten.
    - ✅ **Monitor-Rahmen** (`render_output_separators()`): Statt einer einfachen Trennlinie zwischen Outputs wird ein 1px-Rahmen um jeden einzelnen Monitor gezeichnet — funktioniert korrekt für jedes Layout (horizontal, vertikal, L-förmig).
    - ✅ **Unified Maximize** (`do_maximize()`/`do_unmaximize()`): Maximize-Logik aus Decoration-Click-Handler, XDG-Shell-Handler und Foreign-Toplevel-Handler in zwei zentrale öffentliche Funktionen konsolidiert. Maximize berücksichtigt korrekt den Output des Fensters (via `output_for_window()`) und die `usable_geometry` (abzüglich Layer-Surface-Exklusivzonen).
    - (~250 LoC verteilt über state.rs, render.rs, input.rs, grabs.rs, backend/winit.rs, handlers/xdg_shell.rs)

15. ✅ **Layer-Shell** (`src/handlers/layer_shell.rs`): `wlr-layer-shell-v1` verdrahten (smithay hat Building Blocks). Enables: waybar (externes Panel), ironbar (Taskbar), Highlight-Overlays für PlatynUI, wayvnc-Overlays. Exklusive Zonen korrekt verrechnen (Fenster nicht unter dem Panel platzieren). Layer-Surfaces werden für alle Outputs gerendert. `WlrLayerShellHandler::new_popup` Override: Popup-Constraining wird aufgeschoben bis smithay den Parent gesetzt hat (smithay ruft `XdgShellHandler::new_popup` vor `zwlr_layer_surface.get_popup` auf, daher ist der Parent in `new_popup` noch `None`). (~124 LoC)

16. ✅ **Foreign-Toplevel-Management** (`src/handlers/foreign_toplevel.rs`, `src/handlers/seat.rs`): `wlr-foreign-toplevel-management-v1` (v3, manuell implementiert) + `ext-foreign-toplevel-list-v1` (smithay `delegate_foreign_toplevel_list!()`). Publisht alle Toplevels (Wayland + X11) mit Titel/App-ID/State. Verarbeitet activate/close/minimize/maximize/fullscreen Requests via extrahierte `do_maximize`/`do_fullscreen` etc. Title/app_id-Änderungen werden bei jedem surface-commit diffbasiert weitergeleitet. Focus-Tracking über `SeatHandler::focus_changed` mit `last_focused_window` im State — bei Fokuswechsel sofortiges State-Update an Taskbar-Clients mit explizitem `is_activated`-Flag (umgeht XDG Configure-Roundtrip-Latenz). X11-Surfaces melden State korrekt via `x11.is_maximized()`/`is_activated()`/`is_fullscreen()`. Minimized-State stripped `activated`-Flag. `minimize_window` verschiebt Fokus vor State-Update. `update_toplevel_metadata` nutzt `last_focused_window` als autoritative `activated`-Quelle. Lifecycle-Hooks in `xdg_shell.rs` (new_toplevel, toplevel_destroyed) und `xwayland.rs` (map_window_request, unmapped_window, destroyed_window). Getestet mit ironbar: Minimize/Restore/Activate/Background-to-Front für Wayland- und X11-Fenster funktioniert korrekt. Enables: Taskbar-Buttons in ironbar/waybar, `platynui-cli query` über wlr-foreign-toplevel. (~795 LoC foreign_toplevel.rs + ~55 LoC seat.rs)

18. ✅ **Virtual-Pointer + Virtual-Keyboard** (`src/handlers/virtual_pointer.rs`, `src/handlers/virtual_keyboard.rs`):
    - ✅ `zwlr_virtual_pointer_v1` — empfängt absolute/relative Motion, Button, Axis-Events und injiziert sie in den Smithay Input-Stack. Manager-Global mit Security-Filter, Mutex-basiertes Axis-Frame-Accumulation. (~300 LoC)
    - ✅ `zwp_virtual_keyboard_v1` Server verdrahten, XKB-Keymap-Upload akzeptieren. Smithay hat Teile. (~50 LoC)
    - Enables: Fallback-Input-Pfad für Sway/Hyprland-Kompatibilität im Platform-Crate.

19. ✅ **Screencopy-Server** (`src/handlers/screencopy.rs`, `src/cursor.rs`): `ext-image-copy-capture-v1` + `ext-image-capture-source-v1` — Framebuffer als `wl_shm`-Buffer an Client liefern. Manuelle Implementierung (kein smithay Built-in): GlobalDispatch/Dispatch für 7 Interfaces (3 Managers, Source, Session, Frame, CursorSession). Output- und Toplevel-Capture via offscreen GlowRenderer + OutputDamageTracker. ABGR8888→ARGB8888 Swizzle, presentation_time, damage-tracking, shm-Buffer-Validierung. Vollständige **CursorSession-Implementierung** für VNC Cursor Pseudo-Encoding (RFC 6143, Encoding -239): `CaptureSource::Cursor`-Variante liefert das aktuelle xcursor-Theme-Bild als eigene Capture-Session (echte Cursor-Dimensionen, Hotspot-Position). `CursorImageData`-Struct in `cursor.rs` extrahiert Cursor-Pixel-Daten direkt (ohne GL-Pipeline). `perform_cursor_capture()` Fast-Path kopiert xcursor-ARGB-Daten per `copy_cursor_to_shm()` in den SHM-Buffer; bei fehlendem Cursor-Image wird transparent gefüllt (`fill_shm_transparent()`). `paint_cursors`-Option korrekt respektiert: Frame-Capture bakt den Cursor nur bei gesetztem `PaintCursors`-Flag in den Frame, andernfalls liefert die separate CursorSession das Cursor-Bild für Client-seitiges Rendering (wayvnc Default-Modus). Security-Policy-gefiltertes Global. Enables: wayvnc (Frame + Cursor-Session Dual-Capture), grim, Screenshot im Platform-Crate. (~1.060 LoC screencopy.rs + ~270 LoC cursor.rs Erweiterungen)

19b. ✅ **Data-Control** (`src/handlers/data_control.rs`): `wlr-data-control-v1` verdrahten — ermöglicht Clipboard lesen/schreiben ohne Fenster-Fokus. Smithay hat `delegate_data_control!()`. Enables: `wl-copy`/`wl-paste`, programmatisches Clipboard-Testing im Platform-Crate, Clipboard-Verifikation in Tests. (~50 LoC)

19c. ✅ **Output-Management** (`src/handlers/output_management.rs`): `wlr-output-management-v1` (v4) — Outputs zur Laufzeit konfigurieren (Resolution, Position, Scale, Transform, Enable/Disable). Manuell implementiert (kein smithay Built-in): GlobalDispatch/Dispatch für Manager, Head, Mode, Configuration, ConfigurationHead. Arc<Mutex>-basiertes Shared-State für ConfigHead↔Configuration-Kommunikation. Serial-basierte Invalidierung, security-policy-gefiltertes Global. Enables: `wlr-randr`/`kanshi`, dynamische Multi-Monitor-Tests ohne Compositor-Neustart. (~568 LoC)

**Meilenstein 3 (Automation-Protokolle, abgeschlossene Schritte):** Virtual-Pointer/Keyboard-Input funktioniert. Screenshot via ext-image-copy-capture inkl. CursorSessions für wayvnc. `waybar`/ironbar (extern) funktionieren via Layer-Shell. `wayvnc` kann sich verbinden und die Session anzeigen + fernsteuern (Frame + Cursor Dual-Capture). Clipboard über `wl-copy`/`wl-paste` lesbar/schreibbar. Multi-Monitor per `wlr-randr` dynamisch konfigurierbar. Multi-Monitor-Enhancements (Mixed-Scale-Rendering, Dead-Zone-Handling, Edge-Output-Resize, Monitor-Rahmen, Layer-Surface-Rendering für alle Outputs) sind stabil.

---

### Phase 3a: Härtung & Code-Qualität (~500 LoC Änderungen, ~3–5 Tage) ✅ ERLEDIGT

*Ziel: Alle im Code-Review (2026-03-03) identifizierten Code-Smells, Bugs und Protokoll-Verletzungen sind behoben. Keine bare `.unwrap()` mehr, konsistentes Error-Handling, vollständiges Tracing, Dead Code entfernt. Die Codebasis ist bereit für weitere Feature-Arbeit ohne dass technische Schulden mitgeschleppt werden.*

> **Motivation:** Umfassendes Code-Review über ~11.900 LoC / 42 Quelldateien hat 1 kritisches, 4 hohe, 12 mittlere und 15+ niedrige Findings identifiziert. Von 94 `.unwrap()`/`.expect()`-Stellen sind ~65 bare `.unwrap()` ohne Kontext-Nachricht. Mehrere Protokoll-Invarianten (Screencopy DuplicateFrame, Output-Management already_used) sind nicht erzwungen. JSON-Ausgabe im Control-Socket ist anfällig für Sonderzeichen in Fenstertiteln.

> **Status (2026-03-03):** Steps 19f (Serde-Migration), 19f₂ (Code-Deduplizierung, ~595 Zeilen entfernt),
> 19f₃ (Kommentar-Review), 19f₄ (Focus-Loss Input Release), 19f₅ (Software-Cursor für SSD-Resize)
> und 19f₆ (Session-Scripts AT-SPI-Fix) abgeschlossen. Steps 19g–19z (Protokoll-Korrektheit,
> Unwrap-Eliminierung, Error-Handling, Tracing, Dead Code, Magic Numbers) komplett erledigt.
> ~12.350 LoC gesamt, 1874 Tests. **Phase 3a abgeschlossen.**

**Bereits abgeschlossen (Querschnitts-Arbeiten):**

19f. ✅ **Serde-Migration im Control-Socket** (`src/control.rs`, `apps/wayland-compositor-ctl/src/main.rs`, `Cargo.toml`): Kompletter Ersatz der manuellen JSON-Konstruktion und -Parsing durch typisierte `serde`-Structs — geht über den ursprünglichen Plan (nur `json_escape()`-Helper) hinaus. Typisierte Request/Response-Structs (`Request`, `WindowInfo`, `MinimizedWindowInfo`, `OutputInfo`) mit `#[derive(Serialize, Deserialize)]`. `process_command()` nutzt `serde_json::from_str::<Request>()`, Responses via `serde_json::json!()`. Manuelle Helper (`json_escape()`, `extract_json_string()`, `extract_json_u64()`) und ihre 13 Unit-Tests entfernt (~150 LoC entfernt, netto ~30 LoC hinzugefügt für Struct-Definitionen). CTL-App: `build_command_json()` und `window_selector_json()` ebenfalls auf `serde_json::json!()` umgestellt. Dependency: `serde_json = "1"` zu Compositor-Cargo.toml hinzugefügt.

19f₂. ✅ **Code-Deduplizierung** (compositor-weit, 12 Dateien): Umfassende Deduplizierung über ~12.350 LoC — 12 Duplikations-Muster identifiziert und konsolidiert, ~595 Zeilen entfernt:
  - `ensure_initial_configure()` Helper in state.rs (ersetzt 6× dupliziertes `initial_configure_sent`-Pattern in input.rs, grabs.rs, decorations.rs)
  - `window_surface_id()` Helper (ersetzt 4× duplizierte WlSurface→Id-Extraktion)
  - `send_frame_callbacks()` Helper in render.rs (ersetzt 3× dupliziertes Callback-Pattern in Backends)
  - `paint_output_from_elements()`/`create_render_elements()` in render.rs (ersetzt 3× duplizierte Render-Logik in winit.rs, headless.rs, drm.rs)
  - `configure_output()` Helper (ersetzt 2× duplizierte Output-Setup-Logik)
  - `create_compositor_state!()` Makro (ersetzt 3× dupliziertes State-Setup in Backends)
  - Weitere: Screencopy/Cursor SHM-Helper, Foreign-Toplevel State-Builder, Drag/Resize-Koordinaten, Control-Socket Setup.
  - Alle 1874 Tests grün.

19f₃. ✅ **Kommentar-Review & Dokumentation** (compositor-weit, 12 Dateien): Vollständiges Review aller ~35 Quelldateien (~12.350 LoC) auf Kommentar-Qualität:
  - **16 redundante Kommentare entfernt:** Offensichtliche Beschreibungen (`// Create the Wayland socket`, `// Configure the cursor`, `// Return success`), veraltete TODOs, und Kommentare die nur den Code wiederholen (state.rs, control.rs, child.rs, winit.rs, headless.rs, drm.rs, xdg_activation.rs, xwayland.rs, render.rs).
  - **6 irreführende Kommentare korrigiert:** `Unreachable` → `Exhaustive match` (input.rs), Pixel-Format-Dokumentation (cursor.rs), `No-op` → Stub-Dokumentation (text_input.rs, session_lock.rs), duplizierte Doc-Blöcke entfernt (decorations.rs, multi_output.rs).
  - **2 fehlende Doc-Kommentare ergänzt:** `PointerHitResult`-Varianten (decorations.rs), `TitlebarRenderer::new` (ui.rs).

19f₄. ✅ **Focus-Loss Input Release** (`src/input.rs`, `src/backend/winit.rs`, `src/state.rs`): Wenn das Winit-Host-Fenster den Fokus verliert (z.B. Alt+Tab unter GNOME), werden alle gedrückten Tasten und Mausbuttons automatisch released. GNOME interceptet Alt+Tab und verschluckt das Alt-Release-Event — ohne diesen Fix bleibt die Alt-Taste im Compositor stuck. Implementierung:
  - `release_all_pressed_inputs()` in input.rs: Iteriert `keyboard.pressed_keys()` (smithay `HashSet<Keycode>`) und sendet synthetische Release-Events via `keyboard.input()`. Danach drains `state.pressed_buttons` und sendet `PointerButtonEvent::Released` + Frame für jeden Button.
  - `pressed_buttons: Vec<u32>` in state.rs: Manuelles Tracking der gedrückten Pointer-Buttons, da smithay kein öffentliches `pressed_buttons()` auf `PointerHandle` exponiert (nur auf `PointerInnerHandle` innerhalb von Grabs).
  - `WinitEvent::Focus(false)` in winit.rs: Setzt `focus_lost = true`, was nach dem Event-Dispatch `release_all_pressed_inputs()` auslöst.
  - Release geschieht bewusst bei Focus-**Loss** (nicht bei Focus-Regain), damit der Compositor sofort einen konsistenten Input-State hat. (~60 LoC)

19f₅. ✅ **Software-Cursor für SSD-Resize-Borders** (`src/render.rs`): Im Software-Cursor-Modus (`--software-cursor`) wurden SSD-Resize-Cursor (Pfeile an Fensterrändern) nicht angezeigt — der Software-Cursor-Rendering-Pfad in `collect_render_elements()` prüfte nur `state.cursor_status` (Client-Cursor), ignorierte aber `state.compositor_cursor_shape` (SSD-Resize/Move). Fix:
  - Cursor-Rendering prüft jetzt zuerst `compositor_cursor_shape`: Wenn nicht `Default`, wird das passende xcursor-Theme-Icon gerendert (gleiche Zuordnung wie im Winit Host-Cursor-Pfad: `CursorShape::ResizeN` → `CursorIcon::NResize`, etc.).
  - `compositor_cursor_shape_to_icon()`: Mappt `CursorShape` → `Option<CursorIcon>` (`None` = kein Override, Client-Cursor verwenden).
  - `render_xcursor_icon()`: Extrahierte Hilfsfunktion für xcursor-Rendering via `MemoryRenderBuffer` — wird auch vom `Named`-Branch wiederverwendet (Deduplizierung).
  - Betrifft nur den Software-Cursor-Pfad; im Nicht-Software-Modus setzte winit.rs den Host-Cursor bereits korrekt. (~60 LoC)

19f₆. ✅ **Session-Scripts AT-SPI-Fix** (`scripts/startcompositor.sh`, `scripts/startxsession.sh`, `scripts/startwaylandsession.sh`): AT-SPI-Bus-Setup in allen drei Session-Scripts überarbeitet. Problem: `at-spi2-registryd` konnte in isolierten Sessions nicht gestartet werden — das System-Service-File (`/usr/share/dbus-1/accessibility-services/org.a11y.atspi.Registry.service`) enthält `--use-gnome-session`, was in Nicht-GNOME-Sessions fehlschlägt. Fehler: `Could not activate remote peer 'org.a11y.atspi.Registry': unit failed`. Drei Fixes:
  - **Service-File-Override:** Lokales `org.a11y.atspi.Registry.service` ohne `--use-gnome-session` wird in `$XDG_RUNTIME_DIR/at-spi-services/dbus-1/accessibility-services/` erstellt und via `XDG_DATA_DIRS`-Prepend vorrangig gemacht.
  - **Registryd-Polling statt `sleep 0.2`:** Nach dem Start von `at-spi2-registryd` wird aktiv per `dbus-send --dest=org.a11y.atspi.Registry ... Peer.Ping` gepollt (bis 5s), bevor der Compositor/WM gestartet wird. Verhindert Race-Conditions.
  - **`AT_SPI_BUS_ADDRESS` exportiert:** Die AT-SPI-Bus-Adresse wird als Umgebungsvariable exportiert, damit Child-Prozesse den Bus direkt finden.
  Zusätzlich: `startcompositor.sh` wurde von fragiler `bash -c '...'`-Quoting auf ein temporäres Inner-Script umgestellt (serialisierte Args via `printf '%q'`). Bessere Diagnose-Ausgaben wenn Prozesse unerwartet sterben.

**Kritisch:**

~~19f.~~ *(→ siehe oben, als umfassende Serde-Migration umgesetzt)*

**Hoch — Protokoll-Korrektheit:**

19g. ✅ **Screencopy Dead Guards fixen** (`src/handlers/screencopy.rs`): `has_active_frame` wird initialisiert aber nie auf `true` gesetzt → `DuplicateFrame`-Protokoll-Error ist dead code. `session_created` in CursorSession ebenso. Fix: `Cell<bool>` oder Mutation im richtigen Lifecycle-Punkt. Zusätzlich `Destroyed`-Callback für `ExtImageCopyCaptureFrameV1` implementieren um `pending_captures`-Einträge bei Client-Disconnect aufzuräumen (Memory-Leak). (~40 LoC)

19h. ✅ **Screencopy `unreachable!()` durch Error ersetzen** (`src/handlers/screencopy.rs`): `render_source_impl()` panikt bei `CaptureSource::Cursor` via `unreachable!()`. Stattdessen `Err(...)` returnen. (~5 LoC)

19i. ✅ **Output-Management Protokoll-Fehler** (`src/handlers/output_management.rs`): Doppeltes apply/test auf derselben Configuration wird still ignoriert. Per wlr-output-management-Spec muss `already_used` als Protokoll-Error gesendet werden. Fix: `resource.post_error(Error::AlreadyUsed, ...)`. Zusätzlich: `finished`-Events für Head/Mode-Objekte bei Output-Reconfiguration senden (aktuell fehlt Cleanup → Protokoll-Verletzung bei Hot-Plug). (~30 LoC)

19j. ✅ **`create_resource().unwrap()` absichern** (`src/handlers/output_management.rs`): Zwei `create_resource()` Aufrufe (Head, Mode) können bei Client-Disconnect fehlschlagen und crashen den Compositor. Fix: `let Ok(r) = ... else { return; }`. (~10 LoC)

**Mittel — Unwrap-Eliminierung:**

19k. ✅ **`State::keyboard()` / `State::pointer()` Helper** (`src/state.rs`): Zwei Helper-Methoden die `seat.get_keyboard()` / `seat.get_pointer()` mit `.expect("seat always has keyboard/pointer after init")` wrappen. Eliminiert ~25 bare `.unwrap()` in input.rs, control.rs, xwayland.rs, foreign_toplevel.rs, xdg_activation.rs auf einen Schlag. (~10 LoC state.rs + Umbau in ~8 Dateien)

19l. ✅ **Mutex `.unwrap()` → `.expect("mutex poisoned")`** (compositor-weit): Alle ~35 `mutex.lock().unwrap()` mit beschreibendem `.expect()` versehen — konsistente Panic-Message statt generischem `called Option::unwrap() on a None value`. Alternative: `parking_lot::Mutex` (poisons nie). (~35 Stellen)

19m. ✅ **virtual_pointer.rs: `_data`-Parameter durchreichen** (`src/handlers/virtual_pointer.rs`): Der `Dispatch`-Trait liefert `data: &VirtualPointerUserData` direkt — wird aktuell ignoriert und stattdessen 5× via `resource.data().unwrap()` re-derived. Fix: `_data` → `data` umbenennen und an alle Handler-Funktionen durchreichen. Eliminiert 5 `.unwrap()`. (~20 LoC)

**Mittel — Error-Handling & Tracing:**

19n. ✅ **Stille Fehler loggen** (compositor-weit): Alle `let _ = x11.close()`, `.ok()` und ähnliche silent-discard-Patterns durch `if let Err(e) = ... { tracing::warn!(...) }` ersetzen. Betrifft: input.rs (5×), seat.rs (2×), foreign_toplevel.rs (1×), dmabuf.rs (1×), drm.rs (2×), ready.rs (1×). (~30 LoC)

19o. ✅ **Tracing nachrüsten in virtual_pointer.rs** (`src/handlers/virtual_pointer.rs`): Aktuell null `tracing`-Calls im gesamten Modul. Mindestens `debug!` bei Create/Destroy, `trace!` bei Motion/Button/Axis/Frame, `warn!` bei unbekannten WEnum-Werten. (~20 LoC)

19p. ✅ **text_input.rs Fallback-Geometrie** (`src/handlers/text_input.rs`): `parent_geometry()` gibt `Rectangle::default()` = `(0,0,0,0)` zurück → IME-Popups sind mis-positioniert. Fix: Output-Geometrie als Fallback verwenden. Stubs `new_popup`/`dismiss_popup` mit `tracing::debug!` instrumentieren. (~10 LoC)

**Mittel — Dead Code & Inkonsistenzen:**

19q. ✅ **Theme-Border-Colors verdrahten oder entfernen** (`src/decorations.rs`, `src/config.rs`): `ThemeConfig.active_border`/`inactive_border` werden geparsed aber `render_borders()` verwendet hardcoded `BORDER_COLOR`/`BORDER_COLOR_FOCUSED`. Entweder Config-Felder in Rendering verdrahten oder Dead Code entfernen (inkl. `active_border_rgba()`/`inactive_border_rgba()` in config.rs). (~20 LoC)

19r. ✅ **`decorations.rs` Panic durch Option ersetzen**: `to_xdg_resize_edge()` panikt bei `Focus::Header`. Besser `Option<ResizeEdge>` returnen, Caller passen `if let Some(edge) = ...` an. (~10 LoC)

19s. ✅ **render.rs inkonsistenter Lock** (`src/render.rs`): Zeile 195 `CursorImageSurfaceData` Lock via `.lock().unwrap()`, 60 Zeilen weiter `.lock().ok()`. Einheitlich `.lock().ok().map(|d| d.hotspot).unwrap_or_default()`. (~5 LoC)

19t. ✅ **Screencopy unsafe-Fläche reduzieren** (`src/handlers/screencopy.rs`): Per-Pixel `ptr.add()` in `copy_pixels_to_shm()` durch einmaligen `slice::from_raw_parts_mut()` ersetzen → weniger unsafe-Code, bessere Auto-Vektorisierung, idiomatic safe Iteration mit `chunks_exact(4)`. Gleiches Pattern auf `copy_cursor_to_shm()` und `fill_shm_transparent()` anwenden. (~40 LoC)

**Niedrig — Magic Numbers & Cleanup:**

19u. ✅ **Shared Constants extrahieren** (compositor-weit): Duplizierte Magic Numbers in benannte Konstanten umwandeln:
  - `BTN_LEFT` (`0x110`) und `BTN_RIGHT` (`0x111`) → `src/input.rs` Modul-Konstanten (aktuell in grabs.rs 2× und input.rs)
  - `DOUBLE_CLICK_MS` (`400`) → Named Constant
  - `MIN_WINDOW_WIDTH`/`MIN_WINDOW_HEIGHT` (`100`/`50`) → Named Constants in grabs.rs
  - `DEFAULT_REFRESH_MHTZ` (`60_000`) → Shared Constant für winit.rs, headless.rs, drm.rs
  - `BACKGROUND_COLOR` (`[0.1, 0.1, 0.1, 1.0]`) → Shared Constant
  - `CLOCK_MONOTONIC` (`1`) → Named Constant in state.rs
  - Titlebar Button-Sizes (`26×18`, gap `2.0`, right_pad `6.0`) → Shared zwischen ui.rs und decorations.rs
  - wlr-foreign-toplevel State-Werte (`0`/`1`/`2`/`3`) → Named Constants
  - (~30 LoC Konstantendefinitionen + Umbau)

19v. ✅ **Dead `#[allow]` entfernen** (compositor-weit): `#[allow(clippy::too_many_lines)]` auf 4-Zeilen-Funktion (input.rs L84). Stale `#[allow(clippy::cast_possible_truncation)]` in virtual_pointer.rs L190. Blanket `#[allow(dead_code)]` auf `State`-Struct (state.rs L63) durch per-Field Annotations ersetzen. Unused Parameter `_button: u32` in input.rs. Redundanter `Destroy | _` Match-Arm in screencopy.rs L379. Triviales Binding `let draw_cursor = paint_cursors;` in screencopy.rs. (~15 LoC)

19w. ✅ **Catch-All `_ => {}` mit Tracing versehen** (compositor-weit): Alle stillen Wildcard-Arms in Dispatch-Matches (output_management.rs 2×, virtual_pointer.rs 2×, foreign_toplevel.rs 1×, xdg_shell.rs 1×) um `tracing::debug!("unhandled request")` ergänzen, `Destroy`-Variant explizit matchen. (~20 LoC)

19x. ✅ **foreign_toplevel.rs Refactoring** (`src/handlers/foreign_toplevel.rs`): Byte-Level State-Manipulation (`.windows(4).position() + .drain()`) in `remove_state_value()`-Helper extrahieren (3 Duplikate). Unnötige `.clone()` nach `window.toplevel()` entfernen (6 Stellen). (~30 LoC)

19y. ✅ **DRM Multi-Monitor-Positionierung** (`src/backend/drm.rs`): Alle DRM-Outputs werden auf `(0,0)` gemappt → überlappen sich bei Multi-Monitor. Fix: Outputs nebeneinander arrangieren oder Config-Positionen verwenden (analog zu Winit-Backend). Zusätzlich: `frame_submitted().ok()` → mit Logging, Magic Number `19` → `libc::ENODEV`, Integer-Overflow in Refresh-Rate-Berechnung absichern. (~30 LoC)

19z. ✅ **Backend-Code-Duplikation reduzieren** (teilweise erledigt via 19f₂, `src/backend/*.rs`): Socket-Setup, XWayland-Start, Control-Socket, Readiness-Notification sind noch quasi identisch in winit.rs, headless.rs, drm.rs. Frame-Callbacks (`send_frame_callbacks()`), Render-Logik (`paint_output_from_elements()`/`create_render_elements()`), Output-Setup (`configure_output()`) und State-Initialisierung (`create_compositor_state!()`) wurden bereits in Step 19f₂ konsolidiert. Verbleibend: Socket-Setup, XWayland-Start, Control-Socket, Readiness-Notification in gemeinsame Helper extrahieren. (~50 LoC Umstrukturierung, netto weniger Code)

**Meilenstein 3a:** ✅ **ABGESCHLOSSEN.** Control-Socket JSON ist RFC-8259-konform via typisierter `serde`-Structs (Steps 19f). ~595 Zeilen Code-Duplikation eliminiert (Step 19f₂). Kommentar-Qualität verbessert: 16 redundante entfernt, 6 irreführende korrigiert, 2 fehlende ergänzt (Step 19f₃). Focus-Loss Input Release: Gedrückte Tasten und Mausbuttons werden bei Fokus-Verlust automatisch released — behebt stuck Alt-Key unter GNOME (Step 19f₄). Software-Cursor zeigt SSD-Resize-Cursors korrekt an (Step 19f₅). AT-SPI-Bus startet zuverlässig in isolierten Sessions (Step 19f₆). Steps 19g–19z komplett: Screencopy Guards + unreachable fix, Output-Management `already_used` + `finished`-Events, `create_resource` abgesichert, Keyboard/Pointer Helper (~25 `.unwrap()` eliminiert), Mutex `.expect()`, virtual_pointer data-Parameter, stille Fehler loggen (input/grabs/seat/xwayland/dmabuf), Tracing in virtual_pointer, text_input Fallback-Geometrie, Theme-Border-Colors verdrahtet, decorations Panic→Option, render.rs Lock konsistent, Screencopy unsafe reduziert, Named Constants (`DEFAULT_REFRESH_MHTZ`, `BACKGROUND_COLOR`), Dead `#[allow]` entfernt, Catch-All Tracing, foreign_toplevel `size_of`, DRM Multi-Monitor-Positionierung (Output-Positionen im Wayland-Protokoll korrekt), Backend-Deduplizierung. `cargo clippy --workspace --all-targets -- -D warnings` sauber. `cargo nextest run --all --no-fail-fast` — 1874 Tests grün.

---

### Phase 3a+: Bugfixes & Window-Management-Verbesserungen (~600 LoC, ~3 Tage) ✅ ERLEDIGT

*Ziel: Praxistests mit wayvnc/VNC, DRM-Multi-Monitor und XWayland haben mehrere Edge-Cases in Popup-Handling, Cursor-Rendering, Input-Mapping und Window-Management aufgedeckt. Diese Phase adressiert alle gefundenen Probleme.*

> **Status (2026-03-05):** Alle Steps komplett. ~14.500 LoC, 1874 Tests. DRM-Backend komplett
> überarbeitet für Multi-Monitor. VNC (wayvnc) funktioniert fehlerfrei inkl. Cursor und
> Keyboard-Layout. Popup-Handling und X11-Kompatibilität deutlich verbessert.
> Window-Management: Maximize/Unmaximize/Resize mit korrekter Größenwiederherstellung,
> Floating-Fenster werden bei Output-Resize in den sichtbaren Bereich geclampt.

**Popup- & Input-Korrekturen:**

19aa. ✅ **Popup-Constraining für SSD und Layer-Shell** (`src/handlers/xdg_shell.rs`): Popup-Positionierung korrigiert — Popups die über SSD-Fenster-Grenzen hinausragen wurden nicht korrekt beschnitten. Constraining berücksichtigt jetzt Titlebar-Offset bei SSD-Fenstern. Layer-Shell-Popups nutzen Output-Geometrie statt fehlender Fenster-Geometrie. Pointer-Events an Popups die über SSD-Window-Bounds hinausragen werden korrekt geroutet. (~80 LoC)

19ab. ✅ **X11-Popup-Positionierung** (`src/xwayland.rs`): X11-Override-Redirect-Windows (Menüs, Tooltips, Dropdowns) wurden oft falsch positioniert. Fix: Korrekte Koordinaten-Transformation von X11-Rootfenster-Koordinaten zu Wayland-Space. Menü-Dismissal bei Klick außerhalb verbessert. (~60 LoC)

19ac. ✅ **Virtual-Pointer Koordinaten-Mapping** (`src/handlers/virtual_pointer.rs`): `zwlr_virtual_pointer_v1` absolute Motion-Events wurden 1:1 als Pixel-Koordinaten interpretiert, ohne Berücksichtigung der Output-Geometrie des gebundenen Outputs. Fix: Koordinaten werden auf den korrekten Output gemappt (Position-Offset + Skalierung), sodass wayvnc-Pointer-Input bei Multi-Monitor und Output-Offsets korrekt funktioniert. (~30 LoC)

**VNC-/Cursor-Korrekturen:**

19ad. ✅ **VNC-Cursor-Rendering** (`src/handlers/screencopy.rs`, `src/render.rs`): Zwei VNC-Cursor-Probleme behoben:
  - **Cursor verschwindet über X11-Apps:** `CursorImageStatus::Surface` wurde in Screencopy-Frames nicht gerendert — nur Named-Cursors und der Host-Cursor waren sichtbar. Fix: Surface-Cursor werden jetzt in den Screencopy-Frame eingezeichnet.
  - **SSD-Resize-Cursors fehlen im VNC:** `compositor_cursor_shape` (Resize-Pfeile an Fensterrändern) wurde im Screencopy-Pfad ignoriert. Fix: Compositor-Cursor-Shape wird als xcursor-Icon in den Frame gerendert, mit gleicher Shape-Zuordnung wie im Winit-Host-Cursor-Pfad.
  - `compositor_cursor_icon()` (vorher `compositor_cursor_icon_pub`) als öffentliche Hilfsfunktion extrahiert für Wiederverwendung im Screencopy-Pfad.
  (~80 LoC)

19ae. ✅ **Keyboard-Layout in VNC-Session** (`scripts/platynui-session.sh`): `XKB_DEFAULT_LAYOUT` wird jetzt in der Session-Script gesetzt, damit wayvnc-Input korrekt gemappt wird. Ohne: VNC-Keyboard-Input nutzte US-Layout statt des konfigurierten Layouts. (~5 LoC)

**DRM-Backend-Überarbeitung:**

19af. ✅ **DRM Multi-Monitor-Overhaul** (`src/backend/drm.rs`): Komplette Überarbeitung des DRM-Backends für robusten Multi-Monitor-Betrieb auf echter Hardware:
  - **5-Monitor-Support:** Output-Restructuring mit `DrmOutputState`-Struct pro Connector. Korrekte EDID-Parsing für Monitor-Namen und physische Größen.
  - **VT-Switching:** Session-Pause/Resume Handler für sauberes VT-Switching (`Ctrl+Alt+F1..F12`). DRM-Rendering wird bei inaktiver Session pausiert, Surfaces bei Resume neu gerendert.
  - **Titlebar/Decoration-Rendering:** DRM-Backend nutzt jetzt den gleichen egui-GPU-Pipeline wie Winit — Titlebars werden korrekt auf DRM-Outputs gerendert.
  - **Winit Mode Accumulation Bug:** Output-Modes wurden bei jedem Resize akkumuliert statt ersetzt. Fix: Stale Modes werden vor dem Setzen des neuen Modes gelöscht.
  (~300 LoC)

**Window-Management:**

19ag. ✅ **X11-Maximize-Größenwiederherstellung** (`src/state.rs`, `src/xwayland.rs`, `src/handlers/xdg_shell.rs`, `src/input.rs`, `src/grabs.rs`): `pre_maximize_positions` speicherte nur die Position, nicht die Fenstergröße. Beim Unmaximize wurde die Größe nicht wiederhergestellt — das Fenster behielt die maximierte Breite/Höhe. Fix:
  - `PreMaximizeState`-Typ erweitert um `Option<Size<i32, Logical>>` (3-Tupel statt 2-Tupel).
  - XwmHandler `maximize_request()` und `unmaximize_request()` implementiert (vorher fehlend — X11-Apps' `_NET_WM_STATE`-Requests wurden ignoriert).
  - `remove_x11_window()` räumt `pre_maximize_positions` auf (Memory-Leak-Fix).
  - Alle Konsumenten aktualisiert: state.rs, xdg_shell.rs, input.rs, grabs.rs, xwayland.rs.
  (~120 LoC)

19ah. ✅ **Maximierte Fenster bei Output-Resize anpassen** (`src/backend/winit.rs`, `src/state.rs`): Beim Verkleinern/Vergrößern des Winit-Fensters (Single-Output-Modus) wurde `reconfigure_windows_for_outputs()` nicht aufgerufen — maximierte Fenster behielten die alte Größe. Fix:
  - `reconfigure_windows_for_outputs()` wird jetzt nach BEIDEN Resize-Branches aufgerufen (Single-Output und Multi-Output).
  - `reconfigure_windows_for_outputs()` erweitert um X11-Window-Handling: X11-Fenster mit `is_maximized()` oder `is_fullscreen()` werden jetzt auch per `x11.configure()` reconfigured (vorher: nur Wayland-Toplevels).
  (~60 LoC)

19ai. ✅ **Floating-Fenster bei Output-Verkleinerung clampen** (`src/state.rs`): Wenn der Output kleiner wird (Winit-Window-Resize, wlr-randr-Änderung), konnten normale Floating-Fenster komplett außerhalb des sichtbaren Bereichs landen — unerreichbar für den Nutzer. Fix:
  - `clamp_floating_windows_to_outputs()` wird am Ende von `reconfigure_windows_for_outputs()` aufgerufen.
  - Maximierte/Fullscreen-Fenster werden übersprungen (bereits separat behandelt).
  - Jedes Floating-Fenster wird so repositioniert, dass mindestens `TITLEBAR_HEIGHT` Pixel auf jeder Achse sichtbar bleiben — analog zu GNOME/Mutter und KDE/KWin.
  - Fenster werden nur verschoben, nie verkleinert. X11-Fenster werden zusätzlich via `x11.configure()` benachrichtigt.
  (~80 LoC)

**Meilenstein 3a+:** ✅ **ABGESCHLOSSEN.** Popup-Handling für SSD, Layer-Shell und X11 korrekt. VNC via wayvnc fehlerfrei (Cursor-Rendering, Pointer-Mapping, Keyboard-Layout). DRM-Backend für Multi-Monitor komplett überarbeitet (5 Outputs, EDID, VT-Switching). X11-Maximize speichert und stellt Fenstergröße wieder her. Maximierte Fenster passen sich bei Output-Resize an (Wayland + X11). Floating-Fenster werden bei Output-Verkleinerung in den sichtbaren Bereich geclampt. ~14.500 LoC, 1874 Tests grün.

---

### Phase 3b: Verbleibende Automation-Protokolle & Zusätzliche Protokoll-Unterstützung (~600–900 LoC) ✅ ERLEDIGT

*Ziel: Restliche Protokoll-Features aus der ursprünglichen Phase 3 abschließen. Zusätzlich alle in smithay 0.7.0 verfügbaren Protokolle verdrahten, die für App-Kompatibilität und flüssigen Betrieb sinnvoll sind. Der Compositor soll gängige GTK4/Qt/Chromium/Firefox-Apps ohne Protokoll-Warnungen unterstützen.*

> **Protokoll-Gap-Analyse (2026-03-05, aktualisiert 2026-03-10):** 43 implementierte Protokoll-Globals
> (37 `delegate_*!()`-Makros + 6 manuelle `GlobalDispatch`: pointer-warp-v1, tearing-control,
> toplevel-drag, toplevel-icon, toplevel-tag, virtual-pointer; plus wlr-foreign-toplevel,
> output-management, screencopy via eigene State-Inits).
> Tier 1 komplett (6 Protokolle: commit-timing, fifo, idle-inhibit, xdg-dialog, system-bell,
> alpha-modifier). Tier 2 komplett (5 Protokolle: xwayland-shell, xwayland-keyboard-grab,
> pointer-gestures, tablet-v2, pointer-warp-v1).
> tearing-control + toplevel-drag als Stubs implementiert (Step 19e).
> Tier 3 komplett (3 Protokolle: toplevel-icon mit Pixel-Rendering in SSD-Titlebars,
> toplevel-tag mit In-Memory-Speicherung, ext-foreign-toplevel-list via smithay delegate).
> ext-data-control-v1 implementiert (standardisierte Version parallel zu wlr-data-control).
> **EIS-Test-Client (Step 17b) komplett** — validiert gegen GNOME/Mutter. ~1.780 LoC (portal.rs + app.rs + main.rs).
> 13 CLI-Subcommands (inkl. `type-text`) + 14 interaktive Kommandos (inkl. `type-text`).
> `type-text` nutzt `platynui-xkb-util::KeymapLookup` für XKB-Reverse-Lookup + Compose-Support (Dead-Keys).
> Portal-Restore-Token (`persist_mode=2`) für dauerhafte Berechtigungen.
> reis-Bug Workaround (manueller EiEventConverter statt EiConvertEventIterator).
> Interaktiver REPL-Modus (reedline) mit Semikolon-getrennten Multi-Kommandos.
> Human-readable Key-Names (~80 Einträge: Buchstaben, Zahlen, F-Tasten, Navigation, Modifier, Sonderzeichen)
> + Shortcut-Syntax (`ctrl+a`, `alt+f4`, `ctrl+shift+delete`) mit korrekter Modifier-Sequenzierung.
> Touch-Kommandos (tap, touch-down, touch-move, touch-up) für Touchscreen-Capability.
> **`platynui-xkb-util` Crate (Step 17c)** — ~506 LoC, 9 Tests. XKB-Reverse-Lookup mit `xkbcommon` 0.9 C-Bindings.
> `KeymapLookup` (char→keycode+modifiers), `KeyAction` enum (Simple/Compose), Compose-Table-Support.
> **EIS-Server (Step 17) komplett** — ~370 LoC. Alle Input-Capabilities, XKB-Keymap-Propagation, Regions, Single-Client.
> **Performance-Optimierung:** Press/Release-Gap 20ms→2ms, Settle-Time 50ms→10ms, Modifier-Batching (~10× schneller).
> Erkenntnisse dokumentiert in `docs/eis-libei.md`.
> 3 Protokolle bewusst nicht implementiert (`drm-lease`, `drm-syncobj`, `kde-decoration`).
> ~14.500 LoC Compositor + ~1.780 LoC Test-Client + ~506 LoC xkb-util, 43 Protokolle, 1883 Tests.

**Bestehende Feature-Schritte (Reihenfolge: Test-Client zuerst, dann EIS-Server):**

> **Begründung der Reihenfolge:** Der Test-Client (17b) wird *vor* dem EIS-Server (17) implementiert. Damit können wir libei zuerst gegen existierende Compositors (Mutter/KWin) validieren — Handshake, Capabilities, Keymap, Input-Injection verstehen und debuggen — bevor wir unseren eigenen EIS-Server schreiben. Der Test-Client dient dann auch direkt als Testharness für Step 17.

17b. ✅ **Eigenständiger EIS-Test-Client** (`apps/eis-test-client/`): Separates Binary zum Testen und Debuggen von EIS-Servern — funktioniert mit Mutter (GNOME), KWin (KDE), und unserem Compositor:
    - **Crate:** `apps/eis-test-client/` mit eigenem `Cargo.toml`. Deps: `reis 0.6`, `enumflags2 0.7`, `rustix 1` (event), `clap 4`, `zbus 5` (blocking-api), `reedline 0.40`, `anyhow 1`, `tracing 0.1`, `tracing-subscriber 0.3`, `platynui-xkb-util` (XKB-Reverse-Lookup für `type-text`). Alle Dependencies sind `cfg(target_os = "linux")`-gated.
    - **Verbindungsmodi (3 Pfade):**
      - Portal: `--portal` → `org.freedesktop.portal.RemoteDesktop` (`CreateSession` → `SelectDevices` → `Start` → `ConnectToEIS`) via `zbus` blocking API. Portal-Berechtigungs-Dialog wird bei erstem Start angezeigt; danach wird ein **Restore-Token** gespeichert (`~/.local/share/platynui/eis-restore-token`) und bei nachfolgenden Starts automatisch verwendet (`persist_mode=2`, xdg-desktop-portal v2). Funktioniert mit GNOME 43+ und KDE Plasma 5.27+.
      - Direkt: `--socket <path>` → `ei::Context` über Unix-Socket (unser Compositor, Sway-Fork)
      - Env: Default verbindet zu `$LIBEI_SOCKET` (Pfad absolut oder relativ zu `$XDG_RUNTIME_DIR`)
    - **Kommandos (13 clap Subcommands):**
      - `probe` — Verbinden, Handshake durchführen, Connection-Info (Context-Type, Handshake-Serial, negotiated Interfaces), Seat/Capabilities/Regions/Keymap ausgeben und trennen. 500ms Grace-Period nach letztem Event. Diagnostik-Tool.
      - `move-to <x> <y>` — Absolute Pointer-Bewegung (erfordert `PointerAbsolute`-Capability — Mutter bietet das nicht an, klarer Fehler mit verfügbaren Devices)
      - `move-by <dx> <dy>` — Relative Pointer-Bewegung (✅ gegen Mutter getestet, funktioniert)
      - `click [left|right|middle]` — Button press + 2ms Pause + release (korrekte Press/Release-Semantik, `BTN_LEFT`=0x110, `BTN_RIGHT`=0x111, `BTN_MIDDLE`=0x112)
      - `scroll <dx> <dy>` — Scroll-Event
      - `key <spec>` — Key-Name, Shortcut oder raw Keycode. Human-readable Namen: `a`–`z`, `0`–`9`, `f1`–`f12`, `enter`, `escape`, `space`, `tab`, `backspace`, Pfeiltasten, Modifier, Sonderzeichen. Shortcuts: `ctrl+a`, `alt+f4`, `ctrl+shift+delete` — Modifier werden in Reihenfolge gedrückt, Taste gedrückt+losgelassen, Modifier in umgekehrter Reihenfolge losgelassen. Raw-Keycodes als Fallback (z.B. `30`).
      - `type-text <text>` — Beliebigen Unicode-Text tippen via XKB-Reverse-Lookup. Nutzt `platynui-xkb-util::KeymapLookup` für char→keycode+modifiers. Keymap-Quelle: Device-Keymap (vom EIS-Server) → `--keyboard-layout`/`--keyboard-variant` CLI-Flags → `XKB_DEFAULT_LAYOUT`/`XKB_DEFAULT_VARIANT` Env-Vars → `us`. Unterstützt drei Zeichenarten: (1) einfache Zeichen (`a`, `1`), (2) Shift/AltGr-Zeichen (`A`, `@`, `€`), (3) Compose-Sequenzen (`à` = dead_grave + a, `é` = dead_acute + e, `^` = dead_circumflex + Space). Modifier-Batching (alle Modifier in einem Frame) und 2ms Press/Release-Gap für hohe Performance (~10× schneller als initial).
      - `tap <x> <y>` — Touch-Tap (down + 2ms Pause + up) an Position, Touch-ID 0
      - `touch-down [--id N] <x> <y>` — Touchpoint an Position setzen (für Gesten)
      - `touch-move [--id N] <x> <y>` — Aktiven Touchpoint bewegen
      - `touch-up [--id N]` — Touchpoint loslassen
      - `interactive` — REPL-Modus: einmal verbinden, einmal Permission-Dialog bestätigen, dann beliebig viele Kommandos eingeben. `reedline`-basierter Zeileneditor (Cursor-Tasten, History, Ctrl+C/Ctrl+D). Unterstützt Semikolon-getrennte Mehrfach-Kommandos (`move-by 100 0; click; key ctrl+a`). 14 Kommandos: `move-by`, `move-to`, `click`, `scroll`, `key`, `type-text`, `tap`, `touch-down`, `touch-move`, `touch-up`, `keys` (verfügbare Key-Namen anzeigen), `probe`, `help`, `quit`.
      - `reset-token` — Gespeicherten Portal-Restore-Token löschen (erzwingt neuen Permission-Dialog beim nächsten Start)
    - **EI-Protokoll-Handshake:** Manueller `EiEventConverter` statt `EiConvertEventIterator` (Workaround für [reis Bug](https://github.com/ids1024/reis): Iterator ruft `poll_readable()` vor dem Drain bereits gepufferter Events auf → hängt). Context-Type: Sender. `BitFlags::all()` für Seat-Binding (Mutter erstellt kein Device wenn nur eine einzelne Capability gebunden wird). 5s Timeout für Device-Erkennung.
    - **Key-Name-System:** `KEY_MAP` mit ~80 Einträgen (Buchstaben a–z, Zahlen 0–9, F1–F12, Navigation, Editing, Modifier, Sonderzeichen, Lock-Tasten, Misc). `MODIFIER_NAMES` mit allen Modifier-Aliassen (ctrl/control/lctrl/rctrl/leftctrl, shift/lshift/rshift, alt/altgr/ralt, super/meta/win). `key_name_to_code()` mit Raw-Number-Fallback. `parse_key_spec()` für `+`-getrennte Shortcuts → `(modifier_codes[], key_code)`. `send_key_combo()`: Modifier down in Reihenfolge → Key down → Key up → Modifier up in umgekehrter Reihenfolge, mit Modifier-Batching (alle Modifier in einem Frame) und 2ms Press/Release-Gap. `print_key_names()` gruppierte Anzeige (Letters, Numbers, F-Keys, Navigation, Editing, Modifiers, Other, Symbols).
    - **type-text / XKB-Reverse-Lookup:** `send_text()` nutzt `platynui-xkb-util::KeymapLookup` um pro Zeichen den passenden `KeyAction` nachzuschlagen. `KeymapLookup` wird aus der EIS-Device-Keymap (Server-Propagation), CLI-Flags (`--keyboard-layout`/`--keyboard-variant`), Env-Vars (`XKB_DEFAULT_LAYOUT`/`XKB_DEFAULT_VARIANT`) oder Fallback `us` gebaut. Je nach `KeyAction`-Variante:
      - `KeyAction::Simple(combo)` — Einzelne Taste mit optionalen Modifiern (z.B. `a`, `A` = Shift+a, `@` = AltGr+q auf `de`)
      - `KeyAction::Compose { dead_key, base_key }` — Zwei-Schritt Compose-Sequenz: Dead-Key drücken+loslassen, dann Base-Key drücken+loslassen (z.B. `à` = dead_grave + a, `é` = dead_acute + e, `^` = dead_circumflex + Space)
      Modifier-Batching: alle Modifier eines Zeichens werden in einem einzigen Frame gedrückt, dann Key press/release, dann alle Modifier in umgekehrter Reihenfolge in einem Frame losgelassen. 2ms Gap zwischen jedem Press/Release-Paar. Am Ende 10ms Settle-Time.
    - **Touch-Unterstützung:** `send_touch_tap()`: `down(touchid, x, y)` → frame → flush → 2ms → `up(touchid)` → frame → `stop_emulating` → flush → 10ms. Separate Frames sind EI-Protokoll-Pflicht für Touch down/motion/up. `parse_touch_args()` für interaktiven Modus (optionale Touch-ID, Default 0). Vier Touch-Kommandos: tap, touch-down, touch-move, touch-up.
    - **Press/Release-Semantik:** `send_press_release()`: `start_emulating` → press → frame → flush → 2ms Pause → release → frame → `stop_emulating` → flush → 10ms Settle-Time. `send_key_combo()` mit Modifier-Batching: alle Modifier in einem Frame press, Key press/release, alle Modifier in einem Frame release → minimal nötige Frames. Timing optimiert von initial 20ms/50ms auf 2ms/10ms (~10× schneller) — ausreichend damit Compositors die Events als separate Aktionen registrieren.
    - **Tracing:** Log-Level via `--log-level` (error/warn/info/debug/trace), `RUST_LOG` Env-Var (Vorrang), oder `PLATYNUI_LOG_LEVEL` Env-Var. Output auf stderr.
    - **Architektur:** 3 Quelldateien:
      - `main.rs` — Linux-Gate (`#[cfg(not(target_os = "linux"))]` Compile-Error) (~24 LoC)
      - `portal.rs` — XDG Desktop Portal D-Bus-Integration (zbus-generierte Proxies, `connect_via_portal()` mit Restore-Token-Support) (~204 LoC)
      - `app.rs` — CLI-Parsing, Tracing-Init, EI-Handshake, Key-Name-System, XKB-Reverse-Lookup (via `platynui-xkb-util`), Touch-Support, alle Kommando-Implementierungen (~1.553 LoC)
    (~1.780 LoC gesamt)

17c. ✅ **XKB-Reverse-Lookup Crate** (`crates/xkb-util/`): Eigenständiges Crate `platynui-xkb-util` für XKB-basierten Reverse-Lookup (Zeichen → Keycode + Modifiers). Wird vom EIS-Test-Client (Step 17b, `type-text`) direkt genutzt und ist für das zukünftige Platform-Crate (Phase 4) vorbereitet:
    - **Crate:** `crates/xkb-util/` mit eigenem `Cargo.toml`. Deps: `xkbcommon = "0.9"` (C-Bindings via `xkbcommon-sys`, `cfg(target_os = "linux")`-gated), `tracing = "0.1"`. Modul und Re-Exports sind `#[cfg(target_os = "linux")]`-gated — Crate kompiliert als leeres Crate auf Windows/macOS.
    - **`KeyAction` enum:** Zentrale Abstraktion für die Eingabe eines Zeichens:
      - `KeyAction::Simple(KeyCombination)` — Einzelne Taste mit optionalen Modifiern (Shift, AltGr etc.)
      - `KeyAction::Compose { dead_key: KeyCombination, base_key: KeyCombination }` — Zwei-Schritt Compose-Sequenz (Dead-Key + Base-Key, z.B. `dead_grave` + `a` → `à`)
    - **`KeyCombination`:** `{ keycode: u32, modifiers: u32 }` — evdev-Keycode + Modifier-Bitmaske
    - **`KeymapLookup`:** Hauptstruktur — baut aus einem `xkb::Keymap` eine `HashMap<char, KeyAction>` auf:
      - Phase 1: Level-Iteration über alle Keycodes × Levels. Pro Keysym → Modifier-Kombination bestimmen (`modifier_bit()` für Shift/AltGr/CapsLock). `xkb::keysym_to_utf32()` für char-Konvertierung. Filter: nur druckbare Zeichen, keine Modifier/Control/Dead-Keys.
      - Phase 2: Compose-Table (`xkb::compose::Table` + `xkb::compose::State`): Dead-Keys aus Level-Iteration als Compose-Starter sammeln, dann gegen `xkb::compose::State::feed()` testen. Ergebnis: `KeyAction::Compose` für Zeichen die nur via Dead-Key + Base-Key erreichbar sind (z.B. `é`, `à`, `^`).
    - **Konstruktoren:**
      - `KeymapLookup::new(keymap)` — Aus existierendem `xkb::Keymap` (z.B. vom EIS-Server propagiert)
      - `KeymapLookup::from_string(keymap_string)` — Aus XKB-Keymap-String (z.B. von `xkb_keymap_get_as_string()`)
    - **API:** `lookup(char) → Option<&KeyAction>`, `len()`, `is_empty()`, `iter()`, `evdev_keycode()`, `modifier_names()`, `mod_index_to_bit()`
    - **Tests:** 9 Tests (Default-Layout, Compose-Sequenzen, Modifier-Überprüfung, Edge-Cases)
    (~506 LoC, 2 Quelldateien: `reverse_lookup.rs` + `lib.rs`)

17. ✅ **EIS-Server / libei** (`src/eis.rs`): Via `reis::eis` (Feature `calloop`) — vollständiger EIS-Server im Compositor. Erfahrungen aus Step 17b (Test-Client gegen Mutter/KWin) flossen direkt ein:
    - **Socket:** `$XDG_RUNTIME_DIR/eis-platynui`, `eis::Listener::bind()` + `EisListenerSource` in calloop Event-Loop. Stale-Socket-Cleanup bei Start.
    - **Handshake:** `EisRequestSourceEvent::Connected` → `handle_eis_connected()` mit Seat + Capabilities + Device-Lifecycle.
    - **Seat + Capabilities:** Ein Seat mit allen Input-Capabilities: `ei_pointer` (relativ), `ei_pointer_absolute` (absolut mit Regions), `ei_button`, `ei_scroll`, `ei_keyboard`, `ei_touchscreen`.
    - **Device-Lifecycle:** Device erstellen → `done` → `resumed` (sofort, kein Pause-Grund im Test-Compositor). Minimal-Pause/Resume: Protokoll-Pflicht erfüllt, aber nie aktiv pausiert.
    - **XKB-Keymap-Propagation:** Bei Keyboard-Capability die aktive Smithay-Keymap als tempfile exportieren und per `ei_keyboard.keymap(fd, size)` an Client senden. Clients können daraus den XKB-Reverse-Lookup (char → keycode) aufbauen.
    - **Regions:** Für `ei_pointer_absolute` — eine Region pro Output mit korrektem Offset/Size/Scale. Mappt Client-Koordinaten auf Smithay-globale Koordinaten.
    - **Input-Injection:** Empfangene `KeyboardKey`, `PointerMotion`, `PointerMotionAbsolute`, `Button`, `ScrollDelta`, `ScrollDiscrete`, `ScrollStop`, `TouchDown`/`TouchMotion`/`TouchUp` Events werden in Smithay-Input-Stack injiziert (keyboard via `keyboard.input()` mit FilterResult, pointer via `pointer.motion()`/`pointer.button()`/`pointer.axis()`).
    - **Single-Client:** Ein gleichzeitiger Client. Bei neuer Verbindung wird der vorherige Client disconnected (Device removed, Event-Source entfernt).
    - **Deps:** `reis = { version = "0.6", features = ["calloop"] }` in `apps/wayland-compositor/Cargo.toml`
    - **Testbarkeit:** Der Test-Client aus Step 17b dient als primäres Testmittel — `eis-test-client --socket $XDG_RUNTIME_DIR/eis-platynui probe` validiert den Handshake. `eis-test-client type-text "Hello"` validiert XKB-Keymap-Propagation + Input-Injection.
    Enables: Input-Injection über libei im Platform-Crate, Ökosystem-kompatibel mit Mutter/KWin. (~370 LoC)

19d. ~~*(Optional)* **Legacy-Screencopy**~~ — Übersprungen. `ext-image-copy-capture` deckt alle benötigten Tools ab (wayvnc, grim aktuelle Versionen). `wlr-screencopy-v1` wird nicht implementiert.

19e. ✅ **App-Kompatibilitäts-Stubs** (`src/handlers/tearing_control.rs`, `src/handlers/toplevel_drag.rs`): Manuelle `GlobalDispatch`/`Dispatch`-Implementierungen (smithay 0.7 hat keine High-Level-Abstraktion) für Protokolle die viele Apps abfragen:
    - ✅ `wp-tearing-control-v1` — Tearing-Hint für Games. No-Op-Stub: `set_presentation_hint` wird akzeptiert aber ignoriert (Compositor nutzt immer vsync). Verhindert Protokoll-Warnungen bei Chromium/Games. (~100 LoC)
    - ✅ `xdg-toplevel-drag-v1` — Tab-Detach in Browsern (Firefox/Chromium), Drag-aus-Fenster. Stub: `attach` wird akzeptiert und geloggt, aber die Window-during-Drag-Logik ist noch nicht implementiert. (~105 LoC)

**Tier 1 — Triviale Delegates mit hohem App-Kompatibilitäts-Nutzen** (~90 LoC) ✅:

19e₂. ✅ **`wp-commit-timing-v1`** (`delegate_commit_timing!()`): Frame-perfect Timing — Client sendet Timestamp wann der nächste Commit sichtbar sein soll. GTK4 und Mesa nutzen es für flüssige Animationen. Companion zu `fifo-v1`. Smithay liefert `CommitTimingManagerState`. (~15 LoC)

19e₃. ✅ **`wp-fifo-v1`** (`delegate_fifo!()`): FIFO-Scheduling — Compositor blocked den Client bis der vorherige Frame tatsächlich auf dem Display ist. Verhindert Frame-Drops bei vsync-sensitiven Apps. GTK4 nutzt es. Smithay liefert `FifoManagerState`. (~15 LoC)

19e₄. ✅ **`zwp-idle-inhibit-v1`** (`delegate_idle_inhibit!()`): Video-Player und Präsentations-Apps verhindern Screensaver/DPMS. Fast jede Media-App fragt es ab. Smithay liefert `IdleInhibitManagerState`. Handler trackt aktive Inhibitoren in `HashSet<WlSurface>` auf `State` und ruft `set_is_inhibited()` auf `IdleNotifierState` — bei `inhibit()` wird `true` gesetzt, bei `uninhibit()` nur `false` wenn kein Inhibitor mehr aktiv ist. Zusätzlich ruft `process_input_event()` jetzt `notify_activity()` auf, damit Idle-Timer bei jeder Benutzeraktion zurückgesetzt werden. (~30 LoC)

19e₅. ✅ **`xdg-dialog-v1`** (`delegate_xdg_dialog!()`): Modale Dialoge — Client signalisiert einem Toplevel dass es modal zu einem anderen ist. Compositor erzwingt korrektes Stacking: `find_modal_child()` auf `State` sucht rekursiv modale Kinder, `focus_and_raise()` in `input.rs` leitet Fokus auf das modale Kind um. SSD-Aktionen (Resize, Close, Maximize, Minimize) auf dem Elternfenster werden blockiert wenn ein modaler Dialog offen ist. Bei `modal_changed(true)` wird der Dialog sofort angehoben und fokussiert. Auch `activate_window()` (foreign-toplevel/Taskbar) respektiert die modale Kette. (~80 LoC)

19e₆. ✅ **`xdg-system-bell-v1`** (`delegate_xdg_system_bell!()`): System-Bell-Notification. Trivial, kein State nötig — einfach das Event loggen. Terminal-Emulatoren und viele GTK-Apps nutzen es. (~10 LoC)

19e₇. ✅ **`wp-alpha-modifier-v1`** (`delegate_alpha_modifier!()`): Subsurface-Opacity — Client kann die Transparenz einzelner Subsurfaces steuern ohne Alpha im Buffer anzupassen. Manche Compositing-Szenarien brauchen es. Smithay liefert `AlphaModifierState`. (~15 LoC)

**Tier 2 — Moderate Protokoll-Erweiterungen** (~110 LoC) ✅:

19e₈. ✅ **`xwayland-shell-v1`** (`delegate_xwayland_shell!()`): Bereits in Phase 3 implementiert. Besseres Surface-Mapping zwischen X11-Windows und Wayland-Surfaces. Lazy-Init zusammen mit XWayland-Start. Handler in `xwayland.rs` mit `XWaylandShellHandler::xwayland_shell_state()` + `surface_associated()`. (~30 LoC)

19e₉. ✅ **`xwayland-keyboard-grab`** (`delegate_xwayland_keyboard_grab!()`): Erlaubt X11-Apps exklusive Keyboard-Grabs (Shortcuts, VMs). Lazy-Init zusammen mit XWayland-Start. `XWaylandKeyboardGrabHandler::keyboard_focus_for_xsurface()` sucht in `space.elements()` das Window dessen X11-Surface die angefragte `WlSurface` hat und gibt es als `KeyboardFocusTarget` zurück. (~15 LoC)

19e₁₀. ✅ **`pointer-gestures-v1`** (`delegate_pointer_gestures!()`): Touchpad-Gesten (Swipe, Pinch, Hold) an Clients weiterleiten. Delegate-only, kein Handler-Trait — smithay routet Gesten-Events über `PointerHandle` automatisch. Smithay liefert `PointerGesturesState`. (~5 LoC)

19e₁₁. ✅ **`tablet-v2`** (`delegate_tablet_manager!()`): Drawing-Tablet-Unterstützung (Wacom etc.) — Pressure, Tilt, Button-Events. `TabletSeatHandler` war bereits für `cursor-shape` implementiert (leerer Default-Impl). State-Init + Delegate-Makro ergänzt. (~5 LoC)

19e₁₂. ✅ **`pointer-warp-v1`** (manuelles `GlobalDispatch`/`Dispatch`): Client-requested Pointer-Warping — Accessibility-Tools, Remote-Desktop und App-Drag-Operationen können den Mauszeiger auf eine Surface-relative Position bewegen. Smithay 0.7 bietet noch keine High-Level-Abstraktion, daher manuelle Implementierung über `wayland-protocols 0.32` Bindings (`wp_pointer_warp_v1`). Handler rechnet Surface-lokale in globale Koordinaten um und sendet Motion-Event über `PointerHandle`. Security-Policy-Filter via `can_view`. (~120 LoC)

**Tier 3 — Niedrig / Optional** (~370 LoC) ✅:

19e₁₃. ✅ **`xdg-toplevel-icon-v1`** (manuelles `GlobalDispatch`/`Dispatch`, `src/handlers/toplevel_icon.rs`): Custom Window-Icons — Clients setzen per SHM-Buffer Pixel-Icons für Toplevel-Surfaces. Volle Implementierung: `IconBuilder` akkumuliert `set_name`/`add_buffer`, behält den größten Buffer. `read_icon_buffer()` liest ARGB8888-SHM-Buffer und konvertiert zu RGBA. Icons werden in `state.toplevel_icons` (`HashMap<ObjectId, ToplevelIconPixels>`) gespeichert und als 16×16 egui-Textur links vom Titel in SSD-Titlebars gerendert. Named Icons (XDG Icon Theme) werden geloggt aber nicht aufgelöst (kein Theme-Loader). (~245 LoC)

19e₁₄. ✅ **`xdg-toplevel-tag-v1`** (manuelles `GlobalDispatch`/`Dispatch`, `src/handlers/toplevel_tag.rs`): Persistent Toplevel Identification — Clients setzen untranslated Tags (z.B. `"main window"`, `"settings"`) und translated Descriptions für Toplevels. Dient als Identifizierungsmechanismus damit Compositors Window-Eigenschaften (Position, Größe, Regeln) über Session-Restarts hinweg anwenden können. Tags und Descriptions werden pro Toplevel in `state.toplevel_tags` (`HashMap<ObjectId, ToplevelTagInfo>`) in-memory gespeichert (keine Persistierung — Test-Compositor). (~106 LoC)

19e₁₅. ✅ **`ext-foreign-toplevel-list-v1`** (`delegate_foreign_toplevel_list!()`, bereits in Phase 3 implementiert): Ext-Version der Foreign-Toplevel-Liste — read-only (keine activate/close/minimize), ergänzt `wlr-foreign-toplevel-management-v1`. Handles in `state.ext_toplevel_handles` mit diffbasierter Title/App-ID-Weiterleitung bei surface-commits. (~15 LoC)

**Bewusst nicht implementiert:**
- `drm-lease-v1` — VR-Headset-Lease, nicht relevant für UI-Automation
- `drm-syncobj-v1` — Explicit GPU sync, Hardware-nah, nicht relevant
- `kde-decoration` — KDE-spezifisch, wir nutzen `xdg-decoration`

19e₁₆. ✅ **`ext-data-control-v1`** (`delegate_ext_data_control!()`, `src/handlers/data_control.rs`): Standardisierte Clipboard-Kontrolle — funktional identisch zu `wlr-data-control-v1`, aber als offizielle Staging-Version in wayland-protocols. Mutter und `KWin` implementieren diese Version. Beide Protokolle werden parallel angeboten. Smithay liefert `ext_data_control::DataControlState` + `DataControlHandler`. (~15 LoC)

> **Hinweis wayvnc:** `wayvnc` funktioniert bereits als externer VNC-Server (braucht `wlr-layer-shell` + `ext-image-copy-capture` + `zwlr_virtual_pointer` — alle in Phase 3 abgeschlossen). Befehl: `WAYLAND_DISPLAY=... wayvnc 0.0.0.0 5900`. Die verbleibenden Steps (EIS, Legacy-Screencopy, Stubs) sind Erweiterungen, keine Voraussetzung für wayvnc.

**Meilenstein 3b (Zwischenstand):** ✅ Tier 1 komplett (commit-timing, fifo, idle-inhibit, xdg-dialog, system-bell, alpha-modifier). ✅ Tier 2 komplett (xwayland-shell, xwayland-keyboard-grab, pointer-gestures, tablet-v2, pointer-warp-v1). ✅ tearing-control + toplevel-drag als Stubs (manuelles GlobalDispatch/Dispatch, smithay 0.7 bietet keine Abstraktion). ✅ Tier 3 komplett (toplevel-icon, toplevel-tag, ext-foreign-toplevel-list). ✅ ext-data-control-v1 (standardisierte Clipboard-Kontrolle). ✅ EIS-Test-Client (Step 17b): eigenständiges Binary `platynui-eis-test-client` mit Portal-Support (GNOME/KDE), Restore-Token-Persistenz, 13 Subcommands (probe, move-to, move-by, click, scroll, key, type-text, tap, touch-down, touch-move, touch-up, interactive, reset-token), Human-readable Key-Names (~80 Einträge) + Shortcut-Syntax (ctrl+a, alt+f4), **type-text mit XKB-Reverse-Lookup + Compose-Support** (Dead-Keys, `KeyAction::Simple` + `KeyAction::Compose`), Touch-Unterstützung (tap + Gesten-Primitives), interaktiver REPL-Modus (reedline, 14 Kommandos inkl. type-text), reis-Bug-Workaround, ~1.780 LoC. ✅ `platynui-xkb-util` Crate (~506 LoC, 9 Tests): XKB-Reverse-Lookup mit `xkbcommon` 0.9 C-Bindings, `KeymapLookup` (char→keycode+modifiers), `KeyAction` enum (Simple/Compose), Compose-Table-Support. ✅ EIS-Server (Step 17, ~370 LoC): Vollständiger EIS-Server mit allen Input-Capabilities, XKB-Keymap-Propagation, Regions, Single-Client. Erkenntnisse dokumentiert in `docs/eis-libei.md`. ✅ Performance-Optimierung: Press/Release-Gap 20ms→2ms, Settle-Time 50ms→10ms, Modifier-Batching (~10× schneller). ✅ 43 Protokoll-Globals, ~14.500 LoC Compositor + ~1.780 LoC Test-Client + ~506 LoC xkb-util, 1883 Tests. Alle gängigen GTK4/Qt/Chromium-Protokolle werden unterstützt — keine Protokoll-Warnungen bei Standard-Apps.

**Meilenstein 3b (Ziel): ✅ ERREICHT.** libei-Input funktioniert (Step 17) — EIS-Server akzeptiert Clients, alle Input-Capabilities (pointer, pointer_absolute, button, scroll, keyboard, touchscreen) werden advertisiert und in Smithay injiziert. XKB-Keymap wird an Clients propagiert. ✅ Eigenständiger Test-Client (Step 17b) kann sich per Socket (eigener Compositor) oder Portal (Mutter/KWin) verbinden und Input emulieren — Human-readable Key-Names, Shortcut-Syntax, **type-text mit XKB-Reverse-Lookup + Compose-Support** (Dead-Keys für `à`, `é`, `^` etc.), Touch-Kommandos, interaktiver REPL-Modus (14 Kommandos). `platynui-xkb-util` Crate liefert den XKB-Reverse-Lookup. `ei-debug-events` zeigt korrekten Handshake und Device-Konfiguration. `WAYLAND_DISPLAY=... platynui-cli query "//control:*"` (über wlr-foreign-toplevel) listet Fenster.

---

### Phase 3c: Desktop-Integration & Projekt-Tooling ✅ ERLEDIGT

*Ziel: Winit-Backend-Fenster korrekt in GNOME/KDE integriert (Titel, CSD-Theme, Dark Mode, Icon, App-ID). Einheitliche `org.platynui.*` App-IDs über alle Binaries. Freedesktop `.desktop`-Dateien für Icon-Auflösung. Justfile als Projekt-Task-Runner.*

> **Status (2026-03-05):** Alle Steps abgeschlossen. Build/Clippy/27 Tests sauber.

**Winit-Fenster-Verbesserungen:**

19aj. ✅ **Winit-Fenster-Anpassung** (`src/backend/winit.rs`, `Cargo.toml`): Smithays `winit::init()` setzt hartcodiert `.with_title("Smithay")` und 1280×800. Umstellung auf `winit::init_from_attributes()` mit vollständiger `WindowAttributes`-Kette:
  - `.with_title("PlatynUI Wayland Compositor")` — korrekter Fenstertitel
  - `.with_name("org.platynui.compositor", "platynui-wayland-compositor")` — Wayland `app_id` auf `xdg_toplevel`, damit GNOME/KDE das Fenster einer `.desktop`-Datei zuordnen können
  - `.with_theme(detect_system_theme())` — System-Dark/Light-Mode respektieren
  - `.with_window_icon(load_icon())` — eingebettetes PNG-Icon (funktioniert auf X11, No-Op auf Wayland)
  (~30 LoC)

19ak. ✅ **Adwaita CSD-Theming** (`Cargo.toml`, `src/lib.rs`): Smithay aktiviert winit mit `default-features = false`, daher fehlt `wayland-csd-adwaita` — CSD-Titlebar ist hässlicher Fallback. Fix: direkte `winit = { version = "0.30", features = ["wayland-csd-adwaita"] }` Dependency für Cargo Feature-Unification. `use winit as _;` in lib.rs für `unused-crate-dependencies`-Lint. Ergebnis: sctk-adwaita 0.10.1 rendert Adwaita-konforme CSD. (~5 LoC)

19al. ✅ **System-Theme-Erkennung via zbus** (`src/backend/winit.rs`, `Cargo.toml`, `src/main.rs`): sctk-adwaita's eingebaute Auto-Detection nutzt `dbus-send` mit 100ms Timeout, der unter GNOME oft still fehlschlägt → Fallback auf Light-Theme. Eigene `detect_system_theme()` über zbus (blocking API):
  - `zbus::blocking::Connection::session()` → `call_method("org.freedesktop.portal.Desktop", ..., "Read", &("org.freedesktop.appearance", "color-scheme"))` → Deserialisierung `v v u` → `1=Dark, 2=Light, _=None`
  - Dependency: `zbus = { version = "5", features = ["blocking-api"] }` (bereits im Projekt via provider-atspi).
  (~40 LoC)

19am. ✅ **Eingebettetes Window-Icon** (`src/backend/winit.rs`, `apps/wayland-compositor/assets/icon.png`): `load_icon()` — `include_bytes!("../../assets/icon.png")` → `png::Decoder` → `Icon::from_rgba()`. Icon ist 256×256 RGBA PNG (kopiert von Inspector). Unter Wayland ist `set_window_icon()` ein No-Op — Desktop-Environments lösen Icons über `app_id` + `.desktop`-Datei auf. (~20 LoC)

**App-ID-Standardisierung:**

19an. ✅ **Einheitliche `org.platynui.*` App-IDs** (3 Binaries):
  - Compositor: `org.platynui.compositor` (in `winit.rs` via `.with_name()`)
  - Inspector: `org.platynui.inspector` (in `apps/inspector/src/lib.rs` via `.with_app_id()`)
  - Test-App: `org.platynui.test.egui` (in `apps/test-app-egui/src/main.rs`, Default-Wert des `--app-id` CLI-Flags)
  (~10 LoC)

**Desktop-Dateien:**

19ao. ✅ **Freedesktop `.desktop`-Dateien** (`assets/org.platynui.compositor.desktop`, `assets/org.platynui.inspector.desktop`): Freedesktop-konforme Desktop-Entries für beide Binaries. Compositor: `NoDisplay=true` (Entwickler-Tool, nicht im App-Menü). Inspector: `StartupNotify=true`, `Categories=Development`. Icon-Referenzen (`Icon=org.platynui.compositor` / `org.platynui.inspector`) zeigen auf hicolor-Theme-Icons die per `just install-desktop` installiert werden. (~20 Zeilen)

**Projekt-Tooling:**

19ap. ✅ **Justfile** (`justfile`, `docs/development.md`, `CONTRIBUTING.md`): `just` als Projekt-Task-Runner eingeführt. Recipes:
  - **Bootstrap:** `just bootstrap` (uv sync)
  - **Build:** `just build`, `just build-native [features]`, `just build-cli`, `just build-inspector`
  - **Check:** `just fmt`, `just fmt-check`, `just clippy`, `just ruff`, `just mypy`, `just check`
  - **Test:** `just test`, `just test-crate <crate>`, `just test-python`
  - **Desktop-Integration:** `just install-desktop` (`.desktop` + Icons → `$XDG_DATA_HOME`), `just uninstall-desktop`, `just update-icon-cache`
  - **CI:** `just pre-commit` (bootstrap + fmt + build + clippy + test + ruff)
  - `docs/development.md` dokumentiert alle Recipes mit Installationsanleitung
  - `CONTRIBUTING.md` verweist auf `just` und `docs/development.md`
  (~113 Zeilen justfile, ~95 Zeilen docs/development.md)

**Meilenstein 3c:** ✅ Winit-Fenster zeigt korrekten Titel, Adwaita-CSD, System-Theme und Icon. Alle Binaries nutzen `org.platynui.*` App-IDs. `.desktop`-Dateien für KDE/GNOME-Icon-Auflösung. `just install-desktop` installiert Desktop-Dateien und Icons nach `$XDG_DATA_HOME`. Justfile als zentraler Task-Runner mit Doku.

---

### Phase 3d: Touch-Input & SSD-Touch-Interaktion (~385 LoC) ✅ ERLEDIGT

*Ziel: Vollständige Touchscreen-Unterstützung im Compositor — Touch-Events werden korrekt an Clients weitergeleitet, SSD-Fensterdekorationen (Move, Resize, Close/Maximize/Minimize-Buttons) reagieren auf Touch-Gesten mit dem gleichen Verhalten wie Pointer-Input. Multi-Slot-Isolation und korrekte Koordinaten-Transformation über alle Input-Quellen (Backend + EIS).*

> **Status (2026-03-06):** Alle Steps abgeschlossen. Clippy clean, 27 Tests sauber.

**Touch-Grundlagen:**

19aq. ✅ **Touch-Capability am Seat** (`src/state.rs`): `seat.add_touch()` in `State::new()` direkt nach `add_keyboard()` und `add_pointer()`. Helper-Methode `touch() → TouchHandle<Self>` analog zu `pointer()`. (~8 LoC)

19ar. ✅ **Backend-Touch-Handler** (`src/input.rs`): `handle_touch_down()`, `handle_touch_motion()`, `handle_touch_up()` — generische Handler für `InputBackend`-Events. Koordinaten-Transformation über `combined_output_geometry()` (gleich wie `handle_pointer_motion_absolute`) — Touch-Events sind absolut und umspannen alle Monitore, unabhängig von der Mausposition. (~24 LoC)

19as. ✅ **EIS-Touch-Handler** (`src/eis.rs`): `handle_eis_touch_down()`, `handle_eis_touch_motion()`, `handle_eis_touch_up()` — delegieren an die gleichen `process_touch_*()` Shared-Funktionen wie die Backend-Handler. `TouchSlot` wird aus `touch_id` konstruiert. Einheitliches Verhalten für Backend und libei-Input. (~26 LoC)

19at. ✅ **Shared Touch-Processing** (`src/input.rs`): `process_touch_down()`, `process_touch_motion()`, `process_touch_up()` als `pub(crate)` Funktionen — zentrale Touch-Logik, genutzt von Backend- und EIS-Handlern. (~129 LoC)
  - `process_touch_down()` (~83 LoC): Hit-Test via `surface_under_point()` → Client-Surface (Keyboard-Fokus setzen + `touch.down()`), SSD-Resize-Region (`handle_touch_resize_request()`), SSD-Titlebar-Buttons (Close/Max/Min → deferred in `touch_ssd_button`), SSD-Titlebar (`handle_touch_move_request()`), leerer Bereich (`touch.down(focus=None)`).
  - `process_touch_motion()` (~19 LoC): Aktualisiert Position in `touch_ssd_button` falls Slot übereinstimmt. Hit-Test + `touch.motion()`.
  - `process_touch_up()` (~27 LoC): Prüft ob Slot zum deferred SSD-Button passt → nur der initiierende Finger löst die Button-Aktion aus. Hit-Test gegen `titlebar_button_hit_test()` verifiziert dass der Finger noch über dem gleichen Button ist (analog zu Pointer-Release-Verhalten). Sonst `touch.up()`.

19au. ✅ **`surface_under_point()` Refactoring** (`src/input.rs`): Zentraler Hit-Test extrahiert als `pub(crate)` Funktion — Overlay/Top-Layer → Windows (SSD-aware) → Bottom/Background-Layer. Subsurface/Popup/Toplevel-Auflösung. Wird von allen Touch- und Pointer-Handlern genutzt. (~50 LoC)

**SSD-Touch-Grabs:**

19av. ✅ **`TouchMoveSurfaceGrab`** (`src/grabs.rs`): Touch-Move-Grab für SSD-Titelleisten-Drag — analog zu `MoveSurfaceGrab` für Pointer. (~100 LoC)
  - Tracking nur des initiierenden Slots (zusätzliche Touch-Punkte werden ignoriert)
  - **Inkrementelle Deltas mit Re-Anchoring**: `start_data.location` wird nach jedem Frame auf `event.location` aktualisiert → Frame-zu-Frame-Delta statt kumulativer Drift vom Grab-Start. Verhindert Fenstersprünge bei Rückkehr aus Dead-Zones in L-förmigen Multi-Monitor-Setups.
  - **Dual-Output-Check**: Fenster wird nur bewegt wenn sowohl aktuelle als auch vorherige Position auf einem gültigen Output liegen (`on_output && was_on_output`). Analoges Muster zum Pointer-`MoveSurfaceGrab`.
  - X11-Client-Konfiguration bei Grab-Ende (`up()` + `unset()`).

19aw. ✅ **`TouchResizeSurfaceGrab`** (`src/grabs.rs`): Touch-Resize-Grab für SSD-Resize-Regionen — analog zu `ResizeSurfaceGrab` für Pointer. (~141 LoC)
  - Tracking nur des initiierenden Slots
  - Alle 12 Resize-Richtungen (8 Ecken + 4 Kanten) via `Focus`-Enum-Matching
  - Minimum-Size-Constraints
  - `Resizing`-State auf `xdg_toplevel` setzen/entfernen
  - X11-Surface-Konfiguration mit finalem `Rectangle`

19ax. ✅ **Touch-Grab-Einstiegspunkte** (`src/grabs.rs`): `handle_touch_move_request()` und `handle_touch_resize_request()` — erstellen `TouchGrabStartData` und setzen den Grab via `touch.set_grab()`. (~34 LoC)

**Deferred SSD-Button-Aktionen:**

19ay. ✅ **`touch_ssd_button` State** (`src/state.rs`): `Option<(Window, DecorationClick, TouchSlot, Point<f64, Logical>)>` — speichert bei Touch-Down auf SSD-Buttons: Fenster, Button-Typ, initiierenden Slot und kontinuierlich aktualisierte Finger-Position. Button-Aktion wird erst bei Touch-Up ausgelöst (nach Slot- und Position-Verifikation). Analoges Pattern zu `pressed_titlebar_button` für Pointer. (~4 LoC)

**Meilenstein 3d:** ✅ Touch-Events werden korrekt an Wayland-Clients weitergeleitet. SSD-Fensterdekorationen reagieren auf Touch: Titelleisten-Drag (Move), Rand-Drag (Resize), Close/Maximize/Minimize-Buttons mit deferred Action + Slot-Verifikation. Multi-Slot-Isolation: nur der initiierende Finger steuert Grabs und Button-Aktionen. Koordinaten-Transformation korrekt über alle Input-Quellen. Dead-Zone-Schutz bei Multi-Monitor-Move. ~385 LoC über 4 Dateien (input.rs, grabs.rs, eis.rs, state.rs).

---

### Phase 3e: Platform-Linux Mediator (`crates/platform-linux/`, ~510 LoC) ✅ ERLEDIGT

> **Status (2026-03-08):** Komplett implementiert. Session-Erkennung, delegierender Mediator mit `Resolved`-Struct (einmalige Backend-Auflösung in `initialize()`, 7 `&'static dyn Trait`-Referenzen gecacht in `Mutex<Option<Resolved>>`). Wayland-Sessions nutzen jetzt die Wayland-Platform-Backends (`platform-linux-wayland`), X11-Sessions die X11-Backends — automatische Auswahl via `SessionType`-Match in `initialize()`. Sub-Platforms als Libraries ohne Selbstregistrierung. Alle Consumers (CLI, Inspector, Python Bindings, Link, Playground) auf `platynui-platform-linux` umgestellt. Build/Clippy/Fmt clean, 1902 Tests grün (inkl. 16 Session-Detection-Tests). Dokumentation in `docs/architecture.md` §3 und `docs/platform-linux.md` §0 aktualisiert.

*Ziel: Ein delegierendes `platform-linux` Crate erkennt zur Laufzeit ob X11 oder Wayland und leitet alle Platform-Trait-Aufrufe an das richtige Sub-Platform-Crate weiter. Die Runtime und das Core-Crate bleiben unverändert.*

**Problem:** `initialize_platform_modules()` in der Runtime iteriert **alle** per `inventory` registrierten Module und bricht beim ersten Fehler ab (`?`-Operator). Geräte werden via `pointer_devices().next()` ausgewählt — einfach das erste registrierte. Wenn sowohl `platform-linux-x11` als auch `platform-linux-wayland` sich selbst registrieren, würde auf einer reinen Wayland-Session das X11-Modul beim `initialize()` fehlschlagen (kein `$DISPLAY`), und umgekehrt.

**Lösung: Delegierender Mediator mit ZST-Delegation** — Die Sub-Platform-Crates (`platform-linux-x11`, zukünftig `platform-linux-wayland`) registrieren sich **nicht** selbst im `inventory`. Sie exportieren ihre Device-Typen als öffentliche Zero-Sized Structs (ZSTs) — keine `pub static` Instanzen. Ein neues `platform-linux` Crate:
1. Registriert **ein** `PlatformModule` im `inventory`
2. Registriert **einen Satz** delegierender Wrapper-Devices
3. Erkennt den Session-Typ zur Laufzeit in `initialize()`
4. Konstruiert Sub-Platform-ZSTs **inline am Call-Site** und delegiert — kein Objekt existiert länger als der einzelne Aufruf

**Design-Entscheidung: `Resolved`-Struct mit einmaliger Auflösung** — `initialize()` erkennt den Session-Typ, baut ein `Resolved`-Struct mit 7 `&'static dyn Trait`-Referenzen (eine pro Platform-Trait) auf und cacht es in `Mutex<Option<Resolved>>`. Alle Wrapper-Devices greifen direkt auf das gecachte `Resolved` zu — kein erneutes Session-Matching, kein `?` für die Auflösung. Bei `SessionType::Wayland` werden die Wayland-Backends aus `platform-linux-wayland` verwendet, bei `SessionType::X11` die X11-Backends. Sub-Platform-Device-Structs bleiben ZSTs (Unit-Structs ohne Felder), werden aber als `&'static dyn Trait`-Referenzen im `Resolved`-Struct gehalten.

**Session-Erkennung** (Prioritätskette, gecacht in `Mutex<Option<SessionType>>`):
1. `$XDG_SESSION_TYPE` → `"wayland"` | `"x11"` (autoritativste Quelle)
2. `$WAYLAND_DISPLAY` gesetzt → Wayland
3. `$DISPLAY` gesetzt → X11
4. Keines → `PlatformError`

Hinweis: XWayland auf einer Wayland-Session setzt **beide** Variablen (`$DISPLAY` + `$WAYLAND_DISPLAY`), aber `$XDG_SESSION_TYPE=wayland` — daher hat Schritt 1 Vorrang. Platform-Level Input-Injection nutzt immer das native Session-Protokoll (Wayland→EIS, X11→XTEST). `OnceLock::get_or_try_init` ist bis Rust 1.93 unstable — daher `Mutex<Option<SessionType>>` als Cache.

19e₁. ✅ **Crate anlegen** (`crates/platform-linux/Cargo.toml`): Deps: `platynui-core`, `platynui-platform-linux-x11`, `platynui-platform-linux-wayland`, `inventory`, `tracing`. Alles `#[cfg(target_os = "linux")]`.

19e₂. ✅ **Session-Erkennung** (`src/session.rs`): `SessionType` enum (`X11`, `Wayland`), `detect_session_type() → Result<SessionType, PlatformError>`, `Mutex<Option<SessionType>>` Cache mit Prozess-Lifetime. Unit-Tests für alle Umgebungsvariablen-Kombinationen. (~80 LoC)

19e₃. ✅ **PlatformModule-Implementierung** (`src/lib.rs`): `LinuxModule` — `initialize()` erkennt Session-Typ, baut `Resolved`-Struct mit 7 `&'static dyn Trait`-Referenzen (X11-Backends bei `SessionType::X11`, Wayland-Backends bei `SessionType::Wayland`), cacht es in `Mutex<Option<Resolved>>`, delegiert dann an `r.module.initialize()`. `shutdown()` delegiert analog. `register_platform_module!(&MODULE)`. (~80 LoC)

19e₄. ✅ **Delegierende Wrapper-Devices**: Je ein `struct LinuxPointer`, `LinuxKeyboard`, `LinuxDesktopInfo`, `LinuxHighlight`, `LinuxScreenshot`, `LinuxWindowManager` — jede Trait-Methode greift auf `resolved().field.method()` zu (panicked nur wenn `initialize()` nicht aufgerufen wurde — Programmierfehler). Registrierung via `register_*_device!()`. `use`-Aliases für Lesbarkeit (`use ... as X11Pointer`). (~250 LoC)

19e₅. ✅ **Refactoring `platform-linux-x11`**: Alle `register_*_device!()` Makros und `pub static` Instanzen entfernt. Module (`init`, `pointer`, `keyboard`, `desktop`, `screenshot`, `highlight`, `window_manager`) als `pub mod` exportiert. Device-Structs als `pub struct` sichtbar. `inventory`-Dependency entfernt. Das Crate funktioniert als reine Library ohne Selbstregistrierung. (~60 LoC Änderungen)

19e₆. ✅ **Consumer-Crates aktualisieren**: Link-Crate, CLI, Inspector, Python Bindings (`packages/native`), Playground — alle von `platynui-platform-linux-x11` auf `platynui-platform-linux` umgestellt. Provider (`platynui-provider-atspi`) bleibt unverändert — wird für beide Sessions genutzt.

**Meilenstein 3e:** ✅ `cargo nextest run --workspace` — 1902 Tests grün. Build + Clippy clean. Runtime initialisiert auf X11-Session über den Mediator mit X11-Backends. Auf Wayland-Session: `SessionType::Wayland` erkannt, Wayland-Platform-Backends ausgewählt (`WaylandModule::initialize()` verbindet zum Display, erkennt Compositor-Typ). Doku aktualisiert (`docs/architecture.md`, `docs/platform-linux.md`).

---

### Phase 4: Wayland-Platform-Crate (`crates/platform-linux-wayland/`, ~4.000 LoC + ~300 LoC GJS, ~4 Wochen) 🔄 IN ARBEIT

*Ziel: PlatynUI kann unter Wayland Fenster finden, Input injizieren, Screenshots machen und Highlight-Overlays anzeigen — auf GNOME/Mutter, KDE/KWin, wlroots-basierten Compositors (Sway, Hyprland) und dem eigenen PlatynUI-Compositor.*

**Herausforderung:** Im Gegensatz zu X11 (ein universelles API: XTEST + EWMH + RANDR) bietet Wayland **keine** einheitliche Automation-API. Jeder Compositor unterstützt unterschiedliche Protokoll-Kombinationen für Input-Injection, Screenshots, Fenster-Management und Overlays. Besonders problematisch:
- **Fenster-Positionen** sind unter Wayland ein bewusstes Design-Gap (Clients kennen ihre globale Position nicht). Compositor-spezifisches IPC nötig.
- **GNOME/Mutter** unterstützt weder Layer-Shell (für Highlights) noch liefert `Shell.Introspect` Fenster-Positionen (nur width/height, kein x/y). Lösung: eigene PlatynUI GNOME Shell Extension.
- **`ext-layer-shell-v1`** existiert noch nicht (Stand 2026-03) — nur `wlr-layer-shell-v1` auf wlroots-Compositors.

**Lösung: Compositor-Erkennung zuerst, dann direkte Backend-Instanziierung** — Beim `initialize()` wird zuerst der Compositor-Typ über `SO_PEERCRED` auf dem Wayland-Socket ermittelt. Der Compositor-Typ bestimmt dann direkt, welche Backends für Input, Screenshots, WindowManager und Highlight instanziiert werden — ohne blinde Protokoll-Probing-Phase. Jeder Backend-Konstruktor kann bei Verbindungsfehlern graceful auf den nächsten Fallback wechseln. GNOME/Mutter nutzt eine eigene Shell Extension die per D-Bus kommuniziert.

**Protokoll-Matrix** (was gibt es wo):

| Capability | Standard-Protokoll | wlroots/Sway/Hyprland/COSMIC | Mutter/GNOME | KWin/Plasma | PlatynUI-Compositor |
|---|---|---|---|---|---|
| **Input** | EIS/libei | ✅ + `zwlr-virtual-*` | ✅ (45+) via Portal | Portal RemoteDesktop → EIS | ✅ + `zwlr-virtual-*` |
| **Output-Screenshot** | `ext-image-copy-capture` | ✅ + `wlr-screencopy` | Portal ScreenCast | `ext-image-copy-capture` / Portal ScreenCast | ✅ + `wlr-screencopy` |
| **Window-Screenshot** | `ext-image-capture-source` (per-toplevel) | ✅ (`wlr-screencopy` + Region-Crop) | Portal ScreenCast (WINDOW) / GNOME Extension | `ext-image-copy-capture` / Portal ScreenCast (WINDOW) | ✅ + `wlr-screencopy` |
| **Fenster-Liste** | `ext-foreign-toplevel-list` | ✅ + `wlr-foreign-toplevel-mgmt` | `Shell.Introspect` (kein x/y!) / GNOME Extension | KWin Scripting D-Bus | ✅ + `wlr-foreign-toplevel-mgmt` + Control-Socket |
| **Fenster-Position** | *(keiner — Wayland-Design: Clients kennen ihre Position nicht!)* | Sway: i3-IPC `GET_TREE` / Hyprland: `hyprctl -j clients` / COSMIC: *unklar, ggf. ext-foreign-toplevel + D-Bus* | GNOME Extension (`Meta.Window.get_frame_rect()`) | KWin Scripting D-Bus | Control-Socket |
| **Fenster-Aktionen** | *(keiner)* | `wlr-foreign-toplevel-mgmt` | GNOME Extension | KWin Scripting D-Bus | `wlr-foreign-toplevel-mgmt` + Control-Socket |
| **Highlight** | *(keiner — `ext-layer-shell` existiert noch nicht!)* | `wlr-layer-shell` (COSMIC: `cosmic-ext-layer-shell`) | GNOME Extension (St.Widget Overlay) | `workspace.showOutline(QRect)` via D-Bus | `wlr-layer-shell` |
| **Desktop/Output** | `wl_output` + `xdg-output` | ✅ | ✅ | ✅ | ✅ |

> **Wichtige Erkenntnisse aus der Recherche:**
> - `ext-layer-shell-v1` existiert **noch nicht** (nur `wlr-layer-shell-v1` bei wlroots-Compositors). Mutter/KWin unterstützen kein Layer-Shell.
> - `org.gnome.Shell.Introspect.GetWindows()` liefert **nur width/height, KEIN x/y** — für Positionen braucht GNOME eine eigene Shell Extension oder AT-SPI `GetExtents(SCREEN)`.
> - KWin hat `workspace.showOutline(QRect)` + `hideOutline()` via KWin Scripting D-Bus API — Compositor-Level Highlighting ohne Layer-Shell!
> - AT-SPI `GetExtents(SCREEN)` liefert unter Wayland für **Wayland-native Apps keine korrekten globalen Koordinaten** — der AT-SPI-Provider im Toolkit kennt die Fensterposition nicht (Wayland-Design). Deshalb braucht das Platform-Crate einen WindowManager, der die Positionen via Compositor-IPC ermittelt und dem AT-SPI-Provider zur Verfügung stellt.
> - Portal RemoteDesktop: `restore_token` ist **single-use** — nach Verwendung wird ein neuer Token zurückgegeben, der gespeichert werden muss.
> - Portal ScreenCast unterstützt `WINDOW` als Source-Typ (Bitmask 2) — per-Window-Capture ohne Compositor-spezifisches Protokoll.
> - `ext-image-capture-source-v1` unterstützt **per-Toplevel-Capture** via `ext_foreign_toplevel_image_capture_source_manager_v1`.
>
> **KWin/Plasma eigene Wayland-Protokolle** (`plasma-wayland-protocols`) — **evaluiert, nicht implementiert:**
> KWin exponiert `org_kde_kwin_fake_input` (v6, Input-Injection), `zkde_screencast_unstable_v1` (v5, PipeWire-Screencasting) und `org_kde_plasma_window_management` (v20, Fenster-Management). Nach Analyse kein Mehrwert gegenüber Portal + D-Bus: (1) `fake_input` spart nur initialen Consent-Dialog, Portal mit `restore_token` ist gleichwertig; (2) `zkde_screencast` wird intern vom Portal ScreenCast gewrapped, `ext-image-copy-capture` ist der bessere Standard; (3) `plasma_window_management` hat single-client-binding — in Desktop-Sessions nicht nutzbar da plasmashell den Slot belegt. KWin Scripting D-Bus deckt alles ab. ~470 LoC eingespart. Bei Bedarf nachrüstbar.
>
> **Mutter/GNOME eigene Wayland-Protokolle:**
> - Mutter exponiert **keine vergleichbaren Custom-Wayland-Protokolle** für Clients. Stattdessen D-Bus-Interfaces (`org.gnome.Mutter.ScreenCast`, `org.gnome.Mutter.RemoteDesktop`, `org.gnome.Shell.Introspect`, `org.gnome.Shell.Screenshot`). Bestätigt: GNOME Shell Extension ist der richtige Ansatz für Mutter.

**Crate-Struktur** (modulare Backend-Architektur):
```
crates/platform-linux-wayland/src/
  lib.rs                  # pub fn initialize()/shutdown(), pub static Devices
  connection.rs           # Wayland-Display + wl_registry Scan + Background Event Loop
  capabilities.rs         # CompositorType Erkennung (SO_PEERCRED) + Backend-Instanziierung
  coordinates.rs          # Window-relative → absolute Koordinaten-Transformation
  app_detect.rs           # Per-App Wayland/XWayland-Erkennung

  input/
    mod.rs                # InputBackend trait + Selektion
    eis.rs                # EIS direkt (reis) — eigener Compositor, neuere wlroots
    portal.rs             # Portal RemoteDesktop → ConnectToEIS() — Mutter, KWin, Portal-Fallback
    virtual_input.rs      # zwlr-virtual-pointer/keyboard — wlroots-Fallback

  screenshot/
    mod.rs                # ScreenshotBackend trait
    image_copy.rs         # ext-image-copy-capture-v1 + ext-image-capture-source (per-Toplevel)
    wlr_screencopy.rs     # wlr-screencopy-unstable-v1 + capture_output_region() — legacy wlroots
    portal.rs             # Portal Screenshot (URI) + Portal ScreenCast (WINDOW source)
    gnome_extension.rs    # PlatynUI GNOME Shell Extension → CaptureWindow(id)

  window_manager/
    mod.rs                # WaylandWindowManager (impl WindowManager) + CompositorBackend trait
    ext_foreign.rs        # ext-foreign-toplevel-list-v1 (read-only Liste, keine Positionen)
    wlr_foreign.rs        # wlr-foreign-toplevel-management-v1 (Liste + Aktionen, keine Positionen)
    gnome_extension.rs    # PlatynUI GNOME Extension → volle Geometrie + Aktionen (Mutter)
    platynui_ipc.rs       # Control-Socket IPC (eigener Compositor)
    kwin_dbus.rs          # org.kde.KWin.Scripting → volle Geometrie + Aktionen (primär in Plasma-Sessions)
    sway_ipc.rs           # *(optional, später)* i3-IPC GET_TREE → volle Geometrie (rect: x,y,w,h)
    hyprland_ipc.rs       # *(optional, später)* hyprctl -j clients → volle Geometrie + Window-Management

  highlight/
    mod.rs                # HighlightBackend trait
    layer_shell.rs        # wlr-layer-shell-v1 — Overlay auf eigenem Layer (wlroots)
    kwin_outline.rs       # KWin workspace.showOutline(QRect) via D-Bus
    gnome_extension.rs    # PlatynUI GNOME Extension → St.Widget Compositor-Overlay
    overlay_window.rs     # Letzter Fallback: XWayland-Overlay-Fenster (fragile Z-Order)

  desktop/
    mod.rs                # WaylandDesktopInfo (impl DesktopInfoProvider) + physische Koordinaten + Output-Storage
    output_info.rs        # OutputInfo Datenmodell (wl_output + xdg-output + D-Bus Enrichment)
    display_config.rs     # Compositor-spezifische D-Bus Enrichment (Mutter, KWin)
```

#### Phase 4a: Fundament (~800 LoC) 🔄 IN ARBEIT

20. ✅ **Crate anlegen** — `crates/platform-linux-wayland/` erstellt. Aktuelle Deps: `platynui-core`, `tracing`, `wayland-client`, `wayland-protocols` (Feature `client`, `staging`), `wayland-protocols-wlr` (Feature `client`), `rustix` (Feature `net` für `SO_PEERCRED`). Alles `#[cfg(target_os = "linux")]`. **Keine** `inventory`-Dependency — das Crate registriert sich nicht selbst. Stub-Implementierungen aller 7 Platform-Traits vorhanden (`WaylandModule`, `WaylandPointerDevice`, `WaylandKeyboardDevice`, `WaylandDesktopInfo`, `WaylandScreenshot`, `WaylandHighlightProvider`, `WaylandWindowManager`). Weitere Deps (`reis`, `platynui-xkb-util`, `zbus`, `egui`, `egui_glow`, `pipewire`) kommen in späteren Sub-Phasen bei Bedarf. Build/Clippy clean, 1902 Tests grün (inkl. 1 `classify_known_binaries` Test).

21. ✅ **Connection + Compositor-Erkennung** (`src/connection.rs`, `src/capabilities.rs`): Implementiert. `connection::connect()` verbindet zum Wayland-Display, `detect_compositor()` nutzt `SO_PEERCRED` via `rustix::net::sockopt::socket_peercred()` → PID → `/proc/<pid>/exe` → Binary-Name-Matching. Fallback auf `$XDG_CURRENT_DESKTOP`. `CompositorType`-Enum: `PlatynUI`, `Mutter`, `KWin`, `Hyprland`, `Sway`, `Wlroots`, `Unknown`. Globaler State (`Mutex<Option<WaylandGlobal>>`) wird in `WaylandModule::initialize()` befüllt, `with_global()` für Device-Backends. Plan-Pseudocode (Step 21 original):

    `pub fn initialize()` — Wayland-Display-Verbindung, dann sofort Compositor-Typ-Erkennung via `SO_PEERCRED`. Der Compositor-Typ bestimmt direkt die Backend-Strategie für alle Devices:

    ```rust
    enum CompositorType {
        Mutter,     // → GNOME Extension, Portal-first für Input
        PlatynUI,   // → Control-Socket, alle Protokolle
        KWin,       // → KWin Scripting D-Bus, Portal für Input + Screenshots
        Cosmic,     // → wlr-Protokolle + cosmic-ext-*, iced/D-Bus (Positions-IPC noch unklar)
        Sway,       // → wlr-Protokolle (i3-IPC GET_TREE optional, später)
        Hyprland,   // → wlr-Protokolle (hyprctl IPC optional, später)
        Unknown,    // → nur Standard-Protokolle + Portal + AT-SPI
    }
    ```

    **Backend-Instanziierung basierend auf CompositorType:**
    ```rust
    fn create_backends(compositor: CompositorType, conn: &WaylandConnection) -> Backends {
        match compositor {
            Mutter => Backends {
                input:          PortalInput::new(),        // Portal RemoteDesktop → ConnectToEIS()
                screenshot:     GnomeExtScreenshot::new()  // GNOME Extension CaptureWindow
                                .or(PortalScreencast::new()),
                window_manager: GnomeExtWindowManager::new(), // Extension: Meta.Window.get_frame_rect()
                highlight:      GnomeExtHighlight::new(),    // Extension: St.Widget Overlay
            },
            PlatynUI => Backends {
                input:          EisInput::new()            // EIS direkt
                                .or(WlrVirtualInput::new(conn)),
                screenshot:     ExtImageCopyCapture::new(conn), // ext-image-copy-capture
                window_manager: PlatynUISocket::new(),     // Control-Socket
                highlight:      LayerShellHighlight::new(conn), // wlr-layer-shell
            },
            KWin => Backends {
                input:          PortalInput::new(),        // Portal RemoteDesktop → ConnectToEIS()
                screenshot:     ExtImageCopyCapture::new(conn) // ext-image-copy-capture (KWin 6.1+)
                                .or(PortalScreencast::new()) // Portal ScreenCast (WINDOW)
                                .or(PortalScreenshot::new()),
                window_manager: KWinDbus::new(),           // org.kde.KWin Scripting D-Bus
                highlight:      KWinOutline::new(),        // workspace.showOutline(QRect)
            },
            Sway | Hyprland | Cosmic | Unknown => Backends {
                input:          EisInput::new()            // EIS direkt (wenn Socket vorhanden)
                                .or(WlrVirtualInput::new(conn)) // wlr-virtual-pointer/keyboard
                                .or(PortalInput::new()),   // Portal als letzter Fallback
                screenshot:     ExtImageCopyCapture::new(conn) // ext-image-copy-capture
                                .or(WlrScreencopy::new(conn)) // wlr-screencopy (Legacy)
                                .or(PortalScreencast::new()),
                window_manager: WlrForeignToplevel::new(conn)  // Aktionen, keine Positionen
                                .or(ExtForeignToplevel::new(conn)), // Read-only Liste
                highlight:      LayerShellHighlight::new(conn) // wlr-layer-shell
                                .or(XWaylandOverlay::new()),   // XWayland-Fallback
            },
        }
    }
    ```

    **Vorteil:** Kein blindes Probing von 15+ Capabilities. Der Compositor-Typ (kostengünstig via `SO_PEERCRED` ermittelt) bestimmt direkt, welche Backends instanziiert werden. Jeder Backend-Konstruktor verbindet sich zu seinem Protokoll/D-Bus-Service und liefert `Err` wenn nicht verfügbar — der `.or()`-Fallback wählt dann die nächste Alternative. Die `wl_registry`-Globals werden im Hintergrund beim Wayland-Roundtrip gesammelt und stehen den Wayland-Protokoll-Backends (ext-image-copy-capture, wlr-screencopy, wlr-layer-shell, etc.) direkt zur Verfügung.
    **Compositor-Typ-Erkennung** via `SO_PEERCRED` auf dem Wayland-Socket:
    
    Die zuverlässigste Methode ist, den **Prozess hinter dem Wayland-Socket** zu identifizieren. Da wir bereits eine Wayland-Verbindung haben (sonst gäbe es kein Wayland-Platform-Crate), können wir:
    1. `getsockopt(wayland_fd, SOL_SOCKET, SO_PEERCRED)` → `ucred { pid, uid, gid }` des Compositor-Prozesses
    2. `std::fs::read_link(format!("/proc/{pid}/exe"))` → Binary-Pfad (z.B. `/usr/bin/mutter`, `/usr/bin/sway`)
    3. Binary-Name → `CompositorType` Mapping:
    
    ```rust
    fn detect_compositor(wayland_fd: RawFd) -> CompositorType {
        // 1. Primär: Compositor-Binary über Wayland-Socket identifizieren
        let peer_pid = getsockopt_peercred(wayland_fd).pid;
        let exe = std::fs::read_link(format!("/proc/{peer_pid}/exe"))
            .ok()
            .and_then(|p| p.file_name()?.to_str().map(String::from));
        
        if let Some(compositor) = match exe.as_deref() {
            Some("mutter" | "gnome-shell")          => Some(Mutter),
            Some("platynui-wayland-compositor")      => Some(PlatynUI),
            Some("kwin_wayland")                     => Some(KWin),
            Some("cosmic-comp")                      => Some(Cosmic),
            Some("sway")                             => Some(Sway),
            Some(e) if e.starts_with("Hyprland")     => Some(Hyprland),
            _ => None,
        } {
            return compositor;
        }
        
        // 2. Fallback: Env-Variablen (z.B. in Containern ohne /proc-Zugriff,
        //    oder bei unbekanntem Binary-Namen)
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            for part in desktop.split(':') {
                match part.trim().to_uppercase().as_str() {
                    "GNOME"    => return Mutter,
                    "KDE"      => return KWin,
                    "COSMIC"   => return Cosmic,
                    "SWAY"     => return Sway,
                    "HYPRLAND" => return Hyprland,
                    "PLATYNUI" => return PlatynUI,
                    _ => {}
                }
            }
        }
        
        Unknown
    }
    ```
    
    **Primär:** `SO_PEERCRED` auf dem Wayland-Socket → PID → `/proc/<pid>/exe` → Binary-Name. 100% zuverlässig, ein Syscall + ein Readlink.
    **Fallback:** `$XDG_CURRENT_DESKTOP` für Fälle in denen `/proc` nicht verfügbar ist (Container, Sandboxes) oder der Binary-Name unbekannt ist (Custom-Builds, Forks). (~250 LoC)

21b. **Koordinaten-Transformation** (`src/coordinates.rs`): **Kernproblem:** Unter Wayland liefert AT-SPI `GetExtents(SCREEN)` für Wayland-native Apps **keine korrekten globalen Koordinaten**. Der AT-SPI-Provider im Toolkit (z.B. GTK, Qt) kennt die globale Fensterposition nicht, weil Wayland-Clients ihre Position bewusst nicht erfahren. Das bedeutet:
    - `GetExtents(SCREEN)` gibt für Wayland-native Apps nur **fenster-relative** Koordinaten zurück (als wären sie `GetExtents(WINDOW)`)
    - `GetExtents(SCREEN)` für **XWayland-Apps** funktioniert korrekt (X11-Kontext hat globale Positionen)
    
    **Lösung:** Die `WaylandWindowManager`-Implementierung des `platynui_core::platform::WindowManager`-Traits (Phase 4d) liefert via `bounds(WindowId) → Rect` die Fenster-Position vom Compositor. Dieses Modul kombiniert:
    - **Window-Position** von `WindowManager::bounds()` (via KWin Scripting, GNOME Extension, PlatynUI Control-Socket; später optional: Sway GET_TREE, Hyprland hyprctl)
    - **Element-Offset** von AT-SPI `GetExtents(WINDOW)` (relativ zum Fenster — das funktioniert immer korrekt)
    → **Absolute Screen-Koordinaten** für `PointerDevice::move_to()`, `ScreenshotProvider::capture()` und `UiNode::bounds()`.
    
    Diese Koordinaten werden auch dem AT-SPI-Provider zur Verfügung gestellt, damit `UiAttribute::Bounds` korrekte Werte enthält. (~100 LoC)

21c. **App-Erkennung Wayland/XWayland** (`src/app_detect.rs`): Liest `/proc/{pid}/environ` der Ziel-App um `GDK_BACKEND=wayland`, `QT_QPA_PLATFORM=wayland`, `MOZ_ENABLE_WAYLAND=1` etc. zu prüfen. Entscheidet ob die App unter Wayland-nativ läuft (Fenster-Offset-Korrektur nötig) oder unter XWayland (AT-SPI `GetExtents(SCREEN)` direkt nutzbar). (~80 LoC)

21d. ✅ **Desktop Info** (`src/desktop/`, `src/connection.rs`): Echte Monitor-Enumeration via `wl_output` + `xdg_output_manager_v1`. `connect_and_enumerate()` macht `registry_queue_init` + zwei Roundtrips: (1) sammelt `wl_output`-Globals (Geometry, Mode, Scale, Name, Description) und bindet `zxdg_output_manager_v1`, (2) sammelt logische Koordinaten/Größe/Name/Description via `zxdg_output_v1`. `OutputInfo`-Struct (in `desktop/output_info.rs`) mit `effective_*()` Methoden (bevorzugt xdg-output logisch, Fallback auf wl_output geometry/mode÷scale). Output-Speicherung im `desktop`-Modul (nicht in connection). Compositor-spezifische D-Bus Enrichment (`desktop/display_config.rs`) für fraktionales Scaling + Primary-Erkennung (Mutter `GetCurrentState()`, KWin `primaryOutputName`). Physische Pixel-Koordinaten aus logischem Layout berechnet. Desktop-Name mit Compositor: `Wayland Desktop (Mutter)`. **Background Event Loop** in `connection.rs`: Nach Init läuft ein Dispatch-Thread (`prepare_read` + `poll` + `dispatch_pending`) der auf `wl_output`-Änderungen (Auflösung, Scaling, Geometry), `wl_registry.global` (Hot-Plug) und `wl_registry.global_remove` (Unplug) lauscht. Bei Änderungen werden die Outputs automatisch neu enriched und via `desktop::set_outputs()` aktualisiert.

#### Phase 4b: Input-Backends (~600 LoC)

*Priorisierte Fallback-Kette je nach Compositor: PlatynUI+wlroots → EIS direkt → `zwlr-virtual` / Mutter+KWin → Portal RemoteDesktop → EIS*

22. **Input-Backend-Trait** (`src/input/mod.rs`): Internes `InputBackend` trait. Backend-Instanziierung passiert in `create_backends()` basierend auf `CompositorType`. `pub static POINTER` und `pub static KEYBOARD` delegieren an das gewählte Backend. (~80 LoC)

22a. **EIS-Backend** (`src/input/eis.rs`): `reis`-basierter EI-Client. Verbindet zum EIS-Socket in `$XDG_RUNTIME_DIR`. Pointer (absolute + relative Bewegung, Buttons, Scroll), Keyboard (Keycodes, XKB-Keymap via `platynui-xkb-util`). Priorisiert auf PlatynUI-Compositor und neueren wlroots-Versionen mit nativem EIS-Socket. (~200 LoC)

22b. **Portal-Backend** (`src/input/portal.rs`): `zbus`-Client für `org.freedesktop.portal.RemoteDesktop`. `CreateSession` → `SelectDevices(keyboard+pointer+touchscreen)` → `Start` → `ConnectToEIS()` → EIS-FD → gleiche `reis`-Logik wie 22a. Token-Persistierung (`persist_mode=2`) um Consent-Dialog nach erstem Mal zu vermeiden. **Wichtig:** `restore_token` ist **single-use** — nach jeder Verwendung wird ein neuer Token in der Response zurückgegeben, der sofort gespeichert werden muss. Token-Storage: `$XDG_DATA_HOME/platynui/portal_tokens.json`. Primärer Pfad für Mutter und KWin. (~200 LoC)

22c. **Virtual-Input-Backend** (`src/input/virtual_input.rs`): `zwlr-virtual-pointer-v1` + `zwlr-virtual-keyboard-v1`. Fallback für wlroots-Compositors ohne EIS-Socket. (~120 LoC)

#### Phase 4c: Screenshot-Backends (~720 LoC)

*Priorisierte Fallback-Kette je nach Compositor: wlroots+KWin → `ext-image-copy-capture` → `wlr-screencopy` → Portal / Mutter → GNOME Extension → Portal ScreenCast*

23. **Screenshot-Backend-Trait** (`src/screenshot/mod.rs`): Internes `ScreenshotBackend` trait mit zwei Methoden: `capture_output(output)` (ganzer Bildschirm) und `capture_window(toplevel)` (einzelnes Fenster). `pub static SCREENSHOT` delegiert. (~80 LoC)

23a. **ext-image-copy-capture** (`src/screenshot/image_copy.rs`): `ext-image-copy-capture-v1` — werdender Standard. **Output-Capture** via `ext_image_copy_capture_manager_v1`. **Per-Toplevel-Capture** via `ext-image-capture-source-v1` + `ext_foreign_toplevel_image_capture_source_manager_v1` — erfasst ein einzelnes Fenster direkt, ohne Region-Crop. Optionale Cursor-Session. Bester Pfad für wlroots-Compositors und PlatynUI-Compositor. (~220 LoC)

23b. **wlr-screencopy** (`src/screenshot/wlr_screencopy.rs`): `wlr-screencopy-unstable-v1` — Fallback für ältere wlroots-Versionen. `capture_output()` für ganzen Bildschirm, `capture_output_region(output, x, y, w, h)` für Fenster-Screenshots via Bounding-Box aus dem ToplevelBackend. (~150 LoC)

23c. **Portal-Screenshot** (`src/screenshot/portal.rs`): Zwei Pfade:
    - **Portal ScreenCast** mit `source_type = WINDOW` (Bitmask 2): Per-Window-Capture via PipeWire-Stream. Funktioniert auf Mutter + KWin. `SelectSources(types=WINDOW)` → `Start` → PipeWire Node-ID → Frame lesen. (~150 LoC)
    - **Portal Screenshot**: `org.freedesktop.portal.Screenshot.Screenshot()` → gibt Datei-URI zurück. Einfachster Pfad aber nur ganzer Bildschirm, kein per-Window. Letzter Fallback. (~60 LoC)

23d. **GNOME Extension-Screenshot** (`src/screenshot/gnome_extension.rs`): PlatynUI GNOME Shell Extension → `CaptureWindow(window_id)` via D-Bus. Nutzt Mutter's internen Screenshot-Mechanismus (`Shell.Screenshot`). Nur auf GNOME/Mutter wenn Extension installiert. (~60 LoC)

#### Phase 4d: WindowManager-Backends (~800 LoC, +300 LoC optional)

*Ziel: Da AT-SPI unter Wayland für Wayland-native Apps **keine Fenster-Positionen** liefern kann (Wayland-Design: Clients kennen ihre globale Position nicht), braucht das Platform-Crate eine Implementierung des existierenden `platynui_core::platform::WindowManager`-Traits. Diese Implementierung ermittelt Fenster-Positionen via Compositor-IPC und stellt sie dem AT-SPI-Provider zur Verfügung — genau wie `X11EwmhWindowManager` in `platform-linux-x11` es für X11 tut.*

*Der `WindowManager`-Trait (`crates/core/src/platform/window_manager.rs`) definiert bereits die vollständige Schnittstelle: `resolve_window(&dyn UiNode) → WindowId`, `bounds(WindowId) → Rect`, `is_active()`, `activate()`, `close()`, `minimize()`, `maximize()`, `restore()`, `move_to()`, `resize()`. Der `provider-atspi` konsumiert ihn bereits via `window_managers().next()`. Es wird **kein neuer Trait definiert** — nur eine neue Implementierung.*

*Priorisierte Fallback-Kette für das interne CompositorBackend: Compositor-spezifisches IPC (volle Geometrie + Aktionen) → wlr-foreign-toplevel (Aktionen, keine Positionen) → ext-foreign-toplevel-list (nur Liste). Erster Schritt: GNOME Extension, PlatynUI Control-Socket, KWin D-Bus + wlr-foreign-toplevel + ext-foreign-toplevel. Sway IPC und Hyprland IPC kommen optional später. COSMIC Desktop nutzt zunächst die generischen wlr-Backends.*

24. **WaylandWindowManager** (`src/window_manager/mod.rs`): Implementiert `platynui_core::platform::WindowManager`. Intern delegiert an ein `CompositorBackend`-Trait:
    ```rust
    /// Internes Backend-Trait — Implementierungsdetail, nicht öffentlich.
    trait CompositorBackend: Send + Sync {
        fn resolve_window(&self, pid: u32, app_id: &str, title: &str) -> Result<WindowId>;
        fn bounds(&self, id: WindowId) -> Result<Rect>;
        fn is_active(&self, id: WindowId) -> Result<bool>;
        fn activate(&self, id: WindowId) -> Result<()>;
        fn close(&self, id: WindowId) -> Result<()>;
        fn minimize(&self, id: WindowId) -> Result<()>;
        fn maximize(&self, id: WindowId) -> Result<()>;
        fn restore(&self, id: WindowId) -> Result<()>;
        fn move_to(&self, id: WindowId, position: Point) -> Result<()>;
        fn resize(&self, id: WindowId, size: Size) -> Result<()>;
    }

    pub struct WaylandWindowManager {
        backend: Box<dyn CompositorBackend>,
    }

    impl platynui_core::platform::WindowManager for WaylandWindowManager {
        fn name(&self) -> &'static str { "Wayland" }
        fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId> {
            // Extrahiert PID + app_id + Titel aus dem UiNode
            // (analog zu X11: dort PID + _NET_WM_NAME)
            let (pid, app_id, title) = extract_window_hints(node);
            self.backend.resolve_window(pid, &app_id, &title)
        }
        fn bounds(&self, id: WindowId) -> Result<Rect> { self.backend.bounds(id) }
        // ... alle weiteren Methoden delegieren an self.backend
    }
    ```
    Backend-Selektion in `create_backends()`: Basierend auf `CompositorType` wird das passende `CompositorBackend` instanziiert. `resolve_window()` extrahiert PID/app_id/Titel aus dem `UiNode` (analog zu X11, wo PID + `_NET_WM_NAME` genutzt wird).
    
    **Registrierung:** Via `register_window_manager!()` Macro — oder, falls der `platform-linux` Mediator die Registrierung übernimmt, als `pub static WAYLAND_WINDOW_MANAGER` exportiert. (~120 LoC)

24a. **wlr-foreign-toplevel** (`src/window_manager/wlr_foreign.rs`): `wlr-foreign-toplevel-management-v1` — Fenster-Liste + Aktionen (Activate + Close + Maximize + Minimize + Fullscreen). **Positionen nicht verfügbar** (Protokoll-Limitation). Auf wlroots-Compositors (Sway, Hyprland, COSMIC) liefert `wlr-foreign-toplevel` zunächst Aktionen ohne Positionen — Compositor-spezifisches IPC für volle Geometrie ist optional und kann später nachgerüstet werden. (~180 LoC)

24b. **ext-foreign-toplevel** (`src/window_manager/ext_foreign.rs`): `ext-foreign-toplevel-list-v1` — nur read-only Fenster-Liste (App-ID, Titel). Keine Aktionen, keine Positionen. Verwendet als Ergänzung wenn kein anderer Lister verfügbar. (~100 LoC)

24c. **GNOME Extension** (`src/window_manager/gnome_extension.rs`): PlatynUI GNOME Shell Extension via D-Bus (`org.platynui.GnomeHelper`). Bietet **alles auf Mutter**: Fenster-Liste mit **vollständiger Geometrie** (`Meta.Window.get_frame_rect()` → x, y, width, height), Activate, Close, Maximize, Minimize. Primärer und einziger Pfad für Mutter — ohne Extension sind unter GNOME **keine Fenster-Positionen verfügbar** (Shell.Introspect hat kein x/y, Shell.Eval ist ab GNOME 45+ eingeschränkt). (~150 LoC)

24d. **PlatynUI Control-Socket** (`src/window_manager/platynui_ipc.rs`): JSON-IPC via Unix-Socket. Direkter Zugriff auf Fenster-Liste, Positionen, Geometrie, Activate/Close/Maximize/Minimize. Bester und vollständigster Pfad für den eigenen Compositor. (~100 LoC)

24e. **KWin D-Bus** (`src/window_manager/kwin_dbus.rs`): `org.kde.KWin` D-Bus API:
    - **Positionen:** `getWindowInfo(uuid)` → JSON mit `x`, `y`, `width`, `height`, `frameGeometry`, `clientGeometry`. Alternativ: KWin Scripting API (`KWin::Window.pos`, `.x`, `.y`, `.width`, `.height`, `.frameGeometry`).
    - **Aktionen:** `workspace.activeWindow`, Window-Properties (read-write `frameGeometry`, `closeWindow()`, `setMaximize(vert, horiz)`).
    Offiziell supportete KWin-API. (~150 LoC)

**Optional (später):**

24f. **Sway IPC** *(optional)* (`src/window_manager/sway_ipc.rs`): i3-kompatibles IPC über Unix-Socket. `GET_TREE` (Typ 4) liefert vollständigen Layout-Baum — jeder Knoten hat `rect` mit **absoluten Display-Koordinaten** (x, y, width, height), `window_rect` (relativ im Container), `app_id`, `name`, `pid`, `focused`, `type`. Node-Matching via app_id/pid für Fenster-Identifikation. Events: `window` (new/close/focus/title/move). Binäres i3-Protokoll mit Magic-Bytes + Länge + Typ. (~150 LoC)

24g. **Hyprland IPC** *(optional)* (`src/window_manager/hyprland_ipc.rs`): Zwei Unix-Sockets in `$XDG_RUNTIME_DIR/hypr/{instance}/`:
    - `.socket.sock` (Commands): `hyprctl -j clients` → JSON-Array aller Fenster mit **vollständiger Geometrie** (position, size, class, title, pid, workspace, floating, fullscreen). `hyprctl -j activewindow` für aktives Fenster. `hyprctl -j monitors` für Monitor-Info.
    - `.socket2.sock` (Events): `openwindow`, `closewindow`, `activewindow`, `movewindow`, `windowtitle`, `fullscreen` etc.
    - Aktionen via Dispatcher: `focuswindow address:0x...`, `closewindow address:0x...`, `movewindowpixel exact X Y,address:0x...`, `resizewindowpixel exact W H,address:0x...`.
    Window-Addressing: by class regex, title regex, pid, address. `--batch` für mehrere Kommandos. (~150 LoC)

> **Hinweis:** Sway, Hyprland und COSMIC nutzen wlr-Protokolle (`wlr-foreign-toplevel` für Aktionen, `wlr-layer-shell` für Highlight, `zwlr-virtual-pointer/keyboard` für Input). Im ersten Schritt funktioniert PlatynUI auf diesen Compositors über die generischen wlr-Backends (24a) — ohne Fenster-Positionen, was für viele Test-Szenarien ausreicht. Compositor-spezifisches IPC für volle Geometrie kann bei Bedarf nachgerüstet werden.
>
> **COSMIC Desktop** (System76): Basiert auf `cosmic-comp` (Smithay-basiert, iced-Toolkit). Unterstützt `wlr-foreign-toplevel`, `wlr-layer-shell` (als `cosmic-ext-layer-shell`), `ext-foreign-toplevel-list`, EIS/Portal. Für Fenster-Positionen gibt es noch kein dokumentiertes IPC — ggf. `cosmic-toplevel-info-unstable-v1` oder D-Bus. COSMIC fällt initial in die gleiche Kategorie wie Sway/Hyprland: wlr-Backends für Aktionen, Positionen ggf. erst mit dediziertem Backend.

> **Zusammenspiel WindowManager ↔ AT-SPI:** Der AT-SPI-Provider (`platynui-provider-atspi`) ruft bereits `window_managers().next()` auf und nutzt `resolve_window()` + `bounds()` + `activate()` — daran ändert sich nichts. Unter Wayland liefert die neue `WaylandWindowManager`-Implementierung die Fenster-Positionen via Compositor-IPC (statt via EWMH wie auf X11). Das Koordinaten-Modul (21b) kombiniert `WindowManager::bounds()` mit AT-SPI `GetExtents(WINDOW)` zu absoluten Screen-Koordinaten. Ohne registrierten WindowManager (z.B. unbekannter Compositor) fallen die Positionen auf (0,0) zurück und eine `warn!`-Meldung wird geloggt.

#### Phase 4e: Highlight-Backends (~500 LoC)

*Priorisierte Fallback-Kette: Layer-Shell → KWin showOutline → GNOME Extension → XWayland-Overlay → kein Highlight (warn)*

25. **Highlight-Backend-Trait** (`src/highlight/mod.rs`): Internes `HighlightBackend` trait (`show(rect, color, thickness)`, `hide()`, `is_available() → bool`) + Selektion. `pub static HIGHLIGHT` delegiert. (~60 LoC)

25a. **Layer-Shell** (`src/highlight/layer_shell.rs`): `wlr-layer-shell-v1` + egui-Rendering (farbige semi-transparente Rechtecke als Overlay auf `top`-Layer, `exclusive_zone = 0`, Keyboard-Interaktivität = None). Command-Channel (Show/Hide/Clear). Bester Pfad — funktioniert auf wlroots-Compositors (Sway, Hyprland) + PlatynUI-Compositor. **Hinweis:** `ext-layer-shell-v1` existiert noch nicht (Stand 2026-03) — nur `wlr-layer-shell-v1` verfügbar. (~200 LoC)

25b. **KWin Outline** (`src/highlight/kwin_outline.rs`): `org.kde.KWin` Scripting API via D-Bus: `workspace.showOutline(QRect { x, y, width, height })` zeichnet einen Compositor-Level Outline an beliebiger Position, `workspace.hideOutline()` entfernt ihn. Kein Layer-Shell nötig — KWin hat diese Funktionalität eingebaut (wird intern für Snap-Assist und Window-Placement verwendet). Vorteile: korrekte Z-Order (über allen Fenstern), kein Fokus-Wechsel, offizielle API. Primärer Highlight-Pfad für KDE/Plasma. (~80 LoC)

25c. **GNOME Extension Highlight** (`src/highlight/gnome_extension.rs`): PlatynUI GNOME Shell Extension via D-Bus (`org.platynui.GnomeHelper.ShowHighlight(x, y, w, h, color, thickness)` + `HideHighlight()`). Extension erzeugt ein `St.Widget` mit CSS-Styling (`border: Npx solid color; background: rgba(...)`) und fügt es zu `global.window_group` hinzu — liegt über allen Fenstern im Compositor. Korrekte Z-Order, kein Fokus-Wechsel. Primärer Highlight-Pfad für GNOME/Mutter. (~80 LoC)

25d. **XWayland-Overlay** (`src/highlight/overlay_window.rs`): Letzter Fallback: XWayland-Fenster als undekoriertes, semi-transparentes Overlay. Einschränkungen: Z-Order **nicht garantiert** (kann hinter anderen Fenstern landen), verursacht Fokus-Wechsel, braucht WM-spezifische Hints (`_NET_WM_WINDOW_TYPE_DOCK`). Nur als Notlösung für unbekannte Compositors oder wenn Extension/showOutline nicht verfügbar. (~140 LoC)

> **Design-Entscheidung:** Wenn kein Highlight-Backend verfügbar ist (z.B. unbekannter Compositor ohne Layer-Shell/Extension), wird **kein Fehler** geworfen sondern eine `warn!`-Meldung geloggt. Highlight ist eine Debug-/Diagnose-Hilfe, kein kritisches Feature — fehlende Highlights sollen Tests nicht blockieren.

#### Phase 4f: Integration (~200 LoC)

20b. **Mediator-Integration** (`crates/platform-linux/`): `platynui-platform-linux-wayland` als Dependency zum Mediator hinzufügen. `SessionType::Wayland`-Arme in `initialize()`, `shutdown()` und allen Wrapper-Devices mit Delegation an die Wayland-Exports befüllen. (~100 LoC Änderungen am Mediator)

20c. **Tests** (`crates/platform-linux-wayland/tests/`): Backend-Selektion mit Mock-Capabilities testen. Integration-Tests gegen den eigenen PlatynUI-Compositor (alle Pfade: EIS, wlr-virtual, ext-image-copy-capture, wlr-foreign-toplevel, Layer-Shell). (~100 LoC)

#### Phase 4g: PlatynUI GNOME Shell Extension (~300 LoC GJS)

*Ziel: Eigene GNOME Shell Extension die alle drei Lücken auf Mutter schließt — Fenster-Positionen, Compositor-Level Highlights und per-Window Screenshots. Kleine, fokussierte Extension (~200–400 Zeilen GJS) mit D-Bus-Interface für die Kommunikation mit dem Rust Platform-Crate.*

> **Motivation:** GNOME/Mutter hat kein Layer-Shell, `Shell.Introspect` liefert kein x/y, und `Shell.Eval` ist ab GNOME 45+ eingeschränkt. Eine eigene Extension hat **vollen Zugriff auf die Meta/Clutter/St JavaScript API** im Compositor-Prozess und löst alle drei Probleme sauber.

26a. **Extension-Struktur** (`platynui-gnome-extension/`):
    ```
    platynui-gnome-extension/
    ├── metadata.json          # UUID: platynui@platynui.org, GNOME-Versionen (45, 46, 47, 48)
    ├── extension.js           # Hauptlogik: D-Bus Service registrieren, enable()/disable()
    ├── dbus.js               # D-Bus Interface-Definition (org.platynui.GnomeHelper)
    └── highlight.js          # St.Widget Management für Overlays
    ```

26b. **D-Bus Interface** (`org.platynui.GnomeHelper`):
    ```
    org.platynui.GnomeHelper
    ├── GetWindows() → a{sv}[]           # Alle Fenster: id, app_id, title, x, y, width, height, focused, maximized
    ├── GetWindowGeometry(id: u) → (iiuu) # x, y, width, height für ein Fenster
    ├── ActivateWindow(id: u)             # Fenster fokussieren + raise
    ├── CloseWindow(id: u)                # Fenster schließen
    ├── ShowHighlight(x: i, y: i, w: u, h: u, color: s, thickness: u)
    ├── HideHighlight()
    ├── CaptureWindow(id: u) → s          # Screenshot → Datei-Pfad (temp) via Shell.Screenshot
    ├── GetDesktopSize() → (uu)           # Gesamte Desktop-Größe
    └── Version → s                       # Protokoll-Version für Kompatibilitäts-Checks
    ```

26c. **Interna** (GJS-Implementierung):
    - **Fenster-Liste:** `global.get_window_actors()` → `actor.get_meta_window()` → `Meta.Window`: `get_frame_rect()` (x, y, width, height), `get_wm_class()`, `get_title()`, `get_pid()`, `is_hidden()`, `has_focus()`.
    - **Highlighting:** `new St.Widget({ style: 'border: ...', x, y, width, height })` → `global.window_group.add_child(widget)`. Liegt über allen Fenstern. `HideHighlight()` entfernt das Widget.
    - **Screenshots:** `Shell.Screenshot.new()` → `screenshot_window(meta_window, { include_frame: true })`. Alternative: `screenshot_area(x, y, w, h)` für Region-Capture.
    - **Window-Aktionen:** `meta_window.activate(timestamp)`, `meta_window.delete(timestamp)`, `meta_window.maximize(Meta.MaximizeFlags.BOTH)`, `meta_window.unmaximize(Meta.MaximizeFlags.BOTH)`.

26d. **Auto-Installation im Platform-Crate** (`src/gnome_extension.rs`): Beim `initialize()` auf GNOME/Mutter prüfen ob Extension installiert + aktiviert ist. Falls nicht:
    1. Extension-Dateien nach `$XDG_DATA_HOME/gnome-shell/extensions/platynui@platynui.org/` kopieren (eingebettet als `include_bytes!()` oder aus Paket-Assets).
    2. `gnome-extensions enable platynui@platynui.org` ausführen.
    3. Bei Bedarf: Hinweis loggen dass ein Session-Restart nötig sein kann (GNOME 40+ auf Wayland: Extension-Installation benötigt manchmal Restart).
    4. D-Bus-Service `org.platynui.GnomeHelper` proben — wenn erreichbar, Extension ist aktiv.
    Fallback wenn Extension nicht aktivierbar: Positionen nicht verfügbar (Fenster-relative Koordinaten via AT-SPI `GetExtents(WINDOW)`, aber keine globalen Offsets), kein Highlight, Portal ScreenCast für Screenshots. (~80 LoC Rust)

26e. **GNOME-Versions-Kompatibilität**: Extension deklariert unterstützte Versionen in `metadata.json`. Bei Major-GNOME-Releases (z.B. 46→47) können API-Änderungen nötig sein — Extension ist bewusst minimal gehalten um Brüche zu minimieren. Die D-Bus-Schnittstelle ist stabil (eigenes Protokoll), nur die GJS-Interna müssen ggf. angepasst werden.

**Meilenstein 4:** `cargo nextest run -p platynui-platform-linux-wayland` — alle Backend-Traits getestet. Compositor-Typ-Erkennung (SO_PEERCRED) ermittelt Compositor (Mutter/KWin/Sway/Hyprland/COSMIC/PlatynUI) und wählt korrekte Backends. Input (EIS + Portal + wlr-virtual), Screenshots (ext-image-copy-capture + ext-image-capture-source + wlr-screencopy + Portal ScreenCast/WINDOW + GNOME Extension), WindowManager liefert Fenster-Positionen via Compositor-IPC (KWin Scripting + GNOME Extension + PlatynUI Control-Socket; Sway/Hyprland IPC optional später) an Koordinaten-Modul → AT-SPI-Provider hat korrekte absolute Koordinaten. Fenster-Aktionen (wlr-foreign + Compositor-IPC). Highlight (Layer-Shell + KWin showOutline + GNOME Extension + XWayland-Overlay-Fallback) funktionieren. Koordinaten-Transformation korrekt für Wayland-native und XWayland-Apps. Integration mit dem `platform-linux` Mediator: Wayland-Session wird automatisch erkannt und alle Aufrufe korrekt an das gewählte Backend delegiert.

---

### Phase 5: Eingebauter VNC/RDP-Server (~500 LoC, ~1 Woche) ⬜ OFFEN

*Ziel: Compositor ist direkt per VNC/RDP erreichbar — kein externer wayvnc nötig. Essenziell zum Debuggen von Headless-CI-Sessions: Tester kann sich remote verbinden und sehen was passiert.*

28. **VNC-Server** (`src/remote/vnc.rs`): `rustvncserver` — `update_framebuffer()` im Render-Cycle, Input-Events → Smithay-Stack. CLI-Flag `--vnc [port]` (Default: 5900). (~150 LoC)

29. **RDP-Server** (`src/remote/rdp.rs`): `ironrdp-server` — `RdpServerDisplay` + `RdpServerInputHandler`. TLS. CLI-Flag `--rdp [port]` (Default: 3389). (~200 LoC)

30. **Remote-Abstraktion** (`src/remote/mod.rs`): Frame-Updates an alle aktiven Remote-Sinks (VNC + RDP) verteilen. Input aus allen Quellen (Wayland-Seat + VNC + RDP + EIS + Virtual-Pointer) vereinheitlichen. (~100 LoC)

30b. **Transient-Seat** (`src/handlers/transient_seat.rs`): `ext-transient-seat-v1` — Separate Input-Seats für VNC/RDP-Remote-Clients, damit Remote-Input den lokalen Seat nicht stört. Smithay hat keinen fertigen Building Block, daher manuelle Implementierung mit `wayland-protocols`. (~30 LoC)

**Meilenstein 5:** `platynui-wayland-compositor --backend headless --vnc 5900 -- gtk4-demo` → VNC-Client verbindet sich → sieht die App → kann tippen und klicken. Gleich mit `--rdp`. Kein externer wayvnc nötig. CI-Debugging möglich.

---

### Phase 6: Integration + CI (~1 Woche) ⬜ OFFEN

*Ziel: PlatynUI-Gesamtsystem funktioniert unter Wayland — Provider, Platform-Crate, Compositor und CI-Pipeline sind integriert.*

31. **Link-Crate** — Bereits in Phase 3e aktualisiert: Linux-Arm linkt `platynui_platform_linux` (den Mediator) statt direkt `platynui_platform_linux_x11`. Der Mediator zieht transitiv das passende Sub-Platform-Crate. Hier nur noch verifizieren, dass die Integration end-to-end korrekt funktioniert (Runtime → Link → Mediator → Wayland-Platform → EIS/Screencopy/etc.).

32. **AT-SPI Provider** (`crates/provider-atspi/src/node.rs`): `GetExtents(WINDOW)` statt `SCREEN` unter Wayland.

33. **CI-Scripts**:
    - `scripts/startcompositor.sh` — eigenen Compositor starten (Backend auto-detect: winit bei vorhandenem Display, headless sonst). Isolierte Session mit eigenem `XDG_RUNTIME_DIR`, D-Bus, AT-SPI-Bus, `xdg-desktop-portal-gtk`. Compositor via `cargo run -p platynui-wayland-compositor`. Default-Session: `scripts/platynui-session.sh` (alacritty + wayvnc). CLI: `--backend`, `--xwayland`, `-- session-script`. Bereits implementiert (Phase 3a, Step 19f₆).
    - `scripts/startwaylandsession.sh` — Weston-basierte Session (bleibt für Weston-Tests)
    - `scripts/startxsession.sh` — Xephyr-basierte X11-Session

34. **Tests**:
    - `apps/wayland-compositor/tests/` — Protokoll-Tests als Wayland-Client
    - `crates/platform-linux-wayland/tests/` — Trait-Tests gegen eigenen Compositor (beide Input-Pfade: libei UND wlr-virtual)

35. *(Optional)* **Benchmarks** (`apps/wayland-compositor/benches/`): Performance-Messungen mit `criterion` — Frame-Time, Protokoll-Throughput, Screenshot-Latenz, VNC/RDP-Encoding. Kann jederzeit nachgerüstet werden. (~200 LoC)

**Meilenstein 6:** `cargo nextest run --all` — gesamte Suite grün, inkl. Wayland-Tests. CI-Scripts starten den Compositor, führen Tests aus und beenden sich sauber.

---

### Phase 7: Dokumentation (~1–2 Tage) ⬜ OFFEN

*Ziel: Nutzbar ohne mündliches Wissen — jeder Entwickler/CI-Engineer kann den Compositor einsetzen.*

36a. 🔧 **README** (`apps/wayland-compositor/README.md`): ~~Überblick, Architektur-Diagramm (ASCII), Quick-Start (Build + Run), alle CLI-Flags dokumentiert, Beispiele für jeden Backend-Modus (headless, winit, drm), VNC/RDP-Verbindungsanleitung, Test-Control-IPC-Protokoll-Referenz (JSON-Kommandos), Environment-Variablen.~~
    **Teilweise erledigt:** README umgeschrieben als Projekt-Überblick (Why?, Features, Quick Start, CI Usage, Doku-Links). Technische Details nach `docs/usage.md` (Backends, CLI-Flags, CI-Patterns) und `docs/configuration.md` (TOML-Referenz) verschoben. Compositor-ctl README ebenfalls überarbeitet. **Offen:** Architektur-Diagramm, VNC/RDP-Anleitung, vollständige IPC-Protokoll-Referenz (kommt in Phase 5/7).

36b. **Architektur-Doku** (`docs/compositor.md`): Tiefergehende Dokumentation — Modul-Übersicht, Protokoll-Matrix (welches Protokoll wo implementiert, Version), Rendering-Pipeline, Input-Routing-Diagramm (alle Input-Quellen: Wayland-Seat, VNC, RDP, EIS, Virtual-Pointer → Smithay Input-Stack), Frame-Lifecycle, Multi-Monitor-Setup.

36c. **CI-Integrations-Guide** (`docs/ci-compositor.md`): Anleitung für CI-Pipelines — Compositor starten, Readiness abwarten, Tests ausführen, VNC-Debug-Zugriff konfigurieren, Troubleshooting (häufige Fehler, Socket-Probleme, Timeout-Handling).

36d. **Platform-Crate-Doku**: `crates/platform-linux-wayland/README.md` — unterstützte Compositors, Protokoll-Fallback-Logik, Konfigurations-Optionen.

**Meilenstein 7:** Alle READMEs geschrieben. `docs/compositor.md` enthält Architektur-Diagramm. CI-Guide enthält Copy-Paste-fähige Beispiele.

---

### Phase 8: Portal + PipeWire (optional, ~800 LoC, ~1 Woche) ⬜ OFFEN

*Ziel: Standard-Linux-Desktop-Integration — Portal-API für Drittanbieter-Tools (obs-studio, GNOME-Screenshot), PipeWire für Screen-Sharing. Optional weil der eigene Compositor + libei den Hauptanwendungsfall bereits abdeckt.*

37. **Portal-Backend** (`src/portal/mod.rs`, `remote_desktop.rs`, `screen_cast.rs`): D-Bus-Service via `zbus`:
    - `org.freedesktop.impl.portal.RemoteDesktop` — `CreateSession`, `SelectDevices`, `Start`, **`ConnectToEIS()`** → gibt FD zum EIS-Server aus Step 17 (~300 LoC)
    - `org.freedesktop.impl.portal.ScreenCast` — `SelectSources`, `Start` → PipeWire Node-ID (~200 LoC)
    - Auto-Approve in CI (kein Consent-Dialog)

38. **PipeWire-Producer** (`src/pipewire.rs`): `pipewire` Rust-Bindings — Stream erstellen, Compositor-Framebuffer als PipeWire-Buffer publishen. Damage-basierte Updates. Wird von Portal ScreenCast referenziert. (~300 LoC)

**Meilenstein 8:** Portal `ConnectToEIS()` liefert funktionierenden EIS-FD. `obs-studio` kann via Portal ScreenCast den Compositor streamen. Platform-Crate hat zusätzlichen Portal-Fallback-Pfad für Input-Injection.

---

### Phase 9: Eingebautes Panel + App-Launcher (optional, ~400 LoC, ~2–3 Tage) ⬜ OFFEN

*Ziel: Self-contained Desktop-Erlebnis — Fenster-Liste, App-Starter, Uhr. Nicht für CI nötig (waybar via Layer-Shell reicht), aber nice-to-have für interaktive Nutzung.*

> **Hinweis:** Layer-Shell (Phase 3, Step 15) ermöglicht bereits `waybar` als externes Panel. Diese Phase ist nur relevant wenn ein eingebautes Panel ohne externe Abhängigkeit gewünscht ist.

39a. **Panel-Rendering** (`src/panel/mod.rs`, `src/panel/render.rs`): Internes Overlay am unteren Bildschirmrand via egui (gleicher `egui::Context` wie Titlebars). Exklusive Zone. CLI-Flag `--no-builtin-panel` zum Deaktivieren. (~100 LoC)

39b. **Fenster-Liste** (`src/panel/tasklist.rs`): egui-Buttons für jeden Toplevel. Aktives Fenster hervorgehoben. Minimierte Fenster mit Klick = Restore. **Migration von Interim-Minimize:** Taskleisten-Restore ersetzt Desktop-Klick-Restore. (~80 LoC)

39c. **App-Launcher** (`src/panel/launcher.rs`): Button öffnet egui-Overlay mit Befehlseingabe. Enter = ausführen. (~100 LoC)

39d. **Uhr + Keyboard-Layout-Indikator** (`src/panel/clock.rs`, `src/panel/keyboard_layout.rs`): HH:MM rechts in der Taskbar. Layout-Kürzel daneben, Klick = zyklischer Wechsel. (~60 LoC)

**Meilenstein 9:** Compositor startet mit optionaler Taskbar. Fenster-Buttons, Launcher, Uhr und Layout-Indikator funktionieren. Externes Panel via waybar weiterhin möglich.

---

**Verifikation**

- Nach jeder Phase: `cargo build --all --all-targets` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo nextest run --all --no-fail-fast`
- Phase 1: ✅ GTK4-App läuft im Compositor (nested via Winit), HiDPI/Fractional-Scale korrekt, Pointer-Constraints funktionieren, Keyboard-Shortcuts-Inhibit testbar, Graceful Shutdown via SIGTERM
- Phase 2: Fenster haben SSD-Titelleisten mit Close/Maximize/Minimize. XWayland-Apps laufen. DRM-Modus auf TTY funktioniert. Test-IPC ermöglicht Screenshot und Fenster-Kontrolle. Multi-Monitor mit 2+ Outputs funktioniert.
- Phase 2b: ✅ Config-Datei (`compositor.toml`) mit Font/Theme/Keyboard/Output-Structs. ✅ GPU-residente egui-Titlebars mit Hover-Highlighting. ✅ Child-Programm-Start nach `--`. ✅ Konsolidierung auf `GlowRenderer` (`PixmanRenderer` entfernt). ✅ Fullscreen-Support (Wayland + X11, SSD-Unterdrückung). ✅ Maximize via Protokoll + Doppelklick auf SSD-Titelleiste. ✅ Unmaximize-on-Drag (proportionale Cursor-Positionierung, SSD/CSD-aware Y-Positioning). ✅ Titelleisten-Kontextmenü (Rechtsklick → Minimize/Maximize/Close). ✅ `[[output]]`-Config → per-Output-Geometrie. ✅ Client-Cursor-Surface composited. ✅ Screenshot per-Output-Scale korrekt. ✅ `platynui-wayland-compositor-ctl` CLI-Tool (7 Subcommands). ✅ IPC-Protokoll dokumentiert. ✅ IPC-Tests grün (11 Unit + 17 Integration mit Client-Window-Tests). ✅ egui Test-App (`platynui-test-app-egui`): Wayland-Client mit breiter Widget-Palette + AccessKit/AT-SPI-Accessibility. ✅ `PLATYNUI_TEST_BACKEND=winit` für sichtbare Test-Ausführung.
- Phase 3: `platynui-cli` kann Fenster listen und Input senden. Screenshots via ext-image-copy-capture inkl. CursorSessions. `waybar`/ironbar laufen via Layer-Shell. `wayvnc` funktioniert als externer VNC-Server (Frame + Cursor Dual-Capture). Clipboard via `wl-copy`/`wl-paste` (data-control). Multi-Monitor dynamisch konfigurierbar (output-management).
- Phase 3a: ✅ ERLEDIGT. Control-Socket JSON via typisierter `serde`-Structs (19f). ~595 Zeilen Code-Duplikation eliminiert (19f₂). Kommentar-Review (19f₃). Focus-Loss Input Release (19f₄). Software-Cursor für SSD-Resize (19f₅). Session-Scripts AT-SPI-Fix (19f₆). Steps 19g–19z komplett: Protokoll-Korrektheit (Screencopy, Output-Management), Unwrap-Eliminierung, Error-Handling, Tracing, Dead Code, Magic Numbers, DRM Multi-Monitor-Positionierung. 1883 Tests grün.
- Phase 3a+: ✅ ERLEDIGT. Popup-Korrekturen (SSD, Layer-Shell, X11), VNC-Cursor-Rendering, Virtual-Pointer-Mapping, DRM Multi-Monitor-Overhaul, X11-Maximize-Größenwiederherstellung, Output-Resize-Reconfigure, Floating-Fenster-Clamping. ~14.500 LoC, 1883 Tests grün.
- Phase 3b: ✅ Tier 1 + Tier 2 + Tier 3 komplett (15 Protokolle, 43 Globals). ✅ tearing-control + toplevel-drag Stubs. ✅ Tier 3: toplevel-icon (volle Pixel-Pipeline mit SSD-Titlebar-Rendering), toplevel-tag (In-Memory-Speicherung), ext-foreign-toplevel-list (bereits in Phase 3). ✅ ext-data-control-v1 (standardisierte Clipboard-Kontrolle parallel zu wlr-data-control). ✅ EIS-Test-Client (Step 17b): `platynui-eis-test-client` mit Portal-Support (Mutter/KWin), Restore-Token-Persistenz (`persist_mode=2`), 13 Subcommands (inkl. Touch + Human-readable Keys + Shortcuts + **type-text**), interaktiver REPL-Modus (reedline, 14 Kommandos inkl. type-text), reis-Bug-Workaround (manueller EiEventConverter), ~1.780 LoC. ✅ `platynui-xkb-util` Crate (~506 LoC, 9 Tests): XKB-Reverse-Lookup (`KeymapLookup`: char→keycode+modifiers), `KeyAction` enum (`Simple`/`Compose`), Compose-Table-Support (Dead-Keys für Akzente/Sonderzeichen). ✅ EIS-Server (Step 17, ~370 LoC): Vollständiger EIS-Server mit allen Input-Capabilities (pointer, pointer_absolute, button, scroll, keyboard, touchscreen), XKB-Keymap-Propagation, Regions, Single-Client. ✅ Performance-Optimierung: Press/Release-Gap 20ms→2ms, Settle-Time 50ms→10ms, Modifier-Batching (~10× schneller). Gegen GNOME/Mutter validiert: move-by, click, key, scroll, type-text funktionieren. Erkenntnisse in `docs/eis-libei.md` dokumentiert. 1883 Tests grün.
- Phase 3c: ✅ ERLEDIGT. Winit-Fenster-Verbesserungen (Titel, Adwaita-CSD, System-Theme via zbus, eingebettetes Icon). Einheitliche `org.platynui.*` App-IDs. `.desktop`-Dateien + `just install-desktop`. Justfile als Task-Runner (~113 Zeilen) + `docs/development.md` (~95 Zeilen).
- Phase 3d: ✅ ERLEDIGT. Vollständige Touchscreen-Unterstützung: `seat.add_touch()`, Backend- + EIS-Touch-Handler mit shared `process_touch_*()` Funktionen, `surface_under_point()` Refactoring, `TouchMoveSurfaceGrab` (inkrementelle Deltas + Dead-Zone-Schutz) + `TouchResizeSurfaceGrab` (12 Resize-Richtungen), deferred SSD-Button-Aktionen mit Slot-Verifikation + Position-Tracking. Koordinaten via `combined_output_geometry()` (nicht Pointer-abhängig). Multi-Slot-Isolation. ~385 LoC über 4 Dateien, 27 Tests grün.
- Phase 3e: ✅ ERLEDIGT. `crates/platform-linux/` Mediator (~510 LoC, `lib.rs` + `session.rs`) — delegiert an X11 oder Wayland basierend auf `$XDG_SESSION_TYPE`/`$WAYLAND_DISPLAY`/`$DISPLAY`. `Resolved`-Struct mit 7 `&'static dyn Trait`-Referenzen, einmalig in `initialize()` befüllt, gecacht in `Mutex<Option<Resolved>>`. Wayland-Backends vollständig verdrahtet (7 Imports: `WlModule`, `WlPointer`, `WlKeyboard`, `WlDesktop`, `WlScreenshot`, `WlHighlight`, `WlWindowManager`), `SessionType::Wayland` match statt X11-Fallback. `platform-linux-x11` refactored: Selbstregistrierung + `inventory`-Dep entfernt, Module + Structs `pub` exportiert. Alle Consumers (Link, CLI, Inspector, Native, Playground) auf `platform-linux` umgestellt. 1902 Tests grün.
- Phase 4: 🔄 IN ARBEIT. Phase 4a (Fundament): `crates/platform-linux-wayland/` erstellt mit Wayland-Client-Connection, Compositor-Typ-Erkennung via `SO_PEERCRED` (`rustix::net::sockopt::socket_peercred` → PID → `/proc/<pid>/exe` → Binary-Name-Matching, Fallback `$XDG_CURRENT_DESKTOP`), `CompositorType`-Enum (PlatynUI/Mutter/KWin/Hyprland/Sway/Wlroots/Unknown), globaler State (`Mutex<Option<WaylandGlobal>>`), Stub-Implementierungen aller 7 Traits. ✅ Mediator-Integration: `platform-linux` delegiert korrekt an Wayland-Backends via `SessionType::Wayland` match (nicht mehr X11-Fallback). ✅ Desktop Info: Echte Monitor-Enumeration via `wl_output` + `xdg_output_manager_v1` mit `registry_queue_init` + Dispatch-Pattern, `OutputInfo` mit `effective_*()` Methoden, Union-Bounds-Berechnung. 1902 Tests grün. Nächste Schritte: Phase 4b (Input-Backends), 4c (Screenshots), 4d (WindowManager), 4e (Highlight).
- Phase 5: VNC/RDP eingebaut — Headless-Debugging ohne externe Tools möglich
- Phase 6: `cargo nextest run --all` — gesamte Suite grün, inkl. Wayland-Tests. CI-Scripts funktionieren.
- Phase 7: Alle READMEs und Architektur-Doku geschrieben.
- Phase 8: *(optional)* Portal `ConnectToEIS()` liefert FD, ScreenCast via PipeWire
- Phase 9: *(optional)* Eingebautes Panel als Alternative zu waybar

**Entscheidungen**

- **Reihenfolge smithay-fertig → SSD + Backends → Automation-Protokolle → Härtung → Rest-Protokolle → Platform-Mediator → Platform-Crate → VNC/RDP → Rest**: Core-Protokolle zuerst (Phase 1), dann SSD + XWayland + DRM + Test-Control (Phase 2), dann die PlatynUI-kritischen Automation-Protokolle (Phase 3: Layer-Shell, Foreign-Toplevel, Virtual-Input, Screencopy — Kern abgeschlossen), dann Härtung & Code-Qualität (Phase 3a: alle Code-Review-Findings, bevor technische Schulden sich akkumulieren), dann verbleibende Protokolle (Phase 3b: libei, optionale Stubs), dann der Platform-Linux-Mediator (Phase 3e) als Voraussetzung für Multi-Platform-Linux, dann das Wayland-Platform-Crate (Phase 4) das diese Protokolle nutzt, dann eingebauter VNC/RDP (Phase 5) für Headless-Debugging. Panel, Portal/PipeWire und Doku kommen bei Bedarf.
- **Platform-Linux Mediator-Architektur** (`crates/platform-linux/`): Linux kann entweder eine X11- oder eine Wayland-Session haben. Statt beide Sub-Platform-Crates sich selbst per `inventory` registrieren zu lassen (was bei `initialize_platform_modules()` zum Fehler führt, da diese Funktion beim ersten Fehler abbricht), gibt es ein delegierendes Mediator-Crate. Architektur:
  - Sub-Platform-Crates (`platform-linux-x11`, `platform-linux-wayland`) exportieren Device-Typen als öffentliche ZSTs (keine `pub static` Instanzen, keine `pub fn initialize()`), registrieren sich **nicht** im `inventory`
  - `platform-linux` registriert **ein** PlatformModule + **einen Satz** Wrapper-Devices
  - `initialize()` erkennt Session-Typ zur Laufzeit (`$XDG_SESSION_TYPE` > `$WAYLAND_DISPLAY` > `$DISPLAY`), baut `Resolved`-Struct mit 7 `&'static dyn Trait`-Referenzen, cacht in `Mutex<Option<Resolved>>`. Wayland fällt vorerst auf X11 zurück.
  - Wrapper-Devices greifen direkt auf das gecachte `Resolved`-Struct zu — kein erneutes Session-Matching, kein `?` für die Auflösung (panicked nur bei Programmierfehler: Zugriff vor `initialize()`)
  - Link-Crate linkt `platynui_platform_linux` (Mediator) statt direkt `platynui_platform_linux_x11`
  - Runtime und Core bleiben **unverändert** — der Mediator ist aus deren Sicht ein normales Platform-Modul
  - Provider (`platynui-provider-atspi`) bleibt für beide Sessions identisch
- **Panel auf unbestimmt verschoben**: Das eingebaute Panel (Taskbar, Launcher, Uhr) ist für PlatynUI's Kernmission — UI-Automation in CI — nicht nötig. `waybar` via Layer-Shell (Phase 3) deckt interaktive Nutzung ab. Interim-Minimize (Klick auf Desktop = Restore) ist für CI ausreichend.
- **wayvnc als Sofort-Lösung**: Nach Phase 3 kann `wayvnc` als externer VNC-Server genutzt werden (braucht `wlr-layer-shell` + `ext-image-copy-capture` + `zwlr_virtual_pointer`). Eingebauter VNC/RDP kommt erst in Phase 5 — bis dahin sind externe Tools verfügbar.
- **libei vor Portal**: EIS-Server (libei) wird direkt im Compositor implementiert (Phase 3), unabhängig vom Portal-Backend. Das Platform-Crate kann libei direkt nutzen. Portal (D-Bus + `ConnectToEIS()`) ist nur ein Wrapper und kommt optional in Phase 8.
- **Server-Side Decorations (SSD)**: Compositor rendert Titelleisten mit Close/Maximize/Minimize-Buttons für Apps die SSD anfordern (z.B. Kate/Qt-Apps). Apps die CSD bevorzugen (z.B. GTK4-LibAdwaita) behalten eigene Dekorationen. Rendering via egui GPU-resident `TextureRenderElement<GlesTexture>` auf `GlowRenderer` — einheitlich für alle Backends.
- **egui für Compositor-UI**: `egui` 0.33 + `egui_glow` 0.33 für Compositor-Titlebars. GPU-residenter Render-Pfad inspiriert von smithay-egui. Immediate-Mode-API vereinfacht UI-Implementierung. Echte Fonts mit Antialiasing und Unicode-Support.
- **Konsolidierung auf `GlowRenderer`**: Alle Render-Pfade (Winit, DRM, Headless, Screenshots) nutzen ausschließlich `GlowRenderer`. `PixmanRenderer` komplett entfernt. Software-Rendering bei Bedarf via `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe). DRM-Backend nutzt EGL-on-GBM. Screenshot-Format: Abgr8888 (GL-native RGBA-Byte-Order).
- **Einfaches Stacking-WM**: Kein Tiling, keine Workspaces — reicht für Test-Szenarien
- **`ironrdp-server`** (RDP) + **`rustvncserver`** (VNC): Pure Rust, Safe APIs, Apache-2.0, `unsafe_code = "deny"`-kompatibel
- **Keine optionalen Cargo Features**: Alle Backends (Winit, DRM, Headless) und XWayland werden bedingungslos kompiliert — weniger Komplexität, keine `#[cfg]`-Guards
- **Breite Protokoll-Abdeckung in Phase 1**: Alle gängigen Kompatibilitäts-Protokolle sofort verdrahten — verhindert mysteriöse App-Fehler
- **Multi-Monitor ab Phase 2**: `--outputs N` ermöglicht Multi-Monitor-Testszenarien
- **Graceful Shutdown + Watchdog**: SIGTERM-Handler + `--timeout` CLI-Flag verhindern Endlos-Hänger in CI
- **Koordinaten-Transformation im Platform-Crate**: Transparente Umrechnung Window-relative → absolute Koordinaten, mit Erkennung ob App unter Wayland-nativ oder XWayland läuft
- **DMA-BUF Support**: `linux-dmabuf-v1` für GPU-Buffer-Sharing — Chromium, Firefox, Electron, Vulkan-Apps
- **xdg-output-manager**: Logische Output-Infos die GTK4/Qt6 erwarten
- **xdg-foreign-v2**: Cross-App Window-Beziehungen — Portal-Dialoge, Datei-Öffnen als Child-Window
- **ext-transient-seat**: Separate Seats für VNC/RDP-Remote-Clients (Phase 5)
- **Cursor-Theme-Loading**: `$XCURSOR_THEME`/`$XCURSOR_SIZE` + `wp-cursor-shape-v1`
- **wp-security-context**: Flatpak/Sandbox-Unterstützung mit eingeschränktem Protokoll-Zugang
- **Readiness-Notification**: `--ready-fd` + `--print-env` für Race-freie CI-Integration
- **Compositor-Control CLI als eigenes Crate**: `platynui-wayland-compositor-ctl` — unabhängig deploybar, nur Socket + JSON
- **Socket-Cleanup**: Verwaiste Wayland-Sockets beim Start aufräumen
- **TOML-Konfigurationsdatei**: `$XDG_CONFIG_HOME/platynui/compositor.toml` — Font, Theme, Keyboard, Outputs
- **Dokumentation als Pflicht**: README, Architektur-Doku, CI-Guide — nicht optional, aber priorisiert nach funktionaler Vollständigkeit
- **egui Test-App als separates Crate**: `apps/test-app-egui/` — eframe 0.33 + AccessKit/AT-SPI. Dient als Wayland-Client für IPC-Tests und als Ziel-App für PlatynUI-Funktionstests.
- **`CARGO_BIN_EXE_` statt manueller Binary-Suche**: Integration-Tests referenzieren Binaries via Cargo-Umgebungsvariable
- **`PLATYNUI_TEST_BACKEND` Umgebungsvariable**: `headless` default, `winit` für sichtbare Tests
- **Benchmarks** *(optional)*: Frame-Time, Protokoll-Throughput, Screenshot-Latenz — kann jederzeit nachgerüstet werden
- **Data-Control in Phase 3**: `wlr-data-control-v1` ermöglicht Clipboard-Zugriff ohne Fenster-Fokus — essentiell für UI-Automation (Copy/Paste testen, Clipboard-Inhalt verifizieren). Smithay hat `delegate_data_control!()`, trivial zu verdrahten.
- **Output-Management in Phase 3**: `wlr-output-management-v1` ermöglicht dynamische Multi-Monitor-Konfiguration zur Laufzeit. Standard-Tooling (`wlr-randr`, `kanshi`) funktioniert damit. Nützlicher als nur statische `--outputs`/`[[output]]` Config.
- **Legacy-Screencopy übersprungen** *(im Compositor)*: `wlr-screencopy-v1` wird im eigenen Compositor nicht implementiert — `ext-image-copy-capture` deckt alle benötigten Tools ab (wayvnc, grim). **Im Platform-Crate** wird `wlr-screencopy` allerdings als Fallback-Backend berücksichtigt, da ältere wlroots-Compositors (Sway <1.10) nur dieses Protokoll unterstützen.
- **Tearing-Control + Content-Type als No-Op-Stubs**: `wp-tearing-control-v1` und `wp-content-type-hint-v1` sind triviale No-Op-Handler (~15 LoC je), verhindern aber "unsupported protocol"-Warnungen bei vielen Apps. Standard bei Sway/Hyprland.
- **Compositor-Typ-first statt Capability-Probing**: Statt 15+ Booleans blind zu proben (`ProtocolCapabilities`), wird zuerst der Compositor-Typ via `SO_PEERCRED` auf dem Wayland-Socket ermittelt (ein Syscall + ein Readlink). Der Compositor-Typ bestimmt dann direkt, welche Backends instanziiert werden (`create_backends()` mit `match compositor`). Jeder Backend-Konstruktor verbindet sich zu seinem Protokoll/D-Bus-Service und liefert `Err` wenn nicht verfügbar — `.or()`-Chaining wählt den Fallback. Vorteile:
  - Kein blindes Probing von Globals die nie gebraucht werden (z.B. kein wlr-screencopy-Check auf Mutter)
  - Compositor-Typ ist sofort verfügbar, bevor überhaupt ein `wl_registry` Roundtrip passiert
  - Klarer, lesbarer Code: `match compositor { Mutter => ..., KWin => ..., ... }` statt `if caps.has_X && caps.has_Y && !caps.has_Z`
  - `wl_registry`-Globals werden trotzdem beim Wayland-Roundtrip gesammelt und stehen den Wayland-Protokoll-Backends zur Verfügung — aber die **Entscheidung** welche Backends instanziiert werden, kommt vom Compositor-Typ
  - Neue Compositors = neuer Match-Arm, keine Änderung an bestehenden Backends
  - Fallback-Env (`$XDG_CURRENT_DESKTOP`) für Container/Sandboxes ohne `/proc`-Zugriff
- **PlatynUI GNOME Shell Extension**: Eigene GNOME Shell Extension (`platynui@platynui.org`, ~200–400 Zeilen GJS) als Lösung für alle drei Lücken auf Mutter/GNOME:
  - **Problem:** GNOME hat kein Layer-Shell (Highlight unmöglich), `Shell.Introspect` hat kein x/y (Positionen unbekannt), `Shell.Eval` ist ab GNOME 45+ eingeschränkt.
  - **Lösung:** Extension läuft im Mutter-Compositor-Prozess mit vollem Zugriff auf Meta/Clutter/St JavaScript API. Registriert D-Bus Interface `org.platynui.GnomeHelper` mit Methoden für Fenster-Liste (volle Geometrie via `Meta.Window.get_frame_rect()`), Highlight (`St.Widget` Overlay auf `global.window_group`), Screenshots (`Shell.Screenshot`) und Window-Aktionen.
  - **Auto-Installation:** Platform-Crate prüft beim Init ob Extension installiert und aktiv ist. Falls nicht: kopiert Extension nach `$XDG_DATA_HOME/gnome-shell/extensions/` und aktiviert via `gnome-extensions enable`. Graceful Fallback auf AT-SPI/Portal wenn Extension nicht verfügbar.
  - **Versions-Kompatibilität:** Extension unterstützt GNOME 45–48 via `metadata.json`. D-Bus-Interface ist stabil; nur GJS-Interna müssen bei Major-Releases ggf. angepasst werden.
  - **Sprache:** GJS (GNOME JavaScript), nicht Rust. Extension ist bewusst minimal (~200–400 Zeilen) um Wartungsaufwand und API-Bruch-Risiko zu minimieren.
- **AT-SPI liefert unter Wayland keine Fenster-Positionen**: AT-SPI `GetExtents(SCREEN)` gibt für Wayland-native Apps **nur fenster-relative Koordinaten** zurück, weil der AT-SPI-Provider im Toolkit (GTK, Qt) die globale Fensterposition nicht kennt (Wayland-Design: Clients erfahren ihre Position nicht). Deshalb braucht das Platform-Crate einen **WindowManager**, der die Fenster-Positionen via Compositor-IPC ermittelt (KWin Scripting, GNOME Extension, PlatynUI Control-Socket; Sway/Hyprland IPC optional später). Das Koordinaten-Modul kombiniert dann `WindowManager::bounds()` (globale Fenster-Position) + AT-SPI `GetExtents(WINDOW)` (Element-Offset relativ zum Fenster) → korrekte absolute Screen-Koordinaten. XWayland-Apps sind davon nicht betroffen (X11-Kontext hat globale Positionen).
- **KWin `showOutline` für Highlights**: KWin hat `workspace.showOutline(QRect { x, y, width, height })` + `workspace.hideOutline()` in der KWin Scripting API — zeichnet einen Compositor-Level Outline an beliebiger Position (wird intern für Snap-Assist und Window-Placement verwendet). Kein Layer-Shell nötig, korrekte Z-Order, kein Fokus-Wechsel. Offiziell supportete API.
- **KWin/Plasma eigene Wayland-Protokolle — evaluiert, nicht implementiert**: KWin exponiert über `plasma-wayland-protocols` drei Custom-Wayland-Protokolle: `org_kde_kwin_fake_input` (v6, Input), `zkde_screencast_unstable_v1` (v5, Screencasting), `org_kde_plasma_window_management` (v20, Fenster-Management). Nach Analyse kein Mehrwert für PlatynUI: (1) `fake_input` spart nur den initialen Consent-Dialog — Portal mit `restore_token` + `persist_mode=2` ist gleichwertig; (2) `zkde_screencast` wird intern vom Portal ScreenCast gewrapped, `ext-image-copy-capture` ist der bessere Standard; (3) `plasma_window_management` hat single-client-binding ("only one client can bind") — `plasmashell` belegt den Slot in jeder normalen Plasma-Session, das Protokoll ist im Hauptanwendungsfall nicht nutzbar. KWin Scripting D-Bus + Portal decken alles ab. ~470 LoC eingespart. Bei Bedarf nachträglich implementierbar (~120 LoC für `fake_input` als einfachster Kandidat).
  Mutter exponiert **keine vergleichbaren Custom-Wayland-Protokolle** — auf GNOME läuft alles über D-Bus-Interfaces und unsere GNOME Shell Extension.
- **Portal `restore_token` ist single-use**: Portal RemoteDesktop/ScreenCast `restore_token` ist ein Einmal-Token. Nach jeder Verwendung wird ein **neuer** Token in der Response zurückgegeben, der sofort persistent gespeichert werden muss. Token-Storage: `$XDG_DATA_HOME/platynui/portal_tokens.json`. `persist_mode=2` (persistent across sessions) minimiert Consent-Dialoge.
- **`org.gnome.Shell.Introspect` hat KEIN x/y**: Entgegen früherer Annahme liefert `Shell.Introspect.GetWindows()` nur `width` und `height` — **keine Position** (x/y). Für Positionen auf GNOME/Mutter wird die PlatynUI GNOME Extension oder AT-SPI `GetExtents(SCREEN)` genutzt.
- **`ext-layer-shell-v1` existiert noch nicht**: Anders als `wlr-layer-shell-v1` (von wlroots-Compositors unterstützt) gibt es `ext-layer-shell-v1` als wayland-protocols Extension noch nicht (Stand 2026-03). Nur `wlr-layer-shell-v1` ist verfügbar — auf Sway, Hyprland, PlatynUI-Compositor. Mutter und KWin unterstützen weder `wlr-` noch `ext-layer-shell`.
- **WindowManager ist der existierende Core-Trait**: Das Wayland-Platform-Crate implementiert den bestehenden `platynui_core::platform::WindowManager`-Trait (definiert in `crates/core/src/platform/window_manager.rs`), genau wie `X11EwmhWindowManager` in `platform-linux-x11`. Der Trait definiert bereits `resolve_window(&dyn UiNode) → WindowId`, `bounds(WindowId) → Rect`, `is_active()`, `activate()`, `close()`, `minimize()`, `maximize()`, `restore()`, `move_to()`, `resize()`. Der `provider-atspi` konsumiert ihn bereits via `window_managers().next()`. Es wird **kein neues Trait erfunden** — nur eine neue `WaylandWindowManager`-Implementierung, die intern an Compositor-spezifische `CompositorBackend`s delegiert (KWin D-Bus, GNOME Extension, PlatynUI Control-Socket; Sway i3-IPC und Hyprland IPC optional später). `resolve_window()` extrahiert PID + app_id + Titel aus dem `UiNode` (analog zu X11, wo PID + `_NET_WM_NAME` genutzt wird) und matcht gegen die Compositor-Fensterliste.
- **xdg-toplevel-drag für Browser-Kompatibilität**: Tab-Detach in Firefox/Chromium nutzt `xdg-toplevel-drag-v1`. Wird zunehmend adoptiert, zukunftssicher.
- **Zukunftsidee: Multi-Window Winit-Backend für Multi-Monitor**: Aktuell rendert das Winit-Backend alle Outputs in ein einzelnes Host-Fenster. Bei gemischten Scales (z.B. 1.0 + 2.0) muss ein einheitlicher `max_scale` für das gesamte Framebuffer verwendet werden — niedrig skalierte Outputs werden hochskaliert, Pointer-Mapping ist linear statt per-Output. Eine sauberere Architektur wäre ein separates Winit-Fenster pro Output: jedes Fenster rendert seinen Output mit eigenem Scale und eigenem `OutputDamageTracker`. Pointer-Mapping wird trivial (pro Fenster lokal). Smithays `WinitGraphicsBackend` unterstützt nur ein einzelnes Fenster; die Implementierung erfordert einen Custom-Backend (~300–400 LoC) mit eigenem `winit::EventLoop`, je einem `winit::Window` + `GlowRenderer` pro Output, und Pointer-Event-Routing anhand des aktiven Fensters. Vorteil: pixelgenaues Rendering bei gemischten Scales, natürliches Drag-and-Drop zwischen Fenstern, unabhängige Positionierung der Preview-Fenster auf dem Host-Desktop. Aufwand: mittelhoch, nicht blocking für CI-Workflows (Headless-Backend hat keine Scale-Probleme).
