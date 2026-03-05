## EIS/libei — Erkenntnisse und Dokumentation

**TL;DR:** libei (Emulated Input) ist das Wayland-Pendant zu XTest — ermöglicht Input-Injection (Pointer, Keyboard, Touch, Scroll) in Compositors. Zugang erfolgt über den XDG Desktop Portal (`RemoteDesktop`) oder direkt per Unix-Socket. Der `eis-test-client` (`apps/eis-test-client/`) validiert das Protokoll gegen Mutter/KWin und dient als Referenz für die spätere Platform-Crate-Integration.

---

### Überblick

- **libei** = Emulated Input Protocol. Definiert eine Client-Server-Kommunikation über Unix-Domain-Sockets.
- **EIS** = Emulated Input Server (die Compositor-Seite, z.B. in Mutter eingebaut)
- **EI** = Emulated Input Client (unsere Seite — sendet Input-Events an den Compositor)
- **reis** = Rust-Implementierung von libei/libeis (Crate `reis`, Version 0.6)

### Verbindungsaufbau

Es gibt **keinen** festen EIS-Socket-Pfad im Dateisystem (anders als `$WAYLAND_DISPLAY`). Stattdessen:

1. **Portal-Weg (GNOME, KDE):** Der Client verbindet sich über D-Bus zum `xdg-desktop-portal` und durchläuft den `RemoteDesktop`-Flow:
   - `CreateSession` → Session-Handle
   - `SelectDevices(types=7, persist_mode=2, restore_token=...)` → Gerätekategorien wählen
   - `Start` → Benutzer bestätigt im Permission-Dialog (oder wird durch `restore_token` übersprungen)
   - `ConnectToEIS` → gibt einen File-Descriptor (Unix-Socket) direkt über D-Bus zurück (fd-Passing)

   Der FD wird **nicht** als Dateisystem-Socket exponiert — es gibt kein `$XDG_RUNTIME_DIR/eis-*.socket`.

2. **Direkter Socket (eigener Compositor, Sway-Fork):** Der Compositor bindet einen EIS-Socket und teilt den Pfad über eine Umgebungsvariable (`LIBEI_SOCKET`) oder einen bekannten Pfad mit.

3. **Umgebungsvariable:** `LIBEI_SOCKET` — Pfad zum Socket, absolut oder relativ zu `$XDG_RUNTIME_DIR`.

### Portal Restore-Token (persist_mode)

Um den Permission-Dialog nach dem ersten Mal zu vermeiden:

- `persist_mode=2` in `SelectDevices` → Token wird permanent gespeichert (bis explizit widerrufen)
- `persist_mode=1` → Token gilt nur während die Portal-Instanz läuft
- `persist_mode=0` → kein Token, immer Dialog

Der `restore_token` wird in der `Start`-Response zurückgegeben und beim nächsten `SelectDevices` mitgeschickt. Unser Test-Client speichert ihn unter `~/.local/share/platynui/eis-restore-token`.

**Compositor-Support:**
- GNOME/Mutter 43+ → ✅ `persist_mode=2` funktioniert
- KDE/Plasma 5.27+ → ✅ `persist_mode=2` funktioniert
- wlroots/Sway/Hyprland → ❌ kein `RemoteDesktop`-Portal-Backend

### EI-Handshake

```
Client → Server:  ei_handshake(context_type=Sender, name="eis-test-client")
Server → Client:  ei_handshake(negotiated_interfaces=[...], serial=N)
Server → Client:  SeatAdded(seat, capabilities)
Client → Server:  seat.bind_capabilities(BitFlags::all())
Server → Client:  DeviceAdded(device, ...)
Server → Client:  DeviceResumed(device)
```

**Kritisch:** `seat.bind_capabilities()` muss `BitFlags::all()` binden! Mutter erstellt kein Device wenn nur eine einzelne Capability gebunden wird (z.B. nur `Pointer`). Stattdessen alle binden — der Compositor aggregiert verwandte Capabilities (Pointer + Scroll + Button) in ein Device.

### reis Crate — Bekannte Bugs

**`EiConvertEventIterator` hängt:** Der Iterator ruft intern `poll_readable()` auf **bevor** er bereits gepufferte Protokoll-Events drainet. Wenn der Handshake-Read mehrere Events auf einmal liefert (Seat + Device + Resumed), hängt der Iterator beim Warten auf neue Daten obwohl Events bereits im Buffer liegen.

**Workaround:** Manuellen `EiEventConverter` verwenden:
```rust
let mut converter = EiEventConverter::new(&context, handshake_response);

// Drain nach jedem context.read():
while let Some(result) = context.pending_event() {
    match result {
        PendingRequestResult::Request(event) => converter.handle_event(event)?,
        PendingRequestResult::ParseError(e) => return Err(...),
        PendingRequestResult::InvalidObject(_) => {}
    }
}
```

### Input-Injection — Protokoll-Details

**Sequenz für eine einzelne Aktion (z.B. Pointer-Move):**
```
device.start_emulating(serial, sequence=1)
pointer.motion_relative(dx, dy)
device.frame(serial, timestamp_us)
device.stop_emulating(serial)
connection.flush()
sleep(50ms)  // Settle-Time bevor Portal-Session abgebaut wird
```

**Sequenz für Press/Release (Click, Key):**
```
device.start_emulating(serial, sequence=1)
button.button(code, Press)
device.frame(serial, timestamp_us)
connection.flush()
sleep(20ms)                              // ← nötig, damit Compositor Press/Release trennt
button.button(code, Released)
device.frame(serial, timestamp_us)
device.stop_emulating(serial)
connection.flush()
sleep(50ms)
```

