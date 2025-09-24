# Provider Checklist (Draft)

> Status: Draft – use this list to verify provider compliance before merging changes. Extend or adjust as platform support evolves.

## Gemeinsame Prüfschritte
- [ ] Modul registriert sich über das vorgesehene `inventory`-Makro (`register_platform_module!`, `register_provider!`) und exportiert exakt eine Factory-Implementierung je Rolle (Plattform, Device, Provider).
- [ ] `ProviderDescriptor` ist vollständig ausgefüllt (`id`, Anzeigename, `Technology`, `ProviderKind`) und spiegelt die tatsächliche Quelle wider.
- [ ] `ProviderDescriptor::event_capabilities` beschreibt exakt, welche Ereignisse die Implementierung liefern kann (`None`, `ChangeHint`, `Structure`, `StructureWithProperties`). Wird keine Stufe gesetzt, muss der Provider davon ausgehen, dass die Runtime vor Abfragen Vollabfragen triggert.
- [ ] Provider geben ihren Baum als `Arc<dyn UiNode>` zurück; Attribute implementieren das `UiAttribute`-Trait und liefern Werte erst bei Bedarf (`UiAttribute::value()` → `UiValue`).
- [ ] `UiAttribute::value()` nutzt den passenden `UiValue`-Variant: `bool` → `UiValue::Bool`, Ganzzahlen → `UiValue::Integer`, Gleitkomma → `UiValue::Number`, Strings → `UiValue::String`. Komplexe Strukturen (`Rect`, `Point`, `Size`, JSON-Objekte/-Arrays) werden als entsprechende `UiValue`-Varianten geliefert; die Runtime erzeugt daraus typed-first Atomics für XPath (`Bounds.Width` ⇒ `xs:double`, `IsVisible` ⇒ `xs:boolean`).
- [ ] Steuerelemente erscheinen im `control`-Namespace, Items in Containerstrukturen im `item`-Namespace; andere Namensräume (`app`, `native`) bleiben ergänzend.
- [ ] Alle Koordinaten (`Bounds`, `ActivationPoint`, `ActivationArea`, Fensterrahmen) werden im Desktop-Koordinatensystem geliefert (linke obere Ecke des Primärmonitors = Ursprung, DPI-/Scaling berücksichtigt).
- [ ] `RuntimeId` bleibt stabil, solange das zugrunde liegende Element existiert; bei Neuaufbau ändert sich die ID nachvollziehbar.
- [ ] Quelle der `RuntimeId` dokumentiert (z. B. UIA `RuntimeId`, AT-SPI D-Bus-Objektpfad, macOS `AXUIElement` Identifier); bei fehlender nativer ID existiert ein deterministischer Fallback.
- [ ] `UiTreeProvider::get_nodes(parent)` liefert einen Iterator über Knoten, die unterhalb des angegebenen Parents eingehängt werden. Die Provider-Nodes dürfen keine eigenen Desktop-Attribute besitzen; setze den `parent()`-Verweis korrekt, damit die Runtime sie unterhalb des Desktop-Dokumentknotens einhängen kann.
- [ ] Falls technisch möglich, stellen Provider zwei Sichten bereit: (a) eine flache Liste der obersten `control:`-/`item:`-Knoten direkt unter dem Desktop (Standard-Namespace) und (b) eine gruppierte Sicht, in der dieselben Knoten zusätzlich unter `app:Application` einsortiert sind. Jede Anwendung enthält dabei nur die Fenster/Controls, die ihr tatsächlich zugeordnet sind (z. B. per Prozess-ID, App-Handle, Accessibility-Relation). Sollte nur eine der beiden Varianten praktikabel sein, reicht ein kurzer Hinweis in der Provider-Dokumentation, wie Anwender Anwendungszuordnungen alternativ erkennen können.
- [ ] Alias-Knoten müssen trotz identischer `RuntimeId` stabile Dokumentordnungs-Schlüssel liefern, damit XPath-Deduplikation nicht greift.
- [ ] Falls der Provider eigene Ressourcen hält (Threads, Handles), implementiert `UiTreeProvider::shutdown()` und gibt diese Ressourcen frei.
- [ ] `IsVisible` korrekt gesetzt (Accessibility-API meldet sichtbares Element); falls verfügbar, `IsOffscreen` konsistent mit Koordinaten/Viewports.
- [ ] Koordinatenfelder (`Bounds`, `ActivationPoint`, `ActivationArea`) liefern `Rect`/`Point`-Varianten; die Runtime erzeugt daraus automatisch Ableitungen wie `Bounds.X`, `Bounds.Width`. Provider müssen nur den Basistyp korrekt füllen.
- [ ] `SupportedPatterns` enthält nur Patterns, deren Pflichtattribute vollständig gesetzt sind; optionale Felder sind `null` oder fehlen (Namespace entsprechend `control` oder `item`). Erst wenn diese Bedingungen erfüllt sind, darf die Pattern-ID eingetragen werden.
- [ ] Für RuntimePatterns (Fokus, WindowSurface, Application) stellen Provider konkrete Instanzen bereit (`UiNode::pattern::<T>()` → `Some(Arc<T>)`). Die Einträge in `supported_patterns()` müssen exakt mit den abrufbaren Pattern-Objekten übereinstimmen.
- [ ] Fenster, die `WindowSurface` melden, sollen zugleich `Focusable` unterstützen (`IsFocused`-Attribut, Fokus-Aktion). `WindowSurface::activate()/restore()` setzen den Fokus, `minimize()/close()` geben ihn frei (inkl. `NodeUpdated`), damit der Zustand mit nativen Foreground-Wechseln übereinstimmt.
- [ ] Teure Plattformabfragen dürfen „lazy“ erfolgen: Über `PatternRegistry::register_lazy` kann ein Pattern erst beim ersten Zugriff geprüft werden. Wichtig ist, dass `SupportedPatterns` und `supported_patterns()` das Ergebnis dieser Probe widerspiegeln (bei Nichtverfügbarkeit wird die Pattern-ID entfernt bzw. gar nicht erst veröffentlicht).
- [ ] `Focusable`-Knoten liefern ein dynamisches `IsFocused`-Attribut (`UiValue::Bool`) und emittieren bei Fokuswechsel `ProviderEventKind::NodeUpdated` für den bisherigen sowie den neuen Fokus, damit Caches/Runtime den Zustand aktualisieren.
- [ ] Fehler von Runtime-Aktionen (`focus()`, `activate()`, …) werden als `PatternError` mit prägnanter Nachricht zurückgegeben (kein Panic / unwrap innerhalb der Provider-Schicht).
- [ ] Bereitgestellte Attribute stimmen mit den ClientPattern-Anforderungen aus `docs/patterns.md` überein (Bezeichner in PascalCase, Wertebereiche, optional vs. Pflichtfelder). Das Core-Testkit `platynui_core::ui::contract::testkit` prüft diese Zuordnung automatisiert – Provider sollten die erwarteten Konstanten (`platynui_core::ui::attribute_names`) wiederverwenden.
- [ ] Geometrie-Aliaswerte (`Bounds.X`, `Bounds.Width`, `ActivationPoint.X`, `ActivationPoint.Y` …), sofern geliefert, spiegeln die zugrunde liegenden `Rect`-/`Point`-Attribute wider. Das Contract-Testkit meldet Abweichungen; unterschiedliche Werte gelten als Fehler.
- [ ] `UiTreeProvider::subscribe_events(listener)` implementiert den Event-Weg: Sobald die Runtime (oder ein anderer Host) einen Listener registriert, liefert der Provider zukünftige Baumereignisse über diesen Kanal.
- [ ] Optional, aber empfohlen: Beim Registrieren eines Listeners ein initiales `TreeInvalidated` senden, damit Konsumenten (Runtime, CLI `watch`) unmittelbar einen konsistenten Refresh auslösen können.
- [ ] `Role` entspricht dem normalisierten Namen (lokaler Name im Namespace `control` oder `item`), die native Rolle liegt zusätzlich unter `native:Role` (oder äquivalenten Feldern).
- [ ] Meldet ein Element das Pattern `ActivationTarget`, liefert es `ActivationPoint` (Desktop-Koordinaten, ggf. Fallback auf Rechteckzentrum) und optional `ActivationArea`.
- [ ] `Technology` ist für jede `UiNode` gesetzt (`UIAutomation`, `AT-SPI`, `AX`, `JSONRPC`, ...).
- [ ] Provider erzeugen keinen eigenen Desktop-Dokumentknoten; die Plattform-/Runtime-Schicht stellt Desktop-Metadaten (`Bounds`, `OsName`, `OsVersion`, `DisplayCount`, `Monitors`) bereit.
- [ ] Mapping-Entscheidungen gegen `docs/patterns.md` dokumentiert (insb. bei Mehrfachzuordnungen).
- [ ] Baum-Ereignisse (`NodeAdded`, `NodeUpdated`, `NodeRemoved`) getestet; sicherstellen, dass sie nur zur Synchronisation dienen und keine Pattern-spezifischen Nebenwirkungen haben.
- [ ] `UiNode::invalidate()` räumt alle gecachten Informationen der Node ab (Kind-Iteratoren, Attribute, Pattern-Instanzen). Nach einer Invalidierung muss der nächste Zugriff die Daten frisch aus der nativen API oder dem Provider-Cache laden.

