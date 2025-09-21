# PlatynUI Pattern-Katalog (Entwurf)

> Status: Arbeitsentwurf – Inhalte sind noch nicht final und dienen als Diskussionsgrundlage.

## Einführung & Zielbild

PlatynUI modelliert Fähigkeiten von UI-Knoten mit Patterns. Diese Patterns verhalten sich wie Traits: Sie werden einer `UiNode` zugewiesen, um konkrete Fähigkeiten zu beschreiben, ohne Hierarchien aufzubauen. Controls entstehen durch Kombination mehrerer Patterns. Dieses Dokument schlägt einen kompakten Satz wiederverwendbarer Capability-Patterns vor, die quer über Buttons, Textfelder, Menüs, Baumknoten oder Tabellenzellen eingesetzt werden können.

## Grundprinzipien
- **Komposition statt Vererbung:** Eine `UiNode` kann beliebig viele Patterns deklarieren. Abhängigkeiten werden explizit referenziert, nicht impliziert.
- **Standard-Namespace:** XPath-Ausdrücke nutzen standardmäßig den `control`-Namespace. Weitere Namensräume (`item`, `app`, `native`) ergänzen wir nur bei Bedarf.
- **Item-Namespace:** Elemente, die Teil eines Control-Containers sind (z. B. `ListItem`, `TreeItem`, `MenuItem`), verwenden den Namespace `item`. So lassen sich Einträge großer Sammlungen gezielt filtern.
- **Attribute in PascalCase:** Alle Attribute folgen derselben Konvention (z. B. `Bounds`, `IsSelected`, `Text`).
- **Lesende Fähigkeiten:** Patterns beschreiben ausschließlich Zustände und zusätzliche Attribute. Aktionen liegen in der Verantwortung der Clients, mit Ausnahme von Fokuswechsel und Fenstersteuerung, die die Runtime direkt anbietet.
- **Keine Events:** Statusänderungen spiegeln sich in Attributen wider und können durch erneute XPath-Abfragen ermittelt werden. Baum-Events existieren nur für die Synchronisation zwischen Runtime und Provider und sind kein Bestandteil einzelner Patterns.
- **Erweiterbarkeit:** Neue Patterns lassen sich hinzufügen, ohne bestehende Abfragen zu brechen. Provider melden jedes unterstützte Pattern in `SupportedPatterns`.

## UiNode-Kategorien & Basisvertrag
Wir unterscheiden drei Typen von UiNode-Namespace-Knoten:

1. `control:` – Steuerelemente (Fenster, Buttons, Textfelder …).
2. `item:` – Inhalte innerhalb von Containern (ListItem, TreeItem, TabItem).
3. `app:` – Anwendungsknoten (`app:Application`) mit dem Application-Pattern.

### Gemeinsamer Vertrag für `control` & `item`
### Desktop-Knoten (`control:Desktop`)
- Entspricht einem regulären `control`-Element, bezieht seine Daten jedoch aus dem Plattform-Trait `DesktopProvider` (Auflösung, Monitore, Primäranzeige).
- `Bounds` umfasst den gesamten Desktop; `DisplayCount` und `Monitors` spiegeln die vom Trait gelieferten Informationen wider.

- `Bounds` – Desktop-referenzierte Rechtecke; Aliaswerte `Bounds.X`, `Bounds.Y`, `Bounds.Width`, `Bounds.Height` werden automatisch ergänzt.
- `Role` – Normalisierte PascalCase-Rolle, entspricht dem lokalen XPath-Elementnamen. Die native Rolle bleibt unter `native:Role` erhalten.
- `Name` – Anzeigename für Benutzer.
- `IsVisible` – Sichtbarkeit laut Backend.
- `IsOffscreen` – Optionales Flag, falls das Element außerhalb des sichtbaren Bereichs liegt.
- `RuntimeId` – Technologie-spezifische Laufzeit-ID (oder stabiler Fallback).
- `Technology` – Quelle (`UIAutomation`, `AT-SPI`, `AX`, `JSONRPC`, …).
- `SupportedPatterns` – Liste der aktivierten Pattern in PascalCase (wird über `UiNode::supported_patterns()` gemeldet).
- Attribute werden über das `UiAttribute`-Trait bereitgestellt (`name`, `namespace`, `value -> UiValue`).

