# Windows UIAutomation Provider – Designentscheidungen (19.6)

English summary
- No separate actor thread. We initialize COM once per thread with `CoInitializeEx(..., COINIT_MULTITHREADED)` and call UIA directly. This keeps things simple and works well for our current needs.
- Traverse the tree exclusively via Raw View TreeWalker (Parent/FirstChild/NextSibling). Do not use FindAll. Do not use UIA CacheRequest; we’ll add our own caching later.
- Expose proper iterators for `UiNode.children()` and `UiNode.attributes()`. They fetch the next item on demand instead of materializing a Vec up front.
- No NodeStore in this MVP: `UiaNode` wraps the `IUIAutomationElement` directly. Iterators create walkers on demand and yield lightweight node wrappers.
- Patterns for slice 1: `Focusable` via `SetFocus()`, `WindowSurface` via `WindowPattern`/`TransformPattern` (activate/minimize/maximize/restore/close/move/resize). `accepts_user_input` uses a simple heuristic.
- Attributes are resolved lazily on demand: Role (from ControlType), Name, RuntimeId (encoded), Bounds, IsEnabled, IsOffscreen, ActivationPoint.
- Parents are always proper `UiNode` references: provider attaches the given parent in `get_nodes(...)`, and children set `self` as parent when created.

---

## Ziel und Scope
Dieses Dokument präzisiert die Umsetzung des UIAutomation‑Providers (19.6) für Windows. Es hält grundlegende Design‑Entscheidungen fest, damit Implementierung, Reviews und spätere Erweiterungen (Events, Caching) auf einer klaren, stabilen Basis aufsetzen.

## Threading & COM
- Pro Thread einmal `CoInitializeEx(nullptr, COINIT_MULTITHREADED)` ausführen; wir halten keine separate Actor‑Schicht vor.
- `IUIAutomation` sowie `IUIAutomationTreeWalker` (RawView) liegen als thread‑lokale Singletons vor:
  - `com::uia() -> IUIAutomation` liefert stets dieselbe Instanz (per `Clone` AddRef) auf diesem Thread.
  - `com::raw_walker() -> IUIAutomationTreeWalker` liefert denselben RawView‑Walker.
- Iteratoren/Knoten verwenden diese Singletons; wir vermeiden wiederholte `CoCreateInstance`/`RawViewWalker()` Aufrufe.
- Annahme: Nutzung erfolgt auf demselben (MTA‑)Thread, auf dem die Objekte erzeugt wurden. Events/Message‑Pump sind zukünftige Arbeiten.

## Traversal (ohne FindAll, ohne UIA CacheRequest)
- Baumzugriff ausschließlich über den Raw View TreeWalker:
  - Eltern: `GetParentElement(elem)`
  - Erstes Kind: `GetFirstChildElement(elem)`
  - Nächstes Geschwister: `GetNextSiblingElement(elem)`
- Top‑Level‑Aufzählung (Kinder des „Desktop“): Wir iterieren alle Root‑Kinder (mehrere „Desktop“‑Äste sind möglich) und für jeden Desktop dessen Kinder. So erhalten wir zuverlässig mehr als nur ein Element – ohne `FindAll` und ausschließlich mit dem Walker.
- UIA CacheRequest wird bewusst vermieden; wir planen später ein eigenes, kontrolliertes Caching im Provider.

Implementierungsdetail Iteratoren
- Gemeinsamer `ElementChildrenIter` für alle Eltern‑Elemente (inkl. Root/„Desktop“):
  - Lazy‑Erstaufruf mit `first`‑Marker: `GetFirstChildElement(parent)` startet erst beim ersten `next()`.
  - Danach nur `GetNextSiblingElement(current)` pro Schritt.
  - Iterator hält den modellseitigen Parent (`Arc<dyn UiNode>`) und setzt ihn bei jedem erzeugten Kind.

## Node‑Wrapper und Iteratoren
- `UiaNode` wrappt direkt das `IUIAutomationElement` und hält keine zentrale Store‑Struktur.
- Echte Iteratoren statt vorab gebauter Vektoren:
  - `children(&self) -> Iterator<Item = Arc<dyn UiNode>>`: `ChildrenIter` legt pro Iterator einen `TreeWalker` an (RawView) und bewegt sich mit `FirstChild/NextSibling`.
  - `attributes(&self) -> Iterator<Item = Arc<dyn UiAttribute>>`: `AttrsIter` gibt dünne Wrapper zurück; `value()` ermittelt den Wert erst bei Zugriff.
- Elternbeziehung korrekt setzen:
  - In `get_nodes(parent: Arc<dyn UiNode>)` setzt der Provider beim Erzeugen von Kinder‑Knoten sofort `set_parent(&parent)`.
  - In `UiaNode.children()` setzt jeder erzeugte Kind‑Knoten `set_parent(self.as_ui_node())` bevor er aus dem Iterator zurückgegeben wird.

