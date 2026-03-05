## Plan: PlatynUI Wayland Compositor + Platform-Crate (Final, priorisiert)

**TL;DR:** Smithay-basierter Compositor (`apps/wayland-compositor/`, aktuell ~14.000 LoC, 1874 Tests, 43 Protokoll-Globals) + Wayland Platform-Crate (`crates/platform-linux-wayland/`). Die Implementierung folgt einer klaren Reihenfolge: erst smithay-fertige Core-Protokolle verdrahten (lauffähiger Compositor in Phase 1 ✅), dann SSD + XWayland + DRM + Test-Control (Phase 2 ✅), dann Automation-Protokolle für PlatynUI (Phase 3: Layer-Shell, Foreign-Toplevel, Virtual-Input, Screencopy — Kern abgeschlossen ✅), dann Härtung & Code-Qualität (Phase 3a ✅), dann Bugfixes & Window-Management-Verbesserungen (Phase 3a+ ✅), dann verbleibende Automation-Protokolle (Phase 3b: Tier 1+2+3 + Stubs komplett, libei + Test-Client noch offen — **nächster Schritt**), dann Desktop-Integration & Projekt-Tooling (Phase 3c ✅: Winit-Fenster, App-IDs, `.desktop`-Dateien, Justfile), dann das Platform-Crate (Phase 4), dann eingebauter VNC/RDP-Server für Headless-Debugging (Phase 5). Panel, Portal/PipeWire und Doku kommen danach bei Bedarf. Jede Phase endet mit einem testbaren Meilenstein.

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

### Phase 3b: Verbleibende Automation-Protokolle & Zusätzliche Protokoll-Unterstützung (~600–900 LoC) 🔧 IN ARBEIT

*Ziel: Restliche Protokoll-Features aus der ursprünglichen Phase 3 abschließen. Zusätzlich alle in smithay 0.7.0 verfügbaren Protokolle verdrahten, die für App-Kompatibilität und flüssigen Betrieb sinnvoll sind. Der Compositor soll gängige GTK4/Qt/Chromium/Firefox-Apps ohne Protokoll-Warnungen unterstützen.*

> **Protokoll-Gap-Analyse (2026-03-05, aktualisiert):** 43 implementierte Protokoll-Globals
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
> Verbleibend: EIS-Server (Step 17) + eigenständiger Test-Client (Step 17b).
> 3 Protokolle bewusst nicht implementiert (`drm-lease`, `drm-syncobj`, `kde-decoration`).
> ~14.000 LoC, 43 Protokolle, 1874 Tests.

**Bestehende Feature-Schritte (Reihenfolge: Test-Client zuerst, dann EIS-Server):**

> **Begründung der Reihenfolge:** Der Test-Client (17b) wird *vor* dem EIS-Server (17) implementiert. Damit können wir libei zuerst gegen existierende Compositors (Mutter/KWin) validieren — Handshake, Capabilities, Keymap, Input-Injection verstehen und debuggen — bevor wir unseren eigenen EIS-Server schreiben. Der Test-Client dient dann auch direkt als Testharness für Step 17.

