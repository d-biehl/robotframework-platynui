## Plan: PlatynUI Wayland Compositor + Platform-Crate (Final, priorisiert)

**TL;DR:** Smithay-basierter Compositor (`apps/wayland-compositor/`, aktuell ~7.900 LoC) + Wayland Platform-Crate (`crates/platform-linux-wayland/`). Die Implementierung folgt einer klaren Reihenfolge: erst smithay-fertige Core-Protokolle verdrahten (lauffähiger Compositor in Phase 1), dann SSD + XWayland + DRM + Test-Control (Phase 2), dann Automation-Protokolle für PlatynUI (Phase 3: Layer-Shell, Foreign-Toplevel, libei, Virtual-Input, Screencopy), dann das Platform-Crate (Phase 4), dann eingebauter VNC/RDP-Server für Headless-Debugging (Phase 5). Panel, Portal/PipeWire und Doku kommen danach bei Bedarf. Jede Phase endet mit einem testbaren Meilenstein.

---

**Steps**

### Phase 1: Lauffähiger Minimal-Compositor (~1.500 LoC, ~1 Woche) ✅ DONE

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

### Phase 2: SSD + XWayland + DRM + Test-Control (~1.400 LoC, ~1.5 Wochen) ✅ DONE

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
> **Hinweis Minimize (Interim):** Minimierte Fenster werden aktuell via `space.unmap_elem()` aus dem
> Space entfernt und in `state.minimized_windows` gespeichert. Restore erfolgt durch Klick auf leere
> Desktop-Fläche (stellt das zuletzt minimierte Fenster wieder her). Dieses Verhalten ist ein
> pragmatischer Workaround ohne Panel — in Phase 3 (Step 14b) wird Minimize über die Taskleiste
> implementiert (Klick auf Fenster-Button = Restore), analog zu GNOME/KDE/Windows.
>
> **Hinweis Maximize:** Maximize speichert die Fenster-Position in `state.pre_maximize_positions`
> vor dem Maximieren. Unmaximize (erneuter Klick auf Maximize-Button) stellt die
> ursprüngliche Position wieder her.
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

### Phase 2b: Härtung & Verfeinerung (~650 LoC, ~3–4 Tage) ✅ COMPLETE

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

### Phase 3: Automation-Protokolle (~2.100 LoC, ~2–3 Wochen) ⬜ TODO

*Ziel: Alle Wayland-Protokolle, die PlatynUI und externe Tools (wayvnc, waybar, wl-clipboard, wlr-randr) brauchen, sind im Compositor verfügbar. Nach dieser Phase kann man mit `wayvnc` auf den Compositor zugreifen, mit `waybar` ein externes Panel nutzen, und Clipboard programmatisch lesen/schreiben.*

15. **Layer-Shell** (`src/handlers/layer_shell.rs`): `wlr-layer-shell-v1` verdrahten (smithay hat Building Blocks). Enables: waybar (externes Panel), Highlight-Overlays für PlatynUI, wayvnc-Overlays. Exklusive Zonen korrekt verrechnen (Fenster nicht unter dem Panel platzieren). (~80 LoC)

16. **Foreign-Toplevel-Management** (`src/protocols/toplevel.rs`): `wlr-foreign-toplevel-management-v1` Server — publisht alle Toplevels mit Titel/App-ID/State, verarbeitet activate/close/minimize/maximize/fullscreen Requests. + `ext-foreign-toplevel-list-v1` (read-only, smithay stellt Teile bereit). Enables: `platynui-cli query` über wlr-foreign-toplevel, Fenster-Buttons in waybar. (~400 LoC)

17. **EIS-Server / libei** (`src/eis.rs`): Via `reis::eis` — EIS-Endpoint erstellen, Capabilities (pointer_absolute, keyboard) advertisieren, Input-Events empfangen und in Smithay-Stack injizieren. Socket unter `$XDG_RUNTIME_DIR/eis-platynui`. Enables: Input-Injection über libei im Platform-Crate, Ökosystem-kompatibel mit Mutter/KWin. (~300 LoC)

