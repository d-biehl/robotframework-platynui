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
- **SupportedPatterns-Leitplanken:** Der Eintrag eines Patterns setzt voraus, dass alle Pflichtattribute des jeweiligen ClientPatterns bereitstehen und – falls es sich um ein RuntimePattern handelt – `UiNode::pattern::<T>()` ein Objekt liefert. Optional markierte Attribute dürfen fehlen oder `null` sein; andernfalls darf das Pattern nicht aufgeführt werden.
- **Runtime vs. Client:** Nur Patterns mit Runtime-Aktionen (`Focusable`, `WindowSurface`, `Application`) implementieren das Trait `UiPattern`. Provider liefern sie über `UiNode::pattern::<T>()` aus; deren Methoden geben immer `Result<_, PatternError>` zurück. Alle anderen Patterns agieren als ClientPattern – sie bleiben reine Attributbeschreibungen, deren Interpretation der Client übernimmt.
- **Registry-Unterstützung:** `PatternRegistry` im Core erleichtert die Verwaltung (`PatternId` → `Arc<dyn UiPattern>`) und sorgt dafür, dass `supported_patterns()` und `pattern::<T>()` dieselbe Datenbasis nutzen.

## UiNode-Kategorien & Basisvertrag
Wir unterscheiden drei Typen von UiNode-Namespace-Knoten:

1. `control:` – Steuerelemente (Fenster, Buttons, Textfelder …).
2. `item:` – Inhalte innerhalb von Containern (ListItem, TreeItem, TabItem).
3. `app:` – Anwendungsknoten (`app:Application`) mit dem Application-Pattern.

### Gemeinsamer Vertrag für `control` & `item`
- Jeder Knoten stellt die Basisattribute des `UiNode`-Traits bereit: `Role` (normalisiert, PascalCase), `RuntimeId`, `Technology`, `SupportedPatterns` sowie der sichtbare `Name`. Diese Werte werden als reguläre Attribute im jeweiligen Namespace (Standard: `control`) geliefert und stehen damit auch XPath zur Verfügung.
- Provider müssen `Role` so wählen, dass `local-name()` ohne weiteres Mapping genutzt werden kann (`UIA_Button` → `control:Button`, `ATSPI_ROLE_PUSH_BUTTON` → `control:Button`). Die originale Rollenbezeichnung erscheint zusätzlich in `native:Role`.
- `SupportedPatterns` listet ausschließlich Patterns auf, für die alle Pflichtattribute verfügbar sind **und** für die `UiNode::pattern::<T>()` (bei RuntimePatterns) ein Objekt zurückliefert. Optional beschriebene Felder dürfen fehlen oder `null` sein – erscheinen jedoch Pflichtfelder nicht, muss das Pattern aus der Liste entfernt werden. Damit bleibt die Pattern-Liste konsistent.
- `RuntimeId` bleibt während der gesamten Lebensdauer eines Elements stabil; wird ein Element zerstört und neu erzeugt, darf sich die ID ändern.
- `Technology` kennzeichnet die Quelle (`UIAutomation`, `AT-SPI`, `AX`, `JSONRPC`, …) und hilft bei Debugging sowie gemischten Provider-Szenarien.
- `Name` liefert den zugänglichen Anzeigenamen. Falls die Plattform keinen Namen anbietet, entscheidet der Provider über einen sinnvollen Fallback (z. B. Beschriftung aus Unterelementen) und dokumentiert das Verhalten.
> **Hinweis:** Alias-Attribute (`Bounds.X`, `ActivationPoint.X`, …) müssen immer mit den zugrunde liegenden `Rect`-/`Point`-Werten übereinstimmen. Das Contract-Testkit meldet fehlende oder inkonsistente Alias-Einträge.

### Spezifika für `app:`-Knoten
- `app:`-Knoten repräsentieren Anwendungen oder Prozesse. Sie erfüllen zusätzlich das ClientPattern `Application` (siehe unten) und stellen ihre Metadaten ausschließlich über `app:*`-Attribute bereit.

## Pattern-Übersicht

Wir unterscheiden zwei Kategorien:

1. **ClientPatterns (Attributverträge)** – beschreiben, welche zusätzlichen Attribute Elemente bereitstellen sollten. Die Runtime liefert lediglich die Attribute; ob ein Element ein bestimmtes ClientPattern erfüllt, entscheiden Konsumenten (z. B. XPath-/Clientlogik) selbst.
2. **RuntimePatterns (Aktionen)** – werden direkt in der Runtime umgesetzt und bieten explizite Methoden für Laufzeitaktionen (`Result<_, PatternError>`). Nur diese Patterns besitzen ein `UiPattern`-Trait.

