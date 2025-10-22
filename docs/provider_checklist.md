# Provider Checklist (Draft)

> Status: Draft – use this list to verify provider compliance before merging changes. Extend or adjust as platform support evolves.

## Gemeinsame Prüfschritte
- [ ] Modul registriert sich über das vorgesehene `inventory`-Makro (`register_platform_module!`, `register_provider!`) und exportiert exakt eine Factory-Implementierung je Rolle (Plattform, Device, Provider).
- [ ] Produktive Implementierungen müssen unter ihrem Zielbetriebssystem (`cfg(target_os = …)`) kompiliert werden und registrieren sich automatisch via `register_provider!`. Mock-Provider (`platynui-provider-mock`, `platynui-platform-mock`) registrieren sich **nicht** automatisch – sie sind ausschließlich über explizite Factory-Handles verfügbar (in Rust: `MOCK_PROVIDER_FACTORY.create()`, in Python: `Runtime.new_with_providers([MOCK_PROVIDER])`).
- [ ] Für Linux-spezifische Implementierungen ist perspektivisch ein Vermittlungscrate (`platynui-platform-linux`) vorgesehen, das je Session zwischen X11 und Wayland vermittelt. Stellt sicher, dass `platynui-platform-linux-x11` und `platynui-platform-linux-wayland` kompatible Schnittstellen anbieten, um dort eingebunden zu werden.
- [ ] `ProviderDescriptor` ist vollständig ausgefüllt (`id`, Anzeigename, `Technology`, `ProviderKind`) und spiegelt die tatsächliche Quelle wider.
- [ ] `ProviderDescriptor::event_capabilities` beschreibt exakt, welche Ereignisse die Implementierung liefern kann (`None`, `ChangeHint`, `Structure`, `StructureWithProperties`). Wird keine Stufe gesetzt, muss der Provider davon ausgehen, dass die Runtime vor Abfragen Vollabfragen triggert.
- [ ] Provider geben ihren Baum als `Arc<dyn UiNode>` zurück; Attribute implementieren das `UiAttribute`-Trait und liefern Werte erst bei Bedarf (`UiAttribute::value()` → `UiValue`).
- [ ] `UiAttribute::value()` nutzt den passenden `UiValue`-Variant: `bool` → `UiValue::Bool`, Ganzzahlen → `UiValue::Integer`, Gleitkomma → `UiValue::Number`, Strings → `UiValue::String`. Komplexe Strukturen (`Rect`, `Point`, `Size`, JSON-Objekte/-Arrays) werden als entsprechende `UiValue`-Varianten geliefert; die Runtime erzeugt daraus typed-first Atomics für XPath (`Bounds.Width` ⇒ `xs:double`, `IsVisible` ⇒ `xs:boolean`).
- [ ] Steuerelemente erscheinen im `control`-Namespace, Items in Containerstrukturen im `item`-Namespace; andere Namensräume (`app`, `native`) bleiben ergänzend.
- [ ] Alle Koordinaten (`Bounds`, `ActivationPoint`, `ActivationArea`, Fensterrahmen) werden im Desktop-Koordinatensystem geliefert (linke obere Ecke des Primärmonitors = Ursprung, DPI-/Scaling berücksichtigt).
- [ ] `RuntimeId` bleibt stabil, solange das zugrunde liegende Element existiert; bei Neuaufbau ändert sich die ID nachvollziehbar. Jede ID folgt dem Schema `prefix:value`, wobei `prefix` einen kurzen, eindeutigen Anbieter- oder Technologie-Namen darstellt (z. B. `uia`, `atspi`, `ax`, `mock`). Der Doppelpunkt fungiert als Trenner; der Wert hinter dem Präfix kann frei gewählt werden (native Kennung, Hash ...), muss aber stabil bleiben. Das Präfix `platynui` ist reserviert und wird ausschließlich vom Runtime-Desktop (`platynui:Desktop`) verwendet.
- [ ] Quelle der `RuntimeId` dokumentiert (z. B. UIA `RuntimeId`, AT-SPI D-Bus-Objektpfad, macOS `AXUIElement` Identifier); bei fehlender nativer ID existiert ein deterministischer Fallback.
- [ ] `Id` (optional) dokumentiert und korrekt gemappt:
  - Windows/UIA: `AutomationId` → `control:Id` (leere Zeichenfolge ⇒ nicht gesetzt).
  - Linux/AT‑SPI2: sofern verfügbar `accessible_id` → `control:Id`; andernfalls nicht setzen.
  - macOS/AX: sofern verfügbar `AXIdentifier` → `control:Id`; andernfalls nicht setzen.
  - Für `app:Application`: wenn möglich plattformspezifische, stabile Kennung (z. B. Bundle Identifier); ansonsten Fallback explizit begründen.
  - Emission: `@control:Id` darf nur erzeugt werden, wenn ein Wert vorhanden ist (kein `null`/leer ausgeliefertes Attribut).