18. **Virtual-Pointer + Virtual-Keyboard** (`src/protocols/virtual_pointer.rs`, `src/protocols/virtual_keyboard.rs`):
    - `zwlr_virtual_pointer_v1` — empfängt absolute/relative Motion, Button, Axis-Events und injiziert sie in den Smithay Input-Stack. (~300 LoC)
    - `zwp_virtual_keyboard_v1` Server verdrahten, XKB-Keymap-Upload akzeptieren. Smithay hat Teile. (~50 LoC)
    - Enables: Fallback-Input-Pfad für Sway/Hyprland-Kompatibilität im Platform-Crate.

19. **Screencopy-Server** (`src/protocols/screencopy.rs`): `ext-image-copy-capture-v1` — Framebuffer als `wl_shm`-Buffer an Client liefern. Damage-basiert. Enables: wayvnc (als externer VNC-Server), Screenshot im Platform-Crate. (~500 LoC, aufwändigstes Protokoll)

19b. **Data-Control** (`src/handlers/data_control.rs`): `wlr-data-control-v1` verdrahten — ermöglicht Clipboard lesen/schreiben ohne Fenster-Fokus. Smithay hat `delegate_data_control!()`. Enables: `wl-copy`/`wl-paste`, programmatisches Clipboard-Testing im Platform-Crate, Clipboard-Verifikation in Tests. (~50 LoC)

19c. **Output-Management** (`src/handlers/output_management.rs`): `wlr-output-management-v1` — Outputs zur Laufzeit konfigurieren (Resolution, Position, Scale, Transform, Enable/Disable). Smithay hat Building Blocks. Enables: `wlr-randr`/`kanshi`, dynamische Multi-Monitor-Tests ohne Compositor-Neustart, Output hinzufügen/entfernen im laufenden Betrieb. (~150 LoC)

19d. *(Optional)* **Legacy-Screencopy** (`src/handlers/screencopy_legacy.rs`): `wlr-screencopy-v1` — ältere Screencopy-API für Tools die `ext-image-copy-capture` noch nicht unterstützen (ältere `grim`-Versionen, manche wayvnc-Builds). Smithay hat Building Blocks. Safety-Net, kann übersprungen werden wenn `ext-image-copy-capture` ausreicht. (~200 LoC)

19e. *(Optional)* **App-Kompatibilitäts-Stubs** (`src/handlers/tearing_control.rs`, `src/handlers/content_type.rs`, `src/handlers/toplevel_drag.rs`): No-Op-Stubs für Protokolle die viele Apps abfragen:
    - `wp-tearing-control-v1` — Tearing-Hint für Games. No-Op-Stub verhindert Protocol-Warnungen. (~15 LoC)
    - `wp-content-type-hint-v1` — Content-Type für Rendering-Optimierung (Game/Video/Photo). No-Op-Stub. (~15 LoC)
    - `xdg-toplevel-drag-v1` — Tab-Detach in Browsern (Firefox/Chromium), Drag-aus-Fenster. Wird zunehmend adoptiert. (~80 LoC)

> **Hinweis wayvnc:** Nach Abschluss dieser Phase kann `wayvnc` als externer VNC-Server genutzt werden (braucht `wlr-layer-shell` + `ext-image-copy-capture` + `zwlr_virtual_pointer`). Dies ermöglicht sofortiges Remote-Debugging von Headless-Sessions ohne eingebauten VNC-Server. Befehl: `WAYLAND_DISPLAY=... wayvnc 0.0.0.0 5900`.

**Meilenstein 3:** `WAYLAND_DISPLAY=... platynui-cli query "//control:*"` (über wlr-foreign-toplevel) listet Fenster. Virtual-Pointer/Keyboard und libei-Input funktionieren. Screenshot via ext-image-copy-capture. `waybar` (extern) funktioniert via Layer-Shell. `wayvnc` kann sich verbinden und die Session anzeigen + fernsteuern. Clipboard über `wl-copy`/`wl-paste` lesbar/schreibbar. Multi-Monitor per `wlr-randr` dynamisch konfigurierbar.

