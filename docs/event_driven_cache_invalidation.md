# Event-Driven Cache Invalidation (Option B)

> **English summary:** This document describes the architecture for platform-level
> structure-change events (UIA, AT-SPI, macOS AX) that drive automatic XDM cache
> invalidation. It builds on top of the lazy revalidation cache (Option A) already
> implemented in `crates/runtime/src/xpath.rs`.

## Status Quo

### Vorhandene Event-Pipeline

Die Event-Infrastruktur ist bereits vollständig verdrahtet:

```
Provider  →  ProviderEventListener::on_event()
          →  RuntimeEventListener (ruft node.invalidate())
          →  ProviderEventDispatcher (Fan-out)
          →  ProviderEventSink(s)
```

### Vorhandene Typen

```rust
// crates/core/src/provider/event.rs
pub enum ProviderEventKind {
    NodeAdded { parent: Option<RuntimeId>, node: Arc<dyn UiNode> },
    NodeUpdated { node: Arc<dyn UiNode> },
    NodeRemoved { runtime_id: RuntimeId },
    TreeInvalidated,
}

// crates/core/src/provider/descriptor.rs
bitflags! {
    pub struct ProviderEventCapabilities: u8 {
        const NONE = 0;
        const CHANGE_HINT     = 0b0001;
        const STRUCTURE       = 0b0010;
        const STRUCTURE_WITH_PROPERTIES = 0b0100;
    }
}
```

### Aktuelle Implementierungen

| Provider | `subscribe_events` | Event Capabilities | Sendet Events? |
|---|---|---|---|
| Mock | Vollständig, speichert Listener, feuert `TreeInvalidated` bei Subscribe | `STRUCTURE_WITH_PROPERTIES` | Ja |
| Windows UIA | Default No-op | `NONE` | Nein |
| AT-SPI | Default No-op | `NONE` | Nein |
| macOS AX | Stub-Crate | `NONE` | Nein |

### XDM-Cache (Option A – umgesetzt)

Der Lazy-Revalidierungs-Cache ist implementiert als `XdmCache` in `crates/runtime/src/xpath.rs`:
- Typ: `Rc<RefCell<Option<(RuntimeId, RuntimeXdmNode)>>>` (`Clone`, `!Send`).
- Erzeugt vom Aufrufer via `Runtime::create_cache()`, übergeben über `EvaluateOptions::with_cache()`.
- Revalidierung: `is_valid()` prüft ob der gecachte Wurzelknoten noch gültig ist; `prepare_for_evaluation()` traversiert den Baum und setzt `children_validated`-Flags zurück; ungültige Teilbäume werden beim nächsten Zugriff neu aufgebaut.
- Python-Bindings: Thread-lokales `HashMap<u64, XdmCache>` pro `PyRuntime`-Instanz; `clear_cache()` leert den Eintrag.
- Convenience-Methoden: `evaluate_cached()`, `evaluate_iter_cached()`, `evaluate_single_cached()`.

### Lücke

Kein realer Platform-Provider erzeugt Events. Der XDM-Cache (Option A) erkennt
entfernte Nodes via `is_valid()`, aber **neu hinzugefügte** Children eines
noch-gültigen Parents werden erst erkannt, wenn der Cache manuell geleert wird.

---

## Design

### 1. Dirty-Flag am Runtime

Das zentrale Verbindungsstück zwischen Background-Event-Threads und dem
Evaluierungs-Thread ist ein atomares Dirty-Flag:

```rust
// crates/runtime/src/runtime.rs
pub struct Runtime {
    // ... bestehende Felder ...
    /// Gesetzt von Event-Listenern auf Background-Threads wenn sich
    /// die UI-Baumstruktur geändert hat.
    cache_dirty: AtomicBool,
}
```

**Vor jeder Evaluation** prüft die Runtime das Flag:

```rust
// In Runtime::evaluate*_cached() Methoden
if self.cache_dirty.swap(false, Ordering::Acquire) {
    cache.clear(); // XdmCache::clear()
}
```