### Spezifika für `app:`-Knoten
- Müssen das `Application`-Pattern implementieren und stellen `app:*`-Attribute bereit (Prozessmetadaten, `AcceptsUserInput`, etc.).
- `Bounds`, `Name`, `RuntimeId`, `Technology` gelten analog, wobei `Bounds` typischerweise den Desktop abbildet.

## Capability-Patterns (Draft)
Die folgenden Patterns bilden wiederkehrende Fähigkeiten ab. Beispiel-Mappings auf UIA, AT-SPI oder AX dienen nur als Orientierung.

### Textbezogene Fähigkeiten

#### TextContent
- **Beschreibung:** Stellt dar, dass ein Knoten sichtbaren oder zugänglichen Text transportiert.
- **Pflichtattribute:** `Text` (String).
- **Optionale Attribute:** `Locale`, `IsTruncated`.
- **Verwendung:** Statischer Text, Buttons, Menüeinträge, Tabellenzellen, TreeView-Items.

#### TextEditable
- **Beschreibung:** Ergänzt `TextContent` um Informationen zur Bearbeitbarkeit.
- **Pflichtattribute:** `IsReadOnly` (bool).
- **Optionale Attribute:** `MaxLength`, `SupportsPasswordMode`.
- **Abhängigkeit:** erfordert `TextContent`.

#### TextSelection
- **Beschreibung:** Bietet Zugriff auf Cursor- und Selektionsinformationen.
- **Pflichtattribute:** `CaretPosition`, `SelectionRanges` (Liste von Offsets).
- **Optionale Attribute:** `SelectionAnchor`, `SelectionActive`.
- **Abhängigkeiten:** Erwartet `TextContent`.

### Fokus & Aktivierung

#### Focusable
- **Beschreibung:** Element kann den Eingabefokus aufnehmen.
- **Pflichtattribute:** `IsFocused`.
- **Runtime-Aktion:** `focus()`.

#### Activatable
- **Beschreibung:** Element unterstützt einen primären Aktivierungsbefehl. Die Runtime stellt keine direkte Aktion bereit; Clients lösen die Aktivierung z. B. per Tastatur/Maus aus.
- **Pflichtattribute:** `IsActivationEnabled`.
- **Optionale Attribute:** `DefaultAccelerator`.
- **Verwendung:** Buttons, Menüeinträge, Hyperlinks, Tree-Items mit Default-Aktion.

#### ActivationTarget
- **Beschreibung:** Liefert eine standardisierte Zeiger- bzw. Klickposition innerhalb der Elementgrenzen, damit Clients Interaktionen zuverlässig auf die aktive Fläche richten können.
- **Pflichtattribute:** `ActivationPoint` (absoluter Koordinatenwert im Desktop-Bezugssystem, innerhalb der globalen `Bounds` des Elements). Komponenten stehen zusätzlich als `ActivationPoint.X`/`ActivationPoint.Y` zur Verfügung.
- **Optionale Attribute:** `ActivationArea` (absolutes Rechteck im Desktop-Koordinatensystem für erweiterte Zielzonen), `ActivationHint` (Kurzbeschreibung des empfohlenen Ziels).
- **Verwendung:** Buttons, Checkboxen, Radiobuttons, Listeneinträge, Tree-Items oder andere Steuerelemente mit klar definierter Interaktionsfläche.

### Auswahl & Zustand

#### Selectable
- **Beschreibung:** Element kann ausgewählt / deselektiert werden.
- **Pflichtattribute:** `IsSelected`, `SelectionContainerId`.
- **Optionale Attribute:** `SelectionOrder`.

#### SelectionProvider
- **Beschreibung:** Knoten verwaltet auswählbare Kind-Elemente (Listen, Tabellen, Trees).
- **Pflichtattribute:** `SelectionMode` (`None`, `Single`, `Multiple`), `SelectedIds` (Liste).
- **Optionale Attribute:** `SupportsRangeSelection`.
- **Abhängigkeiten:** Erwartet Kinder mit `Selectable`.