---

### Phase 4: Platform-Crate (`crates/platform-linux-wayland/`, ~2.000 LoC, ~2 Wochen) ⬜ TODO

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

### Phase 5: Eingebauter VNC/RDP-Server (~500 LoC, ~1 Woche) ⬜ TODO

*Ziel: Compositor ist direkt per VNC/RDP erreichbar — kein externer wayvnc nötig. Essenziell zum Debuggen von Headless-CI-Sessions: Tester kann sich remote verbinden und sehen was passiert.*

28. **VNC-Server** (`src/remote/vnc.rs`): `rustvncserver` — `update_framebuffer()` im Render-Cycle, Input-Events → Smithay-Stack. CLI-Flag `--vnc [port]` (Default: 5900). (~150 LoC)

29. **RDP-Server** (`src/remote/rdp.rs`): `ironrdp-server` — `RdpServerDisplay` + `RdpServerInputHandler`. TLS. CLI-Flag `--rdp [port]` (Default: 3389). (~200 LoC)

30. **Remote-Abstraktion** (`src/remote/mod.rs`): Frame-Updates an alle aktiven Remote-Sinks (VNC + RDP) verteilen. Input aus allen Quellen (Wayland-Seat + VNC + RDP + EIS + Virtual-Pointer) vereinheitlichen. (~100 LoC)

30b. **Transient-Seat** (`src/handlers/transient_seat.rs`): `ext-transient-seat-v1` — Separate Input-Seats für VNC/RDP-Remote-Clients, damit Remote-Input den lokalen Seat nicht stört. Smithay hat keinen fertigen Building Block, daher manuelle Implementierung mit `wayland-protocols`. (~30 LoC)

**Meilenstein 5:** `platynui-wayland-compositor --backend headless --vnc 5900 -- gtk4-demo` → VNC-Client verbindet sich → sieht die App → kann tippen und klicken. Gleich mit `--rdp`. Kein externer wayvnc nötig. CI-Debugging möglich.

---

### Phase 6: Integration + CI (~1 Woche) ⬜ TODO

*Ziel: PlatynUI-Gesamtsystem funktioniert unter Wayland — Provider, Platform-Crate, Compositor und CI-Pipeline sind integriert.*

31. **Link-Crate** (`crates/link/src/lib.rs`): `platynui_link_os_providers!()` Linux-Arm erweitern — beide Platform-Crates (X11 + Wayland) linken. Laufzeit-Mediation via `$XDG_SESSION_TYPE` in `PlatformModule::initialize()`.

32. **AT-SPI Provider** (`crates/provider-atspi/src/node.rs`): `GetExtents(WINDOW)` statt `SCREEN` unter Wayland.

33. **CI-Scripts**:
    - `scripts/startcompositor.sh` — eigenen Compositor headless starten (mit `--vnc` für Debug-Zugriff)
    - `scripts/startwaylandsession.sh` bleibt für Weston-Tests

34. **Tests**:
    - `apps/wayland-compositor/tests/` — Protokoll-Tests als Wayland-Client
    - `crates/platform-linux-wayland/tests/` — Trait-Tests gegen eigenen Compositor (beide Input-Pfade: libei UND wlr-virtual)

35. *(Optional)* **Benchmarks** (`apps/wayland-compositor/benches/`): Performance-Messungen mit `criterion` — Frame-Time, Protokoll-Throughput, Screenshot-Latenz, VNC/RDP-Encoding. Kann jederzeit nachgerüstet werden. (~200 LoC)

**Meilenstein 6:** `cargo nextest run --all` — gesamte Suite grün, inkl. Wayland-Tests. CI-Scripts starten den Compositor, führen Tests aus und beenden sich sauber.

---

### Phase 7: Dokumentation (~1–2 Tage) ⬜ TODO

*Ziel: Nutzbar ohne mündliches Wissen — jeder Entwickler/CI-Engineer kann den Compositor einsetzen.*