Das `RuntimeEventListener` setzt das Flag bei strukturellen Events:

```rust
impl ProviderEventListener for RuntimeEventListener {
    fn on_event(&self, event: ProviderEvent) {
        match &event.kind {
            ProviderEventKind::TreeInvalidated
            | ProviderEventKind::NodeAdded { .. }
            | ProviderEventKind::NodeRemoved { .. } => {
                self.cache_dirty.store(true, Ordering::Release);
            }
            ProviderEventKind::NodeUpdated { node } => {
                node.invalidate();
            }
        }
        self.dispatcher.on_event(event);
    }
}
```

### 2. Windows UIA — Structure Changed Events

#### COM-Event-Handler

Neues Modul `crates/provider-windows-uia/src/events.rs`:

```rust
use windows::Win32::UI::Accessibility::*;

#[implement(IUIAutomationStructureChangedEventHandler)]
struct StructureChangedHandler {
    listener: Arc<dyn ProviderEventListener>,
}

impl IUIAutomationStructureChangedEventHandler_Impl for StructureChangedHandler_Impl {
    fn HandleStructureChangedEvent(
        &self,
        _sender: Ref<IUIAutomationElement>,
        change_type: StructureChangeType,
        _runtime_id: *const SAFEARRAY,
    ) -> windows::core::Result<()> {
        match change_type {
            StructureChangeType_ChildAdded
            | StructureChangeType_ChildRemoved
            | StructureChangeType_ChildrenReordered
            | StructureChangeType_ChildrenBulkAdded
            | StructureChangeType_ChildrenBulkRemoved
            | StructureChangeType_ChildrenInvalidated => {
                self.listener.on_event(ProviderEvent {
                    kind: ProviderEventKind::TreeInvalidated,
                });
            }
            _ => {}
        }
        Ok(())
    }
}
```

#### subscribe_events Implementierung

```rust
// In WindowsUiaProvider
fn subscribe_events(
    &self,
    listener: Arc<dyn ProviderEventListener>,
) -> Result<(), ProviderError> {
    let automation = get_automation()?;
    let desktop = unsafe { automation.GetRootElement()? };

    let handler: IUIAutomationStructureChangedEventHandler =
        StructureChangedHandler { listener }.into();

    unsafe {
        automation.AddStructureChangedEventHandler(
            &desktop,
            TreeScope_Subtree,
            None,  // kein CacheRequest
            &handler,
        )?;
    }

    // Handler-Referenz speichern für cleanup in shutdown()
    self.event_handler.lock().unwrap().replace(handler);
    Ok(())
}
```

#### Descriptor-Update

```rust
ProviderDescriptor {
    // ...
    event_capabilities: ProviderEventCapabilities::STRUCTURE,
}
```

#### Threading-Modell

- UIA-Events werden auf einem COM-MTA-Thread zugestellt
- `ProviderEventListener` ist `Send + Sync` → Thread-sicher
- Der Handler setzt nur das atomare `cache_dirty`-Flag → kein Locking nötig
- `XdmCache::clear()` wird auf dem Evaluierungs-Thread aufgerufen (Cache wird explizit übergeben)

### 3. AT-SPI (Linux) — D-Bus Signals

#### Relevante Events

| D-Bus Signal | Mapping |
|---|---|
| `object:children-changed:add` | `NodeAdded` oder `TreeInvalidated` |
| `object:children-changed:remove` | `NodeRemoved` oder `TreeInvalidated` |
| `object:state-changed:*` | `NodeUpdated` |

#### Implementierungsansatz

```rust
// crates/provider-atspi/src/events.rs
fn subscribe_events(
    &self,
    listener: Arc<dyn ProviderEventListener>,
) -> Result<(), ProviderError> {
    let connection = self.connection.clone();

    // Hintergrund-Task für D-Bus Event-Loop
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut events = atspi::events::EventStream::new(&connection).await?;
            while let Some(event) = events.next().await {
                match event {
                    Event::Object(ObjectEvent::ChildrenChanged(change)) => {
                        listener.on_event(ProviderEvent {
                            kind: ProviderEventKind::TreeInvalidated,
                        });
                    }
                    _ => {}
                }
            }
            Ok::<(), ProviderError>(())
        })
    });

    Ok(())
}
```