#### Toggleable
- **Beschreibung:** Element unterstützt einen diskreten Zustand (z. B. Checkbox).
- **Pflichtattribute:** `ToggleState` (`On`, `Off`, `Indeterminate`).
- **Optionale Attribute:** `SupportsThreeState`.

#### StatefulValue
- **Beschreibung:** Liefert numerische oder geordnete Werte.
- **Pflichtattribute:** `CurrentValue`, `Minimum`, `Maximum`.
- **Optionale Attribute:** `SmallChange`, `LargeChange`, `Unit`.
- **Verwendung:** Slider, ProgressBars (lesend), Spinner.

### Struktur & Navigation

#### Expandable
- **Beschreibung:** Knoten kann eine untergeordnete Struktur ein- oder ausblenden.
- **Pflichtattribute:** `IsExpanded`.
- **Optionale Attribute:** `HasChildren`.
- **Verwendung:** Tree-Items, Menüs, Disclosure-Widgets.

#### Scrollable
- **Beschreibung:** Container kann Inhalt scrollen.
- **Pflichtattribute:** `CanScrollHorizontally`, `CanScrollVertically`, `HorizontalPercent`, `VerticalPercent`, `HorizontalViewSize`, `VerticalViewSize`.
- **Optionale Attribute:** `ScrollGranularity`.

#### ItemContainer
- **Beschreibung:** Stellt indirekten Zugriff auf Kinder durch Index oder Schlüssel bereit.
- **Pflichtattribute:** `ItemCount` (optional falls unbekannt), `IsVirtualized` (bool).
- **Optionale Attribute:** `SupportsContainerSearch`.
- **Verwendung:** Tabellen, Listen, virtuelle Kataloge.

### Fenster & Oberflächen

#### WindowSurface
- **Beschreibung:** Bindeglied zum platform-spezifischen Window Manager.
- **Pflichtattribute:** `IsMinimized`, `IsMaximized`, `IsTopmost`.
- **Runtime-Aktionen:** `activate()`, `minimize()`, `maximize()`, `restore()`, `move(bounds)`, `resize(bounds)`, `close()`.

#### DialogSurface
- **Beschreibung:** Spezialisierung für modale Dialoge.
- **Pflichtattribute:** `IsModal`.
- **Optionale Attribute:** `DefaultResult`.
- **Abhängigkeiten:** Erwartet `WindowSurface`.

### Applikationen & Prozesse

#### Application
- **Beschreibung:** Repräsentiert eine ausführende Anwendung oder einen Prozesskontext, aus dem Fenster und UI-Elemente stammen.
- **Pflichtattribute:** `ProcessId`, `ProcessName`, `ExecutablePath`.
- **Optionale Attribute:** `CommandLine`, `UserName`, `StartTime`, `MainWindowIds` (Liste von `RuntimeId`s der führenden Fenster), `Architecture` (z. B. `x86_64`), `AcceptsUserInput` (bool; gibt an, ob die Anwendung aktuell Eingaben annimmt – unter Windows via `WaitForInputIdle`, auf anderen Plattformen bestmögliche Heuristik).
- **Hinweis:** Application-Knoten sind Einstiegspunkte für XPath-Abfragen über den `app`-Namespace; sie bündeln Metadaten, ersetzen aber keine Prozessverwaltung.

### Visualisierung & Annotation

#### Highlightable
- **Beschreibung:** Element kann visuell hervorgehoben werden.
- **Pflichtattribute:** `SupportsHighlight` (bool).
- **Optionale Attribute:** `HighlightStyles` (Liste vordefinierter Stile).
- **Hinweis:** Die Runtime stellt eine eigene Highlight-Funktion bereit; kein Pattern-spezifisches Aktions-API erforderlich.

#### Annotatable
- **Beschreibung:** Element kann Zusatzinformationen tragen (Fehler, Status, Hinweis).
- **Pflichtattribute:** `Annotations` (Liste strukturierter Datensätze).