- [ ] `UiTreeProvider::get_nodes(parent)` liefert einen Iterator über Knoten, die unterhalb des angegebenen Parents eingehängt werden. Die Provider-Nodes dürfen keine eigenen Desktop-Attribute besitzen; setze den `parent()`-Verweis korrekt, damit die Runtime sie unterhalb des Desktop-Dokumentknotens einhängen kann.
- [ ] Falls technisch möglich, stellen Provider zwei Sichten bereit: (a) eine flache Liste der obersten `control:`-/`item:`-Knoten direkt unter dem Desktop (Standard-Namespace) und (b) eine gruppierte Sicht, in der dieselben Knoten zusätzlich unter `app:Application` einsortiert sind. Jede Anwendung enthält dabei nur die Fenster/Controls, die ihr tatsächlich zugeordnet sind (z. B. per Prozess-ID, App-Handle, Accessibility-Relation). Sollte nur eine der beiden Varianten praktikabel sein, reicht ein kurzer Hinweis in der Provider-Dokumentation, wie Anwender Anwendungszuordnungen alternativ erkennen können.
- [ ] Alias-Knoten müssen trotz identischer `RuntimeId` stabile Dokumentordnungs-Schlüssel liefern, damit XPath-Deduplikation nicht greift.
- [ ] Falls der Provider eigene Ressourcen hält (Threads, Handles), implementiert `UiTreeProvider::shutdown()` und gibt diese Ressourcen frei.
- [ ] `IsVisible` korrekt gesetzt (Accessibility-API meldet sichtbares Element); falls verfügbar, `IsOffscreen` konsistent mit Koordinaten/Viewports.
- [ ] Koordinatenfelder (`Bounds`, `ActivationPoint`, `ActivationArea`) liefern `Rect`/`Point`-Varianten; die Runtime erzeugt daraus automatisch Ableitungen wie `Bounds.X`, `Bounds.Width`. Provider müssen nur den Basistyp korrekt füllen.
- [ ] `SupportedPatterns` enthält nur Patterns, deren Pflichtattribute vollständig gesetzt sind; optionale Felder sind `null` oder fehlen (Namespace entsprechend `control` oder `item`). Erst wenn diese Bedingungen erfüllt sind, darf die Pattern-ID eingetragen werden.
- [ ] Für RuntimePatterns (Fokus, WindowSurface, Application) stellen Provider konkrete Instanzen bereit (`UiNode::pattern::<T>()` → `Some(Arc<T>)`). Die Einträge in `supported_patterns()` müssen exakt mit den abrufbaren Pattern-Objekten übereinstimmen.
- [ ] Fenster, die `WindowSurface` melden, sollen zugleich `Focusable` unterstützen (`IsFocused`-Attribut, Fokus-Aktion). `WindowSurface::activate()/restore()` setzen den Fokus, `minimize()/close()` geben ihn frei (inkl. `NodeUpdated`), damit der Zustand mit nativen Foreground-Wechseln übereinstimmt.
- [ ] Plattform-Module mit `KeyboardDevice` dokumentieren die unterstützten Tastennamen konsistent und orientieren sich an den offiziellen OS-Bezeichnungen (`Control`, `Shift`, `Alt`, `Enter`, `Command`, `Windows`, `Super`/`Meta` usw.). Bei Abweichungen (z. B. Plattform-spezifische Zusatztasten) ist die Übersetzung in der Provider-Dokumentation festzuhalten.
- [ ] `key_to_code(&str)` löst Namen/Aliasse zuverlässig auf (`KeyboardError::UnsupportedKey` liefert aussagekräftige Hinweise) und `send_key_event(KeyboardEvent)` erzeugt korrekte Press/Release-Sequenzen. `start_input`/`end_input` dürfen ausschließlich tastaturspezifische Vor- bzw. Nacharbeiten übernehmen (z. B. Layout-/IME-Umschaltung, Pufferleeren), keine Fokusmanipulation oder Fensteraktivierung.
- [ ] Teure Plattformabfragen dürfen „lazy“ erfolgen: Über `PatternRegistry::register_lazy` kann ein Pattern erst beim ersten Zugriff geprüft werden. Wichtig ist, dass `SupportedPatterns` und `supported_patterns()` das Ergebnis dieser Probe widerspiegeln (bei Nichtverfügbarkeit wird die Pattern-ID entfernt bzw. gar nicht erst veröffentlicht).