## Windows (`platynui-provider-windows-uia`)
- [ ] `Bounds` basieren auf `IUIAutomationElement::CurrentBoundingRectangle` und sind in Desktop-Koordinaten umgerechnet.
- [ ] `ActivationPoint` nutzt `GetClickablePoint()`; fehlende Werte werden über das Elementzentrum ersetzt.
- [ ] `TextContent`, `TextEditable`, `TextSelection` nutzen Priority: `NameProperty` → `ValuePattern` → `TextPattern`.
- [ ] `WindowSurface`-Attribute (`IsMinimized`, `IsMaximized`, `IsTopmost`) spiegeln `WindowPattern`/`TransformPattern` wider.
- [ ] `Application`-Knoten liefern Prozessmetadaten (`ProcessId`, `ProcessName`, `ExecutablePath`) und optional `CommandLine`, `UserName`.
- [ ] `WindowSurface::accepts_user_input()` liefert – sofern die Plattform eine zuverlässige Abfrage erlaubt – den aktuellen Eingabestatus (Windows: `WaitForInputIdle`; andere Plattformen dokumentieren die verwendete Heuristik oder geben `None` zurück). Ein optionales Attribut `window:AcceptsUserInput` darf den Wert spiegeln.
- [ ] `Selectable`/`SelectionProvider` synchronisiert über `SelectionItemPattern`/`SelectionPattern`.
- [ ] Highlight/Overlay-Pfad (DirectComposition/GDI/XComposite/…) respektiert die gelieferten `Bounds`, unterstützt optionale Dauerangaben und garantiert, dass maximal ein Overlay aktiv ist: Neue Highlight-Anfragen verschieben den bestehenden Rahmen und setzen die Laufzeit zurück, statt zusätzliche Fenster zu erzeugen.
- [ ] Eigenständige Hilfsfenster des Providers (Highlight-Overlay, Inspectoren, Debug-Panels) tauchen nicht im UiTree auf; der Provider filtert alle Fenster des eigenen Prozesses konsequent heraus.

