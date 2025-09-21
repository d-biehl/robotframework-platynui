# Rückblick auf die ursprüngliche robotframework-PlatynUI Implementierung

## Kontext und Zielsetzung
Die ursprüngliche .NET-basierte Version von PlatynUI sollte Robot Framework plattformübergreifend mit UI-Automatisierungsfunktionen versorgen. Für den Neustart des Projekts wollten wir aus dem vorhandenen Code Erkenntnisse übernehmen, ohne dessen Architektur 1:1 zu portieren. Diese Analyse fasst die wichtigsten Bausteine, Stärken und Schwächen des Legacy-Codes zusammen und leitet daraus Impulse für unsere aktuelle Rust-basierte Runtime ab. Das Dokument ist bewusst als lebende Ideensammlung angelegt und wird nach Bedarf ergänzt oder korrigiert.

## Überblick über die Legacy-Architektur

### Modularer Aufbau (MEF)
- `src/PlatynUI.Runtime/PlatynUiExtensions.cs`: Verwendet das Managed Extensibility Framework (MEF), lädt Assemblies anhand eines `PlatynUiExtension`-Attributes und filtert sie nach `RuntimePlatform`. Erweiterungen können zusätzlich über den Pfad `PLATYNUI_EXTENSIONS` eingebunden werden.
- `src/PlatynUI.Runtime/Desktop.cs`: Aggregiert die von MEF geladenen `INodeProvider`-Implementierungen zu einem Desktop-Root-Knoten und reichert ihn mit Plattform- und Displaydaten an.

**Mitnahme für das neue Konzept:** Unser Runtime-Lader sollte klar zwischen Kern-Traits (`core`-Crate) und optionalen Erweiterungen unterscheiden. Die Plattformfilterung pro Provider bleibt sinnvoll, nur die konkrete Mechanik (MEF → Rust Trait Objects + dynamische Registrierung) ändert sich.

### Knoten- und Attributmodell
- `src/PlatynUI.Runtime/Core/Node.cs`: Definiert `INode`, `IAttribute` und lazy geladene Attribute. Attribute haben einen Namespace (`Namespaces.cs`) und können on-demand aus dem Backing-Element abgefragt werden.
- `src/PlatynUI.Extension.Win32.UiAutomation/ElementNode.cs`: Implementiert `INode`, `IElement` und `IAdapter` gleichzeitig. Wichtige Punkte:
  - Standard-Namespace `http://platynui.io/raw` (entspricht unserem geplanten `ui`-Namespace).
  - `DefaultClickPosition` wird auf Basis des Bounding Rect berechnet.
  - `TryEnsureApplicationIsReady()` nutzt `WaitForInputIdle`, um Busy-Fenster zu vermeiden.
- `src/PlatynUI.Extension.Linux.AtSpi2/NodeProvider.cs`: Konvertiert AT-SPI-Rollen in PascalCase und stellt Zustände wie `Enabled`, `Visible`, `Editable` über Attribute bereit.

**Mitnahme:**
- Lazy Attribute passen zu unserem Pattern-Ansatz; sie erlauben späte Abfragen durch XPath.
- `RuntimeId`, `DefaultClickPoint`, `IsVisible`, `IsEnabled` etc. sind etablierte Pflichtattribute.
- Technologie-spezifische Rollen sollen in PascalCase geliefert werden – Legacy macht das bereits vor.

### Provider-Infrastruktur (Out-of-Process)
- `src/PlatynUI.Provider.Core/Types.cs`: Beschreibt `ElementReference` und die RPC-Verträge (`IApplicationInfoAsync`, `INodeInfoAsync`).
- `src/PlatynUI.Provider.Server/ProviderServerBase.cs`: JSON-RPC-Server über Named Pipes (StreamJsonRpc).
- `src/PlatynUI.Extension.Provider.Client/ProcessProvider.cs`: Client-Konnektor zum Provider-Prozess, rekonstruiert daraus wieder `INode`-Objekte.
- `src/PlatynUI.Provider.Core/PipeHelper.cs`: Pipe-Namensschema `Global\PlatynUI.Provider_{UserId}_{Pid}` – spiegelt das Konzept der namensgebenden Pipes wider.

**Mitnahme:**
- Die strikte Trennung zwischen Runtime und externen Providern validiert unseren `provider-*` Crate-Plan.
- Das Pipe-Schema lässt sich auf Unix Sockets / Named Pipes in Rust übertragen.
- Die Verträge liefern genau die Operationen, die wir für unser `provider-jsonrpc`-Crate benötigen.