Die folgenden Abschnitte listen beide Kategorien auf. Beispiel-Mappings auf UIA, AT-SPI oder AX dienen als Orientierung.

## ClientPatterns – Attributverträge

### Attributübersicht
Die folgende Tabelle fasst die aktuell vorgesehenen Attributnamen zusammen und ordnet sie den jeweiligen ClientPatterns zu. Sie dient als Nachschlagewerk bei Provider-Implementierungen, damit dieselben Bezeichner plattformübergreifend verwendet werden. Zusätzliche technologie-spezifische Felder gehören in den `native`-Namespace und sollten in Provider-Dokumentation begründet werden.

| Pattern | Pflichtattribute | Optionale Attribute |
| --- | --- | --- |
| (Grundvertrag `control`/`item`) | `Role`, `Name`, `RuntimeId`, `Technology`, `SupportedPatterns` | – |
| Element | `Bounds`, `IsVisible`, `IsEnabled` | `IsOffscreen` |
| Desktop | `Bounds`, `DisplayCount`, `Monitors`, `OsName`, `OsVersion` | – |
| TextContent | `Text` | `Locale`, `IsTruncated` |
| TextEditable | `IsReadOnly` | `MaxLength`, `SupportsPasswordMode` |
| TextSelection | `CaretPosition`, `SelectionRanges` | `SelectionAnchor`, `SelectionActive` |
| Selectable | `IsSelected` | `SelectionContainerId` |
| SelectionProvider | `SelectionMode`, `SelectedIds` | – |
| Toggleable | `ToggleState` | `SupportsThreeState` |
| StatefulValue | `CurrentValue`, `Minimum`, `Maximum` | `SmallChange`, `LargeChange` |
| Activatable | `IsActivationEnabled` | – |
| ActivationTarget | `ActivationPoint` | `ActivationArea` |
| Focusable | `IsFocused` | – |
| Scrollable | `HorizontalPercent`, `VerticalPercent` | `CanScrollHorizontally`, `CanScrollVertically`, `HorizontalViewSize`, `VerticalViewSize` |
| Expandable | `IsExpanded`, `HasChildren` | – |
| ItemContainer | `ItemCount`, `IsVirtualized` | `VirtualizationHint` |
| WindowSurface (Leseteil) | `IsMinimized`, `IsMaximized`, `IsTopmost` | `SupportsResize`, `SupportsMove`, `AcceptsUserInput` |
| DialogSurface | `IsModal` | `DefaultResult` |
| Application | `ProcessId`, `ProcessName`/`Name`, `ExecutablePath` | `CommandLine`, `UserName`, `StartTime`, `MainWindowIds`, `Architecture` |
| Highlightable | `SupportsHighlight` | `HighlightStyles` |
| Annotatable | `Annotations` | – |

> Hinweis: Diese Aufstellung ergänzt die textuellen Beschreibungen weiter unten und soll beim Implementieren als Referenz dienen. Die gleichen Konstanten stehen im Code unter `platynui_core::ui::attribute_names::<pattern>::*` bereit, sodass Provider-Implementierungen die Benennung direkt wiederverwenden können. Wenn ein Pattern neue Attribute benötigt oder Technologien zusätzliche Felder erfordern, bitte Tabelle **und** Attribut-Konstanten erweitern.

### Elementbasis (ClientPattern)

#### Element
- **Beschreibung:** Grundlegender Vertrag für sichtbare UI-Elemente (gilt für `control:`- und `item:`-Knoten).
- **Pflichtattribute:**
-  - `Bounds` – Desktop-Koordinaten als `Rect` (JSON-String); Aliaswerte `Bounds.X`, `Bounds.Y`, `Bounds.Width`, `Bounds.Height` werden automatisch als `xs:double` erzeugt.
-  - `Name` – `xs:string`
-  - `IsVisible` – `xs:boolean`
-  - `IsEnabled` – `xs:boolean` (Element nimmt Eingaben an)
- **Optionale Attribute:** `IsOffscreen` (`xs:boolean`), technologie-spezifische Ergänzungen unter `native:*`.
- **Hinweis:** Attribute wie `Role`, `Technology`, `RuntimeId`, `SupportedPatterns` gelten für alle `UiNode`s (siehe Grundvertrag) und werden hier nicht erneut aufgeführt. Provider müssen die Element-Pflichtfelder für alle `control:`-/`item:`-Knoten konsistent bereitstellen; Clients entscheiden anhand dieser Werte, welche weiteren ClientPatterns zutreffen.
- **Contract-Test:** Das Core-Testkit (`platynui_core::ui::contract::testkit`) vergleicht optionale Alias-Werte (`Bounds.X`, `Bounds.Y`, `Bounds.Width`, `Bounds.Height`), sofern vorhanden, mit dem gelieferten `Bounds`-Rechteck.

