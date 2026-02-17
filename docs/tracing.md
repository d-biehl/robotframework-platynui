# Logging & Tracing – Konventionen

> **English summary:** This document defines the logging and tracing conventions for all PlatynUI Rust crates. It covers the `tracing` dependency specification, log level semantics, subscriber setup in binary crates (CLI, Inspector), the log-level priority chain (`RUST_LOG` > `--log-level` > `PLATYNUI_LOG_LEVEL` > default `warn`), structured field conventions, and per-crate instrumentation guidelines. The normative reference is `.github/instructions/tracing.instructions.md`.

> Status: Gültig ab Februar 2026 – spiegelt den aktuellen Implementierungsstand wider.

---

## 1. Überblick

PlatynUI verwendet das [`tracing`](https://docs.rs/tracing)-Ökosystem für strukturierte, gestufte Diagnosemeldungen in allen Rust-Crates. Dieses Dokument beschreibt die verbindlichen Konventionen.

Kernprinzipien:
- **Strukturierte Felder** statt Format-Strings — damit Logs maschinell filterbar bleiben.
- **Keine Subscriber in Library-Crates** — nur Binary-Crates (CLI, Inspector) initialisieren den Subscriber.
- **stderr für Diagnose, stdout für Nutzdaten** — Log-Ausgabe geht immer nach stderr.
- **Einheitliche Dependency-Spezifikation** — jedes Crate deklariert `tracing` individuell (keine Workspace-Dependencies, da maturin damit inkompatibel ist).

## 2. Dependency-Spezifikation

### Library-Crates

Jedes Crate, das Log-Meldungen ausgibt, fügt in seiner eigenen `Cargo.toml` hinzu:

```toml
[dependencies]
tracing = { version = "0.1", default-features = false, features = ["std"] }
```

### Binary-Crates (CLI, Inspector)

Zusätzlich zu `tracing` kommt der Subscriber hinzu:

```toml
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "env-filter"] }
```

### Warum keine Workspace-Dependencies?

Das `packages/native`-Crate wird über maturin gebaut, das mit `[workspace.dependencies]` nicht kompatibel ist. Daher spezifiziert jedes Crate seine `tracing`-Abhängigkeit eigenständig — immer mit derselben Versionsangabe und denselben Features.

## 3. Log-Level-Semantik

| Level   | Zweck                                                                                                 | Beispiele                                                           |
|---------|-------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------|
| `error` | Unerwarteter Fehler, der **nicht** bereits als `Result::Err` an den Aufrufer zurückgegeben wird.       | `SendInput` fehlgeschlagen, `BitBlt` fehlgeschlagen, unsupportete Bildtiefe |
| `warn`  | Degradierter Betrieb, Fallbacks aktiv, langsame Aufrufe (>200 ms), fehlende optionale Fähigkeiten.   | RANDR fehlt, Fallback-Desktop-Info, Connect-Timeout                 |
| `info`  | Einmalige Lebenszyklus-Ereignisse — maximal einmal pro Programmstart.                                | Runtime initialisiert/heruntergefahren, Plattform initialisiert, Bus verbunden |
| `debug` | Operative Details, nützlich bei Entwicklung oder Felddiagnose.                                        | Provider-Anzahl, Geräte-Erkennung, XPath-Ausdruck, Pointer-Koordinaten |
| `trace` | Hot-Path-Iteration — extrem detailliert; nur aktivieren, um spezifische Subsysteme zu debuggen.       | Pro-Knoten AT-SPI-Auflösung, Pro-App-Enumeration                   |

### Faustregeln

- **Niemals** auf `error` loggen, wenn derselbe Fehler bereits als `Result::Err` an den Aufrufer geht — der Aufrufer entscheidet.
- `info`-Meldungen sollen selten sein und sich wie Meilensteine in einem Lebenszyklus-Protokoll lesen.
- Wenn eine Meldung einmal pro UI-Knoten oder einmal pro Event-Loop-Tick feuern würde → `trace`.
- Im Zweifelsfall zwischen `debug` und `trace`: „Wäre das bei 500 Knoten störend?" Wenn ja → `trace`.

## 4. Log-Level-Steuerung

### Prioritätskette (höchste Priorität zuerst)

1. **`RUST_LOG`** — Umgebungsvariable für feingranulare Pro-Crate-Filterung (z.B. `RUST_LOG=platynui_runtime=debug,platynui_xpath=trace`).
2. **`--log-level`** — CLI-Argument (Werte: `error`, `warn`, `info`, `debug`, `trace`).
3. **`PLATYNUI_LOG_LEVEL`** — Umgebungsvariable für einfache Einzel-Level-Steuerung.
4. **Standard: `warn`**.

### Beispiele

```bash
# Nur Fehler anzeigen
platynui-cli --log-level error query "//control:Button"

# Debug-Ausgabe für alle Crates
PLATYNUI_LOG_LEVEL=debug platynui-cli query "//control:Button"

# Feingranular: nur XPath-Evaluierung auf trace, Rest auf warn
RUST_LOG=platynui_runtime::xpath=trace platynui-cli query "//control:Button"

# Inspector mit Debug-Level
platynui-inspector --log-level debug
```

### Subscriber-Initialisierung

Nur Binary-Crates initialisieren den Subscriber. Die Referenzimplementierung (identisch in CLI und Inspector):

```rust
fn init_tracing(cli_level: Option<LogLevel>) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let directive = if let Some(level) = cli_level {
            log_level_directive(level)
        } else if let Ok(val) = std::env::var("PLATYNUI_LOG_LEVEL") {
            val
        } else {
            "warn".to_string()
        };
        EnvFilter::new(directive)
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();
}
```

Das `--log-level`-Argument wird über `clap` definiert:

```rust
#[arg(long = "log-level", value_enum, global = true)]
log_level: Option<LogLevel>,
```

## 5. Strukturierte Felder

### Prinzip

Immer strukturierte Schlüssel-Wert-Felder verwenden statt Format-Strings:

```rust
// Gut — strukturiert, filterbar
tracing::debug!(xpath, cached = options.cache().is_some(), "xpath evaluate");
tracing::info!(providers = count, "Runtime initialized");

// Vermeiden — unstrukturiert, schwerer abzufragen
tracing::debug!("xpath evaluate: {} (cached={})", xpath, cached);
```

### Feld-Formatierung

| Syntax           | Beschreibung                                                    | Beispiel                                           |
|------------------|-----------------------------------------------------------------|----------------------------------------------------|
| `field = value`  | Direkte Übergabe (für `tracing::Value`-Typen: int, bool, &str) | `count = entries.len()`                            |
| `field = %value` | Verwendet das `Display`-Trait                                   | `display = %disp`                                  |
| `field = ?value` | Verwendet das `Debug`-Trait                                     | `button = ?target_button`                          |
| `%err`           | Shorthand für `err = %err` (Display)                            | `tracing::error!(%err, "connection failed")`       |

### Namenskollision mit `display`

Lokale Variablen dürfen **nicht** `display` heißen, wenn sie mit `%` formatiert werden — das tracing-Makro interpretiert `%display` als Aufruf von `tracing::field::display()`. Lösung: Variable umbenennen (z.B. `disp`):

```rust
// Kollision — Kompilierfehler
let display = get_display();
tracing::info!(%display, "connected");

// Korrekt
let disp = get_display();
tracing::info!(display = %disp, "connected");
```

## 6. Import-Stil

Zwei Varianten sind akzeptabel — innerhalb eines Moduls konsistent bleiben:

**Variante A** — Import der Makros (bevorzugt bei vielen Aufrufen):
```rust
use tracing::{debug, info, warn, error, trace};

debug!("XTEST extension available");
info!("Linux X11 platform initialized");
```

**Variante B** — Vollqualifizierter Pfad (bei wenigen, verstreuten Aufrufen):
```rust
tracing::debug!(count = entries.len(), "discovered provider factories");
```

## 7. Instrumentierung nach Crate

### Runtime (`crates/runtime`)

| Modul              | Level   | Was                                                                |
|--------------------|---------|--------------------------------------------------------------------|
| `runtime.rs`       | `info`  | `Runtime::new()` abgeschlossen, `shutdown()` Beginn               |
| `runtime.rs`       | `debug` | Provider-Anzahl, Geräte-Erkennung, Plattformmodul-Init            |
| `runtime.rs`       | `warn`  | Fallback Desktop-Info (kein DesktopInfoProvider)                   |
| `runtime.rs`       | `error` | Provider `get_nodes` fehlgeschlagen (still übersprungen)           |
| `pointer.rs`       | `debug` | move_to, click, scroll, drag mit Koordinaten                      |
| `pointer.rs`       | `warn`  | `ensure_position` fehlgeschlagen                                   |
| `keyboard.rs`      | `debug` | execute mit Modus und Segment-Anzahl                               |
| `keyboard.rs`      | `warn`  | Hängende Tasten bei Fehler                                         |
| `xpath.rs`         | `debug` | evaluate/evaluate_iter mit XPath-Ausdruck und Cache-Status         |
| `registry.rs`      | `debug` | Provider-Factory-Erkennung und Instanziierung                      |

### Plattform-Crates (`crates/platform-*`)

| Modul              | Level   | Was                                                                |
|--------------------|---------|--------------------------------------------------------------------|
| `lib.rs`           | `info`  | Plattform erfolgreich initialisiert (einmalig)                     |
| `lib.rs`/init      | `debug` | Extension-Verfügbarkeit (XTEST, RANDR, DPI)                       |
| `x11util.rs`       | `debug` | X11-Verbindungsaufbau, `info` bei Erfolg                          |
| `x11util.rs`       | `warn`  | Connect-Timeout                                                    |
| `desktop.rs`       | `warn`  | Fallback bei fehlender X11-Verbindung                              |
| `desktop.rs`       | `debug` | RANDR-Monitor-Enumeration, Root-Window-Fallback                   |
| `highlight.rs`     | `warn`  | DISPLAY nicht gesetzt, Connect fehlgeschlagen                      |
| `screenshot.rs`    | `error` | Unsupportete Bildtiefe                                             |
| `pointer.rs` (Win) | `error` | `SendInput` fehlgeschlagen                                        |
| `screenshot.rs` (Win) | `error` | `BitBlt` fehlgeschlagen                                        |
| `init.rs` (Win)    | `info`  | DPI-Awareness gesetzt                                              |

### Provider-Crates (`crates/provider-*`)

| Modul              | Level   | Was                                                                |
|--------------------|---------|--------------------------------------------------------------------|
| `connection.rs`    | `info`  | AT-SPI-Bus verbunden (einmalig)                                    |
| `connection.rs`    | `debug` | Verbindungsversuch (Adresse oder Default-Session)                  |
| `connection.rs`    | `error` | Verbindung fehlgeschlagen oder Timeout                             |
| `ewmh.rs`          | `debug` | XID-Auflösung, activate_window, close_window                      |
| `ewmh.rs`          | `warn`  | Kein X11-Fenster für PID gefunden                                  |
| `node.rs`          | `trace` | Pro-Knoten-Details (AT-SPI-Attribute, Kinder-Auflösung)           |
| `lib.rs`           | `trace` | Pro-App-Enumeration (übersprungene Apps, Kind-Aufbau)              |
| `provider.rs` (Win)| `warn`  | Walker-Erstellung fehlgeschlagen                                   |

## 8. Checkliste für neue Crates

Beim Anlegen eines neuen Crates:

1. `tracing = { version = "0.1", default-features = false, features = ["std"] }` in `Cargo.toml` eintragen.
2. Mindestens `info!` für Lebenszyklus-Ereignisse und `warn!`/`error!` für Fehlerpfade hinzufügen.
3. Level-Tabelle (Abschnitt 3) konsultieren.
4. **Keinen** Subscriber initialisieren — das ist Aufgabe der Binary-Crates.
5. Strukturierte Felder verwenden (Abschnitt 5).
6. Import-Stil konsistent innerhalb des Moduls halten (Abschnitt 6).

## 9. Referenzdateien

| Datei                                          | Inhalt                                                |
|------------------------------------------------|-------------------------------------------------------|
| `.github/instructions/tracing.instructions.md` | Normative Copilot-Instruktionen (englisch, `**/*.rs`) |
| `AGENTS.md` § Logging & Tracing               | Kurzfassung für Agenten                               |
| `.github/copilot-instructions.md` § 12         | Kurzfassung für Copilot                               |
| `crates/cli/src/lib.rs`                        | Referenzimplementierung CLI (init_tracing, LogLevel)  |
| `apps/inspector/src/lib.rs`                    | Referenzimplementierung Inspector                     |