36a. **README** (`apps/wayland-compositor/README.md`): Überblick, Architektur-Diagramm (ASCII), Quick-Start (Build + Run), alle CLI-Flags dokumentiert, Beispiele für jeden Backend-Modus (headless, winit, drm), VNC/RDP-Verbindungsanleitung, Test-Control-IPC-Protokoll-Referenz (JSON-Kommandos), Environment-Variablen.

36b. **Architektur-Doku** (`docs/compositor.md`): Tiefergehende Dokumentation — Modul-Übersicht, Protokoll-Matrix (welches Protokoll wo implementiert, Version), Rendering-Pipeline, Input-Routing-Diagramm (alle Input-Quellen: Wayland-Seat, VNC, RDP, EIS, Virtual-Pointer → Smithay Input-Stack), Frame-Lifecycle, Multi-Monitor-Setup.

36c. **CI-Integrations-Guide** (`docs/ci-compositor.md`): Anleitung für CI-Pipelines — Compositor starten, Readiness abwarten, Tests ausführen, VNC-Debug-Zugriff konfigurieren, Troubleshooting (häufige Fehler, Socket-Probleme, Timeout-Handling).

36d. **Platform-Crate-Doku**: `crates/platform-linux-wayland/README.md` — unterstützte Compositors, Protokoll-Fallback-Logik, Konfigurations-Optionen.

**Meilenstein 7:** Alle READMEs geschrieben. `docs/compositor.md` enthält Architektur-Diagramm. CI-Guide enthält Copy-Paste-fähige Beispiele.

---

### Phase 8: Portal + PipeWire (optional, ~800 LoC, ~1 Woche) ⬜ TODO

*Ziel: Standard-Linux-Desktop-Integration — Portal-API für Drittanbieter-Tools (obs-studio, GNOME-Screenshot), PipeWire für Screen-Sharing. Optional weil der eigene Compositor + libei den Hauptanwendungsfall bereits abdeckt.*

37. **Portal-Backend** (`src/portal/mod.rs`, `remote_desktop.rs`, `screen_cast.rs`): D-Bus-Service via `zbus`:
    - `org.freedesktop.impl.portal.RemoteDesktop` — `CreateSession`, `SelectDevices`, `Start`, **`ConnectToEIS()`** → gibt FD zum EIS-Server aus Step 17 (~300 LoC)
    - `org.freedesktop.impl.portal.ScreenCast` — `SelectSources`, `Start` → PipeWire Node-ID (~200 LoC)
    - Auto-Approve in CI (kein Consent-Dialog)

38. **PipeWire-Producer** (`src/pipewire.rs`): `pipewire` Rust-Bindings — Stream erstellen, Compositor-Framebuffer als PipeWire-Buffer publishen. Damage-basierte Updates. Wird von Portal ScreenCast referenziert. (~300 LoC)

**Meilenstein 8:** Portal `ConnectToEIS()` liefert funktionierenden EIS-FD. `obs-studio` kann via Portal ScreenCast den Compositor streamen. Platform-Crate hat zusätzlichen Portal-Fallback-Pfad für Input-Injection.

---

### Phase 9: Eingebautes Panel + App-Launcher (optional, ~400 LoC, ~2–3 Tage) ⬜ TODO

*Ziel: Self-contained Desktop-Erlebnis — Fenster-Liste, App-Starter, Uhr. Nicht für CI nötig (waybar via Layer-Shell reicht), aber nice-to-have für interaktive Nutzung.*

> **Hinweis:** Layer-Shell (Phase 3, Step 15) ermöglicht bereits `waybar` als externes Panel. Diese Phase ist nur relevant wenn ein eingebautes Panel ohne externe Abhängigkeit gewünscht ist.

39a. **Panel-Rendering** (`src/panel/mod.rs`, `src/panel/render.rs`): Internes Overlay am unteren Bildschirmrand via egui (gleicher `egui::Context` wie Titlebars). Exklusive Zone. CLI-Flag `--no-builtin-panel` zum Deaktivieren. (~100 LoC)