### Geräteinteraktion (Idee)
Diese Patterns sind Diskussionsstoff, da sie eng mit Device-Providern verknüpft sind und `ActivationTarget` ergänzen könnten:
- **PointerTarget:** Liefert detaillierte Hit-Test-Informationen und ggf. dynamische Koordinaten für Zeigegeräte.
- **KeyboardTarget:** Kennzeichnet Elemente, bei denen Tastatursimulation ankommen soll.
- **GestureTarget:** Reserviert für zukünftige Touch-/Gesten-Interfaces.

## Pattern-Mapping (Arbeitsstand)

| Pattern | UI Automation (Windows) | AT-SPI2 (Linux) | macOS AX | Beispielwerte | Hinweise |
| --- | --- | --- | --- | --- | --- |
| `Application` | `IUIAutomationElement` mit `ControlType=UIA_WindowControlTypeId` (Application Root), Prozessinfos über Win32 APIs | `Accessible` über `org.a11y.atspi.Application` Interface | `AXApplication`, NSRunningApplication | `ProcessId=1234`, `AcceptsUserInput=true` | Prozessmetadaten stammen aus Plattform-API; `AcceptsUserInput` unter Windows via `WaitForInputIdle`, andernorts best effort. |
| `TextContent` | `NameProperty`, ggf. `ValuePattern.Value` | `Accessible::name`, `Text::get_text` | `AXValue`, `AXDescription` | `Text="Datei"` | Provider wählen die aussagekräftigste Quelle (Priorität: Name → Value). |
| `TextEditable` | `ValuePattern.IsReadOnly`, `TextPattern` | `EditableText` Interface | `AXEditable`, `AXValue` | `IsReadOnly=false`, `MaxLength=256` | `IsReadOnly = true`, wenn API keine Bearbeitung erlaubt. |
| `TextSelection` | `TextPattern::GetSelection`, `CaretRangeEndpoint` | `Text::get_selection`, `Text::get_caret_offset` | `AXSelectedText`, `AXSelectedTextRange` | `CaretPosition=5`, `SelectionRanges=[(2,4)]` | Leerlisten signalisieren fehlende Selektion. |
| `Selectable` | `SelectionItemPattern.IsSelected`, `SelectionItemPattern.SelectionContainer` | `Selection::is_selected`, `Selection::select_child` | `AXSelected`, `AXParent` | `IsSelected=true`, `SelectionContainerId="list-42"` | `SelectionContainerId` verweist auf übergeordnetes Element. |
| `SelectionProvider` | `SelectionPattern` (`CanSelectMultiple`, `GetSelection`) | `Selection` Interface | `AXChildren`, `AXSelectedChildren` | `SelectionMode="Multiple"`, `SelectedIds=["item-1"]` | Container liefern `SelectedIds` über `RuntimeId` ihrer Kinder. |
| `Toggleable` | `TogglePattern.ToggleState` | `StateSet` (`STATE_CHECKED`, `STATE_INDETERMINATE`) | `AXValue` (Boolean), `AXSelected` | `ToggleState="On"` | Dreistufige Checkboxen melden `SupportsThreeState = true`. |
| `StatefulValue` | `RangeValuePattern` (`Value`, `Minimum`, `Maximum`) | `Value` Interface | `AXValue`, `AXMinValue`, `AXMaxValue` | `CurrentValue=42`, `Minimum=0`, `Maximum=100` | `SmallChange`/`LargeChange` falls API Stufen anbietet. |
| `Focusable` | `HasKeyboardFocus`, `SetFocus()` | `Component::grab_focus`, `StateSet` (`STATE_FOCUSED`) | `AXFocused`, `AXSetFocus` | `IsFocused=false` | Runtime nutzt native Fokusfunktionen. |
| `Activatable` | `InvokePattern.Invoke`, `LegacyIAccessible::DoDefaultAction` | `Action::do_action(0)` | `AXPress` | `IsActivationEnabled=true` | Client ahmt Aktivierung per Tastatur/Maus nach; `IsActivationEnabled` spiegelt `IsEnabled`. |
| `ActivationTarget` | `IUIAutomationElement::GetClickablePoint` | `Component::get_extents`, `Component::get_offset_at_point` | `AXPosition`, `AXFrame` | `ActivationPoint={"x":840,"y":420}` | Provider berechnen Desktop-Koordinaten, ggf. Fallback auf Mittelpunkt. |
| `WindowSurface` | `WindowPattern`, `TransformPattern` | Window-Management API (`org.freedesktop.DBus` + X11/Wayland) | `AXWindow`, Core Graphics | `IsMinimized=false`, `IsTopmost=false` | Aktionen laufen über Plattform-Window-Manager. |
| `Highlightable` | Provider-Overlay, `TransformPattern.GetRuntimeId` | Provider-Overlay via XComposite/Wayland Layer | `AXFrame`, transparentes `NSWindow` | `SupportsHighlight=true` | Runtime zeichnet Highlight, benötigt gültige `Bounds`. |
| `Scrollable` | `ScrollPattern` (`HorizontalPercent`, `VerticalPercent`) | `Component::scroll_to_point`, `Value` | `AXHorizontalScrollBar`, `AXVerticalScrollBar` | `VerticalPercent=55.0`, `CanScrollVertically=true` | Provider melden ViewSize und Scrollbarkeit getrennt. |
| `Expandable` | `ExpandCollapsePattern` | `Action::do_action("expand")` | `AXExpanded`, `AXPress` | `IsExpanded=false`, `HasChildren=true` | Provider geben nur dann `HasChildren=true` an, wenn API dies bestätigen kann. |
| `ItemContainer` | `ItemContainerPattern` (WinUI/Custom) | `Table`, `Collection` Interfaces | `AXChildrenInNavigationOrder` | `ItemCount=500`, `IsVirtualized=true` | Bei virtuellen Listen optional Paging-Attribute ergänzen. |