17b. **Eigenständiger EIS-Test-Client** (`apps/eis-test-client/`): Separates Binary zum Testen und Debuggen von EIS-Servern — funktioniert mit Mutter (GNOME), KWin (KDE), und später unserem Compositor:
    - **Crate:** `apps/eis-test-client/` mit eigenem `Cargo.toml`. Deps: `reis`, `clap`, `xkbcommon`, `tracing`, `tracing-subscriber`, `zbus` (für Portal-Zugang zu Mutter/KWin).
    - **Verbindungsmodi:**
      - Portal: `--portal` → `org.freedesktop.portal.RemoteDesktop.CreateSession()` + `ConnectToEIS()` via `zbus` → bekommt FD zurück (Mutter, KWin). Interaktiver Dialog oder `--portal-restore-token` für headless.
      - Direkt: `--socket <path>` → `ei::Context` über Unix-Socket (unser Compositor, Sway-Fork)
      - Env: Default verbindet zu `$LIBEI_SOCKET` oder `$XDG_RUNTIME_DIR/eis-0`
    - **Kommandos (clap Subcommands):**
      - `probe` — Verbinden, Handshake durchführen, Seat/Capabilities/Regions/Keymap ausgeben und trennen. Diagnostik-Tool.
      - `move-to <x> <y>` — Absolute Pointer-Bewegung
      - `move-by <dx> <dy>` — Relative Pointer-Bewegung
      - `click [left|right|middle]` — Button press + release
      - `scroll <dx> <dy>` — Scroll-Event
      - `key <keyname>` — Taste drücken + loslassen (XKB-Name-Lookup über Keymap)
      - `type <text>` — Text tippen (Keymap-basiert: Zeichen → Keysym → Keycode + Modifier)
      - `sequence` — Mehrere Aktionen hintereinander: `move-to 500 300 click left type "Hello"`
    - **Diagnostik-Output:** `--verbose` zeigt alle EI-Protokoll-Messages (nutzt `REIS_DEBUG` intern).
    - **Portal-Support für Mutter/KWin:** Der Client verhandelt eine RemoteDesktop-Session über D-Bus Portal, ruft `ConnectToEIS()` auf und nutzt den zurückgegebenen FD als Transport. Damit funktioniert er out-of-the-box mit GNOME 45+ und KDE 6.1+.
    (~400–500 LoC)

17. **EIS-Server / libei** (`src/eis.rs`): Via `reis::eis` (Feature `calloop`) — vollständiger EIS-Server im Compositor. Erfahrungen aus Step 17b (Test-Client gegen Mutter/KWin) fließen direkt ein:
    - **Socket:** `$XDG_RUNTIME_DIR/eis-platynui`, `eis::Listener::bind()` + `EisListenerSource` in calloop Event-Loop
    - **Handshake:** Version-Negotiation, `context_type = sender` akzeptieren, Interface-Versionen aushandeln
    - **Seat + Capabilities:** Ein Seat mit allen Input-Capabilities: `ei_pointer` (relativ), `ei_pointer_absolute` (absolut mit Regions), `ei_button`, `ei_scroll`, `ei_keyboard`, `ei_touchscreen`
    - **Device-Lifecycle:** Device erstellen → `done` → `resumed` (sofort, kein Pause-Grund im Test-Compositor). Minimal-Pause/Resume: Protokoll-Pflicht erfüllt, aber nie aktiv pausiert.
    - **XKB-Keymap-Propagation:** Bei Keyboard-Capability die aktive Smithay-Keymap als memfd/tmpfile exportieren und per `ei_keyboard.keymap(fd, size)` an Client senden. Ohne Keymap ist Keyboard-Support sinnlos für reale Tests.
    - **Regions:** Für `ei_pointer_absolute` — eine Region pro Output mit korrektem Offset/Size/Scale. Mappt Client-Koordinaten auf Smithay-globale Koordinaten.
    - **Input-Injection:** Empfangene `ei_pointer.motion_relative`, `ei_pointer_absolute.motion_absolute`, `ei_button.button`, `ei_scroll.scroll`/`scroll_discrete`, `ei_keyboard.key`, `ei_touchscreen.*` Events in Smithay `InputBackend`-Events umwandeln und in den Input-Stack injizieren.
    - **Frame-Handling:** Events erst bei `ei_device.frame` verarbeiten (atomare Gruppierung).
    - **Single-Client:** Für V1 reicht ein gleichzeitiger Client. Multi-Client bei Bedarf nachrüsten (~100 LoC Mehraufwand: `HashMap<ClientId, EisClientState>`).
    - **Deps:** `reis = { version = "0.6", features = ["calloop"] }` in `apps/wayland-compositor/Cargo.toml`
    - **Testbarkeit:** Der Test-Client aus Step 17b dient als primäres Testmittel — `eis-test-client --socket $XDG_RUNTIME_DIR/eis-platynui probe` validiert den Handshake.
    Enables: Input-Injection über libei im Platform-Crate, Ökosystem-kompatibel mit Mutter/KWin. (~500–600 LoC)

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