39b. **Fenster-Liste** (`src/panel/tasklist.rs`): egui-Buttons für jeden Toplevel. Aktives Fenster hervorgehoben. Minimierte Fenster mit Klick = Restore. **Migration von Interim-Minimize:** Taskleisten-Restore ersetzt Desktop-Klick-Restore. (~80 LoC)

39c. **App-Launcher** (`src/panel/launcher.rs`): Button öffnet egui-Overlay mit Befehlseingabe. Enter = ausführen. (~100 LoC)

39d. **Uhr + Keyboard-Layout-Indikator** (`src/panel/clock.rs`, `src/panel/keyboard_layout.rs`): HH:MM rechts in der Taskbar. Layout-Kürzel daneben, Klick = zyklischer Wechsel. (~60 LoC)

**Meilenstein 9:** Compositor startet mit optionaler Taskbar. Fenster-Buttons, Launcher, Uhr und Layout-Indikator funktionieren. Externes Panel via waybar weiterhin möglich.

---

**Verification**

- Nach jeder Phase: `cargo build --all --all-targets` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo nextest run --all --no-fail-fast`
- Phase 1: ✅ GTK4-App läuft im Compositor (nested via Winit), HiDPI/Fractional-Scale korrekt, Pointer-Constraints funktionieren, Keyboard-Shortcuts-Inhibit testbar, Graceful Shutdown via SIGTERM
- Phase 2: Fenster haben SSD-Titelleisten mit Close/Maximize/Minimize. XWayland-Apps laufen. DRM-Modus auf TTY funktioniert. Test-IPC ermöglicht Screenshot und Fenster-Kontrolle. Multi-Monitor mit 2+ Outputs funktioniert.
- Phase 2b: ✅ Config-Datei (`compositor.toml`) mit Font/Theme/Keyboard/Output-Structs. ✅ GPU-residente egui-Titlebars mit Hover-Highlighting. ✅ Child-Programm-Start nach `--`. ✅ Konsolidierung auf `GlowRenderer` (`PixmanRenderer` entfernt). ✅ Fullscreen-Support (Wayland + X11, SSD-Unterdrückung). ✅ Maximize via Protokoll + Doppelklick auf SSD-Titelleiste. ✅ Unmaximize-on-Drag (proportionale Cursor-Positionierung, SSD/CSD-aware Y-Positioning). ✅ Titelleisten-Kontextmenü (Rechtsklick → Minimize/Maximize/Close). ✅ `[[output]]`-Config → per-Output-Geometrie. ✅ Client-Cursor-Surface composited. ✅ Screenshot per-Output-Scale korrekt. ✅ `platynui-wayland-compositor-ctl` CLI-Tool (7 Subcommands). ✅ IPC-Protokoll dokumentiert. ✅ IPC-Tests grün (11 Unit + 17 Integration mit Client-Window-Tests). ✅ egui Test-App (`platynui-test-app-egui`): Wayland-Client mit breiter Widget-Palette + AccessKit/AT-SPI-Accessibility. ✅ `PLATYNUI_TEST_BACKEND=winit` für sichtbare Test-Ausführung.
- Phase 3: `platynui-cli` kann Fenster listen und Input senden. Screenshots via ext-image-copy-capture. `waybar` läuft via Layer-Shell. `wayvnc` funktioniert als externer VNC-Server. Clipboard via `wl-copy`/`wl-paste` (data-control). Multi-Monitor dynamisch konfigurierbar (output-management).
- Phase 4: `cargo nextest run -p platynui-platform-linux-wayland` — alle Traits getestet, Koordinaten-Transformation korrekt für Wayland-native und XWayland-Apps
- Phase 5: VNC/RDP eingebaut — Headless-Debugging ohne externe Tools möglich
- Phase 6: `cargo nextest run --all` — gesamte Suite grün, inkl. Wayland-Tests. CI-Scripts funktionieren.
- Phase 7: Alle READMEs und Architektur-Doku geschrieben.
- Phase 8: *(optional)* Portal `ConnectToEIS()` liefert FD, ScreenCast via PipeWire
- Phase 9: *(optional)* Eingebautes Panel als Alternative zu waybar

**Decisions**

- **Reihenfolge smithay-fertig → SSD + Backends → Automation-Protokolle → Platform-Crate → VNC/RDP → Rest**: Core-Protokolle zuerst (Phase 1), dann SSD + XWayland + DRM + Test-Control (Phase 2), dann die PlatynUI-kritischen Automation-Protokolle (Phase 3: Layer-Shell, Foreign-Toplevel, libei, Virtual-Input, Screencopy), dann das Platform-Crate (Phase 4) das diese Protokolle nutzt, dann eingebauter VNC/RDP (Phase 5) für Headless-Debugging. Panel, Portal/PipeWire und Doku kommen bei Bedarf.
- **Panel auf unbestimmt verschoben**: Das eingebaute Panel (Taskbar, Launcher, Uhr) ist für PlatynUI's Kernmission — UI-Automation in CI — nicht nötig. `waybar` via Layer-Shell (Phase 3) deckt interaktive Nutzung ab. Interim-Minimize (Klick auf Desktop = Restore) ist für CI ausreichend.
- **wayvnc als Sofort-Lösung**: Nach Phase 3 kann `wayvnc` als externer VNC-Server genutzt werden (braucht `wlr-layer-shell` + `ext-image-copy-capture` + `zwlr_virtual_pointer`). Eingebauter VNC/RDP kommt erst in Phase 5 — bis dahin sind externe Tools verfügbar.
- **libei vor Portal**: EIS-Server (libei) wird direkt im Compositor implementiert (Phase 3), unabhängig vom Portal-Backend. Das Platform-Crate kann libei direkt nutzen. Portal (D-Bus + `ConnectToEIS()`) ist nur ein Wrapper und kommt optional in Phase 8.
- **Server-Side Decorations (SSD)**: Compositor rendert Titelleisten mit Close/Maximize/Minimize-Buttons für Apps die SSD anfordern (z.B. Kate/Qt-Apps). Apps die CSD bevorzugen (z.B. GTK4-LibAdwaita) behalten eigene Dekorationen. Rendering via egui GPU-resident `TextureRenderElement<GlesTexture>` auf `GlowRenderer` — einheitlich für alle Backends.
- **egui für Compositor-UI**: `egui` 0.33 + `egui_glow` 0.33 für Compositor-Titlebars. GPU-residenter Render-Pfad inspiriert von smithay-egui. Immediate-Mode-API vereinfacht UI-Implementierung. Echte Fonts mit Antialiasing und Unicode-Support.
- **Konsolidierung auf `GlowRenderer`**: Alle Render-Pfade (Winit, DRM, Headless, Screenshots) nutzen ausschließlich `GlowRenderer`. `PixmanRenderer` komplett entfernt. Software-Rendering bei Bedarf via `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe). DRM-Backend nutzt EGL-on-GBM. Screenshot-Format: Abgr8888 (GL-native RGBA-Byte-Order).
- **Einfaches Stacking-WM**: Kein Tiling, keine Workspaces — reicht für Test-Szenarien
- **`ironrdp-server`** (RDP) + **`rustvncserver`** (VNC): Pure Rust, Safe APIs, Apache-2.0, `unsafe_code = "forbid"`-kompatibel
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
- **Legacy-Screencopy optional**: `wlr-screencopy-v1` als Fallback nur wenn `ext-image-copy-capture` nicht für alle benötigten Tools ausreicht. Kann übersprungen werden.
- **Tearing-Control + Content-Type als No-Op-Stubs**: `wp-tearing-control-v1` und `wp-content-type-hint-v1` sind triviale No-Op-Handler (~15 LoC je), verhindern aber "unsupported protocol"-Warnungen bei vielen Apps. Standard bei Sway/Hyprland.
- **xdg-toplevel-drag für Browser-Kompatibilität**: Tab-Detach in Firefox/Chromium nutzt `xdg-toplevel-drag-v1`. Wird zunehmend adoptiert, zukunftssicher.
