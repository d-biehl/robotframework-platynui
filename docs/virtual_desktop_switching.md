# Virtual Desktop / Workspace Switching im WindowManager

> Status: Entwurf – Pragmatischer Ansatz mit `ensure_window_accessible()`.
>
> English summary: Design document for making `bring_to_front()` work when
> windows are on different virtual desktops.  The pragmatic approach adds a
> single `ensure_window_accessible()` method to `WindowManager` that each
> platform implements using its native mechanism (X11: desktop switch, Windows:
> move window, macOS: no-op via kAXRaiseAction).  A full desktop query API is
> deferred to Appendix A for future use.  Platform APIs for X11/EWMH, Wayland,
> Windows, and macOS are catalogued with their limitations.

## 1. Motivation

PlatynUI automatisiert UI-Interaktionen per Robot Framework.  Wenn ein
Zielfenster auf einem anderen virtuellen Desktop liegt als dem aktuell
sichtbaren, scheitern `activate()`, Highlight und alle nachfolgenden
Input-Aktionen lautlos.  Die Runtime muss daher sicherstellen, dass das
Fenster erreichbar ist — entweder durch Desktop-Wechsel oder indem das
Fenster zum aktuellen Desktop geholt wird.

Eine vollständige Desktop-Query-API (Desktop auflisten, Namen abfragen, etc.)
ist nur auf X11 über dokumentierte APIs abbildbar und aktuell nicht
erforderlich.  Der pragmatische Ansatz löst das Kernproblem mit einer einzigen
neuen Methode: `ensure_window_accessible()`.

## 2. Ist-Zustand

### 2.1 WindowManager-Trait (`crates/core/src/platform/window_manager.rs`)

```rust
pub trait WindowManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;
    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError>;
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;
    fn activate(&self, id: WindowId) -> Result<(), PlatformError>;
    fn close(&self, id: WindowId) -> Result<(), PlatformError>;
    fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn restore(&self, id: WindowId) -> Result<(), PlatformError>;
    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;
    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;
}
```

Keinerlei Kenntnis von virtuellen Desktops / Workspaces / Spaces.

### 2.2 Aufrufkette

```
Runtime::bring_to_front(node)
  → UiNode::pattern::<WindowSurfaceActions>()
    → provider (AT-SPI / UIA) ruft WindowManager::activate(wid)
      → platform-linux-x11: _NET_ACTIVE_WINDOW ClientMessage
      → platform-windows:   SetForegroundWindow(hwnd)
```

### 2.3 Plattform-Implementierungen

| Plattform        | Desktop-Atoms/-APIs vorhanden? |
|------------------|-------------------------------|
| X11 EWMH         | ❌ `_NET_CURRENT_DESKTOP` und `_NET_WM_DESKTOP` fehlen in `EwmhAtoms` |
| Windows (Win32)   | ❌ `IVirtualDesktopManager` nicht eingebunden |
| macOS             | ❌ Stub – kein WindowManager implementiert |

## 3. Designvorschlag: Pragmatischer Ansatz

### 3.1 Kernproblem und Lösung

Das eigentliche Problem ist einfach: Ein Fenster liegt auf einem anderen
virtuellen Desktop → Automation schlägt fehl.  Die Lösung muss nicht
als öffentliche Desktop-API exponiert werden, sondern kann als **internes
Implementierungsdetail** der Plattform-Crates bleiben.

Eine vollständige Desktop-API (`VirtualDesktopId`, `switch_desktop()`,
`list_desktops()`, …) wäre nur auf X11/EWMH komplett über öffentliche APIs
abbildbar.  Windows unterstützt kein `SwitchDesktop()` über dokumentierte
COM-Interfaces, macOS bietet gar keine öffentliche Space-API.  Eine
Abstraktion, die nur auf einer Plattform voll funktioniert, schafft mehr
Komplexität als Nutzen.

**Stattdessen: Eine einzige neue Methode am `WindowManager`-Trait**, die das
Problem pro Plattform auf dem jeweils natürlichen Weg löst.