### Plattformbausteine
- `src/PlatynUI.Platform.Win32/Highlighter.cs`, `KeyboardDevice.cs`, `MouseDevice.cs`, `DisplayDevice.cs`: Kapseln Ein-/Ausgabe und Highlighter-Overlay. Der Highlighter nutzte eigene Fensterrahmen, um UI-Elemente zu markieren.
- `src/PlatynUI.Extension.Win32.UiAutomation/Patterns.cs` & `Helper.cs`: Enthalten heuristiken zum Aktivieren/Maximieren von Fenstern, inklusive Modal-Handling und Owner-Ketten.
- `src/PlatynUI.Platform.X11` und `src/PlatynUI.Platform.MacOS`: Entsprechende Gegenstücke für Linux (X11) und macOS.

**Mitnahme:**
- Unsere `platform-*` Crates sollten ähnliche Dienste anbieten: Window Manager Trait, Highlighter, Screenshot, Devices.
- Die Aktivierungslogik liefert Blaupausen für unseren Window Manager (z. B. Umgang mit Modals, Foreground-Wechsel).

## Auffällige Schwachstellen
- **Zielgruppenfokus:** Der Legacy-Code richtet sich unmittelbar an Robot Framework. Wir entkoppeln stärker und stellen zuerst eine Runtime bereit.
- **API-Konsistenz:** Namensräume (`raw`, `app`, `native`) sind zahlreich; unser Konzept vereinfacht auf `ui`, `app`, `native` mit `ui` als Standard.
- **Fehlende Tests:** Kaum automatisierte Tests für Provider und Devices – das neue Projekt sollte Mocks und Integrationstests früh integrieren.
- **Aktualität:** .NET 8, MEF und StreamJsonRpc sind schwer nach Rust übertragbar; Konzepte müssen neu eingebettet werden.

## Übertragbare Konzepte für das neue Projekt
1. **Registrierungsmodell:** Plattform-/Provider-Pakete melden sich über ein Attribut bzw. Trait-Marker und werden zur Laufzeit injiziert.
2. **Nodes & Patterns:** Pflichtattribute (`RuntimeId`, `Bounds`, `IsVisible`, `IsEnabled`, `SupportedPatterns`) sowie optionale Pattern-spezifische Daten (z. B. Fensterzustand) dienen als Grundlage für `patterns.md`.
3. **Window Manager:** Aktivierungs- und Fokus-Heuristiken zeigen, wie plattformnahe APIs genutzt werden können. Diese Logik wandert in unser `WindowManager`-Trait.
4. **Devices & Highlighting:** Ein dedizierter Highlighter pro Plattform und Device-Abstraktionen (`Mouse`, `Keyboard`, `Display`) sind bereits funktional wertvoll.
5. **JSON-RPC-Bridge:** Provider sollen über Pipes/Sockets RPC anbieten, ohne dass die Runtime sie starten muss. Das Legacy-Vorgehen bestätigt dieses Modell.

## Ergänzungen für das aktuelle Konzept
- Document `RuntimeId`-Pflichten analog zu Windows/AT-SPI (Hex-Strings vs. D-Bus Pfade).
- `AcceptsUserInput`: Der Legacy-Code nutzt `WaitForInputIdle`; wir können das als Plattformstrategie adaptieren.
- `DefaultClickPoint` als eigenes Pattern, statt nur Attribut.
- Erweiterte Provider-Checkliste: Sichtbarkeit, Bounds, Pattern-Flags und Herkunft der IDs dokumentieren.

## Offene Fragen und nächste Schritte
- Wie lösen wir `AcceptsUserInput` unter Linux/macOS ohne 1:1-API-Pendants? (Heuristiken oder Provider-spezifische Flags?)
- Welche Teile der Highlighter-Logik können wir abstrahieren, damit spätere Frontends (CLI, GUI) wiederverwenden?
- Müssen wir für Pipes plattformabhängige Sicherheitsrichtlinien dokumentieren (z. B. Windows Global Namespace vs. Unix Domain Sockets)?

## Fazit
Der Legacy-Code bestätigt viele unserer Annahmen: plattformgetrennte Crates, lauffähige Provider per JSON-RPC, ein Desktop-Root mit PascalCase-Rollen und Pflichtattributen, Geräte-Wrapper sowie ein Window Manager als zentrale Abstraktion. Die neue Architektur kann diese Ideen überführen, sie aber schlanker und technologieagnostischer gestalten. Dieses Dokument bleibt in Bewegung – sobald wir neue Erkenntnisse gewinnen oder erste Rust-Prototypen entstehen, erweitern wir die Analyse entsprechend.