#### Desktop
- **Beschreibung:** Beschreibt den Dokumentknoten des UI-Baums. Der Desktop wird als `document-node()` im `control`-Namespace exponiert. XPath-Abfragen beginnen somit mit `.` bzw. `document-node()`, während `/*` die obersten UI-Kinder (z. B. Anwendungen) liefert. Der Desktop gilt nicht als reguläres UI-Element, sondern stellt System- und Monitorinformationen bereit.
- **Pflichtattribute:**
  - `Bounds` – Desktop-Koordinaten als `Rect` (JSON-String, Aliaswerte wie oben `xs:double`)
  - `DisplayCount` – `xs:integer`
  - `Monitors` – JSON-Array aus Objekten (`Name`, `Bounds` als `Rect`)
  - `OsName` – `xs:string`
  - `OsVersion` – `xs:string`
- **Hinweis:** Weitere Basisattribute entstammen dem allgemeinen UiNode-Vertrag; Ableitungen wie `Bounds.X`/`Bounds.Width` werden automatisch erzeugt. Die Attribute sind über den Dokumentkontext abrufbar (z. B. `./@Bounds.X`).

### Textbezogene Fähigkeiten (ClientPatterns)

#### TextContent
- **Beschreibung:** Stellt dar, dass ein Knoten sichtbaren oder zugänglichen Text transportiert.
- **Pflichtattribute:** `Text` (`xs:string`).
- **Optionale Attribute:** `Locale`, `IsTruncated`.
- **Verwendung:** Statischer Text, Buttons, Menüeinträge, Tabellenzellen, TreeView-Items.

#### TextEditable
- **Beschreibung:** Ergänzt `TextContent` um Informationen zur Bearbeitbarkeit.
- **Pflichtattribute:** `IsReadOnly` (`xs:boolean`).
- **Optionale Attribute:** `MaxLength`, `SupportsPasswordMode`.
- **Abhängigkeit:** erfordert `TextContent`.

#### TextSelection
- **Beschreibung:** Bietet Zugriff auf Cursor- und Selektionsinformationen.
- **Pflichtattribute:**
  - `CaretPosition` – `xs:integer`
  - `SelectionRanges` – JSON-Array (`[{"Start": int, "End": int}]`) zur Beschreibung mehrerer Bereiche.
- **Optionale Attribute:** `SelectionAnchor`, `SelectionActive`.
- **Abhängigkeiten:** Erwartet `TextContent`.

### Fokus & Aktivierung (ClientPatterns)

#### Focusable
- **Beschreibung:** Element kann den Eingabefokus aufnehmen.
- **Pflichtattribute:** `IsFocused` (`xs:boolean`). Der Wert muss den aktuellen Fokusstatus widerspiegeln und bei Änderungen sofort angepasst werden (z. B. durch Lazy-Attribute oder Cache-Invalidierung).
- **Runtime-Hinweis:** Die tatsächliche Aktion (`focus()`) stellt das RuntimePattern `Focusable` bereit (siehe Abschnitt „RuntimePatterns“). Provider rufen native Fokusmechanismen auf und lösen anschließend `ProviderEventKind::NodeUpdated` (für altes und neues Fokusziel) aus, damit nachfolgende XPath-Abfragen und Clients den aktualisierten Zustand erkennen.

#### Activatable
- **Beschreibung:** Element unterstützt einen primären Aktivierungsbefehl. Die Runtime stellt keine direkte Aktion bereit; Clients lösen die Aktivierung z. B. per Tastatur/Maus aus.
- **Pflichtattribute:** `IsActivationEnabled` (`xs:boolean`).
- **Optionale Attribute:** `DefaultAccelerator` (`xs:string`).
- **Verwendung:** Buttons, Menüeinträge, Hyperlinks, Tree-Items mit Default-Aktion.

