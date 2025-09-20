# Provider Checklist (Draft)

> Status: Draft – use this list to verify provider compliance before merging changes. Extend or adjust as platform support evolves.

## Gemeinsame Prüfschritte
- [ ] Alle Koordinaten (`Bounds`, `ActivationPoint`, `ActivationArea`, Fensterrahmen) werden im Desktop-Koordinatensystem geliefert (linke obere Ecke des Primärmonitors = Ursprung, DPI-/Scaling berücksichtigt).
- [ ] `RuntimeId` bleibt stabil, solange das zugrunde liegende Element existiert; bei Neuaufbau ändert sich die ID nachvollziehbar.
- [ ] Quelle der `RuntimeId` dokumentiert (z. B. UIA `RuntimeId`, AT-SPI D-Bus-Objektpfad, macOS `AXUIElement` Identifier); bei fehlender nativer ID existiert ein deterministischer Fallback.
- [ ] `IsVisible` korrekt gesetzt (Accessibility-API meldet sichtbares Element); falls verfügbar, `IsOffscreen` konsistent mit Koordinaten/Viewports.
- [ ] `ui:SupportedPatterns` enthält nur Patterns, deren Pflichtattribute vollständig gesetzt sind; optionale Felder sind `null` oder fehlen.
- [ ] `Role` entspricht dem normalisierten Namen (lokaler Name im Namespace `ui`), die native Rolle liegt zusätzlich unter `native:Role` (oder äquivalenten Feldern).
- [ ] `Technology` ist für jede `UiNode` gesetzt (`UIAutomation`, `AT-SPI`, `AX`, `JSONRPC`, ...).
- [ ] Desktop-Knoten liefern `Bounds`, `OsName`, `OsVersion`, `DisplayCount`, `Monitors`.
- [ ] Mapping-Entscheidungen gegen `docs/patterns.md` dokumentiert (insb. bei Mehrfachzuordnungen).
- [ ] Baum-Ereignisse (`NodeAdded`, `NodeUpdated`, `NodeRemoved`) getestet; sicherstellen, dass sie nur zur Synchronisation dienen und keine Pattern-spezifischen Nebenwirkungen haben.

## Windows (`provider-windows-uia`)
- [ ] `Bounds` basieren auf `IUIAutomationElement::CurrentBoundingRectangle` und sind in Desktop-Koordinaten umgerechnet.
- [ ] `ActivationPoint` nutzt `GetClickablePoint()`; fehlende Werte werden über das Elementzentrum ersetzt.
- [ ] `TextContent`, `TextEditable`, `TextSelection` nutzen Priority: `NameProperty` → `ValuePattern` → `TextPattern`.
- [ ] `WindowSurface`-Attribute (`IsMinimized`, `IsMaximized`, `IsTopmost`) spiegeln `WindowPattern`/`TransformPattern` wider.
- [ ] `Application`-Knoten liefern Prozessmetadaten (`ProcessId`, `ProcessName`, `ExecutablePath`) und optional `CommandLine`, `UserName`.
- [ ] `AcceptsUserInput` verfügbar, sofern die Plattform eine zuverlässige Abfrage erlaubt (Windows: `WaitForInputIdle`; andere Plattformen dokumentieren die verwendete Heuristik oder lassen das Feld weg).
- [ ] `Selectable`/`SelectionProvider` synchronisiert über `SelectionItemPattern`/`SelectionPattern`.
- [ ] Highlight/Overlay Pfad (DirectComposition/GDI) liefert konsistente Ergebnisse im Vergleich zu `Bounds`.

## Linux X11 (`provider-atspi` + `platform-linux-x11`)
- [ ] Koordinaten stammen aus `Component::get_extents(ATSPI_COORD_TYPE_SCREEN)`.
- [ ] `TextContent`/`TextSelection` über `Text`-Interface, UTF-8 sauber gehandhabt.
- [ ] `SelectionProvider` nutzt `Selection`-Interface, `SelectionContainerId` basiert auf `Accessible::path` oder stabilem Handle.
- [ ] `WindowSurface`-Aktionen delegieren an X11 Window-Manager (EWMH) via `platform-linux-x11`.
- [ ] `ActivationTarget` berechnet optional eigene Center-Punkte, wenn `Component::get_offset_at_point` nicht verfügbar ist.

## macOS (`provider-macos-ax`)
- [ ] Koordinaten aus `AXFrame` in Core Graphics Desktop-Koordinaten (unter Berücksichtigung mehrerer Monitore) transformiert.
- [ ] `TextEditable` setzt `IsReadOnly` basierend auf `AXEditable`/`AXEnabled`.
- [ ] `Activatable` verwendet `AXPress`; `IsActivationEnabled` spiegelt `AXEnabled`.
- [ ] `WindowSurface`-Aktionen nutzen Accessibility-API + `CGWindow`/`NSWorkspace`-Hilfen.
- [ ] Monitorliste (Desktop) über `NSScreen` bzw. `CoreGraphics` zur Verfügung gestellt.

## JSON-RPC (`provider-jsonrpc`)
- [ ] Registry-Eintrag dokumentiert `Technology="JSONRPC"` und übergibt Provider-Metadaten (Version, Hostplattform).
- [ ] Schema validiert Pflichtattribute/Pattern-Kombinationen (Serverseitige Contracts oder JSON Schema).
- [ ] Heartbeat-/Reconnect-Logik im Runtime-Client getestet.
- [ ] Sicherheitsrichtlinien (Pipe/Soket-Namensschema, Berechtigungen) umgesetzt.

## Mock (`provider-mock` & `platform-mock`)
- [ ] Referenzdaten decken typische Pattern-Kombinationen ab (z. B. Button, Textfeld, Checkbox, Tree Item).
- [ ] Tests validieren Desktop-Koordinaten und `ActivationTarget`.
- [ ] Scripted-Mock erlaubt negative Szenarien (fehlende Pflichtattribute, ungültige Koordinaten) für Contract-Tests.

## Automatisierte Prüfungen (Ideen)
- [ ] Integrationstest, der `docs/patterns.md` parst und Pflichtattribute pro Pattern mit Provider-Ausgabe vergleicht.
- [ ] CI-Job, der Koordinatenbereiche auf Plausibilität prüft (nicht negativ, innerhalb Monitorfläche).
- [ ] Linter, der verbotene Pattern/Attribut-Kombinationen meldet (z. B. `TextEditable` ohne `TextContent`).