### 3.2 Neue Methode: `ensure_window_accessible()`

```rust
pub trait WindowManager: Send + Sync {
    // ... bestehende Methoden ...

    /// Ensure the window is reachable on the current virtual desktop.
    ///
    /// This is a best-effort operation: each platform uses whatever
    /// mechanism is available to make the window interactable.
    ///
    /// - **X11 EWMH**: switches to the window's desktop via
    ///   `_NET_CURRENT_DESKTOP` client message.
    /// - **Windows**: moves the window to the current desktop via
    ///   the public `IVirtualDesktopManager::MoveWindowToDesktop` COM API.
    /// - **macOS**: no-op — `kAXRaiseAction` in `activate()` handles
    ///   the Space switch implicitly (system-setting dependent).
    ///
    /// Callers should invoke this before `activate()` when the window
    /// might be on a different desktop.
    ///
    /// The default implementation does nothing (returns `Ok(())`).
    fn ensure_window_accessible(&self, id: WindowId) -> Result<(), PlatformError> {
        let _ = id;
        Ok(())
    }
}
```

### 3.3 Plattform-Strategien

| Plattform | Strategie | API | Status |
|-----------|-----------|-----|--------|
| **X11 EWMH** | Desktop zum Fenster wechseln | `_NET_WM_DESKTOP` lesen → `_NET_CURRENT_DESKTOP` ClientMsg setzen | ✅ Dokumentiert, stabil |
| **Windows** | Fenster zum aktuellen Desktop holen | `IVirtualDesktopManager::MoveWindowToDesktop(hwnd, current_guid)` | ✅ Dokumentierte COM-API (Win10+) |
| **macOS** | Implizit über Accessibility | `kAXRaiseAction` → macOS wechselt Space (wenn Systemeinstellung aktiv) | ✅ Öffentlich, aber systemeinstellungsabhängig |
| **Wayland** | Kein universeller Ansatz | — | ❌ Kein Protokoll für Desktop-Switch |

**Beachte:** Das UX-Verhalten unterscheidet sich:
- **X11**: Der Benutzer wird zum Desktop des Fensters gebracht.
- **Windows**: Das Fenster wird zum Benutzer gebracht (auf seinem Desktop).
- **macOS**: Abhängig von Systemeinstellung „When switching to an application,
  switch to a Space with open windows for the application".

Für UI-Automation (Robot Framework) ist beides akzeptabel — Hauptsache, das
Fenster ist danach interagierbar.

### 3.4 Integration in `bring_to_front()`

Die Runtime ruft `ensure_window_accessible()` vor `activate()` auf:

```rust
// In Runtime::bring_to_front():
fn bring_to_front(&self, node: &Arc<dyn UiNode>) -> Result<(), BringToFrontError> {
    let window = self.top_level_window_for(node)...;
    let pattern = window.pattern::<WindowSurfaceActions>()...;

    // Ensure the window is on the current desktop (or vice versa).
    if let Some(wm) = self.window_manager() {
        let wid = wm.resolve_window(&*window)?;
        if let Err(e) = wm.ensure_window_accessible(wid) {
            debug!(error = %e, "could not ensure window accessible, continuing anyway");
        }
    }

    let _ = pattern.restore();
    pattern.activate()?;
    Ok(())
}
```

Vorteile dieses Ansatzes:
- **Keine neue Abstraktion im Core** — kein `VirtualDesktopId`-Typ,
  kein Trait-Objekt, kein `#[cfg]`.
- **Separation of Concerns** — Runtime orchestriert, `WindowManager` führt
  plattformspezifisch aus.
- **Robustheit** — `ensure_window_accessible()` ist best-effort.  Wenn es
  fehlschlägt, wird `activate()` trotzdem versucht (kann auf einigen WMs
  funktionieren, z.B. Mutter wechselt den Desktop bei `_NET_ACTIVE_WINDOW`
  automatisch).