#### ActivationTarget
- **Beschreibung:** Liefert eine standardisierte Zeiger- bzw. Klickposition innerhalb der Elementgrenzen, damit Clients Interaktionen zuverlässig auf die aktive Fläche richten können.
- **Pflichtattribute:** `ActivationPoint` – `Point` (JSON-String, Desktop-Koordinaten); Aliaswerte `ActivationPoint.X`/`ActivationPoint.Y` werden als `xs:double` erzeugt.
- **Optionale Attribute:** `ActivationArea` (`Rect` als JSON-String im Desktop-Bezugssystem), `ActivationHint` (`xs:string`).
- **Verwendung:** Buttons, Checkboxen, Radiobuttons, Listeneinträge, Tree-Items oder andere Steuerelemente mit klar definierter Interaktionsfläche.
- **Contract-Test:** Alias-Werte (`ActivationPoint.X`, `ActivationPoint.Y`), sofern gesetzt, müssen rechnerisch zum `ActivationPoint` passen; das Core-Testkit meldet Abweichungen.

### Mapping-Hinweise (informativ)

Die folgende Tabelle ordnet ausgewählte Patterns den gebräuchlichsten Technologie-Bezeichnern zu. Sie dient als Orientierung; genaue Zuordnungen dokumentieren die jeweiligen Provider.

| Pattern / Rolle                | UIAutomation (Win32)                                        | AT-SPI2 (Linux)                              | macOS AX                                   |
|--------------------------------|--------------------------------------------------------------|---------------------------------------------|---------------------------------------------|
| `control:Button` / `Button`    | `InvokePattern` + `UIA_ButtonControlTypeId`                  | `ATSPI_ROLE_PUSH_BUTTON`                     | `kAXButtonRole`                              |
| `control:CheckBox`             | `TogglePattern` + `UIA_CheckBoxControlTypeId`               | `ATSPI_ROLE_CHECK_BOX`                       | `kAXCheckBoxRole`                            |
| `control:RadioButton`          | `SelectionItemPattern` + `UIA_RadioButtonControlTypeId`     | `ATSPI_ROLE_RADIO_BUTTON`                    | `kAXRadioButtonRole`                         |
| `control:Text` (read-only)     | `TextPattern` / `ValuePattern` (readonly) + `UIA_TextControlTypeId` | `ATSPI_ROLE_LABEL` / `ATSPI_ROLE_TEXT`       | `kAXStaticTextRole`                          |
| `control:Edit` (editierbar)    | `ValuePattern` + `UIA_EditControlTypeId`                     | `ATSPI_ROLE_TEXT` + `Editable` property      | `kAXTextFieldRole`                           |
| `control:List`                 | `SelectionPattern` + `UIA_ListControlTypeId`                 | `ATSPI_ROLE_LIST`                            | `kAXListRole`                                |
| `item:ListItem`                | `SelectionItemPattern`                                      | `ATSPI_ROLE_LIST_ITEM`                       | `kAXRowRole` / `kAXOutlineRowRole`           |
| `control:Tree`                 | `ExpandCollapsePattern` + `UIA_TreeControlTypeId`           | `ATSPI_ROLE_TREE`                            | `kAXOutlineRole`                             |
| `item:TreeItem`                | `ExpandCollapsePattern` + `SelectionItemPattern`            | `ATSPI_ROLE_TREE_ITEM`                       | `kAXOutlineRowRole`                          |
| `control:Window`               | `WindowPattern` + `UIA_WindowControlTypeId`                 | `ATSPI_ROLE_WINDOW`                          | `kAXWindowRole`                              |
| `control:Menu`                 | `ExpandCollapsePattern` + `UIA_MenuControlTypeId`           | `ATSPI_ROLE_MENU_BAR` / `ATSPI_ROLE_MENU`    | `kAXMenuBarRole` / `kAXMenuRole`             |
| `item:MenuItem`                | `InvokePattern` + `UIA_MenuItemControlTypeId`               | `ATSPI_ROLE_MENU_ITEM`                       | `kAXMenuItemRole`                            |
| `control:ScrollBar`            | `RangeValuePattern` + `UIA_ScrollBarControlTypeId`          | `ATSPI_ROLE_SCROLL_BAR`                      | `kAXScrollBarRole`                           |
| `control:Slider`               | `RangeValuePattern` + `UIA_SliderControlTypeId`             | `ATSPI_ROLE_SLIDER`                          | `kAXSliderRole`                              |
| `control:Tab` / Tab-Container  | `SelectionPattern` + `UIA_TabControlTypeId`                 | `ATSPI_ROLE_PAGE_TAB_LIST`                   | `kAXTabGroupRole`                            |
| `item:TabItem`                 | `SelectionItemPattern`                                      | `ATSPI_ROLE_PAGE_TAB`                        | `kAXRadioButtonRole` mit Subrole `Tab`       |
| `app:Application`              | `CurrentProcessId`, `ProcessName`, `ExecutablePath`         | `ATSPI_ROLE_APPLICATION`                     | `kAXApplicationRole`                         |