**Meilenstein 3b (Zwischenstand):** ✅ Tier 1 komplett (commit-timing, fifo, idle-inhibit, xdg-dialog, system-bell, alpha-modifier). ✅ Tier 2 komplett (xwayland-shell, xwayland-keyboard-grab, pointer-gestures, tablet-v2, pointer-warp-v1). ✅ tearing-control + toplevel-drag als Stubs (manuelles GlobalDispatch/Dispatch, smithay 0.7 bietet keine Abstraktion). ✅ Tier 3 komplett (toplevel-icon, toplevel-tag, ext-foreign-toplevel-list). ✅ ext-data-control-v1 (standardisierte Clipboard-Kontrolle). ✅ 43 Protokoll-Globals, ~14.000 LoC, 1874 Tests. Alle gängigen GTK4/Qt/Chromium-Protokolle werden unterstützt — keine Protokoll-Warnungen bei Standard-Apps.

**Meilenstein 3b (Ziel):** Zusätzlich: libei-Input funktioniert (Step 17) — EIS-Server akzeptiert Clients, alle Input-Capabilities (pointer, pointer_absolute, button, scroll, keyboard, touchscreen) werden advertisiert und in Smithay injiziert. XKB-Keymap wird an Clients propagiert. Eigenständiger Test-Client (Step 17b) kann sich per Socket (eigener Compositor) oder Portal (Mutter/KWin) verbinden und Input emulieren. `ei-debug-events` zeigt korrekten Handshake und Device-Konfiguration. `WAYLAND_DISPLAY=... platynui-cli query "//control:*"` (über wlr-foreign-toplevel) listet Fenster.

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

### Phase 4: Platform-Crate (`crates/platform-linux-wayland/`, ~2.000 LoC, ~2 Wochen) ⬜ OFFEN

*Ziel: PlatynUI kann unter Wayland Fenster finden, Input injizieren, Screenshots machen und Highlight-Overlays anzeigen.*

20. **Crate anlegen** — Deps: `platynui-core`, `inventory`, `tracing`, `wayland-client`, `wayland-protocols` (Feature `staging`), `wayland-protocols-wlr`, `reis` (EI-Client), `xkbcommon`, `egui`, `smithay-egui`. Alles `#[cfg(target_os = "linux")]`.

21. **Connection + Protocol-Negotiation** (`src/lib.rs`, `src/connection.rs`): `WaylandPlatformModule` — Wayland-Display-Verbindung, `wl_registry` Scan → `ProtocolCapabilities`. `register_platform_module!()`. Erkennung des Session-Typs via `$XDG_SESSION_TYPE` und `/proc/{pid}/environ` (`GDK_BACKEND`, `QT_QPA_PLATFORM`) um pro App zu ermitteln ob sie unter Wayland-nativ oder XWayland läuft. (~250 LoC)

22. **Pointer** (`src/pointer.rs`): `WaylandPointerDevice` — Fallback-Kette:
    - `reis::ei` → EIS-Server (Mutter, KWin, eigener Compositor)
    - `zwlr_virtual_pointer_v1` (Sway, Hyprland, eigener Compositor)
    - `register_pointer_device!()` (~300 LoC)

23. **Keyboard** (`src/keyboard.rs`): `WaylandKeyboardDevice` — gleiche Fallback-Kette (libei / wlr-virtual). XKB-Keymap via `xkbcommon`. `register_keyboard_device!()` (~300 LoC)

24. **Window Manager** (`src/window_manager.rs`): Zwei-Schicht-Architektur:
    - Protokoll: `wlr-foreign-toplevel-management-v1` + `ext-foreign-toplevel-list-v1`, PID-Matching mit AT-SPI
    - Compositor-IPC (pluggable): Mutter (D-Bus), Sway (`swayipc`), KWin (D-Bus), PlatynUI-Compositor (Control-Socket)
    - `register_window_manager!()` (~400 LoC)