Die 20ms Pause zwischen Press und Release ist **essentiell** — ohne sie registriert der Compositor (Mutter) die Events manchmal nicht korrekt als diskreten Click.

**`serial` und `sequence`:**
- `serial` = `connection.serial()` — der letzte vom Server empfangene Serial
- `sequence` ≥ 1 — monoton steigend pro `start_emulating`-Aufruf

**`timestamp_us`:**
- Monotoner Timestamp in Mikrosekunden. Wir verwenden `Instant::now().elapsed().as_micros()` relativ zu einem `LazyLock<Instant>` Epoch.

### Mutter-Spezifika

- **Kein `PointerAbsolute`:** Mutter bietet `Pointer` (relativ), `Button`, `Scroll` und `Keyboard` an, aber **nicht** `PointerAbsolute`. Absolute Pointer-Bewegung ist mit Mutter über EI nicht möglich — nur relativ.
- **BitFlags::all() Binding:** Zwingend erforderlich (siehe oben).
- **Single Device:** Mutter aggregiert alle Capabilities in ein einzelnes Device. Es gibt nicht separate Pointer/Keyboard-Devices.
- **Device-Name:** Typischerweise `"GNOME Remote Desktop"` oder ähnlich.

### Capabilities und Device-Typen

| Capability | Interface | Typischer Einsatz |
|---|---|---|
| `Pointer` | `ei::Pointer` | Relative Mausbewegung (`motion_relative`) |
| `PointerAbsolute` | `ei::PointerAbsolute` | Absolute Mausbewegung (`motion_absolute`) |
| `Button` | `ei::Button` | Mausklicks (`button(code, state)`) |
| `Scroll` | `ei::Scroll` | Mausrad (`scroll(dx, dy)`) |
| `Keyboard` | `ei::Keyboard` | Tastenanschläge (`key(keycode, state)`) |
| `Touch` | `ei::Touchscreen` | Touchscreen-Events |

**Button-Codes (Linux evdev):**
- `BTN_LEFT` = `0x110` (272)
- `BTN_RIGHT` = `0x111` (273)
- `BTN_MIDDLE` = `0x112` (274)

**Key-Codes (Linux evdev):** z.B. `KEY_A` = 30, `KEY_ENTER` = 28, `KEY_ESC` = 1.

### Event-Loop-Pattern

Für korrektes Event-Handling mit reis:

```rust
// 1. Poll mit optionalem Timeout
let mut pfd = [PollFd::new(context, PollFlags::IN)];
poll(&mut pfd, timeout.as_ref())?;

// 2. Neue Daten lesen
context.read()?;

// 3. Gepufferte Events dispatchen
while let Some(result) = context.pending_event() {
    match result {
        PendingRequestResult::Request(event) => converter.handle_event(event)?,
        // ...
    }
}

// 4. High-Level-Events verarbeiten
while let Some(event) = converter.next_event() {
    match event {
        EiEvent::SeatAdded(seat) => { /* bind capabilities */ }
        EiEvent::DeviceResumed(dev) => { /* ready to use */ }
        // ...
    }
}
```

**Wichtig:** Schritt 3 (dispatch_buffered) muss **immer** nach `context.read()` aufgerufen werden, auch nach dem initialen Handshake — der Handshake-Read kann zusätzliche Events (Seat/Device) mitliefern.

### Architektur des Test-Clients

```
apps/eis-test-client/
├── Cargo.toml         # reis, clap, zbus, reedline, anyhow, tracing
└── src/
    ├── main.rs        # Linux-Gate + fn main() → app::run()
    ├── portal.rs      # XDG Desktop Portal D-Bus (RemoteDesktop) Integration
    │                  # connect_via_portal(restore_token) → (UnixStream, Connection, Option<String>)
    └── app.rs         # CLI, Handshake, alle Kommandos (~930 LoC)
                       # - execute(): Handshake + Command-Dispatch
                       # - cmd_probe(): Diagnostik mit Grace-Period
                       # - find_device(): Wartet auf resumed Device mit Capability
                       # - send_input(): Einfache Aktion (move, scroll)
                       # - send_press_release(): Press→Pause→Release (click, key)
                       # - cmd_interactive(): reedline REPL + Semikolon-Splitting
                       # - Token-Persistence: load/save/reset restore_token
```

### Offene Punkte / Nächste Schritte

- **`type <text>` Kommando:** Noch nicht implementiert. Erfordert XKB-Keymap-Parsing (Zeichen → Keysym → Keycode + Modifier). Dependency: `xkbcommon`-Crate.
- **`sequence` Kommando:** Noch nicht implementiert. Kann durch das interaktive Semikolon-Feature (`interactive` + `cmd1; cmd2; cmd3`) ersetzt werden.
- **EIS-Server (Step 17):** Die Erkenntnisse aus dem Test-Client fließen direkt in die Server-Implementierung ein — insbesondere das Handshake-Protokoll, Device-Lifecycle, und Keymap-Propagation.
- **Keymap-Export im EIS-Server:** Bei Keyboard-Capability muss die aktive Smithay-Keymap als memfd exportiert und per `ei_keyboard.keymap(fd, size)` an den Client gesendet werden. Ohne Keymap ist Keyboard-Support für reale Tests sinnlos (Client weiß nicht welche Keycodes welche Zeichen produzieren).
- **KWin-Validierung:** Bisher nur gegen GNOME/Mutter getestet. KWin-spezifische Unterschiede (angebotene Capabilities, Device-Struktur) sind noch unbekannt.