Provider sollten dokumentieren, wenn sie von den vorgeschlagenen Zuordnungen abweichen (z. B. Proprietäre Rollen oder Unterrollen).

### Auswahl & Zustand (ClientPatterns)

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

### Struktur & Navigation (ClientPatterns)

#### Expandable
- **Beschreibung:** Knoten kann eine untergeordnete Struktur ein- oder ausblenden.
- **Pflichtattribute:** `IsExpanded`.
- **Optionale Attribute:** `HasChildren`.
- **Verwendung:** Tree-Items, Menüs, Disclosure-Widgets.

#### Scrollable
- **Beschreibung:** Container kann Inhalt scrollen.
- **Pflichtattribute:**
  - `CanScrollHorizontally`, `CanScrollVertically` – `xs:boolean`
  - `HorizontalPercent`, `VerticalPercent` – `xs:double` (0–100 oder `NaN` bei nicht scrollbaren Achsen)
  - `HorizontalViewSize`, `VerticalViewSize` – `xs:double`
- **Optionale Attribute:** `ScrollGranularity` (`xs:string`, z. B. `Line`, `Page`).

#### ItemContainer
- **Beschreibung:** Stellt indirekten Zugriff auf Kinder durch Index oder Schlüssel bereit.
- **Pflichtattribute:**
  - `ItemCount` (`xs:integer`, darf fehlen oder `-1`, wenn unbekannt)
  - `IsVirtualized` (`xs:boolean`)
- **Optionale Attribute:** `SupportsContainerSearch` (`xs:boolean`).
- **Verwendung:** Tabellen, Listen, virtuelle Kataloge.

### Fenster & Oberflächen (ClientPatterns)

#### WindowSurface
- **Beschreibung:** Bindeglied zu den plattformspezifischen Fenster-APIs.
- **Pflichtattribute:** `IsMinimized`, `IsMaximized`, `IsTopmost` – jeweils `xs:boolean`.
- **Optionale Attribute:** `SupportsResize`, `SupportsMove`, `AcceptsUserInput` – jeweils `xs:boolean` (Runtime spiegelt `AcceptsUserInput` zusätzlich über die Pattern-Aktion wider).
- **Runtime-Hinweis:** Das RuntimePattern `WindowSurface` stellt Aktionen (`activate()`, `minimize()`, …) sowie `accepts_user_input()` bereit.

#### DialogSurface
- **Beschreibung:** Spezialisierung für modale Dialoge.
- **Pflichtattribute:** `IsModal` (`xs:boolean`).
- **Optionale Attribute:** `DefaultResult` (`xs:string`).
- **Abhängigkeiten:** Erwartet `WindowSurface`.

### Applikationen & Prozesse (ClientPatterns)

- **Beschreibung:** Repräsentiert eine ausführende Anwendung oder einen Prozesskontext, aus dem Fenster und UI-Elemente stammen.
- **Pflichtattribute:**
  - `ProcessId` (`xs:integer`)
  - `ProcessName` bzw. `Name` (`xs:string`)
  - `ExecutablePath` (`xs:string`)
- **Optionale Attribute:** `CommandLine` (`xs:string`), `UserName` (`xs:string`), `StartTime` (`xs:dateTime`), `MainWindowIds` (JSON-Array aus `RuntimeId`s), `Architecture` (`xs:string`).
- **Hinweis:** Application-Knoten sind Einstiegspunkte für XPath-Abfragen über den `app`-Namespace; sie bündeln Metadaten, ersetzen aber keine Prozessverwaltung.

### Visualisierung & Annotation

#### Highlightable
- **Beschreibung:** Element kann visuell hervorgehoben werden.
- **Pflichtattribute:** `SupportsHighlight` (`xs:boolean`).
- **Optionale Attribute:** `HighlightStyles` (JSON-Array vordefinierter Stilkennungen).
- **Hinweis:** Die Runtime stellt eine eigene Highlight-Funktion bereit; kein Pattern-spezifisches Aktions-API erforderlich.
- **Runtime-Hinweis:** Das Highlighting nutzt den `HighlightProvider` (`highlight(&[HighlightRequest])`, `clear()`), der Desktop-Koordinaten verarbeitet und optional eine Dauer erhält, nach der der Overlay-Rahmen automatisch verschwindet oder bei neuen Anfragen verschoben wird.