UiNode‑Basics (lazy)
- `namespace()`: über UIA‑Flags `CurrentIsControlElement`/`CurrentIsContentElement` (Control hat Priorität; Content → `item`; Fallback → `control`).
- `role()`: aus `ControlType` per Mapping; Ergebnisse im Knoten gecached.
- `name()`: `CurrentName()`; lazy gecached.
- `runtime_id()`: `GetRuntimeId()` → `uia://...`; lazy gecached.
- `invalidate()`: Derzeit bewusst No‑Op, da die Trait‑Signaturen Referenzen zurückgeben. Attributwerte bleiben lazy und können unabhängig neu gelesen werden. Echte Cache‑Invalidierung ist möglich, würde aber API‑Anpassungen oder eine interne, referenzstabile Cache‑Schicht erfordern.

## Kein NodeStore im MVP
- Ein separater NodeStore ist zunächst nicht erforderlich. Iteratoren besitzen kurzlebige Walker und erzeugen Knoten on‑demand. Später können wir bei Bedarf ein eigenes Caching ergänzen.

## Attribute (lazy)
- Role: aus `ControlType` → String‑Mapping; Namespace aus dem Rollentyp (z. B. `app:Application`, `control:*`, `item:*`).
- Name: `UIA_NamePropertyId` → `Name`.
- RuntimeId: `GetRuntimeId()` (SAFEARRAY von INT) → String‑Schema `uia://<hex>.<hex>…`.
- Bounds: `UIA_BoundingRectanglePropertyId` → `Rect(x,y,width,height)`.
- Sichtbarkeit/Status: `UIA_IsEnabledPropertyId` → `IsEnabled`, `UIA_IsOffscreenPropertyId` → `IsOffscreen`, abgeleitet `IsVisible = !IsOffscreen && Bounds.Width>0 && Bounds.Height>0`.
- ActivationPoint: Mitte der Bounds (später optional `GetClickablePoint`).
- Alle Werte werden erst in `UiAttribute.value()` zur Laufzeit ermittelt (kein Proaktiv‑Fetch).

## Patterns (Slice 1)
- Focusable: `element.SetFocus()`; Fehler werden als `PatternError` gemeldet.
- WindowSurface:
  - `WindowPattern`: `SetWindowVisualState(Normal/Maximized/Minimized)`, `Close()`.
  - `TransformPattern`: `Move(x,y)`, `Resize(w,h)`, `MoveAndResize`.
  - `accepts_user_input`: Heuristik aus `IsEnabled && !IsOffscreen` → `Some(bool)`, sonst `None`.
- `supported_patterns()` prüft Verfügbarkeit (`GetCurrentPattern(Window/Transform)`) und gibt nur dann `WindowSurface` an, wenn eines der Muster vorhanden ist.

Codeaufteilung
- `com.rs`: COM‑Init (MTA) und thread‑lokale Singletons (UIA, RawView‑Walker).
- `node.rs`: `UiaNode` + Attribute‑Iterator + Pattern‑Implementierungen + `ElementChildrenIter`.
- `provider.rs`: Factory + `get_nodes(...)` für Desktop‑Kinder via `ElementChildrenIter` (Root‑Element → erster Desktop → dessen Kinder). Keine ControlView/FindAll.

## Fehlerabbildung & Shutdown
- Provider‑Fehler werden als `ProviderError` gemeldet; Pattern‑Aufrufe als `PatternError`.
- Shutdown: Actor beendet, UIA‑Objekte freigegeben, `CoUninitialize` gerufen.

## Akzeptanzkriterien (Slice 1)
- Build/Tests auf Windows erfolgreich; Provider registriert.
- Runtime liefert mehrere Top‑Level‑Knoten; `children()` traversiert rekursiv via Raw View Walker.
- Attribute sind verfügbar (Role/Name/Bounds/RuntimeId/IsEnabled/IsOffscreen/ActivationPoint).
- Patterns Focusable/WindowSurface funktionieren auf unterstützten Knoten (Fehlerfälle sauber gemappt).

Bekannte Grenzen/Nächste Schritte
- Root‑Geschwister („mehrere Desktop‑Äste“) werden aktuell nicht in einem Durchlauf zusammengeführt; falls erforderlich, kann ein leichter Mehr-Eltern‑Iterator vorgeschaltet werden (weiterhin Walker‑basiert, ohne `FindAll`).
- `invalidate()` ist No‑Op; bei realem Bedarf API/Cache‑Design anpassen.

## Ausblick (nach MVP)
- Struktur‑/Property‑Events über UIA‑Eventhandler (Actor hält Message‑Pump).
- Eigenes Caching (zeitlich/inhaltlich) im Provider statt UIA‑Cache API.
- Erweiterte Rollen‑/Namespace‑Abbildung und zusätzliche Patterns.