Descriptor: `ProviderEventCapabilities::STRUCTURE`

### 4. macOS AX — Accessibility Notifications

#### Relevante Events

| Notification | Mapping |
|---|---|
| `kAXCreatedNotification` | `TreeInvalidated` |
| `kAXUIElementDestroyedNotification` | `TreeInvalidated` |
| `kAXValueChangedNotification` | `NodeUpdated` |

#### Implementierungsansatz

```rust
// crates/provider-macos-ax/src/events.rs
fn subscribe_events(
    &self,
    listener: Arc<dyn ProviderEventListener>,
) -> Result<(), ProviderError> {
    let observer = AXObserverCreate(pid, ax_callback)?;
    AXObserverAddNotification(observer, element, kAXCreatedNotification, context)?;
    AXObserverAddNotification(observer, element, kAXUIElementDestroyedNotification, context)?;
    CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), kCFRunLoopDefaultMode);
    Ok(())
}
```

Descriptor: `ProviderEventCapabilities::STRUCTURE`

---

## Phasenplan

### Phase 1: Dirty-Flag (Core/Runtime)

- `AtomicBool` `cache_dirty` in `Runtime` einfügen
- `RuntimeEventListener::on_event()` anpassen (Dirty-Flag setzen)
- `Runtime::evaluate*_cached()` Methoden: Flag prüfen → `XdmCache::clear()`
- Tests mit Mock-Provider (sendet bereits Events)

### Phase 2: Windows UIA Events

- `IUIAutomationStructureChangedEventHandler` implementieren
- `subscribe_events()` in `WindowsUiaProvider`
- `shutdown()`: `RemoveStructureChangedEventHandler` aufrufen
- Descriptor auf `STRUCTURE` setzen
- Manueller Test: Element in laufender App hinzufügen/entfernen, Cache wird automatisch invalidiert

### Phase 3: AT-SPI Events

- D-Bus `children-changed` Signal abonnieren
- Mapping auf `TreeInvalidated`
- Integration in `AtspiProvider::subscribe_events()`

### Phase 4: macOS AX Events

- `AXObserver` Setup
- Mapping auf `TreeInvalidated`
- Integration in `MacosAxProvider::subscribe_events()`

### Phase 5: Granulare Property-Events (optional)

- UIA: `AddPropertyChangedEventHandler` für Name, IsEnabled, etc.
- Mapping auf `NodeUpdated` → selektive Attribut-Cache-Invalidierung
- Potenzial: von ~55ms/Query auf ~15ms/Query (Attribut-Re-Read entfällt)

---

## Offene Fragen

1. **Event-Flooding**: Sollen Structure-Changed-Events debounced werden? Bei
   schnellen UI-Änderungen (z.B. ListView-Scroll) könnten hunderte Events
   ankommen. Ein Debounce-Window (z.B. 50ms) würde den Cache nur einmal
   invalidieren.

2. **Scope**: Soll der UIA-Handler auf `TreeScope_Subtree` vom Desktop
   registriert werden, oder nur auf den aktuellen Context-Node (Set Root)?
   Desktop-Scope ist einfacher, aber potentiell lauter.

3. **Granularität**: In Phase 1 wird pauschal `TreeInvalidated` gesendet.
   Lohnt es sich, die SAFEARRAY Runtime-ID aus dem COM-Event zu parsen und
   gezielt nur den betroffenen Teilbaum zu invalidieren?

4. **Shutdown-Reihenfolge**: COM-Event-Handler müssen vor dem `IUIAutomation`-
   Release deregistriert werden. Die aktuelle `shutdown()`-Implementierung
   muss das sicherstellen.