#### Annotatable
- **Beschreibung:** Element kann Zusatzinformationen tragen (Fehler, Status, Hinweis).
- **Pflichtattribute:** `Annotations` (JSON-Array strukturierter Datensätze, z. B. `{ "Kind": "Error", "Message": "..." }`).

## RuntimePatterns – Laufzeitaktionen

| Pattern | Methoden | Beschreibung |
| --- | --- | --- |
| `Focusable` | `focus()` | Wechselt den Eingabefokus des Elements über die Runtime. |
| `WindowSurface` | `activate()`, `minimize()`, `maximize()`, `restore()`, `move_to(Point)`, `resize(Size)`, `move_and_resize(Rect)`, `close()`, `accepts_user_input()` | Delegiert Fensteraktionen an plattformspezifische APIs und liefert den Eingabestatus des Fensters. |

Alle Methoden liefern `Result<_, PatternError>`; Fehler bleiben damit transparent für Clients. Provider registrieren RuntimePatterns im `PatternRegistry`, während ClientPatterns ausschließlich über Attribute beschrieben werden.

### Geräteinteraktion (Idee)
Diese Patterns sind Diskussionsstoff, da sie eng mit Device-Providern verknüpft sind und `ActivationTarget` ergänzen könnten:
- **PointerTarget:** Liefert detaillierte Hit-Test-Informationen und ggf. dynamische Koordinaten für Zeigegeräte.
- **KeyboardTarget:** Kennzeichnet Elemente, bei denen Tastatursimulation ankommen soll.
- **GestureTarget:** Reserviert für zukünftige Touch-/Gesten-Interfaces.

## Pattern-Mapping (Arbeitsstand)

| Pattern | UI Automation (Windows) | AT-SPI2 (Linux) | macOS AX | Beispielwerte | Hinweise |
| --- | --- | --- | --- | --- | --- |
| `WindowSurface` | `IUIAutomationElement` mit `ControlType=UIA_WindowControlTypeId` | `Accessible::get_application` + `Component` | `AXWindow`, `NSWindow` | `accepts_user_input()` → `true` | Aktionen nutzen Plattform-Fenster-APIs (`SetForegroundWindow`, EWMH, AppKit). |
| `Application` | `IUIAutomationElement` (Application Root), Prozessinfos über Win32 APIs | `Accessible` über `org.a11y.atspi.Application` Interface | `AXApplication`, NSRunningApplication | `ProcessId=1234` | Prozessmetadaten stammen aus Plattform-API. |
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
| `Highlightable` | Provider-Overlay, `TransformPattern.GetRuntimeId` | Provider-Overlay via XComposite/Wayland Layer | `AXFrame`, transparentes `NSWindow` | `SupportsHighlight=true` | Runtime zeichnet Highlight, benötigt gültige `Bounds`. |
| `Scrollable` | `ScrollPattern` (`HorizontalPercent`, `VerticalPercent`) | `Component::scroll_to_point`, `Value` | `AXHorizontalScrollBar`, `AXVerticalScrollBar` | `VerticalPercent=55.0`, `CanScrollVertically=true` | Provider melden ViewSize und Scrollbarkeit getrennt. |
| `Expandable` | `ExpandCollapsePattern` | `Action::do_action("expand")` | `AXExpanded`, `AXPress` | `IsExpanded=false`, `HasChildren=true` | Provider geben nur dann `HasChildren=true` an, wenn API dies bestätigen kann. |
| `ItemContainer` | `ItemContainerPattern` (WinUI/Custom) | `Table`, `Collection` Interfaces | `AXChildrenInNavigationOrder` | `ItemCount=500`, `IsVirtualized=true` | Bei virtuellen Listen optional Paging-Attribute ergänzen. |

> Hinweis: Für Runtime-Aktionen stellt `platynui-core` bereits Hilfstypen bereit (`FocusableAction`, `WindowSurfaceActions`). Diese kapseln die zugehörigen Methoden über Closures und werden im Runtime-Code wie auch in Tests wiederverwendet.

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
- Contract-Tests definieren, die Provider gegen diese Pattern-Spezifikation ausführen müssen – Grundlage bilden die hier aufgeführten Pflichtattribute sowie die Konstanten unter `platynui_core::ui::attribute_names`.
- Nach der Abstimmung Version 1.0 des Pattern-Katalogs festschreiben und im Architekturkonzept verlinken.
