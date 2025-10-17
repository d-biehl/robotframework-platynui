# Windows UIAutomation Provider – Designentscheidungen (19.6)

English summary
- No separate actor thread. We initialize COM once per thread with `CoInitializeEx(..., COINIT_MULTITHREADED)` and call UIA directly. This keeps things simple and works well for our current needs.
- Traverse the tree exclusively via Raw View TreeWalker (Parent/FirstChild/NextSibling). Do not use FindAll. Do not use UIA CacheRequest; we’ll add our own caching later.
- Expose proper iterators for `UiNode.children()` and `UiNode.attributes()`. They fetch the next item on demand instead of materializing a Vec up front.
- No NodeStore in this MVP: `UiaNode` wraps the `IUIAutomationElement` directly. Iterators create walkers on demand and yield lightweight node wrappers.
- Patterns for slice 1: `Focusable` via `SetFocus()`, `WindowSurface` via `WindowPattern`/`TransformPattern` (activate/minimize/maximize/restore/close/move/resize). `accepts_user_input` uses a simple heuristic.
- Attributes are resolved lazily on demand: Role (from ControlType), Name, RuntimeId (encoded), Bounds, IsEnabled, IsOffscreen, ActivationPoint.
- Parents are always proper `UiNode` references: provider attaches the given parent in `get_nodes(...)`, and children set `self` as parent when created.
- Error handling: internal UIA calls use a typed `UiaError` (thiserror). Provider boundaries map `UiaError` to `ProviderError` variants; no `Result<_, String>`.

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
  - Rekursion wurde bewusst durch iterative Schleifen ersetzt, um Stacktiefe zu vermeiden und Fehlerpfade klarer zu handhaben.
  - Der eigene Prozess wird frühzeitig herausgefiltert (SELF_PID‑Cache via `OnceCell/Lazy`), sodass der Provider keine UI‑Elemente des eigenen Prozesses zurückliefert (Overlap mit Overlay/Inspector wird vermieden).

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
- `runtime_id()`: `GetRuntimeId()` → gescopte URI (siehe Abschnitt unten); lazy gecached.
- `invalidate()`: Derzeit bewusst No‑Op, da die Trait‑Signaturen Referenzen zurückgeben. Attributwerte bleiben lazy und können unabhängig neu gelesen werden. Echte Cache‑Invalidierung ist möglich, würde aber API‑Anpassungen oder eine interne, referenzstabile Cache‑Schicht erfordern.

## Kein NodeStore im MVP
- Ein separater NodeStore ist zunächst nicht erforderlich. Iteratoren besitzen kurzlebige Walker und erzeugen Knoten on‑demand. Später können wir bei Bedarf ein eigenes Caching ergänzen.

## Attribute (lazy)
- Role: aus `ControlType` → String‑Mapping; Namespace aus dem Rollentyp (z. B. `app:Application`, `control:*`, `item:*`).
- Name: `UIA_NamePropertyId` → `Name`.
- RuntimeId: `GetRuntimeId()` (SAFEARRAY von INT) → gescopte URI, z. B. `uia://desktop/<hex>.<hex>…` bzw. `uia://app/<pid>/<hex>.<hex>…`.

## RuntimeId‑Schema (scoped URIs)
- UIA RuntimeId ist nur zur Laufzeit eindeutig und als Opaque‑Wert zu behandeln. Damit Knoten in unterschiedlichen Sichten (TopLevel vs. App‑Gruppierung) nicht kollidieren, versehen wir die ID mit einem Scope.
- Desktop‑Sicht: `uia://desktop/<rid>`
- App‑Gruppierung: `uia://app/<pid>/<rid>`
- `<rid>` ist der punkt‑separierte Hex‑Body aus `GetRuntimeId()`.

Implementierungsdetails
- Der Hex‑Body wird unverändert aus dem SAFEARRAY erzeugt (32‑Bit‑Ints → Hex → durch Punkte getrennt).
- Der Scope wird basierend auf dem Erzeugungskontext gesetzt: TopLevel‑Nodes erhalten `desktop`, Kinder eines `app:Application` erhalten `app/<pid>`.
- Damit bleiben IDs innerhalb der kombinierten Darstellung eindeutig, ohne das UIA‑Semantik zu verletzen.
- Bounds: `UIA_BoundingRectanglePropertyId` → `Rect(x,y,width,height)`.
- Sichtbarkeit/Status: `UIA_IsEnabledPropertyId` → `IsEnabled`, `UIA_IsOffscreenPropertyId` → `IsOffscreen`, abgeleitet `IsVisible = !IsOffscreen && Bounds.Width>0 && Bounds.Height>0`.
- ActivationPoint: Mitte der Bounds (später optional `GetClickablePoint`).
- Alle Werte werden erst in `UiAttribute.value()` zur Laufzeit ermittelt (kein Proaktiv‑Fetch).

### Native UIA‑Properties (Namespace `native:`)
- Ziel: Alle vom `IUIAutomationElement` effektiv unterstützten UIA‑Properties können direkt als Attribute im Namespace `native` abgefragt werden (z. B. `//control:Button[@native:ClassName="abc"]`).
- Unterstützung ermitteln: Die COM‑API stellt keine direkte Liste „unterstützter Properties“ bereit. Stattdessen wird der Programmatic‑Name‑Katalog über `IUIAutomation::GetPropertyProgrammaticName(propertyId)` im üblichen UIA‑ID‑Bereich aufgebaut. Für jede ID wird der aktuelle Wert via `IUIAutomationElement::GetCurrentPropertyValueEx(propertyId, /*ignoreDefault*/ true)` gelesen.
- Typumsetzung: Rückgabewert (VARIANT) wird in `UiValue` gemappt: `VT_BOOL`→Bool, `VT_I4/VT_UI4`→Integer, `VT_I8/VT_UI8`→Integer, `VT_R8/VT_R4`→Number, `BSTR`→String, `SAFEARRAY`→Array. Unbekannte Typen werden ausgelassen.
- Sentinels: `UiaGetReservedNotSupportedValue` kennzeichnet „nicht unterstützt”; `UiaGetReservedMixedAttributeValue` kennzeichnet gemischte Werte. Beide werden erkannt (VT_UNKNOWN/punkVal‑Vergleich) und gefiltert.
- Exponierung: Für jedes vorhandene Property wird ein `UiAttribute` im Namespace `Native` mit Programmatic Name bereitgestellt. Optional kann ein aggregiertes Objekt (z. B. `native:UIA.Properties`) für Debugging ergänzt werden.
- Lazy‑Evaluation: Property‑Werte werden erst bei Zugriff auf `value()` gelesen, um Traversal schlank zu halten.

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
- Interne UIA‑Aufrufe: typisierter `UiaError` (thiserror), u. a. `Api { context, message }`, `ComInit`, `Null`.
- Provider‑Boundary: Abbildung auf `ProviderError`‑Varianten (z. B. `CommunicationFailure { context }`). Pattern‑Aufrufe melden `PatternError` mit klaren Meldungen.
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