25. **Desktop Info** (`src/desktop.rs`): `wl_output` + `xdg_output_manager`. `register_desktop_info_provider!()` (~150 LoC)

26. **Screenshot** (`src/screenshot.rs`): `ext-image-copy-capture-v1` primär, Compositor-IPC Fallback. `register_screenshot_provider!()` (~250 LoC)

27. **Highlight** (`src/highlight.rs`): `wlr-layer-shell-v1` / `ext-layer-shell-v1` + egui-Rendering (farbige semi-transparente Rechtecke als Overlay). Command-Channel (Show/Clear). `register_highlight_provider!()` (~300 LoC)

27b. **Koordinaten-Transformation** (`src/coordinates.rs`): Unter Wayland liefert AT-SPI `GetExtents(WINDOW)` nur fenster-relative Koordinaten. Dieses Modul kombiniert:
- Window-Position vom `WindowManager` (via `wlr-foreign-toplevel` oder Compositor-IPC)
- Relative Koordinaten von AT-SPI `GetExtents(WINDOW)`
→ Absolute Screen-Koordinaten für `PointerDevice::move_to()` und `ScreenshotProvider::capture()`. Transparente Umrechnung, sodass der Rest des Platform-Crates nur mit absoluten Koordinaten arbeitet. (~100 LoC)

27c. **Wayland/XWayland-Erkennung pro App** (`src/session_detect.rs`): Liest `/proc/{pid}/environ` der Ziel-App um `GDK_BACKEND=wayland`, `QT_QPA_PLATFORM=wayland`, `MOZ_ENABLE_WAYLAND=1` etc. zu prüfen. Entscheidet ob AT-SPI `GetExtents(SCREEN)` (XWayland-App, Koordinaten stimmen) oder `GetExtents(WINDOW)` + Fenster-Offset (Wayland-native App) verwendet wird. (~80 LoC)

**Meilenstein 4:** `cargo nextest run -p platynui-platform-linux-wayland` — alle Traits getestet. Pointer/Keyboard-Input über libei und wlr-virtual funktioniert. Fenster-Liste via Foreign-Toplevel. Screenshots via ext-image-copy-capture. Highlight-Overlays via Layer-Shell. Koordinaten-Transformation korrekt für Wayland-native und XWayland-Apps.

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

31. **Link-Crate** (`crates/link/src/lib.rs`): `platynui_link_os_providers!()` Linux-Arm erweitern — beide Platform-Crates (X11 + Wayland) linken. Laufzeit-Mediation via `$XDG_SESSION_TYPE` in `PlatformModule::initialize()`.

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
- Phase 3a: ✅ ERLEDIGT. Control-Socket JSON via typisierter `serde`-Structs (19f). ~595 Zeilen Code-Duplikation eliminiert (19f₂). Kommentar-Review (19f₃). Focus-Loss Input Release (19f₄). Software-Cursor für SSD-Resize (19f₅). Session-Scripts AT-SPI-Fix (19f₆). Steps 19g–19z komplett: Protokoll-Korrektheit (Screencopy, Output-Management), Unwrap-Eliminierung, Error-Handling, Tracing, Dead Code, Magic Numbers, DRM Multi-Monitor-Positionierung. 1874 Tests grün.
- Phase 3a+: ✅ ERLEDIGT. Popup-Korrekturen (SSD, Layer-Shell, X11), VNC-Cursor-Rendering, Virtual-Pointer-Mapping, DRM Multi-Monitor-Overhaul, X11-Maximize-Größenwiederherstellung, Output-Resize-Reconfigure, Floating-Fenster-Clamping. ~14.500 LoC, 1874 Tests grün.
- Phase 3b: ✅ Tier 1 + Tier 2 + Tier 3 komplett (15 Protokolle, 43 Globals). ✅ tearing-control + toplevel-drag Stubs. ✅ Tier 3: toplevel-icon (volle Pixel-Pipeline mit SSD-Titlebar-Rendering), toplevel-tag (In-Memory-Speicherung), ext-foreign-toplevel-list (bereits in Phase 3). ✅ ext-data-control-v1 (standardisierte Clipboard-Kontrolle parallel zu wlr-data-control). libei-Input noch offen (Step 17: EIS-Server mit allen Input-Capabilities + XKB-Keymap, Step 17b: eigenständiger Test-Client mit Portal-Support für Mutter/KWin).
- Phase 4: `cargo nextest run -p platynui-platform-linux-wayland` — alle Traits getestet, Koordinaten-Transformation korrekt für Wayland-native und XWayland-Apps
- Phase 5: VNC/RDP eingebaut — Headless-Debugging ohne externe Tools möglich
- Phase 6: `cargo nextest run --all` — gesamte Suite grün, inkl. Wayland-Tests. CI-Scripts funktionieren.
- Phase 7: Alle READMEs und Architektur-Doku geschrieben.
- Phase 8: *(optional)* Portal `ConnectToEIS()` liefert FD, ScreenCast via PipeWire
- Phase 9: *(optional)* Eingebautes Panel als Alternative zu waybar