> Diese Tabelle dient als Startpunkt. Bei Abweichungen oder zusätzlichen Quellen sollte der jeweilige Provider die Entscheidung dokumentieren.

## Zusammenspiel: Beispielkompositionen
- **Applikation:** `Application`.
- **Textfeld:** `TextContent` + `TextEditable` + `TextSelection` + `Focusable`.
- **Statischer Text:** `TextContent`.
- **Button:** `TextContent` + `Activatable` + `ActivationTarget` + optional `Focusable`.
- **Checkbox:** `TextContent` + `Toggleable` + `Selectable` + `ActivationTarget` + optional `Focusable`.
- **List Item:** `TextContent` + `Selectable` + optional `Activatable` + optional `ActivationTarget`.
- **Tree Item:** `TextContent` + `Expandable` + `Selectable` + optional `Activatable` + optional `ActivationTarget`.
- **Tabellenzelle:** `TextContent` + optional `Selectable` + optional `StatefulValue`.
- **Fenster:** `WindowSurface` + optional `Focusable` + `Highlightable`.

Diese Beispiele zeigen, dass Kontrollelemente keine eigenen Patterns brauchen: ihre Fähigkeiten ergeben sich aus der Kombination der entsprechenden Traits.

## Offene Fragen
- Welche Patterns sind für den ersten MVP notwendig, welche können nachgelagert folgen?
- Wie präzise möchten wir Zustandsattribute modellieren (z. B. `SelectionRanges` vs. zusammengefasste Felder)?
- Sollen Patterns Versionen besitzen, damit wir spätere Erweiterungen deklarieren können?
- Wie strikt validieren wir Abhängigkeiten (z. B. `TextEditable` ohne `TextContent` verbieten)?
- Wie stellen wir sicher, dass Provider alle Koordinaten (`Bounds`, `ActivationPoint`, `ActivationArea`) konsistent im Desktop-Koordinatensystem liefern (z. B. DPI-Skalierung, Multi-Monitor)?

## Nächste Schritte
- Feedback einholen, Patterns finalisieren und Prioritäten festlegen.
- Mapping-Tabellen zwischen Patterns und UIA/AT-SPI/AX erstellen.
- Contract-Tests definieren, die Provider gegen diese Pattern-Spezifikation ausführen müssen.
- Nach der Abstimmung Version 1.0 des Pattern-Katalogs festschreiben und im Architekturkonzept verlinken.