## Linux X11 (`platynui-provider-atspi` + `platynui-platform-linux-x11`)
- [ ] Koordinaten stammen aus `Component::get_extents(ATSPI_COORD_TYPE_SCREEN)`.
- [ ] `TextContent`/`TextSelection` über `Text`-Interface, UTF-8 sauber gehandhabt.
- [ ] `SelectionProvider` nutzt `Selection`-Interface, `SelectionContainerId` basiert auf `Accessible::path` oder stabilem Handle.
- [ ] `WindowSurface`-Aktionen delegieren an die X11-Fenster-APIs (EWMH) via `platynui-platform-linux-x11`.
- [ ] `ActivationTarget` berechnet optional eigene Center-Punkte, wenn `Component::get_offset_at_point` nicht verfügbar ist.

## macOS (`platynui-provider-macos-ax`)
- [ ] Koordinaten aus `AXFrame` in Core Graphics Desktop-Koordinaten (unter Berücksichtigung mehrerer Monitore) transformiert.
- [ ] `TextEditable` setzt `IsReadOnly` basierend auf `AXEditable`/`AXEnabled`.
- [ ] `Activatable` verwendet `AXPress`; `IsActivationEnabled` spiegelt `AXEnabled`.
- [ ] `WindowSurface`-Aktionen nutzen Accessibility-API + `CGWindow`/`NSWorkspace`-Hilfen.
- [ ] Monitorliste (Desktop) über `NSScreen` bzw. `CoreGraphics` zur Verfügung gestellt.

## JSON-RPC (`platynui-provider-jsonrpc`)
- [ ] Provider-Kennzeichnung erfolgt durch `Technology="JSONRPC"`; Versionsinformationen liefert der `initialize`-Handshake.
- [ ] `initialize`-Handshake liefert Transport-Endpunkt, Version, Technologiekennung, RuntimeId-Schema sowie Heartbeat-Einstellungen; `resolveRuntimeId`-Unterstützung (oder explizite Nicht-Unterstützung) wird deklariert.
- [ ] Baum-Events (`$/notifyNodeAdded`, `$/notifyNodeUpdated`, `$/notifyNodeRemoved`, `$/notifyTreeInvalidated`) sind implementiert und werden in Tests ausgelöst.
- [ ] Nachrichten-Contract (z. B. JSON Schema oder Typsystem) gepflegt, damit Requests/Responses validiert werden können.
- [ ] Heartbeat-/Reconnect-Logik im Runtime-Client getestet.
- [ ] Sicherheitsrichtlinien (Pipe/Soket-Namensschema, Berechtigungen) umgesetzt.
- [ ] Provider liefern ausschließlich den UI-Baum; Eingabe- oder Window-Management-Funktionen verbleiben bei den Plattform-Crates.

## Mock (`platynui-provider-mock` & `platynui-platform-mock`)
- [ ] Referenzdaten decken typische Pattern-Kombinationen ab (z. B. Button, Textfeld, Checkbox, Tree Item).
- [ ] Tests validieren Desktop-Koordinaten und `ActivationTarget`.
- [ ] Scripted-Mock erlaubt negative Szenarien (fehlende Pflichtattribute, ungültige Koordinaten) für Contract-Tests.

## Automatisierte Prüfungen (Ideen)
- [ ] Integrationstest, der `docs/patterns.md` parst und Pflichtattribute pro Pattern mit Provider-Ausgabe vergleicht.
- [ ] CI-Job, der Koordinatenbereiche auf Plausibilität prüft (nicht negativ, innerhalb Monitorfläche).
- [ ] Linter, der verbotene Pattern/Attribut-Kombinationen meldet (z. B. `TextEditable` ohne `TextContent`).