- **Keine plattformlimitierte Abstraktion** — keine API, die nur auf
  einer Plattform voll funktioniert.

### 3.5 Implementierungsskizzen

#### X11 EWMH

Drei neue Atoms in `EwmhAtoms`:

```rust
pub(crate) struct EwmhAtoms {
    // ... bestehende ...
    pub net_current_desktop: Atom,
    pub net_wm_desktop: Atom,
    pub net_number_of_desktops: Atom,
}
```

Implementierung:

```rust
fn ensure_window_accessible(&self, id: WindowId) -> Result<(), PlatformError> {
    let xid = id.as_raw() as u32;
    let conn = &self.connection;
    let root = self.root;

    // 1. Read which desktop the window is on.
    let win_desktop = get_cardinal_property(conn, xid, self.atoms.net_wm_desktop)?;
    let Some(win_desktop) = win_desktop else {
        return Ok(()); // No desktop info → nothing to do.
    };

    // 0xFFFFFFFF = sticky / all desktops → already visible.
    if win_desktop == 0xFFFF_FFFF {
        return Ok(());
    }

    // 2. Read the current desktop.
    let cur_desktop = get_cardinal_property(conn, root, self.atoms.net_current_desktop)?;
    if cur_desktop == Some(win_desktop) {
        return Ok(()); // Already on the right desktop.
    }

    // 3. Switch to the window's desktop.
    send_client_message(conn, root, root, self.atoms.net_current_desktop, [
        win_desktop, 0, 0, 0, 0,
    ])?;
    conn.flush()?;

    Ok(())
}
```

#### Windows

```rust
fn ensure_window_accessible(&self, id: WindowId) -> Result<(), PlatformError> {
    let hwnd = HWND(id.as_raw() as *mut _);

    let vdm: IVirtualDesktopManager =
        CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL)?;

    // 1. Get the desktop GUID of the target window.
    let window_guid = unsafe { vdm.GetWindowDesktopId(hwnd)? };

    // 2. Get the current desktop GUID (via foreground window).
    let fg = unsafe { GetForegroundWindow() };
    let current_guid = unsafe { vdm.GetWindowDesktopId(fg)? };

    // 3. Only move if the window is on a different desktop.
    if window_guid != current_guid {
        unsafe { vdm.MoveWindowToDesktop(hwnd, &current_guid)? };
    }

    Ok(())
}
```

#### macOS

```rust
fn ensure_window_accessible(&self, _id: WindowId) -> Result<(), PlatformError> {
    // No-op: kAXRaiseAction in activate() handles Space switching
    // when "When switching to an application, switch to a Space
    // with open windows" is enabled in System Settings > Desktop & Dock.
    Ok(())
}
```

### 3.6 Zukunft: Vollständige Desktop-API (aufgeschoben)

Falls in Zukunft CLI-Befehle (`platynui desktop --list`, `platynui desktop --switch 2`),
Inspector-Features (Desktop-Spalte im Baum) oder Robot-Keywords
(`Move Window To Desktop 2`) benötigt werden, kann der `WindowManager`-Trait
um eine vollständige Desktop-API erweitert werden.

Der bevorzugte Ansatz dafür wäre ein **Trait-Objekt** `VirtualDesktopId`
(analog zu `UiNode`), das konsistent mit bestehenden Projektmustern ist
und Core plattformunabhängig hält.  Die Details dazu sind in
Anhang A dokumentiert.

## 4. Plattform-APIs im Detail

### 4.1 Linux X11 (EWMH)