## Keyboard (Geräte & Benennung)
- [ ] `KeyboardDevice` implementiert `key_to_code(&str)`, `send_key_event(KeyboardEvent)`, optionale `start_input`/`end_input` sowie `known_key_names()`.
- [ ] `known_key_names()` listet alle benannten Tasten in der offiziellen OS‑Nomenklatur auf; Vergleiche sind case‑insensitiv. Zeichen (Buchstaben/Ziffern) dürfen von `key_to_code()` akzeptiert werden, ohne in der Liste zu erscheinen.
- [ ] Benennung ohne Präfixe/Abkürzungsrauschen (z. B. unter Windows keine `VK_*`‑Präfixe). Gleiche Tasten sollen plattformübergreifend gleich heißen (z. B. `Enter`, `Escape`, `Shift`). Plattform‑spezifische Tasten übernehmen etablierte Namen (`Windows`, `Command`, `Option`, `Super`/`Meta`). Dokumentiere abweichende Aliasse explizit.
- [ ] Windows‑Richtlinie: VK‑Namen ohne `VK_`‑Präfix (`ESCAPE`, `RETURN`, `F24`, `LCTRL`, `RMENU` …); AltGr als `Right Alt` (`RMENU`) statt separatem `Ctrl+Alt`. Extended‑Keys setzen `KEYEVENTF_EXTENDEDKEY`.
- [ ] Mappingentscheidungen dokumentieren (z. B. für Zeichenpfade via `VkKeyScanW`/Unicode, CapsLock‑Korrektur, Layoutabhängigkeit, L/R‑Modifier‑Injektion).
- [ ] CLI/Docs verlinken: `platynui-cli keyboard list` und Python `Runtime.keyboard_known_key_names()` geben die Namensliste programmatisch aus.
- [ ] Reservierte Symbol‑Aliasse: Für Shortcuts sind `+`, `<`, `>` und Whitespace reserviert. Anbieter sollten Aliasse bereitstellen (`PLUS`, `MINUS`, `LESS`/`LT`, `GREATER`/`GT`), die zu den entsprechenden Zeichen aufgelöst werden (layout‑korrekt tippen). Status: im Mock‑Keyboard implementiert; für Linux/macOS‑Provider einzuplanen.
- [ ] L/R‑Modifier‑Aliasse: Neben `Shift`/`Control`/`Alt`/`Windows` werden Links/Rechts‑Varianten und gängige Synonyme unterstützt, z. B. `LSHIFT`/`LEFTSHIFT`, `RSHIFT`/`RIGHTSHIFT`, `LCTRL`/`LEFTCTRL`/`LEFTCONTROL`, `RCTRL`/`RIGHTCTRL`/`RIGHTCONTROL`, `LALT`/`LEFTALT`, `RALT`/`RIGHTALT`/`ALTGR`, `LEFTWIN`/`RIGHTWIN`. Diese sollen auf die passenden platform‑spezifischen Codes/Usages auflösen.
- [ ] VK‑Sondergruppen (Windows): Falls vorhanden, sollten Provider auch regionale/IME‑spezifische und OEM‑Tasten unterstützen (z. B. `ABNT_C1/ABNT_C2`, `DBE_*`, `OEM_*` inkl. `OEM_102`). Namen werden ohne `VK_`‑Präfix exponiert. Unterschiedliche OS‑Versionen/Layouts können das Verhalten einschränken → Best‑Effort genügt, aber Mapping muss stabil sein.
- [ ] `known_key_names()`: Liste der unterstützten Tastennamen (Groß-/Kleinschreibung egal, keine Duplikate) veröffentlichen. Der Rückgabewert muss stabil sortierbar sein (CLI/Python zeigen die Liste an). Zeichen‑Eingaben (Einzelzeichen) dürfen zusätzlich von `key_to_code` akzeptiert werden, auch wenn sie nicht in `known_key_names()` enthalten sind.
- [ ] `Focusable`-Knoten liefern ein dynamisches `IsFocused`-Attribut (`UiValue::Bool`) und emittieren bei Fokuswechsel `ProviderEventKind::NodeUpdated` für den bisherigen sowie den neuen Fokus, damit Caches/Runtime den Zustand aktualisieren.
- [ ] Fehler von Runtime-Aktionen (`focus()`, `activate()`, …) werden als `PatternError` mit prägnanter Nachricht zurückgegeben (kein Panic / unwrap innerhalb der Provider-Schicht).
- [ ] Bereitgestellte Attribute stimmen mit den ClientPattern-Anforderungen aus `docs/patterns.md` überein (Bezeichner in PascalCase, Wertebereiche, optional vs. Pflichtfelder). Das Core-Testkit `platynui_core::ui::contract::testkit` prüft diese Zuordnung automatisiert – Provider sollten die erwarteten Konstanten (`platynui_core::ui::attribute_names`) wiederverwenden.
- [ ] Geometrie-Aliaswerte (`Bounds.X`, `Bounds.Width`, `ActivationPoint.X`, `ActivationPoint.Y` …), sofern geliefert, spiegeln die zugrunde liegenden `Rect`-/`Point`-Attribute wider. Das Contract-Testkit meldet Abweichungen; unterschiedliche Werte gelten als Fehler.
- [ ] Plattform-Crates, die Eingabegeräte exponieren, registrieren `PointerDevice`: `position()`/`move_to()` arbeiten mit Desktop-`f64`, `press()`/`release()` setzen Buttons stabil, `scroll()` liefert horizontale/vertikale Deltas. Double-Click-Metadaten (`double_click_time`, `double_click_size`) werden bereitgestellt, sofern die native API sie liefert; andernfalls dokumentiert der Provider das Fehlen (`None`).
- [ ] `UiTreeProvider::subscribe_events(listener)` implementiert den Event-Weg: Sobald die Runtime (oder ein anderer Host) einen Listener registriert, liefert der Provider zukünftige Baumereignisse über diesen Kanal.
- [ ] Optional, aber empfohlen: Beim Registrieren eines Listeners ein initiales `TreeInvalidated` senden, damit Konsumenten (Runtime, CLI `watch`) unmittelbar einen konsistenten Refresh auslösen können.
- [ ] `Role` entspricht dem normalisierten Namen (lokaler Name im Namespace `control` oder `item`), die native Rolle liegt zusätzlich unter `native:Role` (oder äquivalenten Feldern).
- [ ] Meldet ein Element das Pattern `ActivationTarget`, liefert es `ActivationPoint` (Desktop-Koordinaten, ggf. Fallback auf Rechteckzentrum) und optional `ActivationArea`.
- [ ] `Technology` ist für jede `UiNode` gesetzt (`UIAutomation`, `AT-SPI`, `AX`, `JSONRPC`, ...).
- [ ] Provider erzeugen keinen eigenen Desktop-Dokumentknoten; die Plattform-/Runtime-Schicht stellt Desktop-Metadaten (`Bounds`, `OsName`, `OsVersion`, `DisplayCount`, `Monitors`) bereit.
- [ ] Mapping-Entscheidungen gegen `docs/patterns.md` dokumentiert (insb. bei Mehrfachzuordnungen).
- [ ] Baum-Ereignisse (`NodeAdded`, `NodeUpdated`, `NodeRemoved`) getestet; sicherstellen, dass sie nur zur Synchronisation dienen und keine Pattern-spezifischen Nebenwirkungen haben.
- [ ] `UiNode::invalidate()` räumt alle gecachten Informationen der Node ab (Kind-Iteratoren, Attribute, Pattern-Instanzen). Nach einer Invalidierung muss der nächste Zugriff die Daten frisch aus der nativen API oder dem Provider-Cache laden.