**Entscheidungen**

- **Reihenfolge smithay-fertig → SSD + Backends → Automation-Protokolle → Härtung → Rest-Protokolle → Platform-Crate → VNC/RDP → Rest**: Core-Protokolle zuerst (Phase 1), dann SSD + XWayland + DRM + Test-Control (Phase 2), dann die PlatynUI-kritischen Automation-Protokolle (Phase 3: Layer-Shell, Foreign-Toplevel, Virtual-Input, Screencopy — Kern abgeschlossen), dann Härtung & Code-Qualität (Phase 3a: alle Code-Review-Findings, bevor technische Schulden sich akkumulieren), dann verbleibende Protokolle (Phase 3b: libei, optionale Stubs), dann das Platform-Crate (Phase 4) das diese Protokolle nutzt, dann eingebauter VNC/RDP (Phase 5) für Headless-Debugging. Panel, Portal/PipeWire und Doku kommen bei Bedarf.
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
- **Legacy-Screencopy übersprungen**: `wlr-screencopy-v1` wird nicht implementiert — `ext-image-copy-capture` deckt alle benötigten Tools ab (wayvnc, grim).
- **Tearing-Control + Content-Type als No-Op-Stubs**: `wp-tearing-control-v1` und `wp-content-type-hint-v1` sind triviale No-Op-Handler (~15 LoC je), verhindern aber "unsupported protocol"-Warnungen bei vielen Apps. Standard bei Sway/Hyprland.
- **xdg-toplevel-drag für Browser-Kompatibilität**: Tab-Detach in Firefox/Chromium nutzt `xdg-toplevel-drag-v1`. Wird zunehmend adoptiert, zukunftssicher.
- **Zukunftsidee: Multi-Window Winit-Backend für Multi-Monitor**: Aktuell rendert das Winit-Backend alle Outputs in ein einzelnes Host-Fenster. Bei gemischten Scales (z.B. 1.0 + 2.0) muss ein einheitlicher `max_scale` für das gesamte Framebuffer verwendet werden — niedrig skalierte Outputs werden hochskaliert, Pointer-Mapping ist linear statt per-Output. Eine sauberere Architektur wäre ein separates Winit-Fenster pro Output: jedes Fenster rendert seinen Output mit eigenem Scale und eigenem `OutputDamageTracker`. Pointer-Mapping wird trivial (pro Fenster lokal). Smithays `WinitGraphicsBackend` unterstützt nur ein einzelnes Fenster; die Implementierung erfordert einen Custom-Backend (~300–400 LoC) mit eigenem `winit::EventLoop`, je einem `winit::Window` + `GlowRenderer` pro Output, und Pointer-Event-Routing anhand des aktiven Fensters. Vorteil: pixelgenaues Rendering bei gemischten Scales, natürliches Drag-and-Drop zwischen Fenstern, unabhängige Positionierung der Preview-Fenster auf dem Host-Desktop. Aufwand: mittelhoch, nicht blocking für CI-Workflows (Headless-Backend hat keine Scale-Probleme).