Vollständig standardisiert über die
[Extended Window Manager Hints (EWMH)](https://specifications.freedesktop.org/wm-spec/latest/)
Spezifikation.  Alle gängigen X11-Window-Manager (KWin, Mutter, Xfwm, i3,
Openbox, …) unterstützen diese Atoms.

| EWMH Atom                | Typ      | Zweck |
|--------------------------|----------|-------|
| `_NET_NUMBER_OF_DESKTOPS` | `CARDINAL` auf Root | Anzahl virtueller Desktops |
| `_NET_CURRENT_DESKTOP`    | `CARDINAL` auf Root | Index des aktuellen Desktops (0-basiert) |
| `_NET_WM_DESKTOP`         | `CARDINAL` auf Window | Desktop-Index des Fensters; `0xFFFFFFFF` = sticky |
| `_NET_DESKTOP_NAMES`      | `UTF8_STRING[]` auf Root | Optionale Namen der Desktops |

**Lesen:**
```rust
// Aktueller Desktop
fn current_desktop(conn, root, atom) -> Option<u32> {
    let reply = conn.get_property(false, root, atom, CARDINAL, 0, 1)?.reply()?;
    reply.value32()?.next()
}

// Desktop eines Fensters
fn window_desktop(conn, xid, atom) -> Option<u32> {
    let reply = conn.get_property(false, xid, atom, CARDINAL, 0, 1)?.reply()?;
    reply.value32()?.next()
}
```

**Schreiben (Desktop wechseln):**
```rust
// _NET_CURRENT_DESKTOP ClientMessage an Root
send_client_message(conn, root, root, net_current_desktop, [
    target_desktop,  // data[0]: new desktop index
    0,               // data[1]: timestamp (0 = CurrentTime)
    0, 0, 0,
]);
```

**Schreiben (Fenster auf anderen Desktop verschieben):**
```rust
// _NET_WM_DESKTOP ClientMessage an Root
send_client_message(conn, root, xid, net_wm_desktop, [
    target_desktop,  // data[0]: new desktop index (0xFFFFFFFF = sticky)
    2,               // data[1]: source indication (2 = pager/automation)
    0, 0, 0,
]);
```

**Hinweis:** Einige WMs (Mutter/GNOME) wechseln bei `_NET_ACTIVE_WINDOW`
automatisch zum Desktop des Fensters.  Andere (KWin mit bestimmten
Konfigurationen, i3) tun das nicht.  Expliziter `_NET_CURRENT_DESKTOP`-Switch
vor `_NET_ACTIVE_WINDOW` ist daher die sichere Variante.

**Implementierungsaufwand:** Gering.  `EwmhAtoms` um 3 Atoms erweitern,
4 Methoden implementieren.

### 4.2 Linux Wayland

Wayland hat **kein standardisiertes Protokoll** für Desktop-/Workspace-
Switching durch externe Clients.

| Protokoll / API | Status | Fähigkeit |
|-----------------|--------|-----------|
| `wlr-foreign-toplevel-management-unstable-v1` | wlroots (Sway, Hyprland) | Aktivieren, Minimieren, Maximieren, Schließen von Fenstern.  **Kein Desktop-Switch.** |
| `ext-foreign-toplevel-list-v1` | Entwurf | Nur Read-only Auflistung von Toplevel-Fenstern |
| `cosmic-toplevel-info-unstable-v1` + `cosmic-workspace-unstable-v1` | COSMIC/Pop!_OS | Workspace-Listing und Aktivierung |
| KDE D-Bus (`org.kde.KWin`) | KDE-spezifisch | `setCurrentDesktop`, `windowToDesktop` |
| GNOME Shell D-Bus (`org.gnome.Shell`) | GNOME-spezifisch | Extensions-API, nicht offiziell |

**Fazit:** Unter Wayland kein universeller Ansatz.  Kurzfristig irrelevant
(PlatynUI nutzt X11/XWayland).  Langfristig müsste entweder pro Compositor
ein Backend geschrieben werden, oder ein neues Wayland-Protokoll abgewartet
werden.

**Implementierungsaufwand:** Hoch bis sehr hoch, Compositor-spezifisch.

### 4.3 Windows

Microsoft stellt die COM-Schnittstelle `IVirtualDesktopManager` bereit
(seit Windows 10, in `shell32.dll`):

| Methode | Signatur | Zweck |
|---------|----------|-------|
| `IsWindowOnCurrentVirtualDesktop` | `(HWND) → BOOL` | Prüft ob Fenster auf aktuellem Desktop |
| `GetWindowDesktopId` | `(HWND) → GUID` | Desktop-GUID des Fensters |
| `MoveWindowToDesktop` | `(HWND, GUID) → HRESULT` | Fenster auf anderen Desktop verschieben |

**Problem: Kein `SwitchDesktop(GUID)` in der öffentlichen API!**

Die undokumentierte interne Schnittstelle `IVirtualDesktopManagerInternal`
bietet `SwitchDesktop(GUID)`, ihre vtable-Offsets ändern sich jedoch zwischen
Windows-Versionen (10 → 11 → 24H2).  Tools wie `VirtualDesktop`
(MScholtes/VirtualDesktop auf GitHub) pflegen versionsspezifische COM-Offsets.

**Strategien:**

| Strategie | Beschreibung | Zuverlässigkeit |
|-----------|-------------|----------------|
| **A: Fenster zum aktuellen Desktop holen** | `GetWindowDesktopId(foreground_hwnd)` → `MoveWindowToDesktop(target, current_guid)` | ✅ Stabil (nur öffentliche API) |
| **B: Zum Desktop des Fensters wechseln** | Undokumentiertes `IVirtualDesktopManagerInternal::SwitchDesktop` | ⚠️ Fragil, bricht zwischen Versionen |
| **C: Pinned machen** | `IVirtualDesktopPinnedApps::PinWindow(HWND)` (undokumentiert) | ⚠️ Ebenfalls undokumentiert |

**Empfehlung für PlatynUI:**
- **Strategie A (Fenster holen)** als primärer Weg – zuverlässig und offiziell.
- `switch_desktop()` als `not_supported` oder mit Feature-Flag für
  undokumentierte API.  Alternativ: `switch_desktop()` implementiert intern
  Strategie A (verschiebt alle Fenster des Ziel-Desktops? Nein, unpraktisch).
- Tatsächliches Desktop-Switching (Variante B) nur bei Bedarf mit
  entsprechender Warnung und Versionsprüfung.

**Implementierungsaufwand:** Mittel.  `IVirtualDesktopManager` über
`windows`-Crate ansprechen (CLSID `{aa509086-…}`).

### 4.4 macOS

macOS nennt virtuelle Desktops **Spaces** (Mission Control).

**Alle Space-Management-APIs sind privat** (CoreGraphics SPI):

| Private API | Zweck |
|-------------|-------|
| `CGSGetActiveSpace(cid)` | Aktueller Space |
| `CGSGetWindowWorkspace(cid, wid)` | Space eines Fensters |
| `CGSMoveWorkspaceWindowList(cid, &[wid], target_space)` | Fenster verschieben |
| `CGSManagedDisplayGetCurrentSpace(cid, display_uuid)` | Space pro Display |

Apple bietet **keine öffentliche API** für Space-Switching.

**Workarounds:**

| Ansatz | Beschreibung | Status |
|--------|-------------|--------|
| **AXUIElementPerformAction(kAXRaiseAction)** | Accessibility-API: hebt Fenster an → macOS wechselt zum Space (wenn Systemeinstellung aktiv) | ✅ Öffentlich, aber abhängig von Systemeinstellung „When switching to an application…" |
| **NSWorkspace.shared.open()** | Öffnet/aktiviert App → macOS wechselt zum Space | ✅ Öffentlich, aber nur App-Level, nicht Window-Level |
| **Private CGS-APIs** | Direkter Space-Switch | ⚠️ Kann bei macOS-Updates brechen |
| **AppleScript / `osascript`** | `tell application "System Events" to set active_space to…` | ⚠️ Langsam, erfordert Accessibility-Permissions |

**Empfehlung für PlatynUI:**
- `kAXRaiseAction` für `activate()` nutzen (der Provider `macos-ax` wird dies
  ohnehin tun).  Damit schaltet macOS in den meisten Konfigurationen den Space
  automatisch um.
- `current_desktop()` und `window_desktop()` über private CGS-APIs
  implementieren (mit Feature-Flag `macos-private-api`).
- `switch_desktop()` als `not_supported` auf macOS oder über CGS mit
  Vorbehalt.

**Implementierungsaufwand:** Niedrig für `kAXRaiseAction`-Ansatz.  Mittel
für CGS-basierte Desktop-Queries.

## 5. Zusammenfassung: Implementierungsmatrix

| Methode / Fähigkeit | X11 EWMH | Wayland | Windows | macOS |
|---------------------|----------|---------|---------|-------|
| `ensure_window_accessible()` | ✅ Desktop wechseln (EWMH) | ❌ | ✅ Fenster holen (`MoveWindowToDesktop`) | ✅ No-op (`kAXRaiseAction` in `activate()`) |
| Nur dokumentierte APIs | ✅ | — | ✅ | ✅ |
| Implementierungsaufwand | Gering (3 Atoms, ~30 Zeilen) | — | Mittel (COM-Init) | Keiner (No-op) |

## 6. Offene Fragen

1. ~~**`VirtualDesktopId`-Typ**~~ — Aufgeschoben.  Für den pragmatischen
   Ansatz (`ensure_window_accessible()`) wird kein plattformübergreifender
   Desktop-ID-Typ benötigt.  Falls in Zukunft eine vollständige Desktop-API
   nötig wird, ist der Trait-Objekt-Ansatz (analog zu `UiNode`) der
   bevorzugte Weg (siehe Anhang A).

2. **Wayland:** Ignorieren bis ein Cross-Compositor-Protokoll existiert?
   PlatynUI nutzt aktuell X11/XWayland; `ensure_window_accessible()`
   gibt dort `Ok(())` zurück (No-op).

3. **macOS Systemeinstellung:** `kAXRaiseAction` wechselt den Space nur
   wenn "When switching to an application, switch to a Space with open
   windows" aktiv ist.  Soll PlatynUI den Benutzer warnen, wenn diese
   Einstellung deaktiviert ist?

4. **Windows: Fenster-Pinning?** Statt `MoveWindowToDesktop` könnte man
   `IVirtualDesktopPinnedApps::PinWindow` nutzen (undokumentiert), damit
   das Fenster auf allen Desktops erscheint.  Aktuell nicht geplant.

## 7. Empfohlene Umsetzungsreihenfolge

1. **Phase 1 – Core + X11** (niedrigster Aufwand, sofort testbar):
   - `ensure_window_accessible()` als Default-Methode am `WindowManager`-Trait
   - `EwmhAtoms` um `_NET_CURRENT_DESKTOP`, `_NET_WM_DESKTOP` erweitern
   - Implementierung in `X11EwmhWindowManager`
   - `Runtime::bring_to_front()`: `ensure_window_accessible()` vor
     `activate()` aufrufen
   - Tests mit Mock-Provider (Fenster auf anderem Desktop simulieren)

2. **Phase 2 – Windows**:
   - `IVirtualDesktopManager` COM-Interface einbinden (`IsWindowOnCurrentVirtualDesktop`,
     `GetWindowDesktopId`, `MoveWindowToDesktop`)
   - `ensure_window_accessible()` implementieren (Fenster zum aktuellen Desktop holen)

3. **Phase 3 – macOS** (minimal):
   - Verifizieren, dass `kAXRaiseAction` in der `activate()`-Impl den Space-Switch
     zuverlässig auslöst
   - `ensure_window_accessible()` bleibt No-op

4. **Phase 4 – Vollständige Desktop-API** (bei Bedarf):
   - Nur wenn CLI/Inspector/Robot-Keywords explizite Desktop-Operationen
     brauchen (`list_desktops`, `switch_desktop`, `desktop_name`, …)
   - Trait-Objekt-Ansatz aus Anhang A umsetzen

---

## Anhang A: Vollständige Desktop-API (aufgeschobener Entwurf)

Der folgende Entwurf dokumentiert eine vollständige Desktop-Abstraktion für
den Fall, dass sie in Zukunft benötigt wird (CLI-Befehle, Inspector-Features,
Robot-Keywords).  Er ist aktuell **nicht zur Implementierung vorgesehen**.

### A.1 `VirtualDesktopId` als Trait-Objekt

Desktop-Identifier sind plattformspezifisch:
- X11 EWMH: `u32` (0-basiert, sequentiell)
- Windows: `GUID` (128-bit, opak)
- macOS Spaces: `u64` (private CGS-API, opak)

Der bevorzugte Ansatz ist ein Trait-Objekt analog zu `UiNode`:

```rust
/// Opaque trait for virtual desktop identifiers.
pub trait VirtualDesktopId: Send + Sync + fmt::Debug {
    fn is_all_desktops(&self) -> bool;
    fn eq_id(&self, other: &dyn VirtualDesktopId) -> bool;
    fn hash_id(&self, state: &mut dyn std::hash::Hasher);
    fn display_id(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn VirtualDesktopId>;
}
```

Operator-Impls für `dyn VirtualDesktopId` (`PartialEq`, `Eq`, `Clone`,
`Hash`, `Display`) machen den Ansatz ergonomisch.

Plattform-Crates liefern konkrete Typen:

```rust
// platform-linux-x11:
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EwmhDesktopId(pub u32);

// platform-windows:
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WindowsDesktopId(pub GUID);
```

### A.2 WindowManager-Erweiterung (vollständig)

```rust
pub trait WindowManager: Send + Sync {
    // ... bestehende Methoden + ensure_window_accessible() ...

    fn supports_virtual_desktops(&self) -> bool { false }
    fn desktop_count(&self) -> Result<Option<u32>, PlatformError> { Ok(None) }
    fn current_desktop(&self) -> Result<Option<Box<dyn VirtualDesktopId>>, PlatformError> { Ok(None) }
    fn window_desktop(&self, id: WindowId) -> Result<Option<Box<dyn VirtualDesktopId>>, PlatformError> { let _ = id; Ok(None) }
    fn is_on_current_desktop(&self, id: WindowId) -> Result<Option<bool>, PlatformError> { /* default impl */ }
    fn switch_desktop(&self, desktop: &dyn VirtualDesktopId) -> Result<(), PlatformError> { Err(PlatformError::not_supported("...")) }
    fn move_window_to_desktop(&self, id: WindowId, desktop: &dyn VirtualDesktopId) -> Result<(), PlatformError> { Err(PlatformError::not_supported("...")) }
    fn desktop_name(&self, desktop: &dyn VirtualDesktopId) -> Result<Option<String>, PlatformError> { Ok(None) }
    fn list_desktops(&self) -> Result<Vec<Box<dyn VirtualDesktopId>>, PlatformError> { Ok(Vec::new()) }
}
```

### A.3 Plattformabdeckung (nur öffentliche/dokumentierte APIs)

| Methode | X11 EWMH | Windows | macOS |
|---------|----------|---------|-------|
| `current_desktop()` | ✅ | ✅ (via Foreground-HWND) | ❌ |
| `window_desktop()` | ✅ | ✅ `GetWindowDesktopId` | ❌ |
| `is_on_current_desktop()` | ✅ | ✅ `IsWindowOnCurrentVirtualDesktop` | ❌ |
| `switch_desktop()` | ✅ | ❌ (undokumentiert) | ❌ |
| `move_window_to_desktop()` | ✅ | ✅ `MoveWindowToDesktop` | ❌ |
| `desktop_count()` | ✅ | ❌ | ❌ |
| `desktop_name()` | ✅ | ❌ | ❌ |
| `list_desktops()` | ✅ | ❌ | ❌ |

> **Fazit:** Eine vollständige Desktop-API funktioniert nur auf X11 über
> dokumentierte APIs.  Windows bietet Query + Move, aber kein Switch.
> macOS bietet gar nichts Öffentliches.  Daher bleibt die vollständige API
> aufgeschoben, bis konkrete Use Cases sie rechtfertigen.