## Highlight-Overlays
- [ ] Eigenständige Hilfsfenster des Providers (Highlight-Overlay, Inspectoren, Debug-Panels) tauchen nicht im UiTree auf; der Provider filtert alle Fenster des eigenen Prozesses konsequent heraus. Highlight-Rahmen laufen ausschließlich über das `HighlightProvider`-Trait.

## Windows (`platynui-provider-windows-uia`)
- [ ] `Bounds` basieren auf `IUIAutomationElement::CurrentBoundingRectangle` und sind in Desktop-Koordinaten umgerechnet.
- [ ] `ActivationPoint` nutzt `GetClickablePoint()`; fehlende Werte werden über das Elementzentrum ersetzt.
- [ ] `TextContent`, `TextEditable`, `TextSelection` nutzen Priority: `NameProperty` → `ValuePattern` → `TextPattern`.
- [ ] `WindowSurface`-Attribute (`IsMinimized`, `IsMaximized`, `IsTopmost`) spiegeln `WindowPattern`/`TransformPattern` wider.
- [ ] `Application`-Knoten liefern Prozessmetadaten (`ProcessId`, `ProcessName`, `ExecutablePath`) und optional `CommandLine`, `UserName`.
- [ ] `WindowSurface::accepts_user_input()` liefert – sofern die Plattform eine zuverlässige Abfrage erlaubt – den aktuellen Eingabestatus (Windows: `WaitForInputIdle`; andere Plattformen dokumentieren die verwendete Heuristik oder geben `None` zurück). Ein optionales Attribut `window:AcceptsUserInput` darf den Wert spiegeln.
- [ ] `Selectable`/`SelectionProvider` synchronisiert über `SelectionItemPattern`/`SelectionPattern`.
- [ ] Highlight/Overlay-Pfad (DirectComposition/GDI/XComposite/…) respektiert die gelieferten `Bounds`, unterstützt optionale Dauerangaben und garantiert, dass maximal ein Overlay aktiv ist: Neue Highlight-Anfragen verschieben den bestehenden Rahmen und setzen die Laufzeit zurück, statt zusätzliche Fenster zu erzeugen.

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
- [ ] Pointer-Mock protokolliert Bewegungen/Buttons/Scrolls deterministisch (`take_pointer_log`) und stellt reproduzierbare Double-Click-Werte bereit.
- [ ] Keyboard-Mock protokolliert Press/Release-Ereignisse (`take_keyboard_log`) und erlaubt das Zurücksetzen des Zustands (`reset_keyboard_state`).
- [ ] Provider-Mock bietet dynamische Textpuffer (`append_text`, `replace_text`, `apply_keyboard_events`), damit Tastatureingaben (inkl. Emojis/IME-Strings) in Tests nachvollzogen werden können.

## Automatisierte Prüfungen (Ideen)
- [ ] Integrationstest, der `docs/patterns.md` parst und Pflichtattribute pro Pattern mit Provider-Ausgabe vergleicht.
- [ ] CI-Job, der Koordinatenbereiche auf Plausibilität prüft (nicht negativ, innerhalb Monitorfläche).
- [ ] Linter, der verbotene Pattern/Attribut-Kombinationen meldet (z. B. `TextEditable` ohne `TextContent`).
