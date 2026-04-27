# PlatynUI Python Library — Design & Migrationsplan

<!-- Living document. Diskussionsgrundlage für die Portierung der Python-Schicht
     aus dem Altprojekt (`/home/daniel/develop/tmp/robotframework-PlatynUI`) auf
     den neuen Rust-basierten Kern. Keine Entscheidung ist final. -->

> **Status:** Diskussionsentwurf, **Revision 36**.
>
> **Änderungen seit Rev. 4:**
> - **Rev. 36** — **`Tabs` und `Menus` UI-Klassen (§A.14.23 +
>   §A.14.24).** Phase 4d ergänzt die letzten beiden Standard-
>   Container der UI-Schicht. `TabList`/`TabItem` folgt dem List/
>   ListItem-Muster aus Phase 4c (`TabItem(Item)` mit
>   `Selectable`-Mixin via `SelectableItem`). `Menu`/`MenuBar`/
>   `MenuItem` modellieren Menü-Hierarchien: `MenuItem` erbt
>   bewusst `Control` (nicht `Item`) — ein Menü-Eintrag ist
>   semantisch ein eigenständiges interaktives Control mit
>   eigener Sub-Hierarchie, kein Container-Inhalt im Sinne von
>   `ListItem`/`Cell`. Konsequenz: Z. 4105 wird korrigiert
>   (`MenuItem` raus aus der Item-Aufzählung). `MenuItem.activate()`
>   läuft die Vorfahren-Kette nach oben, expandiert jedes
>   Vorgänger-`MenuItem` von außen nach innen und aktiviert dann
>   self.
> - **Rev. 35** — **`Technology`-Marker entfernt.** Die einzige
>   real existierende Technology der Bibliothek ist `UiNode` (siehe
>   §A.11), und `framework_id` deckt jede aktuell vorstellbare
>   Diskriminierung zwischen Plattformen/Toolkits bereits ab. Die
>   `Technology`-ABC + `UiNodeTechnology`-Singleton + das
>   `technology`-Kriterium des `WeightCalculator` und der
>   `pattern_proxy_for(technology=...)`-Parameter waren in der neuen
>   Architektur funktionslos und sind komplett entfernt. `Adapter`
>   exposeniert kein `.technology` mehr; `core/technology.py` und
>   `TechnologyName` sind weg. Falls jemals eine zweite
>   Python-seitige Technology nötig wird, ist der Re-Add trivial.
> - **Rev. 34** — **Item-Hierarchie + Container-Klassen
>   (§A.14.13–§A.14.21).** Phase 4c führt vier neue Patterns
>   (`Selectable`, `Expandable`, `HasEditor`, `ItemContainer`)
>   und die Context-Klassen für Item-Container ein. Item-
>   Capabilities werden als Mixin-Klassen modelliert: `Item`
>   ist ein register-freier Marker mit `text` (über `TextContent`),
>   `SelectableItem`/`ExpandableItem`/`EditableItem` ergänzen jeweils
>   genau ein Pattern. Konkrete Klassen kombinieren per
>   Mehrfachvererbung (`TreeItem(SelectableItem, ExpandableItem)`,
>   `EditableCell(Cell, EditableItem)`). Container-Klassen
>   (`List`, `Tree`, `Table`, `Row`) wrappen das `ItemContainer`-
>   Pattern (typisierte `item_count`/`row_count`/`column_count`
>   statt generischer `Properties`-Reads aus dem Altprojekt) und
>   exposen `get_item(s)`/`iter_items` über `LocatorScope.Children`.
>   `ComboBox` kombiniert `Expandable` + Item-Selektion + optional
>   `TextEditable` und implementiert den expand→select→collapse-
>   Lifecycle aus dem Altprojekt. Phase 4c bleibt komplett Python-
>   seitig: ABCs + Tests gegen Stubs; Rust-Pattern-Konstanten und
>   Native-Bindings folgen in einer späteren Phase.
> - **Rev. 33** — **`Element.default_click_position` entfernt.** Saubere
>   Trennung Geometrie (`Element`) ↔ Klick-Capability
>   (`ActivationTarget`): das Element-Pattern liefert nur noch
>   `bounds`, `is_visible`, `is_in_view`, `is_enabled`. Der
>   Default-Klickpunkt lebt vollständig im `MouseProxy`. Die
>   `AdapterMouseProxy`-Fallback-Kette ist damit zweistufig:
>   `ActivationTarget.activation_area.center()` →
>   `ActivationTarget.activation_point` (falls Pattern unterstützt),
>   sonst `Element.bounds.center()`. Begründung: Rust kennt nur
>   `ActivationPoint`, kein `DefaultClickPosition`-Attribut; die
>   Property war Convenience ohne Rust-Backing und vermischte
>   Geometrie- und Interaktions-Verantwortung.
> - **Rev. 32** — **Pattern-Hierarchie bleibt flach (Klarstellung).**
>   `TextEditable` erbt bewusst nicht von `TextContent`, obwohl
>   jedes editierbare Feld auch lesbaren Inhalt hat. Begründung:
>   das Resolver-Modell (`pattern_name`-String-Lookup im Adapter)
>   kennt keine Hierarchie — Vererbung auf Python-Seite wäre ein
>   stiller Beobachter ohne Verhaltens-Effekt; Adapter müssten
>   beide Pattern-Namen ohnehin separat in
>   `supported_pattern_names()` listen. Konvention gilt für alle
>   Pattern-Paare (Toggleable/Activatable, Resizable/Movable, …);
>   Beziehungen werden über Adapter-Listen, nicht über Klassen-
>   hierarchie ausgedrückt. Reine Doku-Klarstellung, kein Code-
>   Change.
> - **Rev. 31** — **`is_multi_line` von `TextContent` auf `TextEditable`
>   verschoben.** Die Multi-Line-Eigenschaft ist semantisch nur
>   Felder relevant (Tab- vs. Enter-Verhalten, Zeilenumbruch-
>   Akzeptanz); reine Anzeige-Texte unterscheiden ein-/mehrzeilig
>   nicht durch ein Verhaltens-, sondern durch ein Layout-Merkmal.
>   `TextContent` enthält damit nur noch `text`, `locale`,
>   `is_truncated`. `Edit.is_multi_line` ruft `TextEditable.is_multi_line`;
>   `Text` exposed die Eigenschaft nicht.
> - **Rev. 28** — **Text/Edit (§A.14.10/§A.14.11).** `Text` ist
>   die read-only Default-Klasse für Labels und Anzeige-Texte
>   (nur `TextContent`); `Edit` ist die beschreibbare Default-
>   Klasse für Eingabefelder (`TextContent` + `TextEditable` +
>   `Clearable`). Beide leben in `ui/text.py` ohne Vererbungs-
>   beziehung — die Legacy-Mischung „`Text` ist beschreibbar,
>   `Edit(Text)` ist Marker-Alias" entfällt. Offene-Punkte-
>   Subsection auf §A.14.12 umnummeriert.
> - **Rev. 27** — **Buttons (§A.14.9).** `AbstractButton` als
>   abstrakte Zwischenklasse unter `Control` mit
>   `text`-Convenience über `TextContent` und abstract
>   `activate()`. `Button` wrappt das `Activatable`-Pattern,
>   `CheckBox` das `Toggleable`-Pattern (mit `check`/`uncheck`/
>   `toggle`/`set_state`/`is_checked`-Komfort). `CheckBox.activate()`
>   ruft semantisch `check()` (User-Intent „abhaken"), nicht
>   `Toggleable.toggle()`. Phase 4a verlangt den Provider-Pattern-
>   Pfad; Click-Fallback ist Sache der Default-Proxy-Schicht
>   (Phase 4e). Offene-Punkte-Subsection auf §A.14.10
>   umnummeriert.
> - **Rev. 26** — **Context-Basisklassen (§A.14).** `Element`,
>   `Control`, `Window`, `Frame`, `Desktop`/`DesktopBase`, `Application`
>   als Klassenhierarchie unter `ContextBase`. `Element` ist das
>   Arbeitstier mit Predicates, Mouse/Keyboard-Proxies, Highlight,
>   Screenshot, Bounds-/Visibility-Properties. `Window` wrappt die
>   Window-Capability-Patterns aus §A.13 in Pre/Perform/Post-Verträge.
>   `Application` ist reiner Identity-Container (`ContextBase`-Direkt-
>   Child) mit zweistufigem `exit()` (graceful → force-kill).
>   `_application_is_ready` kombiniert ein Top-Level-`HasUserInput`-
>   Pattern mit einer optionalen User-`Application.is_ready()`-
>   Methode (Tree-Walk-up, lazy gecached). Mouse-/Keyboard-Module
>   ziehen nach `core/devices/`. `bounding_rectangle`
>   heißt jetzt `bounds`. (Anmerkung Rev. 33: `default_click_position`
>   wurde aus dem Element-Pattern komplett entfernt — siehe Rev. 33
>   oben.)
> - **Rev. 25** — **Granulare Window-Capability-Patterns (§A.13).**
>   Das aktuelle Rust-`WindowSurfacePattern` ist eine **Arbeits-
>   version**; im finalen Modell zerfällt es in eine Suite kleiner,
>   orthogonaler Capability-Patterns: `Activatable`, `Focusable`
>   (beide existieren), `Minimizable`, `Maximizable`, `Restorable`,
>   `Closeable`, `Movable`, `Resizable`, `Titled`, `HasUserInput`.
>   Pro Capability **ein** Pattern, das State-Reads und Action
>   bündelt (kein paralleles `Has…`-Read-Pattern wie im Altprojekt).
>   Damit decken ~10–15 Patterns alle UI-Elemente ab — eine Sidebar
>   nutzt `Minimizable`+`Maximizable`+`Restorable` mit *demselben*
>   Code wie ein Top-Level-Window. `Activatable` ist die universelle
>   primary-action-Capability; ein `Window`-Context ist
>   `Activatable` (Default-Proxy mappt auf Window-Manager-API).
>   Zusätzlich zwei kleine Element-Patterns aus dem Altprojekt:
>   `Readable` (`is_readonly`) und `ApplicationReady`
>   (`try_ensure_ready`). Übergangsweise (bis Rust-Refactor) ruft
>   der Default-Window-Proxy weiter `WindowSurface` als Bridge.
> - **Rev. 24** — **`ElementDescriptor` (§A.7) präzisiert + §13.1/§13.2
>   neu gefasst.** `core/descriptor.py` bleibt Robot-frei; das
>   Root-Element wird über einen austauschbaren Storage-Hook
>   (`set_root_element_storage(getter, setter)`) gehalten,
>   Default = prozesssweiter Slot. Begründung jetzt explizit in §13.1
>   als „Geteilter Zustand": `PlatynUI` und `BareMetal` laufen
>   gemeinsam in derselben Robot-Suite, teilen die `Runtime`, und
>   teilen sich perspektivisch eine **gemeinsame Robot-Variable
>   `${PLATYNUI_ROOT_ELEMENT}}`** als Single Source of Truth (beide
>   Library-Inits installieren denselben Hook). §13.1 stellt klar,
>   dass BareMetal eine **dauerhafte** Low-Level-Library ist (keine
>   reine Diagnose-Rolle); §13.2 enthält den Übergangsplan: aktuell
>   noch eigener `UiNodeDescriptor` mit `${PLATYNUI_ROOT_DESCRIPTOR}`,
>   später ersetzt durch eine BareMetal-Variante des Descriptors mit
>   `UiNode`-Resolution. `PatternT` ist ein Phantom-TypeVar
>   (`bound=PatternBase, default=PatternBase`); `__call__` liefert
>   weiterhin `ContextBase`, der Pattern-Check erfolgt im Keyword
>   selbst. `__call__` ist keyword-only (`full_context=True`).
>   `RootElementDescriptor` bleibt Subklasse mit überschriebenem
>   `convert`.
> - **Rev. 23** — **`AdapterFactory` als eigenes Singleton (§A.4b).**
>   Suche und Adapter-Wrapping ist eine eigene Verantwortung,
>   weder am `Adapter` (würde Adapter an Runtime koppeln und das
>   Wrapping-Verhalten je Implementierung duplizieren) noch direkt
>   in `ContextBase` (würde Suchstrategie an Context-Schicht
>   nageln und Test-Stubs erschweren). Der Layer folgt dem
>   Altprojekt (`core/adapterfactory.py`), tauscht aber die
>   per-Technology-Registry gegen ein prozesssweites Singleton
>   `adapter_factory` mit Sealing/Override analog zu `runtime`
>   (§A.5). API auf zwei Methoden reduziert: `find_one(parent,
>   locator)` und `find_all(parent, locator)`. `find_parent`
>   entfällt — `adapter.parent` reicht. `ancestor`/`get_child`/
>   `get_children` aus dem alten `ContextBase` sind dünne
>   Wrapper über `find_one`/`find_all` mit gesetztem
>   `LocatorScope` und gehören in `ContextBase`, nicht in die
>   Factory. Die Default-`RuntimeAdapterFactory` greift
>   `parent.native_node` (neue Public-Property auf `UiNodeAdapter`,
>   §A.4a-Ergänzung) und ruft `runtime.current.evaluate*`. Doku in
>   §A.4b neu; §4.4, §1.2-Tabelle, §6.4, §9-Verzeichnis und
>   Phase-1/2-Reihenfolge entsprechend angepasst.
> - **Rev. 22** — **`ContextBase` erbt nicht mehr von `Assertable`.**
>   Die in §A.4 (Rev. 8) angekündigte Klasse `Assertable` mit
>   `assert_that`/`assert_that_not` als Basis aller Context-Klassen
>   existierte weder im Alt- noch im Neuprojekt. `src/PlatynUI/_assertable.py`
>   enthält nur den `@assertable`-**Keyword-Decorator** für die
>   RF-Library-Schicht (siehe BareMetal-Keywords) — der bleibt
>   unverändert und wird weiter eigenständig genutzt. Pre-/Post-
>   Verifikation auf `ContextBase` läuft ausschließlich über die bereits
>   spezifizierte `ensure_that`-Methode.
> - **Rev. 21** — **Python-Mindestversion auf 3.12 angehoben.** `requires-python`
>   in allen vier `pyproject.toml`-Dateien (root, native, cli, inspector)
>   sowie `.python-version` und `pyo3`-Feature `abi3-py312` (Cargo.toml +
>   maturin-config). 3.10 erreicht im Oktober 2026 EOL — der Bump erfolgt
>   *vor* Veröffentlichung, damit kein Nutzer betroffen ist. Gewinne:
>   PEP 695 Generics-Syntax (`class PatternProxy[P]:` statt
>   `Generic[P]`), `typing.override` als Sicherheitsnetz in der
>   Proxy-Vererbungshierarchie, `typing.Self` ohne `typing_extensions`-
>   Fallback. §2.6 entsprechend angepasst (PEP 695 ist jetzt empfohlen,
>   nicht mehr ausgeschlossen). Begründung: ABC-vs-Protocol-Frage
>   nochmals durchgegangen — ABC bleibt richtig (siehe Rev. 6) auch unter
>   3.12, weil 3.12 keine Protocol-Eigenschaften ändert, die unsere
>   nominale Verwendung beträfen. **ABC-Begründung in §2.6 und §5
>   geschärft:** der bisherige Verweis auf `__init_subclass__` wurde
>   gestrichen (war kein echter ABC-vs-Protocol-Unterschied — Protocols
>   können das auch). Stattdessen das tatsächlich tragende Argument:
>   Default-Methoden auf einer ABC werden nur an nominale Subklassen
>   vererbt — strukturelle Protocol-Implementierer würden Defaults
>   stillschweigend verlieren.
> - **Rev. 5** — modernes Python 3.10+ als verbindlicher Standard
>   (Dataclasses, `match`/`case`, PEP 604, `Self`, Hybrid-Registry für
>   Decorators).
> - **Rev. 6** — ABC bleibt Default für Pattern-Interfaces (statt
>   Protocol); keine globale `PatternRegistry` (Adapter halten lokale
>   Mapping-Tabellen); `StrEnum` für Standardrollen/Technologien
>   verworfen (freie Strings).
> - **Rev. 7** — Selbst-Review: Hybrid-Form (`class X(Base, role=…)`)
>   konsequent in Code-Beispielen, §2.6 mit Konventionen-Spickzettel,
>   §5.4 „Verwendung"-Block präzisiert (kein Widerspruch zur
>   No-Registry-Aussage), §7.1 PascalCase-Konvention dokumentiert,
>   §9 `__init__.py`-Konventionen ergänzt, §13.4 Class-Registries
>   gegen No-Pattern-Registry abgegrenzt.
> - **Rev. 8** — Neuer §9a „API-Spezifikationen" mit zwölf
>   Detail-Verträgen: Settings, Exception-Hierarchie,
>   `ensure_that`/`wait_for` (inkl. Standard-Predicates),
>   Adapter-Interface, `ContextBase`-API, `@locator`-Mechanik,
>   `ElementDescriptor[PatternT]`, Lifecycle/Robot-Library-Init,
>   Devices (Mouse/Keyboard), Pattern-Defaults, Mock-Adapter,
>   Highlight/Diagnose. Damit ist jede in §§ 2–9 referenzierte API
>   konkret unterfüttert. Selbst-Review-Fixes: `## 10.`-Überschrift
>   wiederhergestellt, `HasBounds`/`Visibility`-Patterns in §5
>   deklariert, `supports_pattern` als Adapter-Convenience,
>   Element-Convenience-Properties in §A.5 (Wrapper auf Patterns),
>   Phase-1-Hinweis konsistent zu re-entrant Thread-Local-Stack,
>   Mock-Adapter ohne `MockRuntime` (direkt per `adapter=`-Parameter
>   am Desktop), Desktop-Beispiele vereinheitlicht auf `ContextBase`,
>   `Assertable`-Herkunft verlinkt.
> - **Rev. 9** — Zweite Selbst-Review über das ganze Dokument: §4.3
>   `PatternNotSupportedError` statt generisches „Exception"; §4.4
>   Adapter-Auflösung ohne `locator.technology` (erbt aus Parent-
>   Context, `Desktop` hält den Wurzel-Adapter); §5.4
>   `WindowSurfaceActivate`-Beispiel auf existierendes
>   `WindowSurface`/`HasBounds` korrigiert; §5
>   `HasIsEnabled`/`HasIsReadonly` als eigenständige Patterns
>   deklariert (aus §A.3-Standard-Predicates heraus sichtbar); §7.2
>   auf §A.5 verlinkt; §8-Beispiel auf §A.7 verwiesen; §9 Dateiliste
>   um `Deactivatable` bereinigt, `window.py` als `WindowSurface`-
>   Cluster gekennzeichnet; §11.3 Thread-Local-Widerspruch aufgelöst
>   (bleibt für Re-entrancy; ensure.py ~80 LOC statt ~50); §13.2 als
>   „resolved in §A.7" markiert; §A.8 Adapter-Bootstrap via
>   `core/technology.py` spezifiziert (einzige prozessweite
>   Adapter-Registry; Zweck: Default-Adapter fürs Desktop-Root).
> - **Rev. 10** — Neuer §11a „Testing-Strategie" mit Drei-Ebenen-
>   Pyramide (cargo nextest / pytest / Robot Framework),
>   Test-Matrix über 12 Test-Gegenstände × 5 Kanäle,
>   RF-Mock-Harness-Spezifikation (`Load Mock Tree`, `Set Mock
>   Property`, `Set Mock Pattern Behavior`), Phase-für-Phase-
>   Testreihenfolge und CI-Matrix (Haupt-CI gegen Mock, Nightly
>   gegen echte Adapter pro OS). RF-Tests begleiten die Entwicklung
>   ab Phase 3, nicht erst am Ende.
> - **Rev. 11** — §11a.6/§11a.7 überarbeitet: Mock- und Echt-
>   Provider-Tests laufen **parallel** statt sequentiell. Neue
>   „Phase 0"-Prerequisite: AccessKit-Integration in
>   `apps/test-app-egui` (aktuell ohne AccessKit → AT-SPI blind).
>   §11a.7 spezifiziert Teststrecken pro OS: Linux (Wayland-
>   Compositor + AT-SPI), Windows (UIA), macOS (AX) — alle mit
>   `test-app-egui` als gemeinsamer Ziel-App. Dual-Mode-RF-Suites:
>   dieselbe `.robot`-Datei läuft gegen Mock und Echt-Provider,
>   gesteuert über `provider=${PROVIDER_MODE}`-Variable. CI-Matrix
>   erweitert um OS × Provider-Mode Cross-Product. Echt-Provider-
>   Tests beginnen ab Phase 3 (Linux AT-SPI) statt erst Phase 7.
> - **Rev. 12** — Korrektur in §11a.6/§11a.7.1: Annahme aus Rev. 11
>   („AccessKit fehlt in `test-app-egui`") war **falsch**. Faktencheck
>   gegen `eframe 0.34.1` Feature-Flags: `accesskit` ist Default-
>   Feature und transitiv via `egui-winit/accesskit` aktiv (ab egui
>   0.35+ obligatorisch laut Upstream-PR #7701). Die App ist seit
>   Anbeginn accessibility-aktiv — der Modul-Header (`main.rs:7`)
>   deklariert sie explizit so. Phase 0 reduziert sich von „AccessKit
>   integrieren" auf „AT-SPI-Tree verifizieren + ggf. deterministische
>   Widget-Szenarien ergänzen". Echt-Provider-Tests sind damit ab
>   sofort möglich, kein Blocker mehr.
> - **Rev. 13** — **Phase-0-Smoke-Verifikation durchgeführt** (Linux/
>   Wayland/GNOME). Ergebnis in §11a.7.1/§11a.7.2 eingearbeitet.
>   Zentrale Erkenntnisse:
>   (1) `test-app-egui` ist am AT-SPI-Bus erst sichtbar, wenn
>   `org.a11y.Status.ScreenReaderEnabled == true` ist —
>   `accesskit_unix 0.21/src/context.rs:153–180` aktiviert den Adapter
>   **ausschließlich** bei aktivem Screen Reader. Das ist Design von
>   AccessKit, keine App-Codeänderung kann das umgehen. Konsequenz:
>   Linux-Testläufe müssen das Flag vor Startup toggeln
>   (Setup-Fixture/Helper-Skript). (2) Unser `platynui-cli-rs` findet
>   die App per XPath korrekt, liefert vollständige Attribute
>   (`control:*`, `native:Accessible.*`, `Bounds`, `SupportedPatterns`)
>   und iteriert den Widget-Baum (Frame → Panel → Button/Entry/CheckBox/
>   SpinButton/ScrollBar). AccessKit→AT-SPI-Roundtrip funktioniert. (3)
>   Beim Cold-Start wirft `provider-atspi` einen `D-Bus call timed out
>   elapsed_ms=1000`-Warn — Call liefert trotzdem. Eventuell Timeout-
>   Strategie überdenken (nicht in Phase 0). (4) `native:Application.
>   ToolkitName` kommt `null` zurück — AccessKit oder egui-winit setzt
>   das nicht; Nice-to-have für später.
> - **Rev. 14** — Rust-`PatternId` durchgängig auf Reverse-DNS umgestellt
>   (siehe §13.6).
> - **Rev. 15** — **Attribute-Modell konsolidiert.** Die alten
>   `Properties`-/`NativeProperties`-Patterns und die getrennten
>   `property_*`/`native_property_*`-Adapter-Methoden werden ersatzlos
>   gestrichen. Adapter (Rust und alle künftigen) exposen Attribute
>   einheitlich als `(namespace, name) → UiValue`-Schlüsselraum, parallel
>   zum Rust-Modell (`crates/core/src/ui/node.rs`,
>   `crates/core/src/ui/namespace.rs`). Vier Namespaces sind kanonisch:
>   `control` (Default), `item`, `app`, `native`. Konsequenzen:
>   (a) `Adapter`-Interface (§A.4) hat `attribute_names(namespace=...)`
>   und `attribute_value(name, namespace=...)`, sonst nichts; (b)
>   `WeightCalculator` (§4.1) hat genau ein Attribut-Kriterium
>   `attributes[(ns, name)] == v`; (c) Locator (§A.6) trägt
>   `attributes: dict[str | tuple[str, str], str | re.Pattern[str]]` —
>   bare String = Name im Default-Namespace, Tupel = expliziter
>   Namespace; (d) Context kann den Default-Namespace per
>   Klassenattribut `default_attribute_namespace = "item"` o.Ä.
>   umstellen. Symmetrisch zu Rust und zur XPath-Schreibweise
>   (`@AutomationId` ist im Default-NS `control`, `@native:HWND` ist
>   explizit). Die Robot-Keywords heißen `Get Attribute` / `Get
>   Attributes` (kein `Get Property` mehr); das `Properties`-Pattern
>   entfällt.
> - **Rev. 16** — **Locator-Konstruktor akzeptiert PascalCase-Kwargs**
>   als freie Attribut-Predicates (`Locator(Name="OK", native__HWND=1)`),
>   neben den sechs typisierten snake_case-Convenience-Feldern und dem
>   `attributes`-Dict. Doppelte Schlüssel über mehrere Kanäle werfen
>   `TypeError` mit konkreter Quellenangabe — kein stilles
>   Vorrang-Verhalten. Details in §7.1 und §A.6.
> - **Rev. 20** — **Process-wide Runtime Singleton.** `PlatynUI.core.runtime`
>   exportiert ein Singleton-Objekt `runtime` (Klasse `Runtime`) mit drei
>   Verantwortlichkeiten: *Variantenwahl* (`use_default()`, `use_mock()`,
>   `use_factory(cb)` — nur vor dem ersten `current`-Zugriff erlaubt;
>   danach gesealed), *Konsum* (`current`-Property, lazy gebaut beim
>   ersten Zugriff) und *Test-Override* (`override(...)` /
>   `override_with_mock()` als Context-Manager mit garantiertem
>   Restore). Es gibt **keinen** nackten Setter wie `set(rt)` — alle
>   Wege sind entweder Variantenwahl oder scope-gebundener Override.
>   Adapter, Device-Proxies, künftige Robot-Keywords, BareMetal-Helper
>   und Inspector teilen sich denselben `platynui_native.Runtime`.
>   Konsequenzen: `UiNodeAdapter.create_root()` braucht keinen Runtime-
>   Parameter mehr; `AdapterMouseProxy` / `AdapterKeyboardProxy` greifen
>   intern auf `runtime.current` zu. RF-Library: `Library PlatynUI
>   use_mock=${True}` mappt auf `runtime.use_mock()`. Siehe §A.5.
> - **Rev. 19** — **Rust-API-Symmetrie zu Python: `PatternId` → `PatternName`.**
>   Der Newtype `PatternId` heißt jetzt `PatternName`, das Konstanten-Modul
>   `pattern_ids` heißt `pattern_names`, die Trait-Methoden `UiPattern::id()`
>   / `UiPattern::static_id()` heißen `pattern_name()` / `static_pattern_name()`,
>   und `UiNode::pattern_by_id()` heißt `pattern_by_name()`. Damit haben
>   Rust-API und Python-API dieselbe Vokabelwahl. Wire-Format ist unverändert
>   (`org.platynui.patterns.<Name>`-Strings). PyO3-Klasse `PatternName` wird
>   bewusst NICHT aus `platynui_native.__init__` re-exportiert, weil sie
>   sonst mit dem Python-TypeAlias `PlatynUI.core.types.PatternName: TypeAlias = str`
>   kollidieren würde — Python-User-Code spricht den str-Alias, der Wrapper
>   bleibt intern (`platynui_native._native.PatternName`). Siehe §13.7.
> - **Rev. 18** — **`@locator` ist jetzt eine echte Decorator-Funktion**
>   (kein `Locator`-Alias mehr). Class-Decorator-Form
>   (`@locator(name="X") class Foo: ...`) ist vollständig implementiert
>   und hängt einen `Locator` als `Foo.__locator__` an. Method-/
>   Property-Form (`@property + @locator(...) def n5(self) -> Button`)
>   ist als `LocatorMethodDescriptor`-Stub implementiert: API steht,
>   `__get__` wirft derzeit `NotImplementedError("Phase 3")`. Die volle
>   Resolution braucht `ContextBase.get(annotation, locator=…)` aus
>   Phase 3. Context-Code kann beide Formen heute schreiben — der
>   Phase-3-Übergang erfordert keine Änderung am Context. Details in
>   §A.6.
> - **Rev. 17** — **Pattern-Liste konsolidiert.** Die Python-Pattern-
>   Hierarchie wird an die Rust-Capability-Gruppen
>   (`crates/core/src/ui/attributes.rs`,
>   `crates/core/src/ui/identifiers.rs`) angeglichen. Konkret:
>   (a) **`HasBounds` + `Visibility` + `HasIsEnabled`** werden zu
>   einem Pattern `Element` mit `bounds`, `is_visible`, `is_in_view`,
>   `is_enabled` zusammengeführt — analog zum
>   Rust-Modul `attributes::element`. (Anmerkung Rev. 33: ursprünglich
>   inkl. `default_click_position`; in Rev. 33 wieder entfernt.) (b) **`EditableText`** wird in
>   drei Patterns aufgeteilt: `TextContent` (read-only Properties
>   `text`, `locale`, `is_truncated`), `TextEditable`
>   (`set_text()` + `is_readonly`, `max_length`, `supports_password_mode`,
>   `is_multi_line`) und `Clearable` (`clear()`). `HasIsReadonly` entfällt — Read-only-
>   Status gehört zu `TextEditable`. (c) **`Toggleable` + `HasToggleState`**
>   werden zu einem Pattern `Toggleable` mit `toggle()` + `state` +
>   `supports_three_state` zusammengeführt. (d) **`Activatable`** wird
>   um `is_activation_enabled` und `default_accelerator` erweitert.
>   (e) **`Focusable`** bleibt eigenständig (`is_focused` + `focus()`).
>   `HasFocus` entfällt. (f) **`Point`/`Rect`** werden aus
>   `platynui_native` re-exportiert (kanonische pyo3-Bindings statt
>   eigener Python-Definition).
>
> **Wire-Breaking Change in Rev. 17:** Das Rust-Attribut `IsOffscreen`
> wird zu `IsInView` umbenannt und semantisch invertiert (war: „nicht
> sichtbar im Viewport"; jetzt: „im Viewport sichtbar"). Default-
> Fallback in den Providern dreht von `false` auf `true`. Da das
> Projekt unveröffentlicht ist, ist das kein SemVer-Breaking Change.
>
> **Begriffsänderung in Rev. 4:** Wir verwenden durchgehend den Begriff
> **Pattern** statt Strategy — passend zur neuen Rust-Implementierung
> (`UiPattern`, `FocusablePattern`, `WindowSurfacePattern`). Der frühere
> Begriff „Strategy" (aus dem Altprojekt) und „Pattern" sind synonym; im
> neuen Projekt heißen sie überall Pattern.
>
> **Inhaltlich (unverändert seit Rev. 3):** Patterns werden auf
> **Python-Seite** verwaltet — nicht in Rust. Rust ist *eine* konkrete
> Implementierung der Adapter-Schicht; in Zukunft können weitere Adapter
> daneben stehen (JSON-RPC, andere Protokolle, …). Adapter dürfen Patterns
> mitbringen; zusätzlich kann pro UiNode anhand seiner Attribute eine
> spezialisierte Implementierung (Context-Klasse + Proxy mit
> Patterns) ausgewählt werden. Aus User-Sicht beschreiben wir
> UI-Elemente so, wie sie sich präsentieren — und Robot-Framework-Keywords
> drücken **Outcomes** aus (`Activate`, `Toggle`, `Select`, `Set Value`,
> `Expand`), nicht Mechanismen (`Click`).
>
> Die zugrundeliegende Mental-Model-Vorlage ist die RoboCon-2025-Präsentation
> in `docs/talks/robocon_2025.md` — insbesondere die Slides zu
> „Click ist die falsche Frage" und „semantische Aktionen mit
> Outcome-Vertrag".

## 1. Ausgangslage

### 1.1 Neues Projekt (Ist-Stand)

- **Rust-Kern** (produktiv):
  - `platynui-core` — `UiNode`, `UiAttribute`, `UiValue`, `UiPattern`, Namespaces
  - `platynui-xpath` — vollwertige XPath 2.0-Engine mit Streaming-Evaluator
  - `platynui-runtime` — `Runtime`, `PointerProfile`, `KeyboardProfile`, Provider-Registry, Cache
  - Provider: Windows UIA, AT-SPI2, macOS AX (Stub), Mock
  - **Rust-Patterns aktuell:** `FocusablePattern`, `WindowSurfacePattern`
    (mehr ist möglich, aber kein zwingender Designtreiber — siehe §6).
- **Python-Bindings** (`packages/native`): PyO3-Wrapper über
  `UiNode`/`Runtime`/Patterns.
- **Robot-Framework-Schicht**:
  - `PlatynUI.BareMetal` (917 LOC) — Low-Level-Keywords direkt über
    XPath-Strings. Funktioniert.
  - `PlatynUI` (Main-Library) — aktuell Platzhalter mit `dummy_keyword`.
    **Genau hier passiert der Port.**

### 1.2 Altes Projekt (als Quelle der Konzepte)

Speicherort: `/home/daniel/develop/tmp/robotframework-PlatynUI/src/PlatynUI/`

Wichtige Bausteine, deren Konzepte 1:1 übernommen werden (Implementierung
wird teilweise stark vereinfacht):

| Modul | LOC | Rolle | Übernahme |
|---|---|---|---|
| `core/adapter.py` | 156 | `Adapter`-Interface (UiNode + Pattern-Lookup) | **Konzept ja**, dünner |
| `core/adapterproxy.py` | 170 | `AdapterProxy` + Factory + `@adapter_proxy_for` (alt) → `@pattern_proxy_for` (neu) | **Ja, zentral** |
| `core/contextbase.py` | 473 | `ContextBase` + `ContextFactory` + `@context` | **Ja**, vereinfacht |
| `core/weight_calculator.py` | 114 | Gewichtetes Multi-Kriterien-Matching | **Ja, übernehmen** |
| `core/strategybase.py`, `core/strategies/` (alt) | — | Pattern-Interfaces (Activatable, Toggleable, …) | **Ja**, neu unter `core/patterns/` |
| `core/strategyimpl.py` (alt) | — | Default-Pattern-Implementierungen | **Ja**, leicht angepasst |
| `core/ensure.py` | 155 | Retry mit Prädikaten | **Ja**, vereinfacht (~50 LOC) |
| `core/wait_for.py` | 51 | Polling bis Prädikat | **Ja, 1:1** |
| `core/settings.py` | 66 | Timeouts, Delays | **Ja, 1:1** |
| `AdapterFactory` | — | Bridge zu C#-Providern | `AdapterFactory` als Singleton neu (siehe §A.4b) — die per-Technology-Registry des Altprojekts entfällt zugunsten einer Default-Factory pro Prozess |
| `ui/locator.py` | 433 | XPath-Builder aus Attributen | **Ja**, ~100 LOC dank Rust-XPath |
| `ui/proxies/standardproxies.py` | 408 | Standard-Proxies pro Rolle | **Ja**, das Herzstück |
| `ui/element.py`, `window.py`, `buttons.py`, … | ~1500 | UI-Klassen (Context-Basis) | **Ja** |
| `keywords/*.py` | ~250 | Robot-Framework-Keywords | **Ja**, semantisch geschärft |
| `_assertable.py` | — | bereits portiert | ✅ |

**Gesamtgröße alt:** ~4.600 LOC. **Geschätzte Zielgröße:** ~2.000 LOC, weil
die Adapter-Bridge und der XPath-Builder dramatisch schrumpfen.

## 2. Designprinzipien (das Mental Model)

Diese Prinzipien sind nicht verhandelbar — sie sind die Definition dessen,
was PlatynUI **ist**.

### 2.1 User-Sicht ist führend

Tests beschreiben UI-Elemente so, wie sie sich dem **User** präsentieren —
nicht so, wie sie technisch implementiert sind. Ein Element, das aussieht
und sich verhält wie ein Button, ist im Test ein `Button` — egal ob es
intern ein nativer Button, ein Label mit ClickHandler oder ein
Custom-Composite ist.

### 2.2 Keywords drücken Outcomes aus, nicht Mechanismen

Robot-Framework-Keywords sind **semantische Aktionen mit Outcome-Vertrag**
(siehe RoboCon-Slides 4–7, 10):

- `Activate` → das UI-Element wurde aktiviert; ein verifizierbares
  Outcome ist eingetreten (Dialog öffnet, Label ändert sich, …).
- `Toggle`, `Check`, `Set Check State` → der Zustand hat sich
  beobachtbar geändert.
- `Select`, `Select Item` → die Selektion hat sich beobachtbar geändert.
- `Set Value`, `Clear`, `Append` → der Wert ist beobachtbar gesetzt.
- `Expand`, `Collapse` → der Expand-Zustand hat sich beobachtbar
  geändert.
- `Focus`, `Scroll Into View` → ein verifizierbares Outcome (Caret
  sichtbar, Element im Viewport).

**`Click` ist kein Standard-Test-Keyword.** Es bleibt verfügbar als
Low-Level-Mausoperation (BareMetal-Niveau) für Sonderfälle, aber Tests
sollen es nicht als Default benutzen — es transportiert keinen
Outcome-Vertrag (Slide 4–6).

### 2.3 Pattern-Vertrag: Pre-Conditions → Perform → Postcondition

Jede semantische Aktion hat denselben Lebenszyklus (Slide 7):

1. **Pre-Conditions** sicherstellen: aktives Top-Level-Window, Element in
   View, Element enabled, … (`ensure_that`, blockierend, raises).
2. **Perform**: die rolle-/element-spezifische Aktivierungslogik
   ausführen (was *exakt* passiert, ist die Sache der Pattern-Implementierung
   — Provider-API-Call, Maus, Tastatur, oder Kombination).
3. **Postcondition**: warten bis das beobachtbare Outcome eingetreten ist
   und die App wieder ready ist (`wait_for` + `ensure_that(...,
   raise_exception=False)`).

**Welcher Mechanismus** im Schritt „Perform" verwendet wird, ist eine
Implementierungsdetail-Entscheidung der konkreten Pattern-Implementierung
für dieses Element — nicht des Tests, nicht des Keywords.

### 2.4 Adapter-Schicht abstrahiert die UI-Quelle

PlatynUI hat eine eigene Adapter-Abstraktion in Python. Sie liefert für
ein UiNode:

- Identität, Beziehungen (parent/children, runtime_id)
- Attribute (Name, Role, Bounds, ToggleState, …)
- Verfügbare Patterns (was kann das Element von Haus aus?)
- Pattern-Aufrufe (`adapter.get_pattern(Activatable).activate()`)

**Heute** ist Rust die wichtigste Adapter-Implementierung (über die
PyO3-Bindings auf `UiNode` + `Runtime`). **Morgen** können andere
Implementierungen daneben stehen — JSON-RPC-Provider, In-Process-Mocks,
Web-/Mobile-Bridges, … Der Adapter ist explizit als
**Erweiterungspunkt** designt, nicht als „Rust-Wrapper".

### 2.5 Batterien inklusive: Standard-UI-Elemente out-of-the-box

PlatynUI liefert für die gängigen UI-Rollen (Button, CheckBox, Edit,
ComboBox, List/Tree/Table, Window, Menu, Tabs, …) fertige UI-Klassen
*und* passende Default-Proxies mit. Ein Test-Projekt definiert nur
Locator/Contexts und nutzt direkt die semantischen Keywords —
eigene Pattern-Implementierungen sind nur für App-/Framework-Spezialfälle
nötig. Details und abgedeckte Rollen: §5a.

### 2.6 Modernes Python: eingesetzte Sprachfeatures und Konventionen

Zielversion ist **Python 3.12+** (3.12–3.13, vgl. `pyproject.toml`).
Der Altcode entstand zu Python-3.0-Zeiten und nutzt überwiegend
klassische Idiome (ABC + `__init__`-Boilerplate, `Optional[X]`,
Decorator-Side-Effects). Für die Neufassung legen wir einen modernen
Mindeststandard fest. Diese Konventionen sind **verbindlich** für neuen
Code und werden in jedem nachfolgenden Abschnitt vorausgesetzt:

**Typsystem & Daten**

- **`abc.ABC` + `@abstractmethod`** für Capability-Interfaces (Patterns,
  Adapter, Devices). Drei Gründe: (a) Implementierungspflicht wird zur
  Instanzierungszeit erzwungen — echter `TypeError`, kein stiller
  Protocol-Miss; (b) `isinstance`-Checks laufen billig über die MRO
  ohne `@runtime_checkable`-Overhead; (c) **Default-Method-Vererbung
  funktioniert verlässlich**: wenn ein Pattern-ABC eine konkrete
  Methode mit Default-Implementierung anbietet (z.B. eine abgeleitete
  Bequemlichkeits-API auf Basis abstrakter Properties), erbt jeder
  nominale Implementierer sie automatisch.
  Strukturelle Protocol-Implementierer würden den Default *nicht*
  bekommen — ein stiller Footgun. `typing.Protocol` setzen wir nur
  punktuell ein, wo strukturelles Typing echten Mehrwert bringt (z.B.
  kleine interne Marker ohne Default-Methoden und ohne
  Implementierungspflicht).
- **`@dataclass(frozen=True, slots=True, kw_only=True)`** als Default
  für alle Value-Objekte (`Settings`, `MatchCriteria`, `EnsureResult`,
  `Locator`-Bestandteile). `frozen` erzwingt Immutabilität, `slots`
  spart Speicher bei vielen UiNodes, `kw_only` macht APIs robust gegen
  Reordering.
- **PEP 604 Union-Syntax**: `X | None` statt `Optional[X]`, `int | str`
  statt `Union[int, str]`. Konsequent durchziehen.
- **`typing.Self`** für Builder-Pattern und Methoden, die `self`-Typ
  zurückgeben (z.B. `Locator.with_role(...)`).
- **`typing.TypeAlias`**: zentrale Aliases für `PatternName`,
  `RoleName`, `FrameworkId`. Ein Punkt zum Ändern,
  semantisch klar. Freie Strings — kein Enum-Zwang, damit
  app-spezifische Rollen problemlos mitgeführt werden
  können.
- **Generics** für typisierte Wrapper (`PatternProxy[P]`,
  `AdapterRef[T]`) — ersetzt `cast()`-Aufrufe an Aufrufstellen.
- **`@overload`** für die Locator-API (`by_role`, `by_xpath`,
  `by_properties`) — saubere Trennung statt Mega-`__init__` mit vielen
  Optionals.

**Kontrollfluss**

- **PEP 634 Structural Pattern Matching (`match`/`case`)** für
  Outcome-/Resolution-Dispatch (Capability-Auflösung,
  Pre-/Post-Conditions, Locator-AST-Traversal). Ersetzt verschachtelte
  `isinstance`-Ketten. (Hinweis: „Pattern" hier im Sinne des
  Python-Sprachfeatures, nicht im Sinne unserer
  Capability-Marker — diese heißen ebenfalls Pattern, sind aber etwas
  ganz anderes.)
- **`functools.cached_property`** für teure Properties am `ContextBase`
  (resolved Adapter, Pattern-Lookup-Cache pro Node-Instanz).
- **`contextlib.contextmanager`** für Wait-Loops, Highlight-Sessions,
  Mouse-Drags. Async-Varianten nur an klar abgegrenzten Punkten — Robot
  Framework ist sync-zentriert.
- **`functools.singledispatchmethod`** wo Polymorphie über Typ statt
  Vererbung ausgedrückt werden soll (z.B. Locator-Bauer aus
  unterschiedlichen Quellen).

**Decorators & Registries**

- **Hybrid-Registrierung**: `__init_subclass__` als deklarativer
  Standardweg, Decorator (`@context`, `@pattern_proxy_for`) als
  additive Variante für mehrere Rollen / Sonderfälle. Beide rufen
  intern dieselbe Registrierungsfunktion. Beispiele:

  ```python
  # Auto-Registrierung: role default = cls.__name__
  class Window(Control):
      ...

  # Deklarativ mit explizitem Kriterium
  class Button(Control, role="Button"):
      ...

  # Decorator (mehrere Rollen oder feinere Kriterien)
  @context(role="Button", framework_id="WPF")
  @context(role="ToggleButton", framework_id="WPF")
  class WpfButton(Button):
      ...

  # Opt-out für abstrakte Zwischenklassen
  class Element(ContextBase, register=False):
      ...
  ```

  `__init_subclass__` registriert jede konkrete Subklasse
  automatisch (Default-`role` = `cls.__name__`), außer das
  Kwarg `register=False` schaltet die Registrierung explizit ab
  oder die Klasse besitzt noch ungebundene `__abstractmethods__`.
  Abstrakte Zwischenklassen (`Element`, `Control`, `DesktopBase`)
  müssen daher `register=False` setzen.

- **`typing.ParamSpec` + `Concatenate`** für die Decorator-Wrapper
  (`@ensure(...)`), damit aufgerufene Keyword-Funktionen ihre
  Typsignatur exakt behalten.
- **PEP 695 Generics-Syntax** (3.12+) für neue Generics:
  `class PatternProxy[P: PatternBase]:` statt
  `class PatternProxy(Generic[P]):`. Type-Aliases mit `type X = …`
  statt `X: TypeAlias = …`. Das macht Bound-Constraints direkt am
  Generic-Parameter sichtbar und spart einen Import.
- **`typing.override`** auf jeder Methode in den Proxy-Hierarchien
  (`ElementProxy` → `ControlProxy` → `ButtonProxy` …), die eine
  geerbte Methode überschreibt. Fängt Signatur-Drift bei
  Refactorings zur Type-Check-Zeit.

**Was wir nicht einführen**

- **Kein Pydantic / kein `attrs`** — Standard-Dataclasses reichen,
  zusätzliche Runtime-Abhängigkeiten ohne Mehrwert.
- **Kein flächendeckendes `async`/`await`** — RF ist sync, nur
  punktuell einführen, wenn ein Adapter es erzwingt.

**Querverweise**: §5 (ABC-basierte Patterns), §5.4 (Convention statt
Registry), §7 (Locator-API mit `@overload`), §10 Phase 1 (konkrete
Anwendungsstellen für Dataclasses, `ParamSpec`, `Self`).

**Konventionen-Spickzettel** (gilt für alle Beispiele in diesem
Dokument):

```python
from dataclasses import dataclass
from typing import ClassVar, Self, TypeAlias

PatternName: TypeAlias = str
RoleName: TypeAlias = str

@dataclass(frozen=True, slots=True, kw_only=True)
class MatchCriteria:
    role: str | None = None
    framework_id: str | None = None
    properties: dict[str, object] | None = None

class Button(Control, role="Button"):       # __init_subclass__-Hybrid
    def with_timeout(self, ms: int) -> Self: ...   # Self statt "Button"
    def activate(self) -> None: ...
```

Vermeide: `Optional[X]` (→ `X | None`), `Union[A, B]` (→ `A | B`),
`@dataclass` ohne `slots=True/frozen=True` für Wertobjekte, generische
`Dict`/`List`-Imports aus `typing` (→ Builtins).

## 3. Architekturüberblick

```
┌─────────────────────────────────────────────────────────────┐
│  Robot Framework Layer                                      │
│  Keywords: Activate / Toggle / Select / Set Value / …       │
└──────────────────────┬──────────────────────────────────────┘
                       │ (semantische Verben)
┌──────────────────────▼──────────────────────────────────────┐
│  UI-Klassen (Contexts)                                  │
│  Button, CheckBox, ListItem, Window, …                       │
│  - registriert via @context                                 │
│  - kennen ihre Pattern-ABCs (Activatable, Toggleable)       │
│  - orchestrieren Pre/Perform/Post                           │
└──────────────────────┬──────────────────────────────────────┘
                       │ adapter.get_pattern(Activatable)
┌──────────────────────▼──────────────────────────────────────┐
│  AdapterProxy-Schicht                                       │
│  - registriert via @pattern_proxy_for                       │
│  - überschreibt/ergänzt Patterns für spezifische UiNodes    │
│  - liefert Pattern-Implementierungen (z.B. Click-basiert,   │
│    Tastatur-basiert, Provider-Pattern-basiert, gemischt)    │
└──────────────────────┬──────────────────────────────────────┘
                       │ get_pattern → fallback
┌──────────────────────▼──────────────────────────────────────┐
│  Adapter-Interface (Python)                                 │
│  - liefert Patterns, die der Provider von Haus aus kann     │
│  - exposes Attribute, Beziehungen, runtime_id               │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┬──────────────┐
        ▼              ▼              ▼              ▼
   Rust-Adapter    JSON-RPC      Mock-Adapter    (zukünftig)
   (UIA, AT-SPI,   (zukünftig)   (Tests)
    macOS AX)
```

Drei Auswahlmechanismen, alle gewichtet (siehe §4):

- **Welche UI-Klasse passt für diesen UiNode?** → `ContextFactory` +
  `@context`.
- **Welcher Proxy passt für diesen UiNode?** → `PatternProxyFactory` +
  `@pattern_proxy_for`.
- **Welche Pattern-Impl liefert das gewünschte Pattern?** →
  `proxy._self_implemented_patterns` zuerst, dann `adapter`
  (additiv). Dadurch kann ein Proxy ein Provider-Pattern überschreiben
  oder ein vom Provider fehlendes Pattern hinzufügen.

## 4. Auswahlmechanismus: WeightCalculator + zwei Registries

Das ist der **Kern der Erweiterbarkeit** und wird 1:1 aus dem Altprojekt
übernommen. Idee: Klassen registrieren sich mit Kriterien-Sets;
zur Laufzeit wird für jedes UiNode das am besten passende Match gewählt.

### 4.1 Kriterien

Jede Registrierung gibt eine Teilmenge dieser Kriterien an:

| Kriterium | Beispiel | Gewicht |
|---|---|---|
| `role` (exakt) | `"Button"` | +10000 (oder reject) |
| `role` (in `supported_roles`) | `"ToggleButton" ∈ {"Button", "ToggleButton"}` | +5000 - i |
| `framework_id` | `"WPF"` | +1000 (oder reject) |
| `class_name` | `"Microsoft.Maui.Controls.Button"` | +500 (oder reject) |
| `tag_name` | (DOM-artig) | +400 (oder reject) |
| `attributes[(ns, name)] == v` | `{("control", "AutomationId"): re.compile("submit-.*")}`, `{("native", "HWND"): 0xABCD}` | +200 pro Match (oder reject) |

Werte können `str`, `re.Pattern` oder beliebige `==`-vergleichbare Werte
sein. Höchstes Gewicht > 0 gewinnt; bei keinem Match: Fallback
(`UnknownContext` bzw. roher Adapter ohne Proxy).

**Attribut-Namensraum.** Schlüssel im `attributes`-Dict sind entweder
ein Tupel `(namespace, name)` oder — als Convenience — ein bloßer
String, der dann als `(<default_namespace>, name)` interpretiert wird.
Der Default-Namespace ist `control` und kann von einer Context-
Klasse über das Klassenattribut `default_attribute_namespace`
umgestellt werden (siehe §A.6 / §7.1). Vier Namespaces sind
kanonisch (1:1 zu `crates/core/src/ui/namespace.rs`):

- `control` — semantische UI-Attribute (Default)
- `item` — Container-Item-Attribute
- `app` — Application-Attribute
- `native` — toolkit-/provider-spezifische Roh-Attribute

### 4.2 Zwei Registries

#### `@context` — UI-Klassen-Registry

Beantwortet: *„Welche `ContextBase`-Subklasse repräsentiert diesen UiNode
aus User-Sicht?"*

```python
class Button(Control, role="Button"):
    def activate(self) -> None: ...

@context(role="Button", framework_id="WPF",
         attributes={"ClassName": re.compile("MyApp\\..*PrimaryButton")})
class MyAppPrimaryButton(Button):
    """Spezialisierung für Primary-Buttons unserer App."""
```

Gewicht entscheidet: `MyAppPrimaryButton` schlägt das generische `Button`,
wenn `framework_id="WPF"` UND ClassName matchen. Die Hybrid-Form
(Klassen-Kwarg `role="Button"` vs. expliziter Decorator) ist in §2.6
beschrieben — Klassen-Kwarg für „eine Klasse, eine Rolle", Decorator
für mehrere Rollen oder feinere Kriterien.

#### `@pattern_proxy_for` — Pattern-Implementierung-Registry

Beantwortet: *„Welche Pattern-Implementierung gilt für diesen
UiNode?"* Ein `AdapterProxy` umhüllt den Adapter und fügt
Pattern-Implementierungen hinzu/überschreibt sie.

```python
@pattern_proxy_for(role="Button")
class ButtonProxy(ControlProxy, patterns.Activatable):
    """Standard-Button-Aktivierung."""
    def activate(self) -> None:
        # Nutzt Provider-Pattern wenn vorhanden, sonst Click — die
        # konkrete Wahl trifft die Implementierung dieses Proxys.
        ...

@pattern_proxy_for(role="Label",
                   attributes={"ClassName": "MyApp.FakeButton"})
class FakeButtonProxy(ControlProxy, patterns.Activatable):
    """Label-mit-ClickHandler bedient sich wie ein Button."""
    def activate(self) -> None:
        AdapterMouseProxy(self.adapter).click()
```

Aus Sicht der UI-Klasse `Button`: sie ruft
`adapter.get_pattern(Activatable).activate()` — der `WeightCalculator`
hat den `FakeButtonProxy` ausgewählt, also läuft die Aktivierung über
dessen Click-Implementierung. **Die `Button`-Klasse weiß nichts vom
„Fake".**

### 4.3 Pattern-Auflösung im Detail

`AdapterProxy.get_pattern(PatternT)` (im Altcode `adapterproxy.py:95`,
dort noch `get_strategy`):

1. Wenn der **Proxy** `PatternT` selbst implementiert (durch Mehrfachvererbung
   von `patterns.Activatable` etc.) → der Proxy ist die Implementierung.
2. Sonst → `self.adapter.get_pattern(PatternT)` — der darunterliegende
   Adapter liefert (wenn er kann).
3. Sonst → `None` bei `raise_exception=False`, sonst
   `PatternNotSupportedError` (siehe §A.2).

`AdapterProxy.supported_patterns` ist die **Vereinigung** aus
Proxy-Patterns und Adapter-Patterns.

**Konsequenz für den Adapter (= Rust):** Was Rust an Patterns liefert,
wird genutzt. Was Rust nicht liefert, kann der Proxy ergänzen. Was Rust
liefert, der Proxy aber „besser" macht, wird durch den Proxy
überschrieben. **Es gibt kein Entweder-Oder zwischen „Provider-Pattern"
und „User-Simulation"** — pro Pattern wird der bestmögliche verfügbare
Weg gewählt, lokal pro UiNode entschieden.

### 4.4 Auflösungs-Reihenfolge bei der Element-Erzeugung

Wenn `parent.get(child_ctx, locator=...)` aufgerufen wird (siehe §A.5):

```
1. Adapter-Quelle aus dem Parent-Context erben.
   Der Parent hält bereits einen Adapter; Children werden aus
   dessen Tree resolved.
2. Locator → adapter_factory.current.find_one(parent_adapter, locator)
   bzw. find_all(...). Die RuntimeAdapterFactory rendert intern
   den XPath (locator.to_xpath(...)) und ruft die Native-Engine
   (runtime.current.evaluate*). Ergebnis: Sequenz roher
   UiNodeAdapter-Refs auf gefundene UiNodes (siehe §A.4b).
3. PatternProxyFactory.find_proxy_for(adapter)
   → wickelt einen passenden Proxy darum (gewichtsbasiert).
4. ContextFactory.find_context_class_for(proxied_adapter)
   → bestimmt die UI-Klasse (gewichtsbasiert).
5. context_type(locator, parent, proxied_adapter)
   → fertige Context-Instanz (z.B. Button-Objekt).
```

Schritt 3 passiert **innerhalb** der Adapter-Auflösung:
`RuntimeAdapterFactory._wrap` ruft `PatternProxyFactory.find_proxy_for(adapter)`
direkt nach `UiNodeAdapter.from_node(...)` auf, bevor das Ergebnis
zurückgegeben wird. Da `AdapterProxy` selbst eine `Adapter`-Subklasse
ist (siehe §A.4), bleibt der Rückgabetyp `Adapter | None` und jeder
Code-Pfad — Locator-Resolution, `ContextBase.get`, `ElementDescriptor`,
strukturelle Navigation über `parent`/`children` — sieht dieselben
Proxies.

**Kein Runtime-Singleton** — der Adapter wird über die Parent-Chain
weitergereicht; das Wurzel-Context-Objekt (`Desktop`, siehe §A.8)
hält den initialen Adapter, von dem aus der gesamte Sub-Tree
resolved wird.

## 5. Patterns als Capability-Marker

Patterns sind `abc.ABC`-Klassen (Basis `PatternBase`) mit
`@abstractmethod`-Methoden. Sie definieren **was** ein Element kann,
nicht **wie**. ABC wurde gegenüber `typing.Protocol` bevorzugt, weil
(a) die Instanzierung einer unvollständigen Implementierung sofort
einen echten Fehler wirft, (b) `isinstance`-Checks billig über die MRO
laufen, und (c) Default-Methoden zuverlässig an Subklassen vererbt
werden — bei `typing.Protocol` würden strukturelle Implementierer
Defaults stillschweigend verlieren.

**Jedes Pattern trägt einen stabilen String-Identifier** (`pattern_name`)
im Reverse-DNS-Format `org.platynui.patterns.<Name>`. Zwei Aspekte
sind zu trennen:

- **Identifier-Pflicht (hart):** Jede Pattern-ABC muss einen
  `pattern_name` setzen. Ohne ihn kann der Adapter das Pattern nicht
  über die Drahtverbindung melden — der Identifier ist Teil des
  öffentlichen Vertrags.
- **Format (Konvention):** Reverse-DNS ist die empfohlene Form,
  aber **nicht erzwungen** — symmetrisch zu Rust-`PatternName`, das
  ebenfalls keine Format-Validierung macht. Third-Party-Patterns
  *sollten* einen eigenen Reverse-DNS-Namespace
  (`com.acme.patterns.*`) verwenden, um Kollisionen zu vermeiden.

Begründung der Identifier-Pflicht:

- Externe Adapter (JSON-RPC, Remote-Provider, fremdsprachliche
  Implementierungen) können keine Python-Klassen-Objekte über die
  Drahtverbindung schicken. Sie reden über `pattern_name`-Strings — z.B.
  `supported_patterns() → ["org.platynui.patterns.Activatable",
  "org.platynui.patterns.Focusable"]` und
  `invoke_pattern("org.platynui.patterns.Activatable", "activate", …)`.
- Rust-Bindings mappen Provider-Patterns ebenfalls über den Identifier
  auf Python-Patterns (Tabelle in `core/adapters/rust.py`).
- Die Auflösung `pattern_name ↔ Pattern-Klasse` erfolgt über
  Adapter-lokale Mapping-Tabellen, nicht über eine globale Registry
  (siehe §5.4).

> **Status Rust-Code:** Der Identifier-Mechanismus existiert
> bereits Rust-seitig: `PatternName` (newtype über `Arc<str>`,
> `crates/core/src/ui/identifiers.rs:82`), `UiPattern::pattern_name()` /
> `UiPattern::static_pattern_name() -> PatternName` als Pflicht-Trait-Methoden
> (`crates/core/src/ui/pattern.rs:18`), `PatternRegistry` mit
> `register`/`get`/`get_typed`/`supported`
> (`crates/core/src/ui/pattern.rs:57`) und `supported_patterns_value`
> serialisiert die IDs als String-Array für die FFI-Grenze
> (`pattern.rs:165`).
>
> **Rust verwendet Reverse-DNS-Identifier** (Rev. 14 als `PatternId` eingeführt,
> Rev. 15 zu `PatternName` umbenannt für Symmetrie mit Python — siehe §13.6, §13.7):
> Sowohl `PatternName::from(pattern_names::FOCUSABLE)` (= `"org.platynui.patterns.Focusable"`)
> als auch Python-`Focusable.pattern_name` liefern denselben String. Die
> Konstanten leben in `core::ui::pattern_names` (`crates/core/src/ui/identifiers.rs`)
> und werden überall statt der bare names verwendet. `PatternName` selbst
> bleibt validierungsfrei (Convention statt Format-Check); die Konsistenz
> wird in den Mapping-Tabellen und beim Python-Import sichergestellt.

Der Identifier wird von `PatternBase` als `ClassVar[str]` deklariert.
Eine `__init_subclass__`-Prüfung gibt es nicht: Eine Pattern-ABC ohne
`pattern_name` würde zur Laufzeit beim ersten Adapter-Aufruf
(`pattern.pattern_name`) einen `AttributeError` werfen — das reicht
als Sicherung, weil neue Pattern-ABCs nur in `core/patterns/` und in
Third-Party-Code definiert werden, beides Code, dessen Tests genau
diesen Pfad triggern.

Beispiele:

```python
# core/patterns/base.py
from abc import ABC
from typing import ClassVar

class PatternBase(ABC):
    """Basisklasse für alle Capability-Marker.

    Jede konkrete Pattern-ABC deklariert einen Reverse-DNS-Identifier
    via `pattern_name`. Das Format ist Konvention, keine Validierung
    (symmetrisch zu Rust-PatternName).
    """
    pattern_name: ClassVar[str]


# core/patterns/activation.py
from abc import abstractmethod
from .base import PatternBase

class Activatable(PatternBase):
    pattern_name = "org.platynui.patterns.Activatable"
    @abstractmethod
    def activate(self) -> None: ...
    @property
    @abstractmethod
    def is_activation_enabled(self) -> bool: ...
    @property
    @abstractmethod
    def default_accelerator(self) -> str | None: ...


# core/patterns/toggle.py
class Toggleable(PatternBase):
    """Toggle-Aktion *und* Toggle-Status in einem Pattern.

    Konsolidiert die alten `Toggleable` und `HasToggleState`-Patterns
    (Rev. 17). Spiegel des Rust-Moduls `attributes::toggleable`.
    """
    pattern_name = "org.platynui.patterns.Toggleable"
    @abstractmethod
    def toggle(self) -> None: ...
    @property
    @abstractmethod
    def state(self) -> "ToggleState": ...
    @property
    @abstractmethod
    def supports_three_state(self) -> bool: ...


# core/patterns/text.py
class TextContent(PatternBase):
    """Read-only Textinhalt eines Elements."""
    pattern_name = "org.platynui.patterns.TextContent"
    @property
    @abstractmethod
    def text(self) -> str: ...
    @property
    @abstractmethod
    def locale(self) -> str | None: ...
    @property
    @abstractmethod
    def is_truncated(self) -> bool: ...


class TextEditable(PatternBase):
    """Editable-Status + Schreib-Operation. Ersetzt den alten
    `EditableText` und `HasIsReadonly` (Rev. 17)."""
    pattern_name = "org.platynui.patterns.TextEditable"
    @abstractmethod
    def set_text(self, value: str) -> None: ...
    @property
    @abstractmethod
    def is_readonly(self) -> bool: ...
    @property
    @abstractmethod
    def max_length(self) -> int | None: ...
    @property
    @abstractmethod
    def supports_password_mode(self) -> bool: ...
    @property
    @abstractmethod
    def is_multi_line(self) -> bool: ...


class Clearable(PatternBase):
    """Eigenständige Clear-Operation (separater Capability-Marker)."""
    pattern_name = "org.platynui.patterns.Clearable"
    @abstractmethod
    def clear(self) -> None: ...
```

**Pattern-Hierarchie bleibt flach.** `TextEditable` erbt nicht
von `TextContent`, obwohl jedes editierbare Feld auch lesbaren
Inhalt hat. Pattern-Resolution läuft über den
`pattern_name`-String im Adapter — Vererbung auf Python-Seite
hätte keinen Verhaltens-Effekt; Adapter müssten beide
Reverse-DNS-Namen ohnehin separat in
`supported_pattern_names()` listen. Diese Konvention gilt
durchgängig (Toggleable nicht von Activatable, Resizable nicht
von Movable, …); Beziehungen zwischen Capabilities werden über
Adapter-Listen, nicht über Klassenhierarchie ausgedrückt.

```python


# core/patterns/element.py
class Element(PatternBase):
    """Konsolidiertes Element-Pattern: Geometrie + Sichtbarkeit +
    Enabled-Status. Ersetzt die alten `HasBounds`, `Visibility` und
    `HasIsEnabled` (Rev. 17). Spiegel des Rust-Moduls
    `attributes::element` (`Bounds`, `IsVisible`, `IsInView`, `IsEnabled`).
    """
    pattern_name = "org.platynui.patterns.Element"
    @property
    @abstractmethod
    def bounds(self) -> Rect: ...
    @property
    @abstractmethod
    def is_visible(self) -> bool: ...
    @property
    @abstractmethod
    def is_in_view(self) -> bool: ...
    @property
    @abstractmethod
    def is_enabled(self) -> bool: ...


# core/patterns/focusable.py
class Focusable(PatternBase):
    """Fokus-Status + Fokus-Aktion (`focus()` ist Python-seitig)."""
    pattern_name = "org.platynui.patterns.Focusable"
    @property
    @abstractmethod
    def is_focused(self) -> bool: ...
    @abstractmethod
    def focus(self) -> None: ...


# Selectable, Expandable, HasEditor, ItemContainer,
# Scrollable, HasNativeWindowHandle, HasValue, EditableValue,
# … — alle als ABC mit pattern_name im
# org.platynui.patterns.*-Namespace.
#
# Hinweis: Selectable und Expandable bündeln Status-Read und
# Aktion in einem Pattern (siehe Rev. 32) — kein paralleles
# `HasIsSelected`/`HasIsExpanded` wie im Altprojekt.
#
# Hinweis: Es gibt KEIN „Properties"-Pattern. Generische Attribut-
# Reads laufen direkt am Adapter über
# `adapter.attribute_value(name, namespace=...)` (§A.4) — symmetrisch
# zu `UiNode.attribute(name, namespace)` in Rust.
```

Die Pattern-Liste folgt **eng der Rust-Capability-Gruppierung** in
`crates/core/src/ui/attributes.rs` und `pattern_names` in
`crates/core/src/ui/identifiers.rs`. Konsolidierungen gegenüber dem
Altprojekt (Rev. 17):

- `HasBounds` + `Visibility` + `HasIsEnabled` → **`Element`** (Rust:
  `attributes::element` mit `Bounds`, `IsVisible`, `IsInView`,
  `IsEnabled`).
- `Toggleable` + `HasToggleState` → **`Toggleable`** (Rust:
  `attributes::toggleable` mit `ToggleState`, `SupportsThreeState`).
- `EditableText` + `HasIsReadonly` → **`TextEditable`** (Rust:
  `attributes::text_editable` mit `IsReadOnly`, `MaxLength`,
  `SupportsPasswordMode`); read-only Inhalt → **`TextContent`** (Rust:
  `attributes::text_content`); Clear-Operation → **`Clearable`** (Rust:
  leeres Modul `attributes::clearable`, da reine Aktion ohne Attribute).
- `Activatable` ist um `is_activation_enabled` und
  `default_accelerator` erweitert (Rust: `attributes::activatable`).
- `Focusable` bleibt eigenständig; `HasFocus` entfällt.
- `Point` und `Rect` werden aus `platynui_native` re-exportiert
  (kanonische pyo3-Bindings; `Rect.center()` ist eine Methode, keine
  Property).

### 5.1 Wer implementiert Patterns?

Drei Quellen, in dieser Auflösungsreihenfolge:

1. **Proxy** (`AdapterProxy`-Subklasse via `@pattern_proxy_for`) — die
   Hauptquelle für Standard- und Sonderfälle. Hier liegt die rolle- bzw.
   element-spezifische Logik.
2. **Adapter** (= Provider-Schicht, heute: Rust-Bindings) — wenn der
   Provider direkt eine Pattern-Implementierung liefert, kann der Adapter
   sie als Pattern exposen. Beispiel heute: `Focusable` über
   `FocusablePattern`, Window-Operationen über `WindowSurfacePattern`.
3. **Default-Implementierungen** (im Altcode: `core/strategyimpl.py`,
   neu: `core/patterns/defaults.py`) — generische Implementierungen, die
   nur Adapter-Basics brauchen (`bounding_rectangle`, Maus/Tastatur).
   Greift, wenn weder ein Proxy noch der Adapter eine spezifische
   Implementierung anbietet.

### 5.2 Was passiert konkret in einer Standard-Proxy-Methode?

Aus `ui/proxies/standardproxies.py` im Altcode (vereinfacht, auf neue
Begriffe übertragen):

```python
@pattern_proxy_for(role="Button")
class ButtonProxy(ControlProxy, patterns.Activatable):
    def activate(self) -> None:
        # Heuristik: wenn der Adapter ein "natives" Activatable liefert,
        # nutze das (z.B. bei AT-SPI Action-Interface). Sonst Click.
        native = self.adapter.get_pattern(patterns.Activatable,
                                          raise_exception=False)
        if native is not None and native is not self:
            native.activate()
            return
        AdapterMouseProxy(self.adapter).click()
```

(Der Altcode ist stellenweise einfacher und ruft direkt `click()`. Beide
Varianten sind legitim — die Klasse entscheidet pro Rolle, was der
robusteste Weg ist.)

### 5.3 Verifikation gehört in die UI-Klasse

Die Proxy-Methode führt nur die **Aktion** aus. Die **Verifikation des
Outcomes** ist Sache der UI-Klasse, weil sie weiß, *was* aus User-Sicht
das verifizierbare Outcome ist:

```python
class Button(Control, role="Button"):
    def activate(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class CheckBox(Button, role="CheckBox"):
    def set_state(self, target: ToggleState) -> None:
        for _ in ToggleState:
            toggleable = self.adapter.get_pattern(patterns.Toggleable)
            current = toggleable.state
            if current == target:
                return
            toggleable.toggle()
            wait_for(lambda last=current:
                     self.adapter.get_pattern(patterns.Toggleable).state != last)
        raise CannotEnsureError(f"cannot set checkbox to {target}")
```

### 5.4 Namensauflösung: Convention statt globaler Registry

Patterns tragen ihren Reverse-DNS-Identifier als `ClassVar[str]`. Das
reicht als Vertrag zwischen Adapter und UI-Schicht — eine globale
`PatternRegistry` ist **nicht** nötig:

- **Rust-Adapter** (`core/adapters/rust.py`) hält eine lokale
  Mapping-Tabelle `PatternName-String → Python-ABC`:

  ```python
  # core/adapters/rust.py
  _RUST_PATTERN_MAP: dict[str, type[PatternBase]] = {
      Focusable.pattern_name: Focusable,
      WindowSurface.pattern_name: WindowSurface,
      Element.pattern_name: Element,
      # …
  }
  ```

- **JSON-RPC-Adapter** (zukünftig) hält seine eigene Tabelle. Die
  Patterns, die er versteht, sind Teil seines Vertrags.
- **`adapter.supported_patterns()`** liefert `list[type[PatternBase]]`
  direkt — kein Lookup-Umweg.
- **`adapter.get_pattern(Activatable)`** nimmt die Python-Klasse als
  Schlüssel; die Übersetzung auf den Draht-Identifier passiert im
  Adapter.

**Vorteile dieser Entscheidung:**

- Keine Import-Side-Effects, keine globale Mutable State.
- Jeder Adapter ist explizit darüber, was er unterstützt.
- Third-Party-Patterns funktionieren per Convention: ABC mit
  `pattern_name` deklarieren, Adapter darauf vorbereiten. Kein Hook
  nötig.
- `__init_subclass__` in `PatternBase` prüft nur, dass konkrete
  Subklassen einen `pattern_name` gesetzt haben — reine
  Missbrauchssicherung, keine Registrierung.

**Verwendung am Adapter (Beispiele):**

- `adapter.get_pattern(Activatable)` — typisierter Lookup mit der
  Python-Klasse als Schlüssel; Adapter-intern wird daraus (sofern nötig)
  ein `pattern_name`-String, um an die konkrete Implementierung zu
  kommen. Dies ist der Standardweg für Python-Code.
- `adapter.get_pattern_by_name("org.platynui.patterns.Activatable")` —
  string-basierter Lookup. Wird benötigt, wenn der Aufrufer die
  Python-Klasse nicht importieren kann (externe Adapter, Debug-
  Werkzeuge, RPC-Bridges).
- `adapter.supported_patterns()` — liefert `set[type[PatternBase]]` für
  alle Patterns, die der Adapter zur Python-Klasse auflösen kann. Eine
  zusätzliche `supported_pattern_names() → set[str]` deckt auch externe
  Patterns ab, deren Python-ABC dem Adapter (noch) nicht bekannt ist.
- **`UiNodeAdapter`** übersetzt die `PatternName`-Strings, die
  `UiNode.supported_patterns()` aus dem Rust-Code liefert, über seine
  lokale Builder-Tabelle (`_NATIVE_PATTERN_BUILDERS: dict[str,
  Callable[[UiNodeAdapter], PatternBase | None]]`) in Python-Pattern-
  Implementierungen. Builder werden gewählt, weil das Wrapping pro
  Pattern leicht unterschiedlich ist (z. B. `_NativeFocusable` braucht
  Adapter + native `Focusable`, ein zukünftiger `_NativeElement` würde
  nur den Adapter brauchen). Eine Erweiterung um ein neues Pattern ist
  ein einzelner Eintrag in der Tabelle. Die Pattern-Id-Strings sind
  bereits im Reverse-DNS-Format (`org.platynui.patterns.*`), also
  identisch zwischen Rust- und Python-Seite.

## 5a. Mitgelieferte Standard-UI-Elemente (Batterien inklusive)

**Zentrale Designentscheidung:** PlatynUI liefert für die gängigen
UI-Rollen fertige **UI-Klassen *und* passende Default-Proxies**
out-of-the-box. Ein Test-Autor schreibt nur Locator/Contexts — für
Standardfälle ist *keine eigene Implementierung* von Pattern-Proxies
oder UI-Klassen nötig.

### 5a.1 Was "fertig" heißt

Für jede Standardrolle sind zwei Dinge registriert:

- eine UI-Klasse mit `@context(role=...)` in `ui/` (Context-Ebene,
  orchestriert Pre/Perform/Post, exposed Methoden wie `activate()`,
  `set_state()`, …),
- ein Default-Proxy mit `@pattern_proxy_for(role=...)` in
  `ui/proxies/` (liefert Pattern-Implementierungen: Click-basiert,
  Tastatur-basiert, Provider-Pattern-basiert — je nach Rolle).

Beide sind die **generische Fallback-Stufe** im Gewichtungs-Match. Ein
User-Projekt kann sie jederzeit durch spezifischere Registrierungen
(höheres Gewicht über `framework_id`, `class_name`, `properties`)
überschreiben, ohne die Defaults zu entfernen.

### 5a.2 Abgedeckte Standard-Rollen

Minimal-Set, das mit v1 ausgeliefert werden soll (Rollen entsprechen
`control:*` im XPath-Namespace):

| Rolle | UI-Klasse | Default-Proxy bedient Patterns |
|---|---|---|
| `Desktop` / `Application` / `Window` / `Dialog` / `Frame` | `Desktop`, `Application`, `Window`, `Dialog` | `WindowSurface`-Patterns (über Rust), `Activatable` |
| `Button`, `Link` | `Button` | `Activatable` |
| `CheckBox`, `RadioButton`, `ToggleButton` | `CheckBox`, `RadioButton`, `ToggleButton` | `Toggleable` |
| `Edit`, `Text`, `PasswordBox` | `Edit`, `Text` | `TextContent`, `TextEditable`, `Clearable`, `HasValue` |
| `ComboBox` | `ComboBox` | `Expandable`, `Selectable`, `TextContent`, `TextEditable` (editierbar) |
| `List`, `ListItem` | `List`, `ListItem` | `Selectable`, `ItemContainer`, `Scrollable` |
| `Tree`, `TreeItem` | `Tree`, `TreeItem` | `Expandable`, `Selectable`, `ItemContainer`, `Scrollable` |
| `Table`, `Row`, `Cell`, `Header` | `Table`, `Row`, `Cell` (+ `EditableCell`) | `Selectable`, `ItemContainer`, `HasEditor`, `Scrollable` |
| `TabList`, `TabItem` | `TabList`, `TabItem` | `Selectable` |
| `Menu`, `MenuBar`, `MenuItem` | `Menu`, `MenuBar`, `MenuItem` | `Activatable`, `Expandable` |
| `Label`, `StaticText`, `Image` | `Label`, `Image` | (lesend — kein Action-Pattern) |
| `ScrollBar`, `Slider`, `Spinner`, `ProgressBar` | `Slider`, `Spinner`, `ProgressBar` | `HasValue`, `EditableValue` |

Die Liste ist bewusst am Altprojekt (`ui/proxies/standardproxies.py`,
~400 LOC) orientiert — das dortige Set hat sich in der Praxis bewährt.

### 5a.3 Konsequenzen

- **Ein neues Test-Projekt ist produktiv, ohne eine einzige Zeile
  Proxy-Code zu schreiben.** Der User definiert Locator/Contexts und
  ruft Keywords auf — Standard-Rollen funktionieren.
- **Framework-Spezialfälle überschreiben gezielt.** Erst wenn eine
  konkrete App/ein Framework sich anders verhält (z.B. WPF-CustomControl
  mit eigener Aktivierungslogik), registriert der User einen
  spezifischeren `@pattern_proxy_for(..., framework_id="WPF",
  class_name="...")` — der Gewichtungs-Algorithmus wählt ihn automatisch
  über den Default.
- **Fake-Button-Szenario (siehe §12)** ist genau so umgesetzt: der
  Default-Proxy für `role="Button"` bleibt unberührt; zusätzlich wird
  ein `@pattern_proxy_for(role="Label", attributes={...})` registriert,
  der im Match höheres Gewicht bekommt.
- **Drei-Ebenen-Fallback pro Pattern** (von spezifisch nach generisch):
  1. App/Framework-spezifischer Proxy (User-Registrierung)
  2. Default-Proxy für die Standard-Rolle (PlatynUI)
  3. Adapter-Pattern (Provider-nativ) oder generische
     Pattern-Defaults in `core/patterns/defaults.py`
     (Click-basiertes `Activatable`, Tastatur-basiertes `TextEditable`,
     …) — greifen, wenn keine der oberen Ebenen etwas liefert.

## 6. Was Rust beitragen kann (und was nicht)

### 6.1 Was Rust heute liefert

- **UiNode-Tree** (Provider-Bäume in einer einheitlichen Struktur)
- **Attribute** (Name, Role, Bounds, IsEnabled, IsVisible, Value,
  ToggleState, IsExpanded, IsSelected, …) — über
  `ui_node.attribute(name, namespace)`
- **XPath 2.0** — für Locator-Auflösung
- **`FocusablePattern`** — Provider-natives `focus()`
- **`WindowSurfacePattern`** — Window-Manager-API für activate/min/max/
  restore/close/move/resize (zuverlässiger als Maussimulation)
- **Pointer-/Keyboard-Devices** — niedrige Maus/Tastatur-Primitiven, die
  die Proxies nutzen können

### 6.2 Was Rust *zusätzlich* liefern könnte (optional)

Weitere `UiPattern`-Traits sind sinnvoll, wenn der Provider die Operation
zuverlässiger umsetzen kann als eine User-Simulation, oder wenn die
User-Simulation einen unverhältnismäßig hohen Aufwand hätte. Kandidaten:

- Action-Interfaces (UIA `InvokePattern`, AT-SPI `Action`,
  AX `kAXPressAction`) — für Elemente, deren Mausaktivierung schwer zu
  treffen ist (winzige Hit-Boxes, virtualisierte Listen, …)
- `ScrollPattern` — programmatisches Scrollen ist oft robuster als
  Wheel-Events
- `ValuePattern` (SetValue) — für Locale-/IME-unabhängiges Setzen langer
  Werte

**Alle diese sind Erweiterungen, keine Voraussetzungen.** Die
Python-Schicht funktioniert mit oder ohne sie — der jeweilige Proxy
nutzt das, was verfügbar ist (siehe §5.2). Wir bauen sie, wenn ein
konkreter Use-Case-Schmerz das rechtfertigt, nicht prophylaktisch.

### 6.3 Was Rust *nicht* übernimmt

- Die Pattern-Auflösung (welcher Proxy/welche UI-Klasse für welches
  UiNode?) — das bleibt Python.
- Die Outcome-Verifikation (`ensure_that`/`wait_for`) — das bleibt
  Python.
- Das Context-Modell — das bleibt Python.

### 6.4 Mehrere Adapter-Implementierungen — Designeintragsplatz

Der `Adapter`-Begriff in Python ist explizit **nicht** an Rust gebunden.
Eine zukünftige `JsonRpcAdapter`-Implementierung würde:

- `Adapter`-Interface implementieren (Identität, Attribute, Patterns,
  Beziehungen)
- eine eigene `AdapterFactory`-Implementierung mitbringen (siehe
  §A.4b) und sie über `adapter_factory.use_factory(...)` als
  Default einhängen oder via `adapter_factory.override(...)` scope-
  gebunden aktivieren — die Default-`RuntimeAdapterFactory` setzt
  einen `UiNodeAdapter` mit `native_node`-Property voraus und passt
  daher nicht für Adapter ohne Native-Backing

**Die `pattern_name`-Identifier (§5) sind der Schlüssel dazu.** Externe
Adapter können keine Python-Klassen-Objekte über die Drahtverbindung
tragen — sie reden über Strings. Ein typisches RPC-Protokoll:

```
C → S: { "op": "supported_patterns", "node": "<runtime_id>" }
S → C: { "patterns": ["org.platynui.patterns.Activatable",
                      "org.platynui.patterns.HasValue",
                      "org.platynui.patterns.Focusable"] }

C → S: { "op": "invoke", "node": "<runtime_id>",
         "pattern": "org.platynui.patterns.Activatable",
         "method": "activate", "args": [] }
S → C: { "ok": true }
```

Der `JsonRpcAdapter` auf Client-Seite hält seine eigene
Mapping-Tabelle `pattern_name → PatternBase`, um die Python-Klasse zur
Liste hinzuzufügen, und erzeugt für jede
gemeldete Capability einen dünnen Proxy, der `get_pattern(X)` in einen
`invoke`-Call übersetzt. Server-Seite ist symmetrisch: eingehendes
`pattern`-Feld landet über dieselbe Registry bei der lokalen
Implementierung.

Das System mischt sauber: ein Test kann Rust-basierte Adapter (für
Desktop-Apps) und JSON-RPC-Adapter (für Remote-Backend) parallel im
selben Lauf nutzen — beide sprechen dieselben `pattern_name`-Strings.

## 7. Locator + Contexts

### 7.1 Locator als Beschreibung

Der Locator beschreibt, **wie** ein UiNode gefunden wird. Implementiert
als XPath-Builder, der auf der Rust-XPath-2.0-Engine aufsetzt. Vereinfacht
gegenüber dem Altprojekt (433 → ~100 LOC), weil die XPath-Engine das
schwere Heben übernimmt.

```python
@locator(name="Rechner")
class CalculatorWindow(Window):
    @property
    @locator(AutomationId="num5Button")
    def n5(self) -> Button: ...

    @property
    @locator(AutomationId="equalButton")
    def equal(self) -> Button: ...
```

`@locator` baut intern XPath. Es gibt **keine Pattern-Override-Funktion am
Locator selbst** — das wäre konzeptionell falsch verortet, weil derselbe
UiNode dann je nach Locator unterschiedlich behandelt würde. Pattern-
Auswahl gehört in die `@pattern_proxy_for`-Registry (siehe §4), wo sie
auf Eigenschaften des UiNodes selbst basiert.

> **Attribut-Namenskonvention.** Es gibt **drei** Eingangskanäle für
> Attribut-Predicates am `Locator`. Sie können frei gemischt werden,
> aber dasselbe Attribut darf nur über **genau einen** Kanal gesetzt
> werden — Konflikte werfen `TypeError` (kein stilles Vorrang-Verhalten).
>
> 1. **Sechs verdrahtete Convenience-Felder** (typisierte Parameter):
>    `name`, `id`, `class_name`, `role`, `runtime_id`, `framework_id`.
>    Diese sind **snake_case** (Python-Identifier-Konvention) und
>    werden vom Framework auf ihre PascalCase-XPath-Form gemappt
>    (`Locator(role="Button", name="Hallo")` → `Button[@Name="Hallo"]`,
>    `class_name=` → `@ClassName=`, `runtime_id=` → `@RuntimeId=`,
>    `framework_id=` → `@FrameworkId=`). Das Mapping ist abgeschlossen
>    und nicht erweiterbar — es deckt nur diese sechs hochfrequenten
>    Felder ab. Implementierung: `Locator._standard_attributes()`.
>
> 2. **Freie Kwargs am Konstruktor / Decorator**: Jeder Kwarg, der
>    *kein* reserviertes Locator-Feld ist (also nicht in
>    `Locator.RESERVED_FIELDS`), wird als Attribut interpretiert. Der
>    Kwarg-Name geht **wörtlich** in den XPath, ohne jede
>    Case-Konvertierung — per Konvention ist er PascalCase:
>    `Locator(AutomationId="x")` → `[@AutomationId="x"]`,
>    `@locator(IsEnabled="true")` → `[@IsEnabled="true"]`. Für
>    Attribute außerhalb des `control`-Default-Namespace gilt der
>    Doppelunterstrich-Trenner: `Locator(native__HWND=0xABCD)` →
>    `[@native:HWND="..."]`. Mehrere `__` im Kwarg-Namen sind nicht
>    erlaubt — komplexere Schlüssel gehen über das `attributes`-Dict.
>
> 3. **Freies `attributes`-Dict** (für programmatisch konstruierte
>    Schlüssel oder Attribute, deren Name kein Python-Identifier ist):
>    Schlüssel werden wörtlich in den XPath übernommen. Bare String =
>    Default-Namespace, Tupel `(namespace, name)` = explizit:
>    `attributes={"AutomationId": "x"}` → `[@AutomationId="x"]`,
>    `attributes={("native", "HWND"): 0xABCD}` → `[@native:HWND="..."]`.
>
> Begründung der bewussten Asymmetrie zwischen (1) und (2)/(3):
> dataclass-artige Felder *müssen* Python-Identifier sein und folgen
> damit PEP 8 (snake_case); für Attribut-Schlüssel wäre eine
> heuristische snake_case→PascalCase-Brücke unzuverlässig, weil
> Acronym-Sonderfälle (`HWND`, `OS`, `URL`, `XPath`) jede Heuristik
> brechen. Identitäts-Mapping mit explizitem User-Wording ist
> verlässlicher als „cleveres" Auto-Casing.
>
> **Empfehlungs-Reihenfolge:** Convenience-Feld vor Kwarg vor Dict.
> Den Dict-Weg primär für dynamisch konstruierte Schlüssel oder
> Cross-Namespace-Tupel verwenden.
>
> **Namensraum.** Bare Attribut-Namen (sowohl Kwargs ohne `__` als
> auch String-Schlüssel im Dict) werden im Default-Namespace der
> Context-Klasse aufgelöst — standardmäßig `control`. Eine Klasse
> kann das per Klassenattribut umstellen, z.B.
> `class ListItem(Item): default_attribute_namespace = "item"`. Für
> explizite Cross-Namespace-Attribute nutzt das `attributes`-Dict
> Tupel-Keys oder der Kwarg den `__`-Trenner. Siehe §A.6.

### 7.2 Contexts via `@context`

UI-Klassen sind Context-Bausteine. `@context` registriert sie für
Auflösung über `ContextFactory` (siehe §4.2). Vollständige
`ContextBase`-API und Convenience-Properties (`bounding_rectangle`,
`is_visible`, …) siehe §A.5.

## 8. Robot-Framework-Keywords

Keywords drücken Outcomes aus. Liste der Standard-Keywords (siehe
RoboCon-Slide 10):

| Keyword | Pattern(s) | Outcome |
|---|---|---|
| `Activate` | `Activatable` (+ App-ready) | Aktivierung erfolgt + verifiziert |
| `Focus` | `Focusable` | Element hat Fokus |
| `Toggle` | `Toggleable` | Toggle-State hat sich geändert |
| `Check` / `Uncheck` / `Set Check State` | `Toggleable` | Ziel-State erreicht |
| `Select` / `Deselect` / `Select Item` | `Selectable` + `HasIsSelected` | Selektion verifiziert |
| `Expand` / `Collapse` | `Expandable` + `HasIsExpanded` | Expand-State erreicht |
| `Set Value` / `Append` | `TextEditable` + `HasValue` | Wert verifiziert |
| `Clear` | `Clearable` | Wert ist leer |
| `Scroll Into View` | `Scrollable` + `IsInView`-Check | Element im Viewport |
| `Activate Window` / `Maximize Window` / `Minimize Window` / `Close Window` | `WindowSurface`-Patterns | Window-State verifiziert |
| `Get Attribute` / `Get Attributes` | direkter Attribut-Read am Adapter (`adapter.attribute_value(name, namespace=...)`) | Wert |
| `Wait Until Exists` / `Wait Until Gone` | Locator + `wait_for` | Existenz |

Implementierung: jedes Keyword nimmt ein UI-Element-Argument (typisiert
über `ElementDescriptor[PatternT]`) und ruft die entsprechende Methode
der UI-Klasse auf.

```python
# keywords/activate.py
@keyword
def activate(element: ElementDescriptor[Activatable]) -> None:
    element().activate()
```

Der Robot-Converter prüft beim Argument-Parsing, dass das übergebene
Element das `Activatable`-Pattern unterstützt — fehlt es, wird der
Aufruf bereits *vor* der Keyword-Ausführung abgelehnt. Details zum
Parsing, zum Pattern-Check und zur Fehlerbehandlung siehe §A.7.

## 9. Vorschlag für die Zielstruktur

Konventionen für `__init__.py`-Dateien:

- **`PlatynUI/__init__.py`** — Robot-Framework-Library-Klasse, die alle
  Keywords aus `keywords/*.py` einsammelt und exportiert. Zusätzlich
  Re-Export der wichtigsten Context-Symbole (`Button`, `Window`,
  `@locator`, …) für direkten Python-Import.
- **`core/__init__.py`** — Re-Export der öffentlichen API
  (`Adapter`, `AdapterProxy`, `@pattern_proxy_for`, `@context`,
  `ContextBase`, `ensure_that`, `wait_for`, …).
- **`core/patterns/__init__.py`** — sammelt alle Pattern-ABCs aus den
  Submodulen, sodass `from PlatynUI.core.patterns import Activatable`
  ohne Wissen um die Datei-Aufteilung funktioniert.
- **`ui/__init__.py`** und **`ui/proxies/__init__.py`** — Aggregate
  Re-Exports. Beide importieren ihre Submodule mit Side-Effect, damit
  die `@context`/`@pattern_proxy_for`-Registrierungen beim
  Library-Import passieren.
- **`keywords/__init__.py`** — Sammelpunkt für die Robot-Library.

```
src/PlatynUI/
├── __init__.py                     # PlatynUI Robot Library (high-level)
├── __version__.py
├── _assertable.py                  # ✅ bleibt
├── _our_libcore.py                 # ✅ bleibt
│
├── BareMetal/                      # ✅ bleibt als Low-Level-Library
│   └── __init__.py
│
├── core/                           # NEU — Infrastruktur
│   ├── __init__.py
│   ├── adapter.py                  # Adapter-Interface (~150 LOC)
│   ├── adapter_proxy.py            # AdapterProxy + PatternProxyFactory + @pattern_proxy_for (~170 LOC)
│   ├── adapter_factory.py          # AdapterFactory (ABC) + RuntimeAdapterFactory + Singleton (~150 LOC)
│   ├── context.py                  # ContextBase + ContextFactory + @context (~250 LOC)
│   ├── weight_calculator.py        # 1:1 aus alt (~115 LOC)
│   ├── locator.py                  # Locator, @locator, LocatorScope (~120 LOC)
│   ├── descriptor.py               # ElementDescriptor[PatternT]
│   ├── ensure.py                   # ensure_that, @predicate (~50 LOC)
│   ├── wait.py                     # wait_for (~40 LOC)
│   ├── settings.py                 # Settings dataclass (~70 LOC)
│   ├── exceptions.py               # Exception-Hierarchie
│   ├── types.py                    # TypeAliases (PatternName, RoleName, FrameworkId, …)
│   ├── patterns/                   # Pattern-Interfaces (im Altprojekt: strategies/)
│   │   ├── __init__.py             # re-exports
│   │   ├── base.py                 # PatternBase (ABC) + pattern_name-Check
│   │   ├── activation.py           # Activatable
│   │   ├── toggle.py               # Toggleable (toggle + state + supports_three_state)
│   │   ├── text.py                 # TextContent, TextEditable, Clearable, HasValue
│   │   ├── element.py              # Element (bounds + visibility + enabled + click_position)
│   │   ├── focusable.py            # Focusable (is_focused + focus())
│   │   ├── expand.py               # Expandable, HasIsExpanded
│   │   ├── selection.py            # Selectable, HasIsSelected, …
│   │   ├── window.py               # WindowSurface (Activate/Close/Min/Max als Methoden)
│   │   ├── defaults.py             # Default-Implementierungen (alt: strategyimpl.py)
│   │   └── …
│   ├── devices.py                  # MouseProxy/KeyboardProxy (Wrapper über platynui_native.Runtime)
│   └── adapters/                   # Adapter-Implementierung(en)
│       ├── __init__.py
│       └── ui_node.py              # UiNodeAdapter, wraps platynui_native.UiNode
│
├── ui/                             # UI-Klassen + Standard-Proxies
│   ├── __init__.py
│   ├── element.py                  # Element (Base-UI-Klasse)
│   ├── control.py                  # Control
│   ├── window.py                   # Window, Frame, Dialog
│   ├── buttons.py                  # Button, CheckBox, RadioButton, ToggleButton, Link
│   ├── text.py                     # Text, Edit, Label
│   ├── lists.py                    # List, ListItem
│   ├── tree.py                     # Tree, TreeItem
│   ├── table.py                    # Table, Row, Cell, Header
│   ├── tabs.py                     # TabList, TabItem
│   ├── menus.py                    # Menu, MenuBar, MenuItem
│   ├── combobox.py
│   ├── desktop.py                  # Desktop (Root)
│   ├── application.py              # Application
│   └── proxies/                    # Standard-AdapterProxies pro Rolle
│       ├── __init__.py
│       ├── base.py                 # ControlProxy, ContainerProxy, ItemProxy
│       ├── standard.py             # Button/CheckBox/Menu/… Proxies (~400 LOC, port aus altem standardproxies.py)
│       ├── text.py                 # Text/Edit/ComboBox-Proxies
│       ├── list_tree.py            # List/Tree-Item-Proxies
│       └── window.py               # WindowProxy (nutzt WindowSurfacePattern)
│
└── keywords/                       # Robot-Framework-Keywords
    ├── __init__.py
    ├── activate.py                 # Activate, Focus
    ├── toggle.py                   # Toggle, Check, Uncheck, Set Check State
    ├── select.py                   # Select, Deselect, Select Item
    ├── text.py                     # Set Value, Clear, Append, Get Value
    ├── expand.py                   # Expand, Collapse
    ├── scroll.py                   # Scroll Into View, Scroll
    ├── window.py                   # Activate/Close/Min/Max Window
    ├── properties.py               # Get Attribute, Get Attributes
    ├── wait.py                     # Wait Until Exists / Gone
    └── application.py              # Start/Close Application
```

## 9a. API-Spezifikationen

Dieser Abschnitt füllt die Detail-Verträge, auf die §§ 2–9 verweisen.
Reihenfolge so gewählt, dass spätere Abschnitte auf frühere aufbauen
(Settings → Exceptions → ensure/wait → Adapter → ContextBase → Locator
→ Descriptor → Lifecycle → Devices → Defaults → Mock → Highlight).

### A.1 Settings (`core/settings.py`)

`Settings` ist eine `@dataclass(frozen=True, slots=True, kw_only=True)`
mit prozessweitem Singleton-Zugriff über `Settings.current()` und einem
`with`-Block für lokale Overrides. Felder (1:1 aus Altcode `core/settings.py:5`,
nur Defaults konsolidiert):

```python
@dataclass(frozen=True, slots=True, kw_only=True)
class Settings:
    # Wartezeiten
    wait_for_timeout: float = 1.0
    wait_for_delay: float = 0.1
    ensure_timeout: float = 15.0
    ensure_delay: float = 0.1
    exists_timeout: float = 1.0
    window_close_timeout: float = 1.0
    # Tastatur
    input_after_input_delay: float = 0.001
    keyboard_after_press_key_delay: float = 0.01
    keyboard_after_release_key_delay: float = 0.01
    keyboard_after_press_release_delay: float = 0.05
    # Maus
    mouse_before_next_click_delay_multiplicator: float = 1.5
    mouse_after_click_delay: float = 0.010
    mouse_multi_click_delay_multiplicator: float = 0.5
    mouse_press_release_delay: float = 0.010
    mouse_after_move_delay: float = 0.010
    mouse_move_delay: float = 0.001
    mouse_move_time: float = 0.2
    # Display / Diagnose
    display_screenshot_format: str = "png"
    display_screenshot_quality: int = -1
    display_screenshot_basename: str = "screenshot"
    element_highlight_time: float = 2.0
    element_highlight_ensure_timeout: float = 2.0
```

**Konfiguration:**

- **Programmatisch (default):** `Settings.set_current(Settings(ensure_timeout=30))`
  ersetzt das Singleton. Der Setter ist explizit, um „magisches Mutieren"
  einer eingefrorenen Dataclass zu vermeiden.
- **Mit-Block (skoped):** `with Settings(ensure_timeout=30): ...`
  pusht/popt das Singleton; nestable, automatisch restauriert.
- **Robot-Variablen:** Eine schmale Brücke
  (`PlatynUI/keywords/settings.py`, neu) liest beim Library-Init
  `${PLATYNUI_ENSURE_TIMEOUT}` etc. aus dem RF-Variablenscope und ruft
  `Settings.set_current(...)`. Dadurch sind Settings pro Suite/Test
  konfigurierbar, ohne Python-Code zu schreiben.
- **Kein Env-Var-Loading.** Konsequent: der Altcode hat es nicht, der
  RF-Weg ist standardisiert, und Env-Var-Loading vermischt sich
  schlecht mit dem `with`-Block.

**Scope:** prozessglobal, **nicht thread-safe** (wie im Altcode). Für
`pabot` reicht das, weil jeder Worker-Prozess eigenen State hat.

### A.2 Exception-Hierarchie (`core/exceptions.py`)

```text
PlatynUIError                         # Basis (alt: PlatyUiError, Typo gefixed)
├── PlatynUIFatalError                # nicht-recoverable; ensure/wait re-raisen sofort
│   └── AdapterNotFoundFatalError
├── AdapterError                      # Basis für Adapter-Probleme
│   ├── AdapterNotValidError          # Adapter-Lifetime abgelaufen
│   ├── AdapterNotFoundError          # Locator hat nichts gefunden (recoverable)
│   ├── PatternNotSupportedError      # alt: AdapterNotSupportsStrategyError
│   └── NotAPatternTypeError          # alt: NotAStrategyTypeError
├── EnsureError
│   └── CannotEnsureError             # ensure_that-Timeout
├── LocatorError
│   ├── NoLocatorDefinedError
│   ├── MultipleElementsFoundError    # neu: get_one mit mehreren Treffern
│   └── ElementNotFoundError          # Alias für AdapterNotFoundError im Element-Kontext
└── DeviceError
    ├── NoMouseDeviceError            # alt: NoMouseProxyError
    ├── NoKeyboardDeviceError         # alt: NoKeyboardProxyError
    └── NoDisplayDeviceError          # alt: NoDisplayProxyError

# Außerhalb der Hierarchie (Standard-Python-Erwartungen):
NotSupportedError(NotImplementedError)
InvalidArgumentError(ValueError)
```

`Ensure.that()` und `wait_for()` lassen `PlatynUIFatalError`,
`KeyboardInterrupt`, `SystemExit` ohne Retry durch.

### A.3 `ensure_that` / `wait_for` (`core/ensure.py`, `core/wait.py`)

Beide sind die **Verifikations-Primitive** der Outcome-Vertrags-Schicht
(siehe §2.3).

```python
# core/wait.py — schlanker Polling-Helper, kein Caching, kein Raise
def wait_for(
    *predicates: Callable[[], bool],
    timeout: float | None = None,    # default Settings.wait_for_timeout (1.0)
    delay: float | None = None,      # default Settings.wait_for_delay (0.1)
    invalidate: Callable[[], None] | None = None,
) -> bool:
    ...

# core/ensure.py — kraftvoller Verifikations-Driver mit Retry, Stage-Memo, Hooks
def ensure_that(
    context: object,
    *predicates: Callable[[], bool],
    timeout: float | None = None,        # default Settings.ensure_timeout (15.0)
    raise_exception: bool | None = None, # default True
    failed_func: Callable[[], None] | None = None,  # i.d.R. context.invalidate
) -> bool:
    ...
```

**`ensure_that`-Verhalten (vereinfacht ggü. Altcode `core/ensure.py:32`):**

1. **Stage-Memo:** Predicates, die bereits einmal `True` lieferten,
   werden in dieser `ensure_that`-Invocation übersprungen. Sobald
   irgendein Predicate aktuell `False` liefert, wird das Memo zurück­
   gesetzt — alle Pre-Conditions müssen *gleichzeitig* gelten.
2. **Hook:** `failed_func()` (typisch `context.invalidate`) zwischen
   Retries; cached Adapter werden so verworfen, der nächste Versuch
   resolved frisch.
3. **Re-entrant** über Thread-Local-Stack: verschachtelte Aufrufe in
   z.B. UI-Klassen-Predicates erben den äußeren Timeout (gemeinsame
   Uhr, kein Doppelwarten). Für `raise_exception` gilt: setzt der
   verschachtelte Aufruf den Wert *explizit* (also nicht `None`), so
   bleibt seine eigene Policy aktiv; nur ein nicht spezifiziertes
   `raise_exception=None` erbt vom äußeren Scope. Damit ergibt
   `exists(raise_exception=False)` auch innerhalb eines
   Pre-Condition-Blocks `False` statt zu propagieren.
   (Altcode `core/ensure.py:60` mit `_EnsureLocal`.)
4. **Timeout** → wenn `raise_exception=True`: `CannotEnsureError`
   mit der Message des zuletzt fehlgeschlagenen Predicates und
   `repr(context)` als Format-Argument.
5. **Predicates** sind zero-arg-Callables (i.d.R. gebundene Methoden);
   sie tragen via Decorator ein `message`-Attribut für die
   Fehlermeldung:

   ```python
   from PlatynUI.core import predicate

   class Element(Control):
       @predicate("element {0} is enabled")
       def _element_is_enabled(self) -> bool:
            return self.adapter.get_pattern(patterns.Element).is_enabled
   ```

**Standard-Predicates** (in `ui/element.py` als Methoden, übernommen
aus Altcode `ui/element.py:111`):

| Predicate | Message | Quelle |
|---|---|---|
| `_adapter_exists` | „{0} exists" | `ContextBase` (Adapter resolved + valid) |
| `_parent_exists` | „parent for {0} exists" | `ContextBase` (rekursiv) |
| `_element_is_visible` | „{0} is visible" | `Element.try_ensure_visible()` |
| `_element_is_enabled` | „{0} is enabled" | `is_enabled` |
| `_element_is_not_readonly` | „{0} is not readonly" | `is_readonly` negiert |
| `_application_is_ready` | „application of {0} is ready" | `Element.try_ensure_application_is_ready()`; gibt `True` wenn Adapter ungültig (idempotent) |
| `_toplevel_parent_is_active` | „toplevel parent of {0} is active" | `try_ensure_toplevel_parent_is_active()` |
| `_element_is_in_view` | „{0} is in view" | `_element_is_visible` + `try_bring_into_view()` |

**Verwendung in einer UI-Klasse:**

```python
class Button(Control, role="Button"):
    def activate(self) -> None:
        self.ensure_that(                      # Pre-Conditions
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(patterns.Activatable).activate()  # Perform
        self.ensure_that(                      # Postcondition (best-effort)
            self._application_is_ready,
            raise_exception=False,
        )
```

`wait_for` ist die schlanke Variante für Implementierungs-Details
*innerhalb* einer Pattern-Implementierung — z.B. „warte bis sich der
Toggle-State geändert hat" in `CheckBox.set_state` (siehe §5.3-Beispiel).

### A.4 Adapter-Interface (`core/adapter.py`)

```python
class Adapter(ABC):
    """Abstraktion über einen UI-Knoten zur Pattern-Wrapping-Ebene.

    In der Praxis gibt es **eine** produktive Implementierung —
    `UiNodeAdapter` (§A.4a) — die einen `platynui_native.UiNode`
    umhüllt. Die ABC existiert dennoch, weil sie den Vertrag bildet,
    den `AdapterProxy` (§4 / §A.4) per Komposition delegieren muss,
    und weil Test-Code lokale Fakes davon ableiten kann (siehe
    `tests/PlatynUI/test_adapter.py`)."""
    pattern_name: ClassVar[str] = "org.platynui.core.Adapter"

    # Identität & Lebenszeit
    @property @abstractmethod
    def valid(self) -> bool: ...
    @property @abstractmethod
    def runtime_id(self) -> str: ...

    # Strukturelle Beziehungen
    @property @abstractmethod
    def parent(self) -> "Adapter | None": ...
    @property @abstractmethod
    def children(self) -> "Sequence[Adapter]": ...

    # Such-Kriterien (für WeightCalculator)
    @property @abstractmethod
    def name(self) -> str: ...
    @property @abstractmethod
    def class_name(self) -> str: ...
    @property
    def tag_name(self) -> str: return ""
    @property @abstractmethod
    def role(self) -> str: ...
    @property @abstractmethod
    def supported_roles(self) -> set[str]: ...
    @property @abstractmethod
    def framework_id(self) -> str: ...

    # Attribute (einheitlicher (namespace, name)-Schlüsselraum,
    # symmetrisch zu UiNode in Rust:
    #   crates/core/src/ui/node.rs (`attribute(namespace, name)`,
    #   `attributes()`),
    #   crates/core/src/ui/namespace.rs (control|item|app|native).
    # Default-Namespace ist "control".
    @abstractmethod
    def attribute_names(self, namespace: str | None = None) -> set[str]:
        """Alle Attribut-Namen. Mit `namespace=None` werden Namen aus
        ALLEN Namespaces zurückgegeben — i.d.R. nur für Inspector/
        Debug-Zwecke. Mit explizitem `namespace="native"` (oder
        anderem) nur die des jeweiligen Namespaces."""
    @abstractmethod
    def attribute_value(self, name: str, namespace: str = "control") -> object: ...
    @abstractmethod
    def attributes(self) -> Iterator[tuple[str, str, object]]:
        """Iterator über (namespace, name, value) — direkte Entsprechung
        zu `UiNode.attributes()` in Rust."""

    # Pattern-Discovery & -Aufruf (Kern!)
    @abstractmethod
    def supported_patterns(self) -> set[type[PatternBase]]: ...
    @abstractmethod
    def supported_pattern_names(self) -> set[str]: ...

    def supports_pattern(self, pattern_type: type[PatternBase]) -> bool:
        """Convenience-Default: prüft `supported_patterns()`. Adapter
        können effizienter überschreiben (Rust: PyO3-Call zu
        `UiNode.has_pattern`)."""
        return pattern_type in self.supported_patterns()

    @overload
    def get_pattern(self, pattern_type: type[P]) -> P: ...
    @overload
    def get_pattern(self, pattern_type: type[P], *,
                    raise_exception: Literal[False]) -> P | None: ...
    @abstractmethod
    def get_pattern(self, pattern_type, *, raise_exception=True): ...

    def get_pattern_by_name(self, pattern_name: str, *,
                            raise_exception: bool = True) -> PatternBase | None:
        """Lookup über den Reverse-DNS-Identifier; nötig für externe
        Adapter ohne Python-Klassen-Import."""
        ...

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Adapter) and self.runtime_id == other.runtime_id
```

**Visuelle/State-Properties** (`bounding_rectangle`, `is_visible`,
`is_enabled`, `is_in_view`, `is_focused`, …) sind **keine** Adapter-
Methoden, sondern werden über das jeweilige Pattern abgerufen
(`Element.bounds`, `Element.is_visible`, `Element.is_enabled`,
`Focusable.is_focused`, …). `Element` (§7.1) bietet
`@cached_property`-Convenience-Wrapper, die intern
`adapter.get_pattern(patterns.Element).bounds` etc. aufrufen. So bleibt
das Adapter-Interface schmal und Pattern-orientiert.

**Pattern-Resolution-Reihenfolge** (siehe auch §4.3):
1. `self` ist `isinstance(pattern_type)` → `self`.
2. Adapter-internes Mapping (`_pattern_impls: dict[str, PatternBase]`) →
   gecached.
3. Adapter-spezifischer Lookup (`UiNodeAdapter`: PyO3-Call zu
   `UiNode.get_pattern`) → cachen.
4. Sonst: `PatternNotSupportedError` (oder `None` bei
   `raise_exception=False`).

**mypy-Konfiguration: `type-abstract` deaktiviert.** Die Signaturen
`get_pattern(pattern_type: type[P])` und
`supports_pattern(pattern_type: type[PatternBase])` nehmen das
Pattern-Klassenobjekt als **Lookup-Schlüssel** entgegen, nie als
Konstruktor. Pattern-Instanzen werden ausschließlich von Adaptern bzw.
Proxies erzeugt und an den Aufrufer zurückgegeben. mypys
`type-abstract`-Diagnose würde an jeder Aufrufstelle anschlagen
(`Only concrete class can be given where "type[Element]" is expected`),
weil ABCs theoretisch instanziierbar sind. Da wir genau das nie tun und
Pyright (zweiter Type-Checker im CI) den Check standardmäßig nicht
führt, ist `disable_error_code = ["type-abstract"]` in der
Root-`pyproject.toml` die saubere Lösung — pro-Aufrufstellen-Ignores
würden mit den ~120 geplanten `get_pattern`-Aufrufen nicht skalieren.
Echte „Pattern direkt instanziiert"-Fehler werden weiterhin von
`abstract` (mypy) und `reportGeneralTypeIssues` (pyright) gefangen.

**`AdapterProxy`** (siehe §4 / §5.1) ist eine `Adapter`-Subklasse, die
ihre Adapter-Identität vollständig per Komposition aus einem
gewrappten `adapter: Adapter` bezieht. Alle Adapter-ABC-Methoden
(`valid`, `runtime_id`, `parent`, `children`,
Suchkriterien, Attribute, `_resolve_pattern`) delegieren transparent
an den Wrapped-Adapter; eigenständige Logik gibt es nur in
`get_pattern` (eigene Patterns zuerst, dann `adapter.get_pattern`),
`get_pattern_by_name` (analog) und `supported_patterns` /
`supported_pattern_names` (Vereinigung Proxy ⊕ Adapter).

Subclassing wurde gegenüber reiner Komposition gewählt, damit die
Adapter-Auflösungspipeline (`AdapterFactory.find_one/find_all` →
`PatternProxyFactory.find_proxy_for`) durchgängig `Adapter | None`
zurückgeben kann und Konsumenten (`ContextBase`, `ElementDescriptor`,
`devices.py`) nicht zwischen Adapter und Proxy unterscheiden müssen.

### A.4a `UiNodeAdapter` (`core/adapters/ui_node.py`)

Die einzige produktive `Adapter`-Implementierung. Wickelt einen
`platynui_native.UiNode` so ein, dass das Python-`Adapter`-Interface
(§A.4) erfüllt ist.

Aufgabenkatalog:

- **API-Mapping**: `UiNode.attribute(name, namespace) -> UiValue` (wirft
  `AttributeNotFoundError`) → `Adapter.attribute_value(name, namespace)
  -> object` (wirft `KeyError` mit Schlüssel `"<ns>:<name>"`).
  `UiNode.attributes()` liefert `UiAttribute`; der Adapter exposed sie
  als `(namespace, name, value)`-Tupel. `attribute_names(namespace)`
  filtert in Python (kein dedizierter Native-Endpoint).
- **Defensive Default-Felder**: optionale Suchkriterien-Properties
  fangen `AttributeNotFoundError` ab und liefern den ABC-Default:
  - `class_name` ↔ `attribute('ClassName', 'control')`, default `""`
  - `framework_id` ↔ `attribute('FrameworkId', 'native')`, default `""`
  - `supported_roles` enthält bis auf Weiteres nur `{role}` — bis das
    Native-Attribut-Set `SupportedRoles` exposed (siehe §13.x), gibt es
    keinen anderen Lieferanten.
- **Pattern-Resolution-Hook (`_resolve_pattern`)**: schlägt den Reverse-
  DNS-Namen in einer modul-lokalen Builder-Tabelle nach
  (`_NATIVE_PATTERN_BUILDERS`). Treffer ruft `UiNode.get_pattern(name)`
  per **String** (nicht per Python-Klasse — die native Klasse besitzt
  kein `pattern_name`-ClassVar) und wrappt das Resultat in eine
  `core.patterns.*`-Subklasse. Beispiel: `platynui_native.Focusable` →
  `_NativeFocusable(Focusable)`, das `focus()` ans native Objekt
  delegiert und `is_focused` aus `UiNode.attribute('IsFocused',
  node.namespace.as_str())` zieht. Der Namespace folgt **dem Knoten**:
  Window/Button-Focus liegt in `control:IsFocused`, ListItem/TreeItem-
  Focus in `item:IsFocused` — der Wrapper liest entsprechend dynamisch.
- **`supports_pattern`-Override**: gibt nur dann `True` zurück, wenn
  (a) die native Seite das Pattern advertised UND (b) ein Python-
  Wrapper im `_NATIVE_PATTERN_BUILDERS`-Dict steht. Ohne (b) würde
  `get_pattern` später trotzdem in `PatternNotSupportedError` laufen
  und der Vertrag (`supports → get`) wäre gebrochen. Erweiterung um
  weitere Patterns = neuer Builder-Eintrag, sonst nichts.
- **Identity / Caching**: `runtime_id` aus `UiNode.runtime_id` (stabil
  über Tree-Reloads); `__eq__` / `__hash__` folgen der Adapter-ABC
  (§A.4 Z. 1617). `parent` / `children` werden bei jedem Aufruf neu
  aus `UiNode` geholt — der Rust-Cache regelt Wiederverwendung.
- **Native-Node-Zugriff (`native_node` Property)**: read-only Property,
  liefert das gewrappte `platynui_native.UiNode`. Public, weil die
  `RuntimeAdapterFactory` (§A.4b) das Handle zum Aufruf von
  `runtime.current.evaluate(xpath, node)` braucht. Der Zugriff ist
  bewusst spezifisch für `UiNodeAdapter` — Adapter-Implementierungen
  ohne Native-Backing brauchen eine eigene `AdapterFactory`-Variante
  und exposen die Property nicht.

Tests laufen gegen den **Rust-Mock-Provider** (`Runtime.new_with_mock()`)
und prüfen alle Mappings end-to-end. Reine Algorithmus-Tests für die
ABC selbst (Cache, Resolution-Steps) bleiben in `test_adapter.py` mit
Inline-Fakes.



`ContextBase` ist die Wurzel aller UI-Klassen (Context-Basis).
Vereinfacht ggü. Altcode (473 → ~250 LOC), siehe §11.2.

> **Hinweis (Rev. 22):** Frühere Fassungen dieses Abschnitts ließen
> `ContextBase` von einer Klasse `Assertable` erben, die
> `assert_that`/`assert_that_not` für RF-Style-Assertions auf
> Context-Ebene anbieten sollte. Eine solche Klasse existierte
> weder im Alt- noch im Neuprojekt — `src/PlatynUI/_assertable.py`
> enthält ausschließlich den `@assertable`-**Keyword-Decorator**,
> der RF-Keywords drei Assertion-Parameter (`assertion_operator`,
> `assertion_expected`, `assertion_message`) anhängt und bei einer
> mitgelieferten Operator-Angabe `assertionengine.verify_assertion`
> aufruft. Dieses Muster bleibt für die Keyword-Schicht erhalten
> (siehe BareMetal-Keywords wie `get_pointer_position`,
> `get_attribute`); `ContextBase` selbst braucht es nicht. Pre-/Post-
> Verifikation auf Context-Ebene läuft ausschließlich über
> `ensure_that(*predicates)` (Doc unten).

```python
class ContextBase:
    default_role: ClassVar[str | None] = None
    default_prefix: ClassVar[str | None] = None

    def __init__(self,
                 locator: Locator,
                 context_parent: "ContextBase | None" = None,
                 adapter: Adapter | None = None) -> None: ...

    # --- Adapter & Lifetime ---
    @property
    def adapter(self) -> Adapter: ...           # forciert ensure, raises
    def get_adapter(self, *, timeout: float | None = None,
                    raise_exception: bool = True) -> Adapter | None: ...
    def invalidate(self) -> None: ...           # rekursiv über Children
    def exists(self, *, timeout: float | None = None,
               raise_exception: bool = False) -> bool: ...

    # --- Pre/Post-Verifikation ---
    def ensure_that(self, *predicates,
                    timeout: float | None = None,
                    raise_exception: bool | None = None) -> bool:
        return ensure_that(self, *predicates, timeout=timeout,
                           raise_exception=raise_exception,
                           failed_func=self.invalidate)

    # --- Element-Suche ---
    def get(self, ctx: type[T], *args,
            locator: Locator | None = None, **kw) -> T: ...
    def get_one(self, ctx: type[T], *args, **kw) -> T: ...     # raise wenn !=1
    def get_all(self, ctx: type[T], *args, **kw) -> list[T]: ...
    def iter_all(self, ctx: type[T], *args, **kw) -> Iterator[T]: ...
    def get_child(self, ctx: type[T], *args, **kw) -> T: ...   # scope=Children
    def get_children(self, ctx: type[T], *args, **kw) -> list[T]: ...
    def ancestor(self, ctx: type[T], *args, **kw) -> T: ...
    def ancestors(self, ctx: type[T], *args, **kw) -> list[T]: ...

    # --- Iteration ---
    def __iter__(self) -> Iterator["ContextBase"]: ...   # alle Children
    @property
    def children(self) -> list["ContextBase"]: ...
    @property
    def parent(self) -> "ContextBase | None": ...

    # --- Property-Durchreichung ---
    @cached_property
    def name(self) -> str: ...
    @cached_property
    def role(self) -> str: ...
    # … class_name, framework_id, runtime_id, supported_roles,
    # supported_patterns, is_valid

    # --- Context-Manager (No-op, Convenience) ---
    def __enter__(self) -> Self: return self
    def __exit__(self, *exc) -> None: pass
```

**Generische Attribut-Reads** (für Locator-`attributes=`-Match und
Inspector-Zwecke; delegieren 1:1 an den Adapter, siehe §A.4):

```python
def attribute_names(self, namespace: str | None = None) -> set[str]: ...
def attribute_value(self, name: str, namespace: str = "control") -> object: ...
def attributes(self) -> Iterator[tuple[str, str, object]]: ...
```

`ContextFactory` lebt im selben Modul, hält die Klassen-Registry für
`@context` und liefert via `find_context_class_for(adapter)` die beste
Subklasse (`WeightCalculator`-basiert, siehe §4).

**Element-Convenience-Properties.** `Element` (§7.1, Subklasse von
`ContextBase`) ergänzt Pattern-basierte Wrapper als `@cached_property`
bzw. `@property`, damit Context-Code ohne expliziten
`get_pattern`-Aufruf auskommt:

```python
class Element(ContextBase):
    """Context-Basisklasse. Nicht zu verwechseln mit dem
    gleichnamigen *Pattern* `patterns.Element` (§5), das hier durch
    diese Properties gewrappt wird."""

    @property
    def _element_pattern(self) -> patterns.Element:
        return self.adapter.get_pattern(patterns.Element)

    @property
    def bounding_rectangle(self) -> Rect:
        return self._element_pattern.bounds

    @property
    def is_visible(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_visible if e is not None else True

    @property
    def is_in_view(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_in_view if e is not None else self.is_visible

    @property
    def is_enabled(self) -> bool:
        e = self.adapter.get_pattern(patterns.Element, raise_exception=False)
        return e.is_enabled if e is not None else True

    @property
    def is_focused(self) -> bool:
        f = self.adapter.get_pattern(patterns.Focusable, raise_exception=False)
        return f.is_focused if f is not None else False
```

Diese Properties sind die Quelle für Default-Predicates
(`is_visible`, `is_enabled`, …; siehe §A.3) und für Devices
(`MouseProxy.click(element)` liest `bounding_rectangle`).

### A.4b `AdapterFactory` (`core/adapter_factory.py`)

Die `AdapterFactory` trennt **Such- und Wrapping-Strategie** von
der reinen Knoten-Abstraktion `Adapter` (§A.4) und vom Native-
Runtime-Wrapper (§A.5). `Adapter` bleibt ein passiver Knoten-
Wrapper; die Übersetzung *Locator → Trefferliste roher Adapter*
ist ein eigenständiges Konzept und braucht einen eigenen Layer.

Diese Trennung folgt dem Altprojekt (`core/adapterfactory.py`,
`AdapterFactoryImpl`), wo die Suche bewusst nicht am Adapter
hängt. Im neuen Code-Stand ersetzt das Singleton-Pattern die
frühere per-Technology-Registry: es gibt eine Default-Factory
für die produktive Native-Runtime; Tests und alternative
Provider tauschen sie scope-gebunden über einen Override-
Context-Manager (analog `runtime`, §A.5).

**Begründung gegen Adapter.evaluate(...).** Würde der Adapter
selbst eine `evaluate(xpath) -> list[Adapter]`-Methode tragen,
müsste jede Adapter-Implementierung die Runtime-Abhängigkeit
mitschleppen und die Wrapping-Strategie (UiNode → UiNodeAdapter,
Filterung von skalaren XPath-Treffern, …) selbst implementieren.
Mit der Factory bleibt der Adapter dünn, die Suchstrategie ist
einmal an einer Stelle, und Mocks/Tests können den Suchpfad
isoliert tauschen, ohne Adapter zu ersetzen.

**API.** `core/adapter_factory.py` exportiert die ABC
`AdapterFactory`, die Default-Implementierung
`RuntimeAdapterFactory` und ein Singleton-Objekt
`adapter_factory`:

```python
from PlatynUI.core import adapter_factory

class AdapterFactory(ABC):
    @abstractmethod
    def find_one(
        self,
        parent: Adapter,
        locator: Locator,
    ) -> Adapter | None:
        """Return the first matching adapter, or None."""

    @abstractmethod
    def find_all(
        self,
        parent: Adapter,
        locator: Locator,
    ) -> list[Adapter]:
        """Return every matching adapter; empty list if none."""
```

Bewusst nur zwei Methoden:

- `parent.parent` und `parent.children` decken **strukturelle**
  Navigation locator-frei direkt am Adapter ab. Eine
  `find_parent(adapter)` braucht die Factory daher nicht;
  `adapter.parent` liefert das gleiche.
- `ancestor` / `get_child` / `get_children` aus dem alten
  `ContextBase` sind **Wrapper über `find_one`/`find_all`** mit
  einem Locator, dessen `scope` auf `Ancestor` bzw. `Children`
  gesetzt ist (siehe §A.4c). Sie werden in `ContextBase`
  konstruiert, nicht in der Factory.

Damit reduziert sich die Factory-Verantwortung auf das eine
Konzept *„Locator gegen einen Parent auflösen"*.

**RuntimeAdapterFactory (Default).** Die Default-Implementierung
ruft `runtime.current.evaluate(...)` und `evaluate_single(...)`:

```python
class RuntimeAdapterFactory(AdapterFactory):
    @override
    def find_one(self, parent: Adapter, locator: Locator) -> Adapter | None:
        xpath = self._render_xpath(parent, locator)
        node = self._parent_node(parent)
        result = runtime.current.evaluate_single(xpath, node)
        return self._wrap_or_none(result)

    @override
    def find_all(self, parent: Adapter, locator: Locator) -> list[Adapter]:
        xpath = self._render_xpath(parent, locator)
        node = self._parent_node(parent)
        results = runtime.current.evaluate(xpath, node)
        return [a for a in (self._wrap_or_none(r) for r in results) if a is not None]
```

- `_render_xpath` ruft `locator.to_xpath(parent_is_root_like=…,
  default_role=…, default_prefix=…)`. Die `default_*`-Parameter
  kommen vom *Ziel-Context-Type*, den `ContextBase` kennt — nicht
  vom Parent-Adapter; daher reicht `ContextBase` sie via Wrapper
  durch (siehe §A.4c). Die Factory selbst kennt keine
  Context-Klassen.
- `_parent_node` liest `parent.native_node` (neue Public-Property
  auf `UiNodeAdapter`, siehe §A.4a). Adapter ohne `native_node`
  (z.B. zukünftige Mock-Adapter ohne Rust-Backing) brauchen eine
  eigene `AdapterFactory`-Implementierung — dafür gibt es das
  Override.
- `_wrap` wrappt `UiNode`-Treffer in `UiNodeAdapter`. Skalare
  XPath-Treffer (`EvaluatedAttribute`, `UiValue`) lösen
  `InvalidResultTypeError` (TypeError-Subklasse, außerhalb der
  `PlatynUIError`-Hierarchie) aus, da sie auf einen fehlerhaft
  konstruierten Locator-XPath hinweisen — das ist ein
  Programmierfehler, keine Laufzeitbedingung.

**Singleton-Accessor.** Symmetrie zu §A.5:

```python
adapter_factory.current                    # property: aktive AdapterFactory
adapter_factory.is_initialised()           # bool
adapter_factory.is_sealed()                # bool

adapter_factory.use_default()              # RuntimeAdapterFactory()
adapter_factory.use_factory(cb)            # cb: Callable[[], AdapterFactory]

with adapter_factory.override(factory) as f:
    ...                                    # scope-bound, restores on exit
```

Gleiche Sealing-Regeln wie `runtime`: erste `current`-Lesung
fixiert die Wahl; `use_*` nach Sealing wirft `RuntimeError`;
`override(...)` ist jederzeit erlaubt und LIFO-stackbar. Es gibt
**kein** `override_with_mock()` — Mock-Adapter sind nicht
geplant (§A.11), und die Default-Factory funktioniert auch
gegen die Mock-Runtime, weil sie einfach `runtime.current`
benutzt; ein laufender `runtime.override_with_mock()`-Block
liefert automatisch eine Mock-XPath-Engine.

**Thread-Safety.** Wie `runtime`: `RLock` um den Accessor;
`AdapterFactory`-Implementierungen müssen selbst thread-safe
sein, falls sie internen Zustand halten. `RuntimeAdapterFactory`
ist zustandslos.

**Tests.** Pytest-Fixtures verwenden den Override-Context-Manager
(kein manuelles Setzen):

```python
@pytest.fixture
def fake_factory():
    from PlatynUI.core import adapter_factory
    with adapter_factory.override(lambda: FakeFactory()) as f:
        yield f
```

Die Default-`RuntimeAdapterFactory` wird gegen einen
`runtime.override_with_mock()`-Scope getestet — keine eigene
Mock-Factory nötig.

### A.5 Process-wide Runtime (`core/runtime.py`)

Die PlatynUI-Bibliothek bündelt einen einzigen
`platynui_native.Runtime` pro Prozess. Provider-Cache,
Pointer-/Keyboard-Profile und der Desktop-Tree gehören diesem
Runtime-Objekt; Adapter, Device-Proxies, Robot-Keywords, der
BareMetal-Helper und der Inspector arbeiten alle gegen dieselbe
Instanz. Den Runtime explizit durch jeden Konstruktor zu reichen
würde dem Robot-Framework-Idiom (Keywords ohne Per-Call-Context)
widersprechen und die User-API unnötig aufblähen.

**Designprinzipien.** Die API trennt drei Verantwortlichkeiten
strikt:

1. *Variantenwahl* — vor der ersten Benutzung darf der Aufrufer
   wählen, *welche* Runtime gebaut werden soll (Default,
   Mock-Provider oder eigene Factory). Sobald ein Konsument
   `runtime.current` liest, wird die Wahl eingefroren („sealed").
2. *Konsum* — Konsumenten lesen ausschließlich `runtime.current`
   und bekommen immer dieselbe, einmal gebaute Instanz.
3. *Test-Override* — Tests installieren eine alternative Runtime
   stets über den Context-Manager `runtime.override(...)`, der
   den vorherigen Zustand garantiert wiederherstellt.

Es gibt **keinen** nackten Setter, der eine externe Native-Runtime
in das Singleton injiziert. Alle Wege führen entweder über eine
Variantenwahl (`use_*`) oder über einen scope-gebundenen Override.
Damit ist „vergessenes Reset" konstruktiv ausgeschlossen, und
versehentliches Tauschen während laufender Operationen wird durch
das Sealing verhindert.

**API.** `core/runtime.py` exportiert ein Singleton-Objekt
`runtime` (Klasse `Runtime`):

```python
from PlatynUI.core import runtime

# --- Konsum (für alle Aufrufer: Adapter, Devices, Keywords, …) ---
runtime.current             # property: aktive platynui_native.Runtime
                            # — sealed beim ersten Zugriff
runtime.is_initialised()    # bool: wurde current je gebaut/gewählt
runtime.is_sealed()         # bool: wurde current bereits ausgelesen

# --- Variantenwahl (nur vor Sealing erlaubt) ---
runtime.use_default()       # Auto-Discovery via inventory (=Werkseinstellung)
runtime.use_mock()           # Runtime.new_with_mock() (mock-provider Feature)
runtime.use_factory(cb)     # cb: Callable[[], _NativeRuntime] — Custom-Builder

# --- Test-Override (jederzeit erlaubt, scope-gebunden) ---
with runtime.override(factory) as rt:    # factory: Callable[[], _NativeRuntime]
    ...                     # innerhalb: rt ist aktiv
                            # exit: shutdown(rt), Vorzustand restored
with runtime.override_with_mock() as rt:
    ...                     # Convenience für Tests
```

**Variantenwahl & Sealing.**

- `use_*()`-Methoden setzen einen *Builder* (Closure), nicht die
  Instanz selbst. Mehrfach-Aufruf vor Sealing ersetzt den Builder
  still — der zuletzt gewählte gewinnt.
- Beim ersten `current`-Zugriff wird der Builder ausgeführt, das
  Ergebnis gecacht und der Accessor *sealed*.
- `use_*()` nach Sealing wirft `RuntimeError("runtime already
  initialised; use override() instead")`. Wer im laufenden Prozess
  scopiert tauschen will, nutzt `override(...)`.
- Wird vor Sealing kein `use_*()` aufgerufen, gilt `use_default()`
  implizit.

**Override-Semantik.**

- `override(factory)` akzeptiert ein zero-arg Callable
  `Callable[[], _NativeRuntime]`. Das Callable wird im Enter genau
  einmal ausgeführt; die produzierte Instanz wird aktiv.  Wer eine
  bereits gebaute Instanz wiederverwenden möchte, wrappt sie:
  `runtime.override(lambda: existing_rt)`.  Diese explizite Callable-
  Konvention vermeidet Ambiguität (Mock-Objekte sind callable —
  `callable()`-Heuristiken wären fragil).
- `override_with_mock()` ist Zucker für
  `override(NativeRuntime.new_with_mock)`.
- Im Enter wird der aktuelle Zustand (Builder *und* Instanz) auf
  einem Stack gesichert; danach ist die Override-Instanz aktiv und
  bereits sealed.
- Im Exit wird `shutdown()` der Override-Instanz aufgerufen
  (Exceptions werden geschluckt — best-effort) und der Vorzustand
  restauriert. Verschachtelte `override(...)`-Blöcke sind erlaubt
  (LIFO-Stack).

**Thread-Safety.** Der Accessor wird durch ein `RLock` geschützt.
Die zugrundeliegende `platynui_native.Runtime` hält intern ein
Rust-`Mutex` und ist sicher zwischen Python-Threads teilbar.
Pointer- und Keyboard-Methoden adressieren absolute
Bildschirmkoordinaten bzw. die OS-Event-Queue und sind damit
node-unabhängig: ein Override mitten in einer Session invalidiert
keine zuvor geholten `UiNode`-Instanzen — diese referenzieren
weiterhin ihren ursprünglichen Provider-Tree.

**Robot-Framework-Integration.** Die spätere
`PlatynUI`-Robot-Library nimmt die Variantenwahl als
Konstruktor-Argument entgegen:

```robotframework
*** Settings ***
Library    PlatynUI    use_mock=${True}
```

```python
class PlatynUI:
    def __init__(self, use_mock: bool = False) -> None:
        if use_mock:
            runtime.use_mock()
        # sonst: Default-Lazy reicht
```

**Tests.** Mock-Tests verwenden ausschließlich den Override-
Context-Manager — kein manuelles Setzen, kein vergessenes
Reset:

```python
@pytest.fixture
def native_runtime():
    from PlatynUI.core import runtime
    with runtime.override_with_mock() as rt:
        yield rt
```

`override(...)` schluckt Exceptions aus `shutdown()` (best-effort
teardown) — Test-Cleanup soll niemals den eigentlichen Test
verschleiern.

### A.6 `@locator`-Mechanik (`core/locator.py`)

`Locator` ist eine `@dataclass`-ähnliche Builder-Klasse, die intern
einen XPath-2.0-Ausdruck für die Rust-XPath-Engine baut. Die
Implementierung ist eine handgeschriebene Klasse mit `__slots__`
(kein `@dataclass`-Decorator), weil der Konstruktor zusätzlich zu den
typisierten Feldern beliebige `**kwargs` als freie Attribut-Predicates
entgegennimmt (siehe §7.1).

**Felder & Kwargs** (vereinfacht aus Altcode `ui/locator.py:21`):

```python
@dataclass(slots=True, kw_only=True)
class Locator:
    # XPath-Bestandteile
    path: str | None = None              # wörtlicher XPath, override
    node: str | None = None              # Knotenname (Rolle)
    prefix: str | None = None            # Namespace-Prefix
    use_default_prefix: bool = False
    axis: str | None = None              # raw axis-Prefix
    scope: LocatorScope | None = None
    index: int | None = None
    position: int | None = None

    # Standard-Attribute (alle in @-Notation)
    name: str | None = None              # → @Name=
    id: str | None = None                # → @Id=
    class_name: str | None = None        # → @ClassName=
    role: str | None = None              # auch Knotenname-Default
    runtime_id: str | None = None        # → @RuntimeId=
    framework_id: str | None = None      # → @FrameworkId=

    # Freie Attribute (PascalCase erwartet, siehe §7.1).
    # Schlüssel: bare String → (default_namespace, name);
    #            Tupel (namespace, name) → expliziter Namespace.
    # Default-Namespace = "control"; eine Context-Klasse kann das
    # über das Klassenattribut `default_attribute_namespace`
    # umstellen (z.B. `default_attribute_namespace = "item"`). Die
    # Auflösung passiert beim Build des XPath, nicht beim Setzen der
    # Dict-Einträge — das hält die Locator-Konstruktion deklarativ.
    attributes: dict[str | tuple[str, str], str | re.Pattern[str]] = field(default_factory=dict)
    custom_attributes: list[str] = field(default_factory=list)  # raw Prädikate

    # Plus beliebige `**extra_attributes: str | re.Pattern[str]` am __init__:
    # Locator(AutomationId="x")        → attributes["AutomationId"] = "x"
    # Locator(native__HWND=0xABCD)     → attributes[("native","HWND")] = 0xABCD
    # Konflikte mit den typisierten Feldern oder dem attributes-Dict
    # (nach Namespace-Normalisierung) werfen TypeError.
```

**`LocatorScope`** als String-Enum-Alias (kein `StrEnum`-Zwang):

```python
LocatorScope: TypeAlias = Literal[
    "root", "descendants", "children", "parent", "ancestor",
    "ancestor-or-self", "descendants-or-self",
    "following", "following-sibling", "preceding", "preceding-sibling",
]
```

**XPath-Bau** (`Locator.to_xpath()`):

1. `path` gesetzt → wörtlich übernommen, alle anderen Felder ignoriert
   außer `prefix`/`axis`-Präfix.
2. Sonst: Knotenname = `node` ?? `role` ?? `default_role` ?? `*`.
3. Achse aus `axis` ?? `LocatorScope`-Mapping (`children` → leer,
   `descendants` → `.//`, `ancestor` → `ancestor::`, …).
4. Prädikate aus drei Quellen (siehe §7.1 für die User-Konvention):
   - **Convenience-Felder** (`name`, `id`, `class_name`, `runtime_id`,
     `framework_id`) werden über `_standard_attributes()` zu
     `@Name='v'`, `@Id='v'`, `@ClassName='v'`, `@RuntimeId='v'`,
     `@FrameworkId='v'` (immer im `control`-Namespace, daher
     unprefixed).
   - **Freie Kwargs** am Konstruktor werden bereits in `__init__`
     in das `attributes`-Dict gemergt: ein Kwarg ohne `__`-Trenner
     landet als Bare-String-Key (`AutomationId="x"` →
     `attributes["AutomationId"] = "x"`); ein Kwarg mit
     `<ns>__<name>`-Trenner landet als Tupel-Key (`native__HWND=...`
     → `attributes[("native", "HWND")] = ...`). Damit fließen sie
     durch denselben Pfad wie der Dict-Weg.
   - **`attributes`-Dict**:
     - Bare-String-Schlüssel werden auf `(default_namespace, name)`
       normalisiert; `default_namespace` ist `"control"`, sofern die
       verwendende Context-Klasse nicht
       `default_attribute_namespace = "<ns>"` setzt.
     - `(namespace, name)` → `@<ns>:<name>='v'` (bzw. nur `@<name>` wenn
       der Namespace dem XPath-Default `control` entspricht), oder
       `matches(@<ns>:<name>, 'v')` für Regex.
   - **`custom_attributes`** werden als rohe Prädikat-Strings übernommen.
   - Alle Teilbedingungen mit ` and ` verbunden.
   - **Konflikt-Regel** (im Konstruktor durchgesetzt, nicht erst hier):
     Sammelt eine `(namespace, name)`-Map über alle drei Quellen.
     Doppelte Schlüssel (auch nach Normalisierung — z.B.
     Convenience-Feld `name=` vs. Kwarg `Name=` vs.
     `attributes={('control','Name'): ...}`) werfen `TypeError` mit
     genauer Fehlermeldung.
5. Suffix `[N]` aus `index`, `[position()=N]` aus `position`.
6. Default-Scope-Regel: ohne Parent → `children`; mit Parent →
   `children` falls Parent ein `Application`/`Desktop`, sonst
   `descendants`. (1:1 aus Altcode.)

**`@locator`-Decorator** ist eine eigene Funktion (nicht die
`Locator`-Klasse selbst — siehe Rev. 18). Sie nimmt dieselben Kwargs wie
`Locator.__init__` entgegen und liefert je nach Decorator-Target zwei
Verhalten:

| Target | Verhalten | Status |
|---|---|---|
| Klasse | hängt einen `Locator` als `__locator__`-Klassenattribut an, gibt die Klasse unverändert zurück | **Phase 1 / Rev. 18 — DONE** |
| Methode/Property | wickelt die Funktion in einen `LocatorMethodDescriptor` ein, der den Locator + die Wrapped-Function speichert; beim Instanz-Zugriff wird `ContextBase.get(annotation, locator=…)` aufgerufen | **Phase 3 — STUB** (wirft derzeit `NotImplementedError`) |

Die Method-Form ist als Stub bereits API-stabil; Context-Code kann
beide Formen heute schreiben — die Resolution wird in Phase 3 transparent
nachgereicht, ohne Quelltext-Änderungen am Context.

Verwendung:

```python
# Klassen-Default (am Context) — funktioniert heute
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    pass

# Property-Variante (typisierter Child-Locator) — Stub bis Phase 3
class CalculatorWindow(Window, role="Window", name="Rechner"):
    @property
    @locator(AutomationId="num5Button")
    def n5(self) -> Button: ...

# Default-Namespace umstellen (z.B. für Item-Container)
class FileListItem(Item):
    default_attribute_namespace = "item"
    # bare-String-Keys im attributes-Dict landen jetzt im
    # `item:`-Namespace; expliziter Namespace bleibt jederzeit
    # via Tupel-Key möglich.

# Cross-Namespace via Tupel
@locator(attributes={
    "AutomationId": "submit",          # → @AutomationId=… (control)
    ("native", "HWND"): 0x12AB,         # → @native:HWND=…
})
class SubmitButton(Button): ...
```

**Property-Vererbung:** `Locator.copy_from(parent)` übernimmt aus dem
Parent-Locator alle Felder, die in `self` `None` sind, und mergt
`attributes`/`custom_attributes`. So vererben `@locator`-Defaults von
Klasse zu Instanz und von Parent-Context zu Child.

**Match-Verhalten** (über die Rust-XPath-Engine):

- `Locator` allein liefert keinen `UiNode` — er ist eine *Beschreibung*.
  Aufgelöst wird er via `runtime.evaluate(xpath, root_node)` aus dem
  Rust-Backend.
- `get(...)` → erstes Match (oder `ElementNotFoundError`).
- `get_one(...)` → genau ein Match (oder `ElementNotFoundError` /
  `MultipleElementsFoundError`).
- `get_all(...)` → alle Matches.
- Kein-Match: retry über `ensure_that` (Pre-Condition
  `_adapter_exists`) bis Timeout, dann `ElementNotFoundError`.

### A.7 `ElementDescriptor[PatternT]` (`core/descriptor.py`)

Im Altprojekt `keywords/types.py:20` bereits vorhanden, im neuen
Projekt zusätzlich im `BareMetal`-Modul als `UiNodeDescriptor`
(XPath-basiert, direkt an `UiNode` gekoppelt — Übergangscode, siehe
§13.2 für den Plan zur Ablösung).

`ElementDescriptor` ist die High-Level-Variante für die `PlatynUI`-
Library: er wrappt `Locator`/`ContextBase`, nicht `UiNode`. Er teilt
sich mit der zukünftigen BareMetal-Variante eine **gemeinsame
Robot-Variable `${PLATYNUI_ROOT_ELEMENT}`** als Single Source of
Truth für das aktive Root-Element (siehe §13.1, „Geteilter Zustand").

`core/descriptor.py` selbst bleibt **Robot-frei**. Das Root-Element
wird über einen austauschbaren Storage-Hook
(`set_root_element_storage(getter, setter)`) gehalten, dessen Default
ein prozesssweiter Slot ist. Die Library-Inits
(`src/PlatynUI/__init__.py`, später auch `BareMetal/__init__.py`)
installieren denselben Override gegen
`EXECUTION_CONTEXTS.current.variables[${PLATYNUI_ROOT_ELEMENT}]`,
sodass `Set Root` in einer Library für die andere wirkt. Tests und
programmatische Nutzung außerhalb von Robot fallen auf den
In-Process-Slot zurück.

```python
PatternT = TypeVar("PatternT", bound=PatternBase, default=PatternBase)


class ElementDescriptor(Generic[PatternT]):
    """Lazy reference to a UI element used as Robot keyword argument."""

    def __init__(
        self,
        locator: Locator | None = None,
        context_type: type[ContextBase] | None = None,
        parent: "ElementDescriptor[Any] | None" = None,
        context: ContextBase | None = None,
    ) -> None: ...

    def __call__(self, *, full_context: bool = True) -> ContextBase:
        """Resolve and cache the underlying context.

        With ``full_context=True`` the best-matching `ContextBase`
        subclass is picked via `ContextFactory.find_context_class_for`;
        with ``False`` a bare `ContextBase` is returned for cheap
        property reads.
        """

    # Robot converter (registered in PlatynUI/__init__.py)
    @staticmethod
    def convert(value: str | ContextBase) -> "ElementDescriptor[Any]":
        if isinstance(value, ContextBase):
            return ElementDescriptor(context=value)
        return ElementDescriptor(
            Locator(path=value),
            parent=ElementDescriptor.get_root_element(),
        )

    @staticmethod
    def set_root_element(
        element: "ElementDescriptor[Any] | None",
    ) -> "ElementDescriptor[Any] | None": ...

    @staticmethod
    def get_root_element() -> "ElementDescriptor[Any] | None": ...


class RootElementDescriptor(ElementDescriptor[PatternT]):
    """Variant whose ``convert`` ignores the ambient root element."""

    @staticmethod
    def convert(value: str | ContextBase) -> "ElementDescriptor[Any]":
        if isinstance(value, ContextBase):
            return ElementDescriptor(context=value)
        return RootElementDescriptor(Locator(path=value))
```

`PatternT` is a *phantom* marker. It does not constrain the runtime
return type of `__call__`; it exists so that
`ElementDescriptor[patterns.Activatable]` (a `_GenericAlias`) can be
registered as a distinct Robot converter and surfaced as its own type
in the Robot IDE documentation. The actual pattern check happens in
the keyword body via `ctx.adapter.supports_pattern(...)`.

```python
# keywords/activate.py
@keyword
def activate(element: ElementDescriptor[patterns.Activatable]) -> None:
    ctx = element()                         # resolve
    if not ctx.adapter.supports_pattern(patterns.Activatable):
        raise PatternNotSupportedError(
            f"{ctx} does not support Activatable")
    ctx.activate()                          # UI-Klasse hat die Methode
```

**Root-Element storage.** `core/descriptor.py` defines a module-level
hook:

```python
RootElementGetter = Callable[[], "ElementDescriptor[Any] | None"]
RootElementSetter = Callable[
    ["ElementDescriptor[Any] | None"], "ElementDescriptor[Any] | None"
]


def set_root_element_storage(
    getter: RootElementGetter, setter: RootElementSetter
) -> None: ...
```

The default in-process storage is a single module-level slot. The
Robot-library entry point (`src/PlatynUI/__init__.py`) installs an
override that reads/writes `${PLATYNUI_ROOT_ELEMENT}` via
`EXECUTION_CONTEXTS.current.variables`. With no Robot context the
fallback applies, so `BareMetal` and unit tests work identically.

When `get_root_element()` returns `None`, `convert(string)` builds an
`ElementDescriptor` with `parent=None`; the resulting `Locator(path=...)`
resolves desktop-relatively via `Locator.scope` defaults
(`/.//control:Foo`).

### A.8 Lifecycle & Robot-Library-Init

```python
# src/PlatynUI/__init__.py
@library(scope="GLOBAL", version=__version__,
         converters={
             ElementDescriptor: ElementDescriptor.convert,
             RootElementDescriptor: RootElementDescriptor.convert,
             # Pattern-getypte Konverter (auto-generiert aus
             # patterns.__all__):
             patterns.Activatable: ElementDescriptor[patterns.Activatable].convert,
             patterns.Toggleable: ElementDescriptor[patterns.Toggleable].convert,
             patterns.TextContent: ElementDescriptor[patterns.TextContent].convert,
             patterns.TextEditable: ElementDescriptor[patterns.TextEditable].convert,
             patterns.Clearable: ElementDescriptor[patterns.Clearable].convert,
             patterns.Element: ElementDescriptor[patterns.Element].convert,
             patterns.Focusable: ElementDescriptor[patterns.Focusable].convert,
             # …
             Locator: convert_locator,
         })
class PlatynUI(DynamicCore):
    def __init__(self) -> None:
        # Side-effect: importiert ui/* und keywords/*; das löst die
        # @context- und @pattern_proxy_for-Registrierungen aus.
        from . import ui            # noqa: F401  (registers UI classes)
        from .core.adapters import rust as _rust   # default adapter
        super().__init__([
            ApplicationKeywords(),
            ActivateKeywords(),
            ToggleKeywords(),
            SelectKeywords(),
            ExpandKeywords(),
            TextKeywords(),
            ScrollKeywords(),
            WindowKeywords(),
            PropertyKeywords(),
            WaitKeywords(),
            DiagnosticsKeywords(),    # Highlight, Screenshot
        ])

    @keyword
    def set_root_element(self, element: RootElementDescriptor) -> None:
        ElementDescriptor.set_root_element(element)
```

**Bootstrap-Reihenfolge bei Suite-Start:**

1. Robot lädt `Library  PlatynUI` → `PlatynUI.__init__` wird gerufen.
2. `from . import ui` triggert alle `@context`/`@pattern_proxy_for`-
   Registrierungen (Side-Effect-Imports in `ui/__init__.py` und
   `ui/proxies/__init__.py`).
3. `core.adapters.ui_node`-Import lädt die `UiNodeAdapter`-
   Implementierung. Sie ist die einzige produktive Adapter-Klasse;
   andere Adapter sind nicht vorgesehen.
4. Die Robot-Library ist nutzungsbereit. Der **Runtime-Singleton**
   `PlatynUI.core.runtime.runtime` (Rev. 20, §A.5) liefert die
   prozessweite `platynui_native.Runtime`; `UiNodeAdapter` greift dort
   lazy zu und hält keine eigene Runtime-Referenz.

**Desktop-Root** ist konzeptionell das Wurzelelement des UI-Trees.
Praktisch gibt es ihn als Klasse:

```python
@locator(path="/.")
class Desktop(ContextBase, role="Desktop"):
    default_prefix = "control"
    # Stellt MouseProxy mit base_point=(0,0) und base_rect=Bildschirm
    # bereit, damit absolute Mouse-Operationen ohne Element möglich sind.
```

**Adapter-Bootstrap.** Da es nur eine produktive Adapter-Klasse gibt,
entfällt jede Multi-Backend-Registry. Der `Desktop`-Konstruktor ruft
schlicht `UiNodeAdapter.create_root()` auf, sofern kein expliziter
`adapter=`-Parameter übergeben wurde. Tests (in Python wie Rust)
geben einen über `Runtime.new_with_mock()` gebauten Adapter ein.

**Application Start.** Im Altprojekt unimplementiert
(`keywords/application.py` raised `NotImplementedError`). Im neuen
Projekt:

```python
@keyword
def start_application(command: str | list[str], *,
                      timeout: float | None = None,
                      window_locator: ElementDescriptor | None = None,
                      ) -> ElementDescriptor:
    """Startet einen Prozess und wartet auf das Top-Level-Window.

    - command: Programmpfad oder Argv-Liste (subprocess.Popen-konform)
    - window_locator: optionaler Locator, der das erscheinende Hauptfenster
      identifiziert (z.B. nach Title oder ProcessId). Default: warten auf
      ein Window mit der vom Prozess geerbten ProcessId.
    Liefert einen ElementDescriptor auf das Hauptfenster zurück.
    """
    ...
```

`exit_application` und `close_application` bleiben getrennt:
`exit_application` ruft das `Application`-Pattern (sauberes Quit per
API), `close_application` schickt ein Window-Close (X-Knopf-Äquivalent).

### A.9 Devices: Mouse / Keyboard (`core/devices.py`)

Die Python-Devices sind **dünne Element-Wrapper** über die im Rust-
Runtime bereits vollständig implementierte Pointer/Keyboard-Pipeline.
Die komplette Low-Level-Logik (Profile, `PointerOverrides`, Multi-
Click-Multiplikator, Press/Release-Delays, Move-Interpolation, Sequenz-
Parser für `<Ctrl+A>`, Modifier-Stacking) lebt in `crates/runtime` und
ist via `runtime.current.pointer_*` / `runtime.current.keyboard_*` von
Python aus direkt erreichbar.

Die Python-Schicht trägt nur noch drei Verantwortlichkeiten:

1. **Element-bezogenes Coord-Resolving**: aus einer `Point | VirtualPoint
   | None`-Override plus optionalen `x`/`y`-Offsets eine absolute
   Desktop-Koordinate berechnen, die an Rust übergeben wird.
2. **Default-Click-Position**: über die Adapter-Pattern-Kette
   (`ActivationTarget` → `Element`) den Standard-Klickpunkt eines
   Elements bestimmen.
3. **Pre/Post-Hooks**: `before_action`/`after_action` als
   Erweiterungspunkt für Element-Integration in Phase 3 (`ensure_that(
   toplevel_active, in_view)`); in `core/devices.py` selbst no-op.

Es gibt **kein** `MouseDevice`/`KeyboardDevice`-Zwischenlayer mehr —
der Rust-Runtime *ist* das Device.

#### A.9.1 Aktions-Enums und Typ-Aliase

```python
class MouseAction(StrEnum):
    MOVE = "move"
    PRESS = "press"
    RELEASE = "release"
    CLICK = "click"
    DOUBLE_CLICK = "double_click"

class KeyboardAction(StrEnum):
    TYPE = "type"
    PRESS = "press"
    RELEASE = "release"

# core/types.py
MouseButton: TypeAlias = PointerButton  # 1:1 aus platynui_native
```

`PointerButton` (LEFT=1, MIDDLE=2, RIGHT=3) wird unverändert aus dem
Rust-Modul re-exportiert. `MouseButton` ist der historische Python-Name
für dieselbe Enum.

#### A.9.2 VirtualPoint und Anchor

`VirtualPoint` ist eine reine Funktion `Rect → Point`, die einen
Anker-Punkt innerhalb eines Bounding-Rects beschreibt.

```python
@dataclass(frozen=True)
class VirtualPoint:
    name: str
    func: Callable[[Rect], Point]

    def resolve(self, rect: Rect) -> Point:
        return self.func(rect)


class Anchor:
    """Vordefinierte VirtualPoints für Element-Bounds."""
    TOP_LEFT     = VirtualPoint("top_left",     lambda r: r.position())
    TOP          = VirtualPoint("top",          lambda r: Point(r.x + r.width / 2, r.y))
    TOP_RIGHT    = VirtualPoint("top_right",    lambda r: Point(r.right(), r.y))
    LEFT         = VirtualPoint("left",         lambda r: Point(r.x, r.y + r.height / 2))
    CENTER       = VirtualPoint("center",       lambda r: r.center())
    RIGHT        = VirtualPoint("right",        lambda r: Point(r.right(), r.y + r.height / 2))
    BOTTOM_LEFT  = VirtualPoint("bottom_left",  lambda r: Point(r.x, r.bottom()))
    BOTTOM       = VirtualPoint("bottom",       lambda r: Point(r.x + r.width / 2, r.bottom()))
    BOTTOM_RIGHT = VirtualPoint("bottom_right", lambda r: Point(r.right(), r.bottom()))
```

Im Gegensatz zum Altcode (`tmp/.../core/types.py:76`) gibt es **keinen
None-Achsenwert-Trick** mehr — der neue `Point` ist immer vollständig.
Y-only-Anker (Altcode `Anchor.TOP` mit `x=None`) sind durch konkrete
Halbierung der Width abgelöst.

#### A.9.3 MouseProxy

```python
class MouseProxy(ABC):
    """Element-relativer Maus-Wrapper.

    Berechnet absolute Desktop-Koordinaten aus base_rect plus
    Override (Point | VirtualPoint | None) plus optionalen x/y-Offsets
    und delegiert die eigentliche Pointer-Aktion an den Rust-Runtime.
    """

    @property
    @abstractmethod
    def base_rect(self) -> Rect:
        """Element-Bounding-Box im Desktop-Koordinatensystem."""

    @property
    def default_click_position(self) -> Point:
        """Default-Klickpunkt (absolut). Wird verwendet wenn pos=None."""
        return self.base_rect.center()

    def before_action(self, action: MouseAction) -> None: ...
    def after_action(self, action: MouseAction) -> None: ...

    def _resolve_point(
        self,
        pos: Point | VirtualPoint | None,
        x: float | None,
        y: float | None,
    ) -> Point:
        if pos is None:
            base = self.default_click_position
        elif isinstance(pos, VirtualPoint):
            base = pos.resolve(self.base_rect)
        else:  # Point — relativ zu Element-TopLeft
            base = self.base_rect.position() + pos
        return base.translate(x or 0.0, y or 0.0)

    def move_to(
        self,
        pos: Point | VirtualPoint | None = None,
        *,
        x: float | None = None,
        y: float | None = None,
    ) -> Point:
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.MOVE)
        runtime.current.pointer_move(target.x, target.y, origin="desktop")
        self.after_action(MouseAction.MOVE)
        return target

    def click(
        self,
        *,
        button: MouseButton = MouseButton.LEFT,
        times: int = 1,
        pos: Point | VirtualPoint | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        target = self._resolve_point(pos, x, y)
        self.before_action(MouseAction.CLICK)
        runtime.current.pointer_multi_click(
            target.x, target.y, button=button, clicks=times, origin="desktop",
        )
        self.after_action(MouseAction.CLICK)

    # press / release / double_click analog
```

#### A.9.4 AdapterMouseProxy mit ActivationTarget-Fallback-Kette

```python
class AdapterMouseProxy(MouseProxy):
    """Standard-Implementierung: liest Bounds und Default-Klickpunkt
    über die Adapter-Pattern-Kette."""

    def __init__(self, adapter: Adapter) -> None:
        self._adapter = adapter
        self._logger = logging.getLogger("platynui.devices")

    @property
    def base_rect(self) -> Rect:
        return self._adapter.get_pattern(patterns.Element).bounds

    @property
    def default_click_position(self) -> Point:
        # Fallback-Kette gemäß patterns.md / Spec §A.9:
        # ActivationArea-Center → ActivationPoint → Element.bounds.center()
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            target = self._adapter.get_pattern(patterns.ActivationTarget)
            if target.activation_area is not None:
                return target.activation_area.center()
            return target.activation_point
        return self._adapter.get_pattern(patterns.Element).bounds.center()

    def before_action(self, action: MouseAction) -> None:
        if self._adapter.supports_pattern(patterns.ActivationTarget):
            hint = self._adapter.get_pattern(patterns.ActivationTarget).activation_hint
            if hint:
                self._logger.debug("mouse %s: %s", action.value, hint)
```

#### A.9.5 KeyboardProxy

Der Rust-Runtime parst `<Ctrl+A>`, `<Shift+.>`, Unicode-Escapes etc.
über `KeyboardSequence::parse` (pest-basiert, siehe
`crates/runtime/src/keyboard_sequence.rs`). Die Python-API ist deshalb
**kein Variadic** mehr — sie nimmt eine einzige Sequenz-String und
reicht sie unverändert an Rust weiter.

```python
class KeyboardProxy(ABC):
    """Tastatur-Wrapper. Sequenz-Format wird komplett von Rust geparst."""

    def before_action(self, action: KeyboardAction) -> None: ...
    def after_action(self, action: KeyboardAction) -> None: ...

    def type_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.TYPE)
        runtime.current.keyboard_type(sequence)
        self.after_action(KeyboardAction.TYPE)

    def press_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.PRESS)
        runtime.current.keyboard_press(sequence)
        self.after_action(KeyboardAction.PRESS)

    def release_keys(self, sequence: str) -> None:
        self.before_action(KeyboardAction.RELEASE)
        runtime.current.keyboard_release(sequence)
        self.after_action(KeyboardAction.RELEASE)


class AdapterKeyboardProxy(KeyboardProxy):
    """Tastatur-Wrapper für ein Element. Aktuell ohne adapter-spezifische
    Logik — Hook für Phase 3 (Element-Fokus, Verifikation)."""

    def __init__(self, adapter: Adapter) -> None:
        self._adapter = adapter
```

#### A.9.6 Element-Integration (Phase 3, Vorausschau)

In `ui/element.py` wird `Element` `mouse` und `keyboard` als property-
cached `AdapterMouseProxy`/`AdapterKeyboardProxy` exposen, deren
`before_action` dann ein echtes `ensure_that(toplevel_active, in_view)`
ausführt. In `core/devices.py` selbst sind die Hooks no-op — die
Element-Schicht hängt sich später per Subclass oder Override ein.

#### A.9.7 Logging

`AdapterMouseProxy` nutzt den Standard-Python-Logger
`platynui.devices`. Wenn ein Element ein `ActivationTarget`-Pattern
mit nicht-leerem `activation_hint` bietet, wird dieser auf DEBUG-Level
vor jeder Maus-Aktion geloggt. Default-Level ist WARNING — User müssen
explizit `logging.getLogger("platynui.devices").setLevel(logging.DEBUG)`
setzen, um die Hints zu sehen.

### A.10 Pattern-Default-Implementierungen (`core/patterns/defaults.py`)

Generische Pattern-Implementierungen, die als Fallback greifen, wenn
weder ein `@pattern_proxy_for`-Proxy noch der Adapter das Pattern direkt
liefern (Drei-Ebenen-Fallback, siehe §5a.3).

**`Activatable` (Default):** Click auf `default_click_position`.

```python
class DefaultActivatable(patterns.Activatable):
    pattern_name = patterns.Activatable.pattern_name

    def __init__(self, adapter: Adapter) -> None:
        self._adapter = adapter

    def activate(self) -> None:
        AdapterMouseProxy(self._adapter).click()
```

**`TextEditable` (Default):** Fokus + Clear-Sequenz (Ctrl+A, Del) +
`type_keys`. Properties (`is_readonly`, `max_length`,
`supports_password_mode`) liefern konservative Defaults
(`is_readonly=False`, `max_length=None`, `supports_password_mode=False`),
solange der Adapter sie nicht überschreibt.

```python
class DefaultTextEditable(patterns.TextEditable):
    pattern_name = patterns.TextEditable.pattern_name
    def set_text(self, value: str) -> None:
        self._adapter.get_pattern(patterns.Focusable).focus()
        kb = AdapterKeyboardProxy(self._adapter)
        kb.type_keys("<Control+A><Delete>")
        kb.type_keys(value)
    @property
    def is_readonly(self) -> bool: return False
    @property
    def max_length(self) -> int | None: return None
    @property
    def supports_password_mode(self) -> bool: return False
```

**`Clearable` (Default):** Fokus + Ctrl+A + Del.

**`Toggleable` (Default):** kein generischer Default — dieses Pattern
*muss* vom Adapter oder Proxy kommen, weil ohne State-Read keine
Verifikation möglich ist (`state` und `supports_three_state` lassen sich
nicht generisch bestimmen).

`core/patterns/defaults.py` enthält diese Defaults und bietet sie
**nur auf explizite Anforderung** an: Der `AdapterProxy.get_pattern`-
Lookup (siehe §A.4) erweitert sich um Stufe 4: „falls keine spezifische
Implementierung gefunden, prüfe, ob ein
`DEFAULT_PATTERN_FACTORIES[pattern_name]` existiert und instanziiere
ihn lazy". Das Mapping ist eine reine Modul-Konstante, keine globale
Registry mit Side-Effects.

### A.11 Mock-Adapter — *gestrichen*

Ursprünglich war hier ein Python-`MockAdapter` über einen Python-
`MockNode`-Tree vorgesehen. Diese Idee wurde verworfen: Tests laufen
stattdessen gegen den **Rust-Mock-Provider** (`provider-mock` mit
`Runtime.new_with_mock()`). Begründung:

- `UiNode` (aus `platynui_native`) ist die einzige *Technology* der
  Bibliothek. Eine zweite Technology auf Python-Ebene aufzubauen, nur
  um sie zu testen, doppelt die Adapter-Mechanik ohne neuen Nutzen.
- Der Rust-Mock-Provider liefert vollwertige `UiNode`-Bäume mit
  Pattern-Implementierungen — exakt das, was Tests brauchen.
- Lokale Test-Fakes für eng begrenzte Adapter-Algorithmus-Tests
  (z.B. `test_adapter.py`) bleiben als kleine Inline-Helper bestehen
  und werden nicht zu einem öffentlichen API gehoben.

Stub-Patterns für Spy-Verhalten (z.B. *„hat `activate()` genau einmal
diesen Stub aufgerufen?"*) werden über die **`AdapterProxy`-Schicht**
realisiert: ein Proxy mixt das Pattern als Spy ein und überschreibt
damit das vom Native-Node gelieferte Pattern (siehe §A.4 / §4.2).

### A.12 Highlight & Diagnose (`core/devices.py` + `keywords/diagnostics.py`)

Highlight und Screenshots gehören nicht zum Element-Vertrag, sondern
zur **Diagnose-Schicht**. Sie erweitern `Element` um zwei Methoden,
die auf das `DisplayDevice` der aktiven Runtime zugreifen.

```python
class DisplayDevice(ABC):
    @abstractmethod
    def highlight_rect(self, rect: Rect, *, time: float) -> None: ...
    @abstractmethod
    def get_screenshot(self, rect: Rect, *,
                       format: str = "png",
                       quality: int = -1) -> bytes: ...

# Element-Mixin
class Element(Control):
    def highlight(self, rect: Rect | None = None,
                  time: float | None = None) -> None:
        time = time or Settings.current().element_highlight_time
        rect = rect or self.bounding_rectangle
        if not self.is_visible:
            return
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            raise_exception=False,
            timeout=Settings.current().element_highlight_ensure_timeout,
        )
        self.adapter.display_device.highlight_rect(rect, time=time)

    def get_screenshot(self, rect: Rect | None = None, *,
                       format: str | None = None,
                       quality: int | None = None) -> bytes: ...
    def save_screenshot(self, filename: str | None = None, **kw) -> str: ...
```

**Robot-Keywords:**

```python
@keyword
def highlight(element: ElementDescriptor, time: float | None = None) -> None: ...

@keyword
def take_screenshot(element: ElementDescriptor | None = None, *,
                    filename: str | None = None) -> str: ...
```

**Inspector-Anbindung.** Die GUI-Inspector-App (`apps/inspector`) ist
*kein* Teil der Python-Library — sie ist ein eigenständiges Rust-
Binary mit eigener `egui`-UI. Die Python-Library exposed jedoch die
gleichen Diagnose-Bausteine (Highlight, Screenshot, Tree-Dump), damit
Tests ohne Inspector denselben Output produzieren können.

### A.13 Capability-Patterns (`core/patterns/*.py`)

**Sinn der Pattern-Schicht.** Patterns übersetzen die heterogenen
technischen Realitäten verschiedener UI-Stacks (UIA, AT-SPI, AX;
WPF, Qt, GTK, Win32, …) in eine **konstante semantische API**.
Egal ob ein Fenster über UIA `WindowPattern.SetWindowVisualState`
oder über AT-SPI `Action.do_action("minimize")` minimiert wird —
auf Pattern-Ebene heißt die Capability `Minimizable.minimize()`,
und auf Robot-Ebene heißt sie `Minimize <locator>`. Was der
RF-User schreibt, ändert sich nicht, wenn dasselbe Programm auf
einer anderen Plattform läuft.

```
RF-User schreibt:        Minimize  //control:Window[@Name='X']
Keyword-Layer (Phase 5): handgeschriebener @keyword-Wrapper
Pattern-Layer (Python):  Minimizable.minimize() — konstante API
Proxy-Implementation:    framework-/provider-spezifische Logik
                         (kann Adapter-Attribute lesen, mehrere
                          kombinieren, ableiten, Rust-Calls machen)
Adapter-Attribute:       rohe Provider-Daten (Inspector zeigt diese)
```

Pattern-Properties wie `is_minimized` müssen **nicht** spiegelbildlich
zu einem Adapter-Attribut existieren. Eine Pattern-Implementation
kann den Wert aus einem einzelnen Attribut lesen, aus mehreren
Attributen ableiten, oder Provider-spezifische Logik aufrufen. Der
Inspector zeigt dem User die rohen Adapter-Attribute (siehe §A.12);
die Pattern-Properties leben darüber als Implementations-Detail des
jeweiligen Proxies.

**Designprämisse — kleine, orthogonale Patterns.** Ein Test-Autor
ruft Pattern-Methoden auf, unabhängig davon, ob das Element ein
Top-Level-Window, eine einklappbare Sidebar oder ein verschiebbares
Canvas-Item ist. Wer `Minimizable` implementiert, kann minimiert
werden; wer es nicht implementiert, kann es nicht. Es gibt **kein**
Window-Mega-Pattern.

Damit deckt eine kleine Patterns-Suite alle Element-Klassen ab und
macht selbst unbekannte Custom-Controls über die ihnen vom Provider
oder einer Custom-Proxy-Implementation gemeldeten Patterns sicher
bedienbar.

**Verhältnis zu Robot-Keywords.** Jedes Pattern bekommt in Phase 5
einen handgeschriebenen `@keyword`-Wrapper (siehe §A.8 / §10):
`Activatable` → `Activate`, `Minimizable` → `Minimize`,
`Maximizable` → `Maximize`, `Closeable` → `Close` usw. Pattern-
Klassen tragen **keine** Keyword-Metadaten — die Wrapper-Funktionen
sind die einzige Quelle der Wahrheit für Keyword-Namen, Argument-
Reihenfolgen und Doku-Strings. Ob auch Read-Properties (z.B.
`is_minimized`) eigene Keywords bekommen oder ob der RF-User dafür
das generische `Get Attribute`-Keyword (analog BareMetal) nutzt,
wird im Keyword-Designschritt entschieden.

**Read+Action in einem Pattern.** Pro Capability **ein** Pattern,
das State-Reads (`is_minimized`, `can_minimize`) und Action
(`minimize()`) bündelt — anders als im Altprojekt, wo es zu jeder
Action ein paralleles `Has…`-Read-Pattern gab. Begründung: wer
minimieren kann, kann auch lesen, ob das Element minimiert ist;
zwei Patterns für eine Capability erhöhen die Kombinatorik ohne
Nutzen. Ein Adapter, der einen State **nur lesen** kann (z.B. ein
Beobachter), liefert für die Action `NotSupportedError` oder
`can_*` = `False`.

**Pure-Action-Patterns sind erlaubt.** Nicht jedes Pattern braucht
Read-Properties. `Restorable` exponiert nur `restore()`, weil
"wiederherstellen" keinen sinnvollen eigenen Read-State hat (der
Status liegt in `Minimizable.is_minimized` / `Maximizable.is_maximized`).
Genauso ist `Activatable` reine Action. Ein Pattern listet **nur die
Mitglieder, die für die Capability gebraucht werden** — Symmetrie
zwischen Patterns ist kein Selbstzweck.

**Pattern-Suite (Phase-4-Scope).**

| Pattern | Methoden / Properties | Status |
|---|---|---|
| `Activatable` | `activate()` | ✓ vorhanden (§5) |
| `Focusable` | `focus()`; Adapter-Attr `IsFocused` | ✓ vorhanden (§5) |
| `Minimizable` | `is_minimized`, `can_minimize`, `minimize()` | **neu** |
| `Maximizable` | `is_maximized`, `can_maximize`, `maximize()` | **neu** |
| `Restorable` | `restore()` | **neu** |
| `Closeable` | `can_close`, `close()` | **neu** |
| `Movable` | `can_move`, `move_to(point)` | **neu** |
| `Resizable` | `can_resize`, `resize(size)` | **neu** |
| `Titled` | `title` (read-only) | **neu** |
| `HasUserInput` | `accepts_user_input() -> bool \| None` | **neu** |

`Activatable` ist die universelle "primary action"-Capability:
Buttons aktivieren = klicken, MenuItems aktivieren = ausführen,
**Windows aktivieren = Fokus + Foreground**. Ein `Window`-Context
implementiert `Activatable` ganz normal; der mitgelieferte Default-
Proxy für `role="Window"` mappt `Activatable.activate()` auf die
Plattform-Window-Activation. Damit erreicht das Robot-Keyword
`Activate` sowohl Buttons als auch Windows ohne Sonderfall.

**Beispiel — Sidebar:**

```python
class Sidebar(ContextBase, role="Pane", class_name="Sidebar"):
    pass

# Nutzung:
sidebar = window.sidebar
sidebar.adapter.get_pattern(Minimizable).minimize()
assert sidebar.adapter.get_pattern(Minimizable).is_minimized
```

Dieselben drei Zeilen funktionieren auch für ein `Window`. Die
Context-Klasse muss nichts Spezielles tun — die Capability lebt
im Pattern, nicht in der Klasse.

**`accepts_user_input()` als Methode (nicht Attribut).** Anders als
die Read-Properties der anderen Patterns ist
`HasUserInput.accepts_user_input()` eine Methode mit
`Optional[bool]`-Rückgabe (`None` = "Provider weiß es nicht").
Begründung: für Modal-Dialog-Erkennung kann der Wert kurzlebig sein
(Pop-up erscheint und blockiert), und Provider unterscheiden sich
darin, ob sie das überhaupt melden können. Eine Methode signalisiert
diesen Polling-Charakter klarer als ein Attribut.

**Verhältnis zum aktuellen Rust-`WindowSurface` (eine Datenquelle
unter mehreren).** Pattern-Definition und Pattern-Implementation
sind getrennt. Die Pattern-Klassen in `core/patterns/` sind
**Python-ABCs** und kennen Rust gar nicht. Eine konkrete
Implementation für ein konkretes Element kommt aus einem Proxy
(Default-Proxy oder Custom-Proxy), und der Proxy entscheidet, woher
er die Daten und Aktionen nimmt — Adapter-Attribute lesen, mehrere
Attribute kombinieren, native Read-Escape-Hatch (§13.5), eigene
Klick-/Tastatur-Sequenzen, oder eben Rust-Calls.

Für **`role="Window"`** liefert PlatynUI einen Default-Proxy mit, der
`Activatable`, `Minimizable`, `Maximizable`, `Restorable`,
`Closeable`, `Movable`, `Resizable`, `Titled` und `HasUserInput`
implementiert, indem er das aktuelle Rust-Pattern `WindowSurfacePattern`
(siehe `crates/core/src/ui/pattern.rs`) aufruft. Das ist eine
**Implementations-Wahl dieses einen Proxies**, kein Vertrag der
Pattern-Schicht. Ein User, der für seine Custom-Sidebar `Minimizable`
implementieren will, schreibt einen eigenen Proxy ohne jeden
Rust-`WindowSurface`-Bezug.

Das Rust-`WindowSurfacePattern` bündelt heute mehrere Capabilities
in einem Trait. In einem späteren, separaten Rust-Designschritt wird
es in einzelne Traits aufgesplittet. Für die Python-Schicht ist das
unsichtbar — der Default-Window-Proxy in `ui/proxies/window.py`
ändert dann seine internen Aufrufe; die Pattern-Klassen, Keyword-
Wrapper und alle anderen Proxies bleiben unverändert.

**Offene Fragen für die Rust-Aufsplittung.** Diese werden in einem
separaten Designschritt geklärt, **nicht** hier:

- `is_active` (Window-Aktivierungs-Status) — finale Quelle ist der
  Rust-`WindowManager` (auf Rust-Seite halb definiert, noch nicht
  fertig). Bis dahin liest die Python-Schicht `is_active`
  übergangsweise aus `Focusable.is_focused` am Window-Adapter.
- `can_minimize` / `can_maximize` / `can_close` / `can_move` /
  `can_resize` als Read-Capabilities — eigene Attribute oder pauschal
  "Pattern wird vom Provider gemeldet ⇒ ja"? Heute existieren auf
  Rust-Seite `window_surface::SUPPORTS_MOVE` und `SUPPORTS_RESIZE`,
  die anderen drei fehlen.
- `title` — `control:Name` wiederverwenden oder dediziertes
  `titled::TITLE`?
- Cross-Provider-Konsistenz der oben genannten Attribute.

**Granulare Element-Patterns (Phase 4).** Zusätzlich zur
Window-Suite werden zwei kleine Patterns aus dem Altprojekt
übernommen, die Lücken im aktuellen `core/patterns/element.py`
schließen:

| Pattern | Methoden / Properties | Zweck |
|---|---|---|
| `Readable` | `is_readonly` | aktuell in Legacy `Element.is_readonly`; gehört nicht in das Geometrie-`Element`-Pattern |
| `ApplicationReady` | `try_ensure_ready() -> bool` | Polling-Hook für "App ist nicht responding"; Predicate-Basis für `_application_is_ready` im Context |

Damit bleibt `core/patterns/element.py` auf Geometrie + Sichtbarkeit
+ Enabled fokussiert, und beide neuen Capabilities sind unabhängig
kombinierbar (z.B. ein read-only Edit-Feld implementiert
`Readable`+`TextContent`, kein `TextEditable`).

**Aktualisierte `__init__.py`-Exports von `core/patterns/`:**

```python
__all__ = [
    'Activatable',
    'ActivationTarget',
    'ApplicationReady',          # neu
    'Closeable',                 # neu
    'Element',
    'Focusable',
    'HasUserInput',              # neu
    'Maximizable',               # neu
    'Minimizable',               # neu
    'Movable',                   # neu
    'PatternBase',
    'Readable',                  # neu
    'Resizable',                 # neu
    'Restorable',                # neu
    'TextContent',
    'Titled',                    # neu
    'Toggleable',
]
```

`pattern_name`-Identifier folgen dem Reverse-DNS-Schema aus §5:
`org.platynui.patterns.Minimizable`, `org.platynui.patterns.Closeable`
usw.


### A.14 Context-Basisklassen (`ui/*.py`)

Die User-API der UI-Hierarchie. Jede Klasse erbt direkt oder
indirekt von `ContextBase` (§A.4 / `core/context.py`) und stellt
die typisierten Contexts bereit, mit denen Robot-Keywords und
Python-User arbeiten.

#### A.14.1 Schichten (Pattern → Context → Keyword)

Drei Schichten, klare Verantwortung:

| Schicht | Aufgabe |
|---|---|
| **Pattern** (`core/patterns/*.py`) | Roher Provider-Aufruf. Z. B. `Activatable.activate()` → UIA `Invoke`, AT-SPI `do_action`. Kein `ensure_that`, kein Warten, keine Komposition. |
| **Context-Klasse** (`ui/*.py`) | Wrappt das Pattern in **Pre/Perform/Post-Vertrag**: `ensure_that(<predicates>)` → `pattern.action()` → `ensure_that(<post>)`. Darf die Pattern-Aktion **semantisch überladen**: `CheckBox.activate()` ruft nicht `Activatable.activate()`, sondern `Toggleable.set_state(Checked)` — `Activate` heißt aus User-Sicht „primäre Aktion", nicht „Pattern-X aufrufen". |
| **Robot-Keyword** (`keywords/*.py`, Phase 5) | Dünner `@keyword`-Wrapper, ruft `context.action()`. Ein Wrapper pro Pattern: `Activatable` → `Activate`, `Minimizable` → `Minimize`, … |

**Beispiel CheckBox.activate():**

```python
class CheckBox(Control):
    def activate(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        toggle = self.adapter.get_pattern(Toggleable, raise_exception=False)
        if toggle is not None:
            toggle.set_state(ToggleState.Checked)
        # Post-Check optional: state == Checked
```

`Activate <my_checkbox>` aus Robot landet so beim richtigen
User-Intent („abhaken"), unabhängig von der Toolkit-Mechanik.

#### A.14.2 Klassenhierarchie

```
ContextBase  (core/context.py)
├── UnknownContext              (core/context.py — Generic-Fallback)
├── Application                 (ui/application.py — Identity-Container)
├── DesktopBase                 (ui/desktopbase.py — Element-Verhalten ohne App-Ready)
│   └── Desktop                 (ui/desktop.py — DesktopBase + @locator(path="/."))
└── Element                     (ui/element.py — Arbeitstier)
    ├── Control                 (ui/control.py — + Focus)
    │   ├── Window              (ui/window.py — + Window-Capabilities)
    │   │   └── Frame           (ui/window.py — Marker-Subklasse)
    │   ├── AbstractButton      (ui/buttons.py — register=False)
    │   │   ├── Button          (ui/buttons.py)
    │   │   └── CheckBox        (ui/buttons.py)
    │   ├── Text                (ui/text.py)
    │   ├── Edit                (ui/text.py)
    │   ├── List                (ui/lists.py — ItemContainer)
    │   ├── Tree                (ui/tree.py — ItemContainer)
    │   ├── Table               (ui/table.py — ItemContainer)
    │   └── ComboBox            (ui/combobox.py — Expandable + Items + optional Editable)
    └── Item                    (ui/item.py — register=False, + text)
        ├── SelectableItem      (ui/item.py — register=False, + Selectable)
        │   ├── ListItem        (ui/lists.py)
        │   └── TreeItem        (ui/tree.py — auch ExpandableItem)
        ├── ExpandableItem      (ui/item.py — register=False, + Expandable)
        ├── EditableItem        (ui/item.py — register=False, + HasEditor + TextEditable)
        ├── Cell                (ui/table.py)
        │   └── EditableCell    (ui/table.py — auch EditableItem)
        └── Row                 (ui/table.py — ItemContainer von Cells)
```

`Element` ist die zentrale Basisklasse für sichtbare UI-Elemente.
`Control` ergänzt `has_focus`/`focus()`. `Window` wrappt die
Window-Capability-Patterns (§A.13). `Frame` bleibt als Marker-
Subklasse von `Window` für Toolkits, die Frame und Window
unterscheiden (Legacy-Parität).

`UnknownContext` (existiert in `core/context.py:536`) bleibt der
Fallback, wenn `ContextFactory.find_context_class_for()` keine
Klasse mit Score > 0 findet. Eigene Element-Subklassen mit
spezifischer Rolle entstehen via `class MyElement(Element, role="..."):`
oder `Element` mit `role=`-kwarg.

`Element`, `Control` und `DesktopBase` sind abstrakte Zwischen-
klassen und werden mit `register=False` von der automatischen
Registrierung ausgenommen (§2.6). `Window`, `Frame`, `Application`
und `Desktop` registrieren sich automatisch über
`__init_subclass__` mit ihrem Klassennamen als `role` — das ist
nötig, damit `ContextBase.parent` Adapter mit `role="Window"`
auch tatsächlich als `Window` wrappt (statt als `UnknownContext`),
und damit Tree-Walks (`top_level_parent`, `parent_window`,
`_resolve_application`) korrekt durchlaufen.

#### A.14.3 `Element` (`ui/element.py`)

Properties (Adapter-Pass-Through, alle Werte aus
`pattern::element::*` Attributen, siehe `crates/core/src/ui/attributes.rs`):

```python
class Element(ContextBase, register=False):
    @property
    def bounds(self) -> Rect: ...           # Bounds
    @property
    def is_visible(self) -> bool: ...       # IsVisible
    @property
    def is_enabled(self) -> bool: ...       # IsEnabled
    @property
    def is_in_view(self) -> bool: ...       # IsInView
    @property
    def is_readonly(self) -> bool:
        # Convenience-Shortcut über Readable-Pattern;
        # default False wenn Pattern fehlt
        ...

    @property
    def top_level_parent(self) -> 'Element':
        # Walk-up via self.parent bis zum direkten Kind von DesktopBase
        ...

    @property
    def parent_window(self) -> 'Window | None':
        # Walk-up via self.parent bis Window-Instanz, sonst None
        ...

    @property
    def mouse(self) -> Mouse: ...
    @property
    def keyboard(self) -> Keyboard: ...
```

`default_click_position` aus dem Altprojekt entfällt sowohl auf
`Element` als auch als API-Property auf `Element` (Context); den
richtigen Klick-Punkt liefert die `AdapterMouseProxy`-Fallback-Kette
über das `ActivationTarget`-Pattern, mit `Element.bounds.center()`
als letztem Fallback (siehe §A.9.4).

**Predicates** (Underscore-Prefix wie Altprojekt; intern aber zur
Override durch Subklassen vorgesehen):

```python
@predicate("application for {0} is ready")
def _application_is_ready(self) -> bool:
    """Self-Check + Top-Level-HasUserInput-Pattern + lazy User-Application-Lookup."""
    if self is not self.top_level_parent:
        return self.top_level_parent._application_is_ready
    pattern = self.adapter.get_pattern(HasUserInput, raise_exception=False)
    pattern_says = pattern.accepts_user_input() if pattern else None
    if self.__application is _UNRESOLVED:
        self.__application = self._resolve_application()
    user_says = self.__application.is_ready() if self.__application else None
    return pattern_says is not False and user_says is not False

@predicate("element {0} is visible")
def _element_is_visible(self) -> bool:
    self.ensure_that(self._application_is_ready)
    return self.is_visible

@predicate("element {0} is in view")
def _element_is_in_view(self) -> bool:
    self.ensure_that(self._element_is_visible)
    # Pragmatisch (d1): kein BringIntoView-Pattern → ehrlicher Read-Check.
    # Wenn später ein BringIntoViewable-Pattern kommt, hier vor dem
    # Return ein try_pattern.bring_into_view() einbauen.
    return self.is_in_view

@predicate("element {0} is enabled")
def _element_is_enabled(self) -> bool:
    self.ensure_that(self._element_is_visible)
    return self.is_enabled

@predicate("element {0} is not readonly")
def _element_is_not_readonly(self) -> bool:
    self.ensure_that(self._element_is_enabled)
    return not self.is_readonly

@predicate("top-level parent of element {0} is active")
def _toplevel_parent_is_active(self) -> bool:
    """Aktiviert Top-Level via Activatable-Pattern, wenn nicht bereits aktiv."""
    top = self.top_level_parent
    if top.is_active:
        return True
    top.activate()  # via Window.activate() Context-Methode
    return top.is_active
```

**`_resolve_application()`** läuft `self.parent` aufwärts (über
`ContextBase.parent`, das via `adapter.parent` resolved) und
returnt die erste `Application`-Instanz oder `None`. Cache-Slot
auf der Element-Instanz (`__application: Application | None | _Unresolved`).

`Application` registriert sich als Default-Context-Klasse für
`role=Application`-Adapter-Knoten (`default_role = "Application"`),
sodass der Walk-up im Normalfall *immer* eine `Application`-Instanz
findet, sobald ein Application-Adapter im Tree existiert. User-
Subklassen mit spezifischerem Locator gewinnen über den
WeightCalculator.

**Methoden:**

```python
def activate_parent_window(self) -> None:
    pw = self.parent_window
    if pw is not None:
        pw.activate()

def bring_to_view(self) -> bool:
    return self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)

def highlight(self, duration: float = 3.0) -> None:
    self.ensure_that(self._element_is_in_view)
    # delegiert an Runtime.highlight(rects=[self.bounds],
    #                                 duration_ms=int(duration * 1000))

def get_screenshot(self) -> bytes:
    self._before_get_screenshot()
    # delegiert an Runtime.screenshot(rect=self.bounds)

def save_screenshot(self, path: str | Path) -> Path:
    Path(path).write_bytes(self.get_screenshot())
    return Path(path)

def _before_get_screenshot(self) -> None:
    self.ensure_that(self._element_is_in_view)
```

`_before_get_screenshot` ist Hook für `DesktopBase` (No-op Override:
Desktop ist immer in view).

**Mouse/Keyboard-Proxies** kommen aus `core/devices/`
(`core/devices/mouse.py`, `core/devices/keyboard.py`). `Element`
liefert in den Properties ein neues `Mouse(MouseProxy(self))` bzw.
`Keyboard(KeyboardProxy(self))`. `MouseProxy` (private Element-
interne Adapter-Klasse) implementiert das `MouseDeviceProxy`-ABC
aus `core/devices/`:

- `get_base_point()` → fragt `ActivationTarget.activation_point`
  am Element; Fallback `bounds.center`.
- `before_action(action)` → `ensure_that(_toplevel_parent_is_active,
  _element_is_in_view, _element_is_enabled)`.
- `after_action(action)` → leer (Hook).

`KeyboardProxy` analog, aber `before_action` ist leer (Element
verlangt keinen Focus für Tastatur — das macht erst `Control`,
siehe §A.14.4).

#### A.14.4 `Control` (`ui/control.py`)

```python
class Control(Element, register=False):
    @property
    def has_focus(self) -> bool:
        # Convenience über Focusable-Pattern; default False wenn fehlt
        ...

    def focus(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        focusable = self.adapter.get_pattern(Focusable, raise_exception=False)
        if focusable is not None:
            focusable.focus()

    @predicate("control {0} has focus")
    def _control_has_focus(self) -> bool:
        if self.has_focus:
            return True
        self.focus()
        return self.has_focus
```

Plus eigener `KeyboardProxy`-Override, dessen `before_action`
zusätzlich `_control_has_focus` aufruft (Tastatureingaben gehen an
das fokussierte Element).

#### A.14.5 `Window` (`ui/window.py`)

Wrappt die Window-Capability-Patterns aus §A.13 in
Pre/Perform/Post-Verträge. Jede Methode folgt demselben Muster:

```python
class Window(Control):
    # ---- Read-Properties (Convenience-Shortcuts über Patterns) ----
    @property
    def is_active(self) -> bool: ...           # Activatable.is_active
    @property
    def is_minimized(self) -> bool: ...        # Minimizable.is_minimized
    @property
    def is_maximized(self) -> bool: ...        # Maximizable.is_maximized
    @property
    def title(self) -> str: ...                # Titled.title

    # ---- Capability-Methoden (Pre → Pattern → Post) ----
    def activate(self) -> None: ...
    def minimize(self) -> None: ...
    def maximize(self) -> None: ...
    def restore(self) -> None: ...
    def close(self, timeout: float | None = None) -> None: ...
    def move_to(self, point: Point) -> None: ...
    def resize(self, size: Size) -> None: ...
```

Jede Capability-Methode:

1. **Pre**: `ensure_that(<predicates>)` — meist `_application_is_ready`
   plus capability-spezifische Vorbedingungen (z. B. `can_minimize`,
   `_window_can_close`).
2. **Perform**: `pattern.action()` über `adapter.get_pattern(...)`
   (raise default → `PatternNotSupportedError`, wenn Pattern fehlt).
3. **Post**: `ensure_that(<post-predicate>)` — z. B. nach `minimize()`
   warten bis `is_minimized` oder `not is_active`.

`Frame(Window)` bleibt als leere Marker-Subklasse für Toolkits, die
Frame/Window unterscheiden.

#### A.14.6 `Desktop` und `DesktopBase` (`ui/desktopbase.py`, `ui/desktop.py`)

```python
class DesktopBase(Element, register=False):
    default_role = "Desktop"

    # MouseProxy-Override: Origin (0,0) statt bounds.center
    # KeyboardProxy-Override: kein Focus-Check
    # _before_get_screenshot: No-op (Desktop ist immer in view)
    # _application_is_ready: returnt immer True (Desktop hat keine App)


@locator(path="/.")
@context
class Desktop(DesktopBase):
    pass
```

Trennung beibehalten wie Altprojekt, damit User eigene Desktop-
Varianten ohne den `/.`-Locator von `DesktopBase` ableiten können
(`class MyAppDesktop(DesktopBase, ...): ...`).

#### A.14.7 `Application` (`ui/application.py`)

Reiner Identity-Container, *kein* `Element`-Verhalten (keine Mouse/
Keyboard-Proxies, keine Predicates wie `_element_is_visible`).
`name`, `role`, `runtime_id` etc. kommen aus `ContextBase`.

```python
@context
class Application(ContextBase):
    default_role = "Application"
    default_prefix = "app"

    @property
    def process_id(self) -> int: ...        # ProcessId
    @property
    def process_name(self) -> str: ...      # ProcessName

    def is_ready(self) -> bool:
        """User overrides für app-spezifische Readiness-Checks."""
        return True

    def exit(self, timeout: float | None = None) -> None:
        if timeout is None:
            timeout = Settings.current().application_exit_timeout
        self._request_exit()
        self._force_exit(timeout)
        self.invalidate()

    def _request_exit(self) -> None:
        """Stage 1: graceful close. Default schließt alle Top-Level-Windows."""
        ...

    def _force_exit(self, timeout: float) -> None:
        """Stage 2: pollt process_id, killt nach Timeout. Plattform-Switch via os.kill / ctypes."""
        ...
```

Beide Stages sind via Underscore-Prefix als überschreibbar markiert
(intern, aber Subklassen-Hook). Überschreiben:
- `_request_exit` für app-spezifische graceful-shutdown-Sequenzen
  (z. B. `Ctrl+Q` ans fokussierte Window, File-→-Exit-Menü).
- `_force_exit` für app-spezifische Force-Strategien (z. B. `SIGINT`
  statt `SIGKILL`, längere Timeouts für asynchron beendende Apps).

Restliche Application-Adapter-Attribute (`ExecutablePath`,
`CommandLine`, `UserName`, `StartTime`, `Architecture`) sind über
`get_attribute("ExecutablePath")` zugänglich, ohne dedizierte
Property.

`Settings.application_exit_timeout: float = 10.0` ergänzt §A.1.

#### A.14.8 Hierarchie-Erweiterung durch User

Standard-UI-Element-Klassen wie `Edit`, `ComboBox`, `MenuItem`,
`Tab`, `Tree`, `List` etc. (Altprojekt-Verzeichnis `ui/edit.py`,
`ui/combobox.py` …) werden in den weiteren Phase-4-Sub-Phasen
nach `ui/text.py`, `ui/lists.py`, `ui/menus.py`, `ui/tabs.py`
portiert. `Button` und `CheckBox` decken §A.14.9 ab. Alle
Standard-Widgets folgen demselben Schema:

- subclassen `Control` (oder eine widget-spezifische Zwischen-
  klasse wie `AbstractButton`),
- registrieren sich über `__init_subclass__` mit
  `role=cls.__name__`,
- wrappen ein oder mehrere Capability-Patterns aus §A.13 in
  Pre/Perform/Post-Verträgen,
- liefern Convenience-Properties über `adapter.get_pattern(X,
  raise_exception=False)` für read-only Spiegel von Pattern-State.

Auswahl der Klasse pro Adapter erfolgt via WeightCalculator
(`@locator`-Score) — spezifische Rolle/Class-Name gewinnt über
generische `Element`-Default-Klasse.

#### A.14.9 Buttons (`ui/buttons.py`)

`AbstractButton`, `Button` und `CheckBox` als ersten Schritt der
Standard-Widget-Migration (Phase 4a). Weitere Button-Varianten
(`PushButton`, `Link`, `RadioButton`) folgen erst, wenn ein
konkreter Bedarf besteht — sie waren im Altprojekt leere Marker-
Subklassen und tragen ohne eigenes Verhalten nichts bei.

```
Control
└── AbstractButton              (register=False)
    ├── Button                  (role="Button")
    └── CheckBox                (role="CheckBox")
```

`AbstractButton` ist abstrakte Zwischenklasse (`register=False`,
`abstract activate()`) und bündelt Verhalten, das alle Button-
artigen Widgets teilen — primär die `text`-Property als
Convenience-Spiegel über das `TextContent`-Pattern.

```python
class AbstractButton(Control, register=False):
    """Context base for button-like widgets.

    Adds a `text` convenience over `TextContent` and declares an
    abstract primary `activate()` action that subclasses
    implement using the appropriate capability pattern.
    """

    @property
    def text(self) -> str:
        """The button's label text.

        Convenience shortcut over the `TextContent` pattern;
        returns the empty string when the adapter does not expose
        `TextContent`.
        """
        self.ensure_that(self._application_is_ready)
        content = self.adapter.get_pattern(TextContent, raise_exception=False)
        return content.text if content is not None else ''

    @abstractmethod
    def activate(self) -> None:
        """Trigger the widget's primary action."""
```

**`Button`** wrappt das `Activatable`-Pattern. `activate()` folgt
dem Pre/Perform/Post-Vertrag aus §A.14.5:

```python
class Button(AbstractButton):
    @override
    def activate(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)
```

`get_pattern(Activatable)` raises `PatternNotSupportedError`
(default), wenn der Adapter das Pattern nicht liefert — Phase 4a
verlangt den Provider-Pattern-Pfad. Ein Click-Fallback über
`MouseProxy.click()` ist Sache der Default-Proxy-Schicht
(Phase 4e, §A.14.22).

**`CheckBox`** wrappt das `Toggleable`-Pattern. Die Klasse fügt
`is_checked` / `is_unchecked` als Bequemlichkeits-Properties
hinzu sowie `check()` / `uncheck()` / `toggle()` / `set_state()`:

```python
class CheckBox(AbstractButton):
    @property
    def state(self) -> ToggleState:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Toggleable).state

    @property
    def is_checked(self) -> bool:
        return self.state is ToggleState.ON

    @property
    def is_unchecked(self) -> bool:
        return self.state is ToggleState.OFF

    @override
    def activate(self) -> None:
        """Primäre Aktion = abhaken (User-Intent)."""
        self.check()

    def check(self) -> None:
        self.set_state(ToggleState.ON)

    def uncheck(self) -> None:
        self.set_state(ToggleState.OFF)

    def toggle(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
        )
        self.adapter.get_pattern(Toggleable).toggle()
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def set_state(self, state: ToggleState) -> None:
        """Toggle bis `state` erreicht ist (max. 3 Iterationen für Tri-State)."""
        for _ in ToggleState:
            if self.state is state:
                return
            self.toggle()
```

`set_state` entspricht semantisch §A.14.1 („Activate" = primäre
User-Aktion, *nicht* das gleichnamige Pattern aufrufen):
`CheckBox.activate()` ruft `check()`, nicht `Toggleable.toggle()`.

**Tri-State.** `Toggleable.supports_three_state` zeigt an, ob
`state` legitim `INDETERMINATE` zurückgeben darf. `set_state` ist
gegenüber Tri-State sicher: die Schleife läuft maximal so oft
wie `len(ToggleState)` (=3), erreicht also auch
`INDETERMINATE → ON` über genau einen `toggle()`-Aufruf, wenn der
Provider die Reihenfolge `OFF → ON → INDETERMINATE → OFF`
implementiert. Bei zwei-Zustands-Toggles wird `INDETERMINATE`
übersprungen, der zweite Aufruf erreicht das Ziel.

**Predicate-Verifikation.** `toggle()` verlangt zusätzlich
`_element_is_not_readonly` (im Gegensatz zu `Button.activate()`),
weil ein read-only Toggle-Element den Zustand nicht ändern kann.
Buttons können nicht read-only sein im klassischen Sinn — ein
disabled Button blockt schon über `_element_is_enabled`.

#### A.14.10 Text (`ui/text.py`)

`Text` ist die Default-Context-Klasse für rein lesende
Text-Widgets — Labels, statische Texte, Status-Anzeigen,
read-only Display-Felder. Beschreibbare Felder sind nicht
`Text`, sondern `Edit` (§A.14.11) — die alte Legacy-Mischung
von „`Text` ist beschreibbar, `Edit(Text)` ist Marker-Alias"
wird hier nicht übernommen.

```
Control
└── Text                        (role="Text")
```

`Text` wrappt allein das `TextContent`-Pattern. Kein `set_text`,
kein `clear` — wer schreiben will, nutzt `Edit`.

```python
class Text(Control):
    """Read-only text widget (label, status text, …)."""

    @property
    def text(self) -> str:
        """The current text content."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).text

    @property
    def is_truncated(self) -> bool:
        """Whether the displayed text is shortened (e.g. ellipsis)."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).is_truncated

    @property
    def locale(self) -> str:
        """The BCP-47 locale tag for `text`, or empty if unknown."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).locale
```

`text` raises `PatternNotSupportedError`, wenn der Adapter
`TextContent` nicht liefert — Phase 4b verlangt den
Provider-Pattern-Pfad. Ein Display-Lookup-Fallback über das
Element-Bounds-Rechteck wäre Sache der Default-Proxy-Schicht
(Phase 4e, §A.14.22).

**`is_truncated`/`locale`.** Stellt `TextContent`-Properties
direkt durch. Eine Multi-Line-Eigenschaft existiert auf `Text`
bewusst nicht: ob eine reine Anzeige ein- oder mehrzeilig
gerendert wird, ist eine Layout-Frage ohne Verhaltens-Konsequenz
für den Test. Editierbare Felder unterscheiden ein-/mehrzeilig
dagegen sehr wohl (Tab- vs. Enter-Verhalten, Zeilenumbruch-
Akzeptanz) — `is_multi_line` lebt deshalb auf
`TextEditable`/`Edit` (§A.14.11), nicht auf `TextContent`/`Text`.

#### A.14.11 Edit (`ui/text.py`)

`Edit` ist die Default-Context-Klasse für beschreibbare
Eingabefelder — Single-Line-Edits, Multi-Line-Edits, Such-/
URL-/Passwort-Felder.

```
Control
└── Edit                        (role="Edit")
```

`Edit` lebt im selben Modul wie `Text`, weil beide die gleiche
Pattern-Familie (`TextContent` lesen) teilen, aber sie stehen
nicht in einer Vererbungsbeziehung — `Edit` ist kein „Text mit
Schreibfähigkeit", sondern ein eigenständiges Widget mit
eigenen Pre-Conditions (`focus`, `not_readonly`).

```python
class Edit(Control):
    """Editable text input widget."""

    @property
    def text(self) -> str:
        """The current text content."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).text

    @text.setter
    def text(self, value: str) -> None:
        self.set_text(value)

    @property
    def max_length(self) -> int | None:
        """The maximum length in characters, or `None` if unbounded."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextEditable).max_length

    @property
    def supports_password_mode(self) -> bool:
        """Whether the field can mask its content."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextEditable).supports_password_mode

    @property
    def is_multi_line(self) -> bool:
        """Whether the field accepts line breaks."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextEditable).is_multi_line

    def set_text(self, value: str) -> None:
        """Replace the current content with `value`."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(TextEditable).set_text(value)
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def clear(self) -> None:
        """Remove the current content."""
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(Clearable).clear()
        self.ensure_that(self._application_is_ready, raise_exception=False)
```

**Predicate-Verifikation.** `set_text` und `clear` verlangen
zusätzlich zu den Standard-Predicates (`active`, `in_view`,
`enabled`) noch `_element_is_not_readonly` und
`_control_has_focus`. Read-only-Status bezieht das Predicate aus
dem `Readable`-Pattern auf Element-Ebene (siehe §A.14.3); ein
zusätzliches `Edit.is_readonly` über `TextEditable.is_readonly`
gibt es bewusst nicht, um zwei konkurrierende Quellen für
denselben Status zu vermeiden. `TextEditable.is_readonly` bleibt
als interne Pattern-Eigenschaft erhalten — Default-Proxies
können es als Fallback heranziehen, wenn `Readable` fehlt
(Phase 4e). Der Focus-Check stellt sicher, dass die Tastatur-
Eingabe (im Default-Proxy-Fallback, §A.14.22) am richtigen
Widget landet — für den Provider-Pattern-Pfad ist er strenggenommen
nicht nötig, aber er hält das Verhalten zwischen Pattern- und
Fallback-Pfad konsistent.

**`set_text` als Property-Setter.** `Edit.text = "neu"` ist die
empfohlene Schreibweise; der Setter ruft intern `set_text`. Für
explizite Sequenzen (z. B. `clear` + `set_text`) bleibt
`set_text` direkt zugänglich.

**Pattern-Aufteilung.** `Edit` braucht alle drei Text-Patterns:
`TextContent` (lesen), `TextEditable` (schreiben + Constraints),
`Clearable` (Inhalt löschen). Adapter, die `Clearable` nicht
liefern, raisen beim `clear()`-Aufruf — eine Default-Sequenz
„select-all + delete" gehört in den Default-Proxy
(`EditProxy`, Phase 4e).

**`is_multi_line` als Editable-Constraint.** `is_multi_line` sitzt
auf `TextEditable`, nicht auf `TextContent` — die Eigenschaft
hat nur für editierbare Felder Verhaltens-Konsequenzen
(Tab- vs. Enter-Verhalten, Zeilenumbruch-Akzeptanz, andere
Predicates für `set_text` mit `\n`). Bei reinen Anzeige-Texten
ist sie eine reine Layout-Frage und wird auf `Text` deshalb gar
nicht erst angeboten.

**Trennung von `Text`.** `Text` und `Edit` sind unabhängige
Klassen, kein gemeinsames `AbstractText`. Begründung: die
einzige geteilte Methode wäre `text` (eine Zeile via
`TextContent`), und beide Widgets haben unterschiedliche
Pre-Conditions (`Text` braucht keinen Focus). Eine
Zwischenklasse wäre Code-Overhead ohne Gegenwert.

#### A.14.12 Item-Hierarchie (`ui/item.py`)

Items sind UI-Elemente innerhalb eines Containers — Listen-
einträge, Tree-Knoten, Tabellenzellen, Tabs, Menü-Einträge.
Sie unterscheiden sich von `Control` darin, dass sie typisch
**keinen eigenen Focus** halten (der Container ist fokussiert,
das Item ist „selektiert") und dass ihre Aktionen über
Container- oder Editor-Lifecycles laufen.

```
Element
└── Item                          (register=False, default_prefix="item")
    ├── SelectableItem            (register=False, + Selectable)
    ├── ExpandableItem            (register=False, + Expandable)
    └── EditableItem              (register=False, + HasEditor + TextEditable)
```

`Item`, `SelectableItem`, `ExpandableItem`, `EditableItem` sind
alle `register=False` — sie sind Capability-Mixins, keine
selbstständigen Rollen. Konkrete Klassen (`ListItem`, `TreeItem`,
`Cell`, `Row`, `TabItem`) erben von `Item` plus
einer beliebigen Kombination der Mixins per Mehrfachvererbung.
`MenuItem` erbt dagegen `Control` (siehe §A.14.24), da ein
Menü-Eintrag semantisch ein interaktives Control mit eigener
Sub-Hierarchie ist und nicht der Inhalt eines Auswahl-Containers.

```python
class Item(Element, register=False):
    """Container element (list entry, tree node, cell, …)."""

    default_prefix: ClassVar[str] = "item"

    @property
    def text(self) -> str:
        """The item's display text."""
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).text


class SelectableItem(Item, register=False):
    """Item that can be selected within its container."""

    @property
    def is_selected(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Selectable).is_selected

    def select(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        selectable = self.adapter.get_pattern(Selectable)
        if not selectable.is_selected:
            selectable.select()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class ExpandableItem(Item, register=False):
    """Item that can be expanded/collapsed (tree node, …)."""

    @property
    def is_expanded(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Expandable).is_expanded

    @property
    def can_expand(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Expandable).can_expand

    def expand(self) -> bool:
        if not self.can_expand or self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(Expandable).expand()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def collapse(self) -> bool:
        if not self.can_expand or not self.is_expanded:
            return False
        self.ensure_that(self._toplevel_parent_is_active, self._element_is_in_view)
        self.adapter.get_pattern(Expandable).collapse()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True


class EditableItem(Item, register=False):
    """Item whose value can be edited inline (cell editor, …)."""

    def set_text(self, value: str) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        editor = self.adapter.get_pattern(HasEditor)
        editor.open_editor()
        try:
            self.adapter.get_pattern(TextEditable).set_text(value)
        finally:
            editor.accept()
        self.ensure_that(self._application_is_ready, raise_exception=False)

    def clear(self) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        editor = self.adapter.get_pattern(HasEditor)
        editor.open_editor()
        try:
            self.adapter.get_pattern(Clearable).clear()
        finally:
            editor.accept()
        self.ensure_that(self._application_is_ready, raise_exception=False)
```

**Mehrfachvererbung.** `TreeItem(SelectableItem, ExpandableItem)`
und `EditableCell(Cell, EditableItem)` kombinieren orthogonale
Capabilities. Die Mixins berühren sich nicht (jedes greift auf
ein anderes Pattern zu); Python-MRO ist hier unkritisch.

**`text`-Setter bewusst nicht auf `Item`.** Der Property-Setter
`item.text = "..."` würde `set_text` voraussetzen und damit den
Editor-Lifecycle implizieren. Da nicht jedes Item editierbar ist,
ist `text` auf `Item` rein lesend; `EditableItem.set_text(value)`
ist die explizite Schreib-API.

**`HasEditor.open_editor()` + `accept()`.** Der Lifecycle wird in
einem `try/finally` umschlossen, damit ein Fehler in `set_text`
den Editor nicht offen lässt. Die `cancel()`-Variante des
Patterns ist im Public-API der Item-Klassen (noch) nicht
exposed — wer sie braucht, ruft das Pattern direkt an.

#### A.14.13 `List` und `ListItem` (`ui/lists.py`)

```
Element                          Element
└── Control                      └── Item
    └── List                         └── SelectableItem
                                          └── ListItem
```

`List` ist ein `Control` mit `ItemContainer`-Pattern; `ListItem`
ein `SelectableItem`. Items werden über `scope='children'`
gesucht — eine Liste enthält ihre Einträge direkt.

```python
class List(Control):
    @property
    def item_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).item_count

    def get_items(self, *, locator: Locator | None = None) -> list[ListItem]:
        return self.get_all(ListItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[ListItem]:
        return self.iter_all(ListItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> ListItem:
        return self.get(ListItem, locator=locator, scope='children')

    def select(self, *, locator: Locator | None = None) -> ListItem:
        item = self.get_item(locator=locator)
        item.select()
        return item


class ListItem(SelectableItem):
    pass
```

`List.select(...)` ist ein Convenience-Wrapper: holt das Item
über die Locator-Argumente und ruft `select()`. Liefert das Item
zurück, damit der Caller weiterarbeiten kann.

#### A.14.14 `Tree` und `TreeItem` (`ui/tree.py`)

```
Element                          Element
└── Control                      └── Item
    └── Tree                         └── SelectableItem  ExpandableItem
                                              \           /
                                               TreeItem
```

```python
class Tree(Control):
    @property
    def item_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).item_count

    @property
    def column_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).column_count

    def get_items(self, *, locator: Locator | None = None) -> list["TreeItem"]:
        return self.get_all(TreeItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator["TreeItem"]:
        return self.iter_all(TreeItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> "TreeItem":
        return self.get(TreeItem, locator=locator, scope='children')


class TreeItem(SelectableItem, ExpandableItem):
    @property
    def item_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).item_count

    def get_items(self, *, locator: Locator | None = None) -> list["TreeItem"]:
        return self.get_all(TreeItem, locator=locator, scope='children')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator["TreeItem"]:
        return self.iter_all(TreeItem, locator=locator, scope='children')

    def get_item(self, *, locator: Locator | None = None) -> "TreeItem":
        return self.get(TreeItem, locator=locator, scope='children')
```

`TreeItem` ist sowohl `SelectableItem` als auch `ExpandableItem`
**und** ein Container seiner eigenen Kinder — daher die
`get_items`/`item_count`-Methoden auch auf der Item-Klasse.

#### A.14.15 `Table`, `Row`, `Cell` (`ui/table.py`)

```
Element                          Element
└── Control                      └── Item
    └── Table                        ├── Cell
                                     │   └── EditableCell  EditableItem
                                     │            \         /
                                     │             EditableCell
                                     └── Row
```

```python
class Cell(Item):
    pass


class EditableCell(Cell, EditableItem):
    pass


class Row(Item):
    @property
    def column_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).column_count

    def get_cells(self, *, locator: Locator | None = None) -> list[Cell]:
        return self.get_all(Cell, locator=locator, scope='children')

    def iter_cells(self, *, locator: Locator | None = None) -> Iterator[Cell]:
        return self.iter_all(Cell, locator=locator, scope='children')

    def get_cell(self, *, locator: Locator | None = None) -> Cell:
        return self.get(Cell, locator=locator, scope='children')


class Table(Control):
    @property
    def row_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).row_count

    @property
    def column_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).column_count

    def get_rows(self, *, locator: Locator | None = None) -> list[Row]:
        return self.get_all(Row, locator=locator, scope='children')

    def iter_rows(self, *, locator: Locator | None = None) -> Iterator[Row]:
        return self.iter_all(Row, locator=locator, scope='children')

    def get_row(self, *, locator: Locator | None = None) -> Row:
        return self.get(Row, locator=locator, scope='children')
```

**`Cell` als Marker, `EditableCell` getrennt.** Die meisten
Tabellen sind read-only (Anzeige-Tabellen, Reports). Editierbare
Tabellen liefern für ihre Zellen Adapter mit `HasEditor` —
solche Adapter werden über `@context(role="Cell", properties={...})`
auf die `EditableCell`-Klasse gemappt (Gewichtung über
`framework_id`/`class_name`/`properties` aus §5a). Der Standardpfad
ohne weitere Heuristik liefert `Cell`; ein `EditableCell`-Aufrufer
weiß explizit, dass die Zelle editierbar ist.

**`Row` als `Item`-Container.** `Row` erbt von `Item`, weil sie
in einem Container (`Table`) lebt und i.d.R. selektierbar ist —
wenn der Adapter ein `Selectable` liefert, exposed das die
Standard-`select()`-API über `SelectableItem`-Mixin (offen für
Anwendungen, die das brauchen — derzeit erbt `Row` nur von `Item`,
da Row-Selektion seltener ist als Cell-Selektion; bei Bedarf
nachziehen).

#### A.14.16 `ComboBox` (`ui/combobox.py`)

```
Control
└── ComboBox
```

`ComboBox` kombiniert drei Capability-Bereiche:

- **Expand/Collapse** (`Expandable`-Pattern): das Dropdown auf-
  und zumachen.
- **Item-Selektion** (`ListItem`-Kinder via Locator): aus dem
  geöffneten Dropdown einen Eintrag selektieren.
- **Editierbarer Text** (`TextContent`/`TextEditable`-Patterns,
  optional): bei editierbaren ComboBoxen den Anzeigetext direkt
  setzen.

```python
class ComboBox(Control):
    @property
    def can_expand(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Expandable).can_expand

    @property
    def is_expanded(self) -> bool:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(Expandable).is_expanded

    def expand(self) -> bool:
        if self.is_expanded:
            return False
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._control_has_focus,
        )
        self.adapter.get_pattern(Expandable).expand()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def collapse(self) -> bool:
        if not self.is_expanded:
            return False
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._control_has_focus,
        )
        self.adapter.get_pattern(Expandable).collapse()
        self.ensure_that(self._application_is_ready, raise_exception=False)
        return True

    def get_items(self, *, locator: Locator | None = None) -> list[ListItem]:
        with self._expanded():
            return self.get_all(ListItem, locator=locator, scope='descendants')

    def iter_items(self, *, locator: Locator | None = None) -> Iterator[ListItem]:
        with self._expanded():
            yield from self.iter_all(ListItem, locator=locator, scope='descendants')

    def get_item(self, *, locator: Locator | None = None) -> ListItem:
        with self._expanded():
            return self.get(ListItem, locator=locator, scope='descendants')

    def select(self, *, locator: Locator | None = None) -> ListItem:
        with self._expanded():
            item = self.get(ListItem, locator=locator, scope='descendants')
            item.select()
            return item

    @property
    def selected(self) -> ListItem | None:
        # Locator-based lookup — returns None if no item is selected.
        ...

    @property
    def text(self) -> str:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(TextContent).text

    @text.setter
    def text(self, value: str) -> None:
        self.set_text(value)

    def set_text(self, value: str) -> None:
        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
            self._element_is_not_readonly,
            self._control_has_focus,
        )
        self.adapter.get_pattern(TextEditable).set_text(value)
        self.ensure_that(self._application_is_ready, raise_exception=False)

    @contextmanager
    def _expanded(self) -> Iterator[None]:
        was_collapsed = self.expand()
        try:
            yield
        finally:
            if was_collapsed:
                self.collapse()
```

**`_expanded()`-Context-Manager.** Faktorisiert das Legacy-Muster
(`expanded = self.expand(); try: ...; finally: if expanded:
self.collapse()`) in einen wiederverwendbaren Helper. `expand()`
liefert `True`, wenn das Dropdown vor dem Aufruf zu war —
`collapse()` läuft nur dann am Ende.

**Items via `scope='descendants'`.** Ein offenes ComboBox-
Dropdown hängt je nach Toolkit nicht direkt unter dem ComboBox-
Adapter (Popup-Window, Overlay-Layer). Descendants-Scope deckt
beide Fälle ab.

**Locator-basierte `selected`-Property.** Der Altcode nutzt
`get_item(IsSelected=True)`. Die neue Version geht denselben
Weg über die Locator-API auf der Item-Adapter-`Selectable`-
Property. Implementations-Detail: liefert `None` statt zu
raisen, wenn keine Selektion existiert (gängiger Initial-Zustand
einer ComboBox).

**Text-Read auch ohne Editierbarkeit.** `text` liest immer über
`TextContent` — auch eine read-only ComboBox hat einen sichtbaren
Text. Erst der Setter (`set_text`) verlangt `TextEditable` und
die Edit-Predicates (`not_readonly`, Focus).

#### A.14.17 Pattern-Spec — `Selectable`

```python
class Selectable(PatternBase):
    pattern_name = "org.platynui.patterns.Selectable"

    @property
    @abstractmethod
    def is_selected(self) -> bool: ...

    @abstractmethod
    def select(self) -> None: ...
```

`Selectable` bündelt Status-Read und Aktion in einem Pattern
(siehe Rev. 32 zur flachen Hierarchie). Eine `deselect()`-Methode
gibt es bewusst nicht: Single-Selection-Container deselektieren
implizit beim nächsten `select()`, Multi-Selection-Container sind
selten genug, dass eine zweite Methode auf Item-Ebene das Modell
unnötig aufbläht — wer Multi-Selection braucht, ruft im Test
die nächste Selektion oder einen container-spezifischen
Toggle-Modus auf.

**`is_selectable` weggelassen.** Der Altcode hatte
`HasSelected.is_selectable`. In der neuen Modellierung ist das
implizit: ein Adapter, der `Selectable` exposed, ist selektierbar.
Wer prüfen will, ob die Capability vorhanden ist, fragt
`adapter.get_pattern(Selectable, raise_exception=False) is not None`.

#### A.14.18 Pattern-Spec — `Expandable`

```python
class Expandable(PatternBase):
    pattern_name = "org.platynui.patterns.Expandable"

    @property
    @abstractmethod
    def can_expand(self) -> bool: ...

    @property
    @abstractmethod
    def is_expanded(self) -> bool: ...

    @abstractmethod
    def expand(self) -> None: ...

    @abstractmethod
    def collapse(self) -> None: ...
```

**`can_expand` bleibt erhalten.** Anders als `is_selectable`
hat `can_expand` einen sinnvollen Use-Case: ein TreeItem ohne
Kinder kann technisch das `Expandable`-Pattern liefern, aber
faktisch nichts auf-/zumachen. `can_expand` erlaubt der UI-Klasse,
diesen Zustand vor dem Action-Call abzufragen.

#### A.14.19 Pattern-Spec — `HasEditor`

```python
class HasEditor(PatternBase):
    pattern_name = "org.platynui.patterns.HasEditor"

    @abstractmethod
    def open_editor(self) -> None: ...

    @abstractmethod
    def accept(self) -> None: ...

    @abstractmethod
    def cancel(self) -> None: ...
```

Beschreibt den Inline-Editor-Lifecycle für editierbare
Container-Items (Cell-Editor, Tree-Item-Rename, …). Die
`EditableItem`-UI-Klasse umschließt die Sequenz mit
`open_editor → set_text/clear → accept` in einem `try/finally`,
damit Fehler nicht den Editor offen lassen.

`cancel()` ist Teil des Patterns (nicht jeder Editor lässt sich
mit `accept` korrekt schließen), wird aber von den Default-Item-
Methoden nicht aufgerufen — wer den Edit verwerfen will, ruft das
Pattern direkt.

#### A.14.20 Pattern-Spec — `ItemContainer`

```python
class ItemContainer(PatternBase):
    pattern_name = "org.platynui.patterns.ItemContainer"

    @property
    @abstractmethod
    def item_count(self) -> int: ...

    @property
    @abstractmethod
    def row_count(self) -> int: ...

    @property
    @abstractmethod
    def column_count(self) -> int: ...
```

Bündelt typisierte Größenangaben für Listen-, Tree- und
Tabellencontainer. Im Altprojekt wurden `ItemCount`/`RowCount`/
`ColumnCount` als generische Attribute über
`Properties.get_property_value("ItemCount")` gelesen — ein Pattern
mit `Properties`-Marker, der jeden String akzeptiert, ist explizit
**nicht** Teil des neuen Modells (siehe §5, Hinweis nach der
Pattern-Liste). Stattdessen hat jeder Container-Typ genau die
Eigenschaften, die für ihn sinnvoll sind:

| Container | sinnvolle Properties |
|---|---|
| `List` | `item_count` |
| `Tree`, `TreeItem` | `item_count`, `column_count` |
| `Table` | `row_count`, `column_count` |
| `Row` | `column_count` |

**Drei Properties statt drei Patterns.** Ein splittendes Modell
(`ListContainer`/`TreeContainer`/`TableContainer`) wäre semantisch
sauberer, würde aber Adapter zwingen, denselben Provider-State
in drei verschiedenen Pattern-Implementierungen zu wrappen. Der
unbenötigte Property-Read raised `NotImplementedError` in den
Default-Pattern-Implementierungen; konkrete Adapter implementieren
nur die für ihre Rolle relevanten Properties.

Eine Properties-Property mit z. B. `column_count` für eine reine
`List` ist also ein Programmierfehler — die UI-Klasse ruft sie
nicht, der Adapter exposed sie nicht.

#### A.14.21 Item-Lifecycle und Predicates

Die meisten Item-Aktionen brauchen kein separates Focus-Predicate:
der Container ist fokussiert, das Item wird durch Selektion
„aktiv". `SelectableItem.select()` und `ExpandableItem.expand()`
verlangen daher nur das Standard-Tripel
(`_toplevel_parent_is_active`, `_element_is_in_view`,
`_element_is_enabled`).

`EditableItem.set_text()` braucht zusätzlich nichts mehr —
`HasEditor.open_editor()` zwingt die Anwendung in den Editor-
Modus, und das Pattern selbst ist verantwortlich, das Item
zuvor zu fokussieren. Sollte sich diese Annahme in der Praxis
nicht halten, kommt ein `_item_is_active`-Predicate dazu, das
über `Activatable` den Doppelklick-Pfad abdeckt — bislang nicht
umgesetzt, da der Altcode-Pfad (`activate()` über `Activatable`)
den gleichen Effekt hatte und in den meisten Toolkits redundant
ist.

#### A.14.22 Offene Punkte

- **`BringIntoViewable`-Pattern** (siehe §A.14.3, `_element_is_in_view`):
  perspektivisch eigenes Pattern für UIA `IScrollItemProvider.ScrollIntoView`
  / AT-SPI `Component.scroll_to`. Aktuell pragmatisch deferred (d1):
  Predicate failt bei out-of-view ehrlich. Post-Phase-4.
- **`Scrollable`-Pattern**: post-Phase-4 (siehe §A.13).
- **`HasUserInput.accepts_user_input()`**-Implementierung im Default-
  Window-Proxy (§A.13): aktuell Best-Effort über vorhandene Window-
  State-Bits (`is_active`, `is_modal_dialog_blocking`); finale Heuristik
  beim Implementieren festlegen.
- **`exit()` Stage 2 — Cross-Plattform-Process-Polling**: konkrete
  Wahl zwischen reinem `os.kill(pid, 0)`-Polling (Unix) plus
  Windows-`ctypes`-Pfad versus späterem Wechsel zu `psutil` ist eine
  Implementations-Entscheidung beim Code-Schreiben.

#### A.14.23 `TabList` und `TabItem` (`ui/tabs.py`)

`TabList` ist ein Container mit `TabItem`-Kindern. Ein TabItem
erbt `SelectableItem` (analog `ListItem`); die TabList exponiert
`items`/`iter_items`/`get_item` und einen `select(...)`-Shortcut.

```
Element
└── Control
    └── TabList                       (default_prefix="control")
└── Item
    └── SelectableItem
        └── TabItem                   (default_prefix="item")
```

```python
class TabItem(SelectableItem):
    """A selectable tab within a TabList."""


class TabList(Control):
    """Container of TabItems."""

    @property
    def item_count(self) -> int:
        self.ensure_that(self._application_is_ready)
        return self.adapter.get_pattern(ItemContainer).item_count

    @property
    def items(self) -> list[TabItem]:
        return list(self.iter_items())

    def iter_items(self) -> Iterator[TabItem]:
        return self.iter_all(TabItem, scope=LocatorScope.Children)

    def get_item(self, *args: object, **kwargs: object) -> TabItem:
        return self.get(TabItem, scope=LocatorScope.Children, *args, **kwargs)

    def select(self, *args: object, **kwargs: object) -> TabItem:
        item = self.get_item(*args, **kwargs)
        item.select()
        return item
```

`select(...)` ist eine Bequemlichkeits-API, die den passenden
`TabItem` lokalisiert und `select()` darauf aufruft — typische
Keyword-Form `Select Tab    name=Settings`.

#### A.14.24 `Menu`, `MenuBar`, `MenuItem` (`ui/menus.py`)

Drei eng verwandte Klassen, die Menü-Hierarchien modellieren.

```
Element
└── Control
    ├── Menu                          (popup-/sub-menu container)
    ├── MenuBar                       (top-level menu strip)
    └── MenuItem                      (individual entry, may have submenu)
```

`MenuItem` erbt **`Control`**, nicht `Item`. Begründung:

- Menü-Einträge sind keine Auswahl-Inhalte eines Containers (wie
  `ListItem` oder `Cell`), sondern eigenständige interaktive
  Controls. Sie haben oft selbst Untermenüs und reagieren auf
  Tastenkürzel, Hover-Aktivierung und nicht zuletzt auf
  echte Aktivierung (Click).
- Praktische Konsequenz: ein `MenuItem` exponiert eine
  `activate()`-Methode wie ein Button, kein `select()` wie ein
  ListItem.

`MenuItem.activate()` muss die Menü-Hierarchie vorbereiten:
jedes Vorgänger-`MenuItem` zwischen Wurzel-Menu (oder MenuBar)
und self muss zuerst geöffnet werden, sonst ist self gar nicht
sichtbar/anklickbar. Algorithmus: vom self ausgehend nach oben
laufen, alle `MenuItem`-Vorfahren sammeln, in der Reihenfolge
*außen → innen* `expand()` aufrufen (übersprungen wenn bereits
expandiert), dann auf self `Activatable.activate()`.

```python
class MenuItem(Control):
    """A single menu entry that may host a submenu."""

    def activate(self) -> None:
        ancestors: list[MenuItem] = []
        parent = self.parent
        while parent is not None and not isinstance(parent, (Window, DesktopBase)):
            if isinstance(parent, MenuItem):
                ancestors.append(parent)
            parent = parent.parent

        # Open from outermost to innermost.
        for ancestor in reversed(ancestors):
            if ancestor.adapter.get_pattern(Expandable, raise_exception=False):
                _expand_if_needed(ancestor)

        self.ensure_that(
            self._toplevel_parent_is_active,
            self._element_is_in_view,
            self._element_is_enabled,
        )
        self.adapter.get_pattern(Activatable).activate()
        self.ensure_that(self._application_is_ready, raise_exception=False)


class Menu(Control):
    """Popup or submenu container of MenuItems."""


class MenuBar(Control):
    """Top-level menu strip, typically anchored to a Window."""
```

`Menu` und `MenuBar` sind in dieser Phase reine Container ohne
eigene Methoden — die Aktion sitzt am `MenuItem`. Falls später
ein `MenuBar.activate("File", "Open")`-Convenience benötigt wird
(Pfad-Aktivierung), wird er hier ergänzt.

`Expandable` an `MenuItem` ist optional: viele Blatt-Einträge
liefern es gar nicht. Der Vorfahren-Walk fragt das Pattern daher
mit `raise_exception=False` ab und überspringt Vorfahren ohne
Expand-Pattern stillschweigend (z. B. ein `MenuBar`-Eintrag, der
sich auf Hover öffnet und kein explizites `Expandable` exposeniert).


## 10. Migrations-Reihenfolge

Reihenfolge der Implementierung in Phasen, aufsteigend nach
Abhängigkeit. Jede Phase ist eigenständig testbar; spätere Phasen
bauen auf früheren auf.

### Phase 1 — Fundament

1. `core/types.py` — `TypeAlias`es (`PatternName`, `RoleName`,
   `FrameworkId`) als freie String-Aliases; zusätzlich
   Re-Export `Point`/`Rect` aus `platynui_native` (Rev. 17)
2. `core/settings.py` — als `@dataclass(frozen=True, slots=True, kw_only=True)`,
   mit `with`-Block und RF-Variablen-Brücke (siehe §9a.1)
3. `core/exceptions.py` — Hierarchie nach §9a.2 (Typo-Fix
   `PlatyUiError` → `PlatynUIError`)
4. `core/wait.py` — `wait_for` nach §9a.3
5. `core/ensure.py` — `ensure_that` mit Stage-Memo und re-entrant
   Thread-Local-Stack (siehe §9a.3); Decorator mit `ParamSpec`/
   `Concatenate` typisiert, `match`/`case` für Outcome
6. `core/predicate.py` — `@predicate`-Decorator (für `ensure_that`-
   Standard-Predicates und Context-Predicates, siehe §A.3)
7. `core/weight_calculator.py` — Port aus Altprojekt, `MatchCriteria`
   als Dataclass; `attribute_value(name, namespace)`-basiert (Rev. 15)
8. ~~`core/technology.py`~~ — entfallen (siehe Rev. 35); die
   Nummerierung der nachfolgenden Punkte bleibt zur Stabilität der
   Querverweise erhalten.
9. `core/locator.py` — neu, XPath-basiert über Rust, API nach §A.6
   (`@overload`, Builder-Methoden geben `Self` zurück, `copy_from`-
   Vererbung; `attributes` mit Tupel- oder String-Keys; Default-NS
   per Klassenattribut `default_attribute_namespace`; freie
   PascalCase-Kwargs nach Rev. 16)
10. `core/patterns/` — **vorgezogen aus Phase 2 Punkt 11**, weil
    `Locator` und `WeightCalculator` bereits `pattern_name`
    referenzieren. Pattern-Klassen nach §5 (Element, TextContent,
    TextEditable, Clearable, Toggleable, Activatable, Focusable —
    konsolidiert in Rev. 17, parallel zu den Rust-Capability-Gruppen
    in `crates/core/src/ui/attributes.rs`). Ohne Default-
    Implementierungen (`patterns/defaults.py` bleibt Phase 4).

**Ergebnis:** ~750 LOC Core-Infrastruktur + Pattern-ABCs, unabhängig
testbar (119 pytest, mock-basiert).

### Phase 2 — Adapter-Schicht

11. `core/adapter.py` — Interface
12. `core/adapter_proxy.py` — `AdapterProxy`, `PatternProxyFactory`,
    `@pattern_proxy_for` (portiert aus altem `adapterproxy.py`, nur
    umbenannt)
13. `core/adapters/ui_node.py` — `UiNodeAdapter`, der `UiNode` aus
    `platynui_native` umhüllt; mappt Native-Patterns auf Python-Patterns
    (`platynui_native.Focusable` → `core.patterns.Focusable` usw.).
    Siehe §A.4a. Exposed `native_node` als Public-Property für die
    `RuntimeAdapterFactory`.
14. `core/runtime.py` — Process-wide Native-Runtime-Singleton (§A.5).
15. `core/adapter_factory.py` — `AdapterFactory` (ABC),
    `RuntimeAdapterFactory` (Default), Singleton-Accessor
    `adapter_factory` (§A.4b). Setzt §A.4a (`native_node`) und §A.5
    (`runtime.current.evaluate*`) voraus.
16. `core/devices.py` — `MouseProxy`/`KeyboardProxy` über
    `platynui_native.Runtime`

(Punkt 11 „`core/patterns/` — Pattern-ABCs" wurde nach Phase 1
vorgezogen, siehe Phase-1-Punkt 10. Ein dedizierter Python-`MockAdapter`
entfällt; Tests gegen den UI-Tree nutzen den Rust-Mock-Provider via
`Runtime.new_with_mock()`. Siehe §A.11.)

### Phase 3 — Context-Schicht

17. `core/context.py` — `ContextBase`, `ContextFactory`, `@context`
18. `core/descriptor.py` — `ElementDescriptor[PatternT]` (aus
    BareMetal extrahieren, gemeinsam nutzen)

### Phase 4 — UI-Klassen + Standard-Proxies

16. `ui/proxies/base.py`, `ui/element.py`, `ui/control.py`
17. `ui/proxies/standard.py` + `ui/buttons.py` — Button/CheckBox/RadioButton
18. `ui/proxies/window.py` + `ui/window.py` — Window/Dialog
19. `ui/proxies/text.py` + `ui/text.py` + `ui/combobox.py`
20. `ui/proxies/list_tree.py` + `ui/lists.py` + `ui/tree.py` + `ui/table.py`
21. `ui/menus.py`, `ui/tabs.py`, `ui/desktop.py`, `ui/application.py`

> Implementierungsreihenfolge (siehe `python-migration-status.md`,
> Phase 4): die UI-Teile der Items 17–21 werden vor allen Default-
> Proxies gebaut. Eine abschließende Sub-Phase bündelt
> `ui/proxies/base.py` und sämtliche Widget-Proxies, sobald reale
> Click-/Tastatur-Fallbacks motiviert sind. Die Item-Nummerierung
> oben dokumentiert die konzeptionelle Kopplung UI↔Proxy, nicht die
> Reihenfolge der Commits.

### Phase 5 — Keywords + Robot-Library

22. `keywords/*.py` — semantische Keywords
23. `__init__.py` als Robot-Library
24. Libdoc-Generierung ins CI

### Phase 6 — Iterative Erweiterungen

- Weitere Provider-Patterns in Rust hinzufügen, wenn ein konkreter Bedarf
  auftritt (siehe §6.2). Pro neues Pattern: Trait + Provider-Impl(s) +
  Mapping in `UiNodeAdapter` auf ein Python-Pattern. UI-Klassen und
  Proxies bleiben unverändert.

## 11. Drastische Vereinfachungen gegenüber dem Altprojekt

### 11.1 `Locator` (433 → ~120 LOC)

XPath-Aufbau dramatisch kürzer dank Rust-XPath-2.0-Engine. Wegfallend:
manueller Path-Aufbau mit Achsen-Präfix-Logik, Index/Position-Spezialfälle,
parent/child-Relativierungen (Rust kennt stabile `runtime_id`s).

### 11.2 `ContextBase` (473 → ~250 LOC)

Wegfallend:
- Adapter-Validierung-Schleife (`UiNode` macht Lebenszeit selbst)
- Property-Read-Convenience-Methoden (`UiNode.attribute(...)` direkt)
- WeakSet-basiertes Children-Tracking — Rust-Cache regelt
- `invalidate()`-Propagation — `runtime.clear_cache()` reicht

Bleibend (1:1):
- Locator-Verwaltung, Parent-Beziehungen
- `ensure_that` / `wait_for`
- `get`/`get_all`/`get_child`/`ancestor`/`iter_all`
- Context-Manager (`with MyWindow() as w:`)

### 11.3 `ensure.py` (155 → ~80 LOC)

Wegfallend: Predicate-Cache mit ad-hoc Eviction, Sonderfall-Hooks für
Adapter-Lifecycle, manuelle `failed_func`-Registrierung pro Predicate.
Bleibend: **Re-entrant Thread-Local-Stack** (siehe §A.3) — wird für
verschachtelte `ensure_that`-Aufrufe in Element-Predicates gebraucht
und ist nicht ersetzbar; `failed_func` als optionaler Parameter
(Default: `context.invalidate`); Stage-Memo für stabile
Multi-Predicate-Verifikation.

### 11.4 `WeightCalculator` (114 → ~110 LOC)

**Mechanismus 1:1 übernommen** (gewichtetes Multi-Kriterien-Match —
elegant, hebt Spezialfälle über generische). Einzige inhaltliche
Änderung: das alte Doppel-Kriterium `properties[k]==v` /
`native_properties[k]==v` wird zu **einem** Kriterium
`attributes[(ns, name)] == v`, weil Adapter (Rust und alle künftigen)
Attribute jetzt einheitlich über den `(namespace, name)`-Schlüsselraum
exposen (siehe §A.4 / §4.1). LOC bleibt praktisch identisch — die
Cache-Map nutzt Tupel-Keys statt zwei separate Dicts.

## 11a. Testing-Strategie

Tests laufen auf **drei komplementären Ebenen**. Zielpublikum der
Bibliothek ist Robot Framework — Acceptance-Tests in `.robot`-Syntax
sind das Oberflächenziel, aber nicht der alleinige Test-Kanal.

```
                    ┌─────────────────────────────┐
                    │  Ebene 3: Robot Framework   │  Acceptance
                    │     .robot Suites           │  (Keyword-Syntax)
                    ├─────────────────────────────┤
                    │  Ebene 2: pytest            │  Python-Unit/
                    │  (Rust-Mock via Maturin     │  -Integration
                    │   + Inline-Fakes für ABC)   │
                    ├─────────────────────────────┤
                    │  Ebene 1: cargo nextest     │  Rust-Unit/
                    │  (Rust-Mock-Provider)       │  -Integration
                    └─────────────────────────────┘
```

### 11a.1 Ebene 1 — Rust-Tests (`cargo nextest`)

**Scope:** Runtime-Orchestrierung, XPath-Parser/Evaluator, Provider-
Implementierungen, PyO3-Bindings (sofern reine Rust-Logik).

**Werkzeuge:**
- `cargo nextest run --workspace` (Standard)
- `cargo nextest run -p platynui-runtime --features mock-provider`
- Der Rust-`provider-mock` ist bereits umfangreich (~3259 LOC,
  `tree.rs`/`input.rs`/`window.rs`/`focus.rs`/`events.rs`) und
  unterstützt XML-Fixtures via `quick_xml::de`.

**Ergänzungen, die evtl. nötig werden** (on-demand, nicht auf Verdacht):
- Property-Change-Events für `wait_for`-Tests
- Scripted Behavior: MockNodes, die nach einer Aktion State ändern
- Timing-Injection: kontrollierte Delays für Timeout-Tests
- Failure-Injection: Patterns, die gezielt Exceptions werfen

### 11a.2 Ebene 2 — Python-Tests (`pytest`)

Zwei komplementäre Spielarten, je nach Test-Ziel:

**2a. Inline-Fakes für ABC-/Algorithmus-Tests**
- Kein Rust-Build nötig.
- Lokale `class _FakeAdapter(Adapter)` o.ä. direkt im Test-Modul; nicht
  als öffentliches Test-API gehoben.
- Für: `Adapter`-ABC-Resolution-Algorithmus, `AdapterProxy`-Komposition,
  `PatternProxyFactory`-Match-Logik, `WeightCalculator`,
  `wait_for`/`ensure_that`-Re-Entrancy — alles, was rein in Python
  liegt und keinen UI-Tree braucht.

**2b. Tests gegen den Rust-Mock-Provider**
- Build: `uv run maturin develop -m packages/native/Cargo.toml --features mock-provider`
- Für: `UiNodeAdapter` (Native-Pattern-Mapping, PyO3-Bindings),
  `ContextBase.get`-End-to-End, UI-Klassen-Hybrid-Form, Keyword-
  Outcome-Verträge — alles, was den realen UI-Tree-Pfad durchläuft.
- Stub-/Spy-Verhalten wird via `AdapterProxy`-Overlays realisiert
  (Proxy mixt das gewünschte Pattern als Spy ein).

### 11a.3 Ebene 3 — Robot-Framework-Acceptance-Tests

**Zielbild:** `.robot`-Suites sind *die* primäre Nutzer-Erfahrung.
Wir müssen sie früh haben, nicht erst am Ende — sobald UI-Klassen
existieren (ab Phase 3), schreiben wir parallel Smoke-Suites.

**3a. RF gegen Rust-Mock-Provider** (CI-freundlich, deterministisch)
- Python-Aufruf: `Runtime.new_with_mock()` (setzt `mock-provider`-
  Feature-Build voraus — analog zur bestehenden BareMetal-Praxis mit
  `use_mock=${true}`)
- Läuft headless auf Linux/macOS/Windows ohne Display-Server
- **Coverage:** Keyword-Semantik, Locator-Syntax, Pre/Post-
  Conditions, Outcome-Vertrag, Context-Verwendung

**3b. RF gegen echte Adapter** (plattformabhängig)
- Läuft gegen `apps/test-app-egui` oder plattform-native Test-Apps
- **Coverage:** Echte Accessibility-API (UIA/AT-SPI/AX), Wayland-
  Compositor, Plattform-Integration
- Separate CI-Jobs pro OS; nightly oder on-demand

### 11a.4 RF-Mock-Harness — benötigte Keywords

Für RF-Tests gegen den Rust-Mock brauchen wir ein kleines
RF-Library-Set (idealerweise als Teil von `PlatynUI.Mock` oder
als Erweiterung der BareMetal-Library):

```robotframework
*** Settings ***
Library    PlatynUI    use_mock=${true}
Library    PlatynUI.Mock    # Mock-Steuerung

*** Test Cases ***
Login Dialog Becomes Enabled After Typing
    Load Mock Tree    fixtures/login-dialog.xml
    Type Text    ${USERNAME_FIELD}    alice
    Set Mock Property    ${LOGIN_BUTTON}    IsEnabled    True
    Activate    ${LOGIN_BUTTON}
    Wait For Window    name=Dashboard
```

**Benötigte Mock-Library-Keywords:**
- `Load Mock Tree  <xml-fixture>` — deklarativer Tree-Load
- `Set Mock Property  <element>  <name>  <value>` — State-Mutation
- `Set Mock Pattern Behavior  <element>  <pattern>  <script>` —
  Scripted Actions (z.B. „Klick öffnet Window X nach 100ms")
- `Clear Mock Tree` — Tear-down

**Implementierungsgrundlage:** `tree.rs` hat bereits
`quick_xml::de::from_str` (Z.11). Set-Property/Set-Behavior sind
Erweiterungen über bestehende `MockNode`-Mutationspfade.

### 11a.5 Test-Matrix

| Test-Gegenstand                    | nextest | pytest(Inline-Fakes) | pytest(Rust-Mock) | RF(Rust-Mock) | RF(echt) |
|------------------------------------|:-------:|:--------------------:|:-----------------:|:-------------:|:--------:|
| XPath-Parser/Evaluator             |   ✅    |          —           |         —         |       —       |    —     |
| Runtime-Orchestrierung             |   ✅    |          —           |        ✅         |       —       |    —     |
| Provider-Adapter (Rust)            |   ✅    |          —           |         —         |       —       |   ✅     |
| PyO3-Bindings                      |    —    |          —           |        ✅         |       —       |    —     |
| Pattern-ABC-Verträge               |    —    |         ✅           |         —         |       —       |    —     |
| `Adapter`-Resolution-Algorithmus   |    —    |         ✅           |         —         |       —       |    —     |
| `AdapterProxy`-Komposition         |    —    |         ✅           |         —         |       —       |    —     |
| `@pattern_proxy_for` Match-Logik   |    —    |         ✅           |         —         |       —       |    —     |
| `UiNodeAdapter` (Native-Mapping)   |    —    |          —           |        ✅         |       —       |    —     |
| `wait_for`/`ensure_that` Re-entry  |    —    |         ✅           |         —         |       —       |    —     |
| `ContextBase.get` Parent-Chain     |    —    |          —           |        ✅         |       —       |    —     |
| UI-Klassen (Button, Window, …)     |    —    |          —           |        ✅         |       —       |    —     |
| Keyword-Outcome-Verträge           |    —    |          —           |        ✅         |      ✅       |   ✅     |
| Locator-Syntax in RF               |    —    |          —           |         —         |      ✅       |   ✅     |
| Plattform-Provider (UIA/AT-SPI/AX) |    —    |          —           |         —         |       —       |   ✅     |
| End-to-End Context             |    —    |          —           |         —         |      ✅       |   ✅     |

### 11a.6 Reihenfolge & CI-Integration

**Grundprinzip:** Mock-Tests und Echt-Provider-Tests laufen **parallel**,
nicht sequentiell. Sobald technisch möglich, wird jede Keyword-/Pattern-
Funktion auf beiden Kanälen validiert. Der Mock ist nicht „erste Stufe"
und der Echt-Provider „zweite Stufe" — sie sind komplementär.

**Phase 0 (Verifikation):** AccessKit ist in `apps/test-app-egui` bereits
über das `eframe`-Default-Feature aktiv (siehe §11a.7.1) — keine
Code-Änderung nötig. Phase 0 ist eine Smoke-Verifikation: AT-SPI-Tree
inspizieren, ggf. deterministische Widget-Szenarien ergänzen.

**Phase 1 (Foundation):** pytest für `types`, `settings`, `wait`,
`ensure`, `weight_calculator`, `locator`.

**Phase 2 (Patterns/Adapter):** pytest mit Inline-Fakes für die ABC-/
Resolution-Algorithmen (`Adapter`, `AdapterProxy`, `PatternProxyFactory`)
sowie pytest gegen den Rust-Mock-Provider für `UiNodeAdapter`.

**Phase 3 (Standard-UI-Klassen):** Zusätzlich zu pytest die ersten
**Smoke-RF-Suites** — und zwar **dual-mode** (siehe §11a.7): dieselbe
Suite läuft gegen Rust-Mock UND gegen `test-app-egui` auf Linux AT-SPI.
Ziel: `click_button.robot`, `wait_for_window.robot`, `type_text.robot`.

**Phase 4 (Rust-Adapter via PyO3):** pytest gegen Rust-Mock (Bindings)
+ pytest gegen echten Linux-AT-SPI-Provider.

**Phase 5 (Devices, Highlight):** pytest + RF-Keywords für Maus/
Keyboard. Mock-Pfad via `Runtime.new_with_mock()`, Echt-Pfad via
Wayland-Compositor + `test-app-egui`.

**Phase 6 (Keywords):** Vollständige RF-Suite gegen Rust-Mock UND
gegen Linux-AT-SPI im Haupt-CI. Windows-UIA + macOS-AX als Job-Matrix
dazu, sofern Runner verfügbar.

**Phase 7 (Härtung):** Zusätzliche plattform-spezifische Eigenheiten
(komplexe Controls, DPI-Scaling, Multi-Monitor, HighContrast-Themes).

**CI-Matrix:**
- **Haupt-CI (PR-Gate):**
  - Linux: Ebene 1 (nextest) + Ebene 2 (pytest Mock+Rust-Mock) +
    Ebene 3a (RF vs. Rust-Mock) + Ebene 3b-Linux (RF vs.
    Wayland-Compositor + `test-app-egui` AT-SPI).
  - Windows (falls Runner): Ebene 1 + Ebene 3b-Windows (RF vs.
    `test-app-egui` UIA).
  - macOS (falls Runner): Ebene 1 + Ebene 3b-macOS (RF vs.
    `test-app-egui` AX).
- **Nightly:** Erweiterte Echt-Provider-Suiten gegen native Apps
  (nicht nur `test-app-egui`), plattform-spezifische Edge-Cases,
  Langläufer.
- **Release-Gate:** Alle OS-Jobs müssen grün sein.

### 11a.7 Echt-Provider-Teststrecken

Jede OS-Plattform braucht eine reproduzierbare Teststrecke mit
(a) einer Ziel-App, (b) aktivem Accessibility-Stack und (c) einem
deterministischen Display-Server-Setup. Die Teststrecken werden als
Build-Abhängigkeit der RF-Acceptance-Jobs eingebunden.

#### 11a.7.1 `apps/test-app-egui` als gemeinsame Ziel-App

**Status quo:** `apps/test-app-egui/Cargo.toml` nutzt `eframe = "0.34"`
ohne `default-features = false`. Da `eframe 0.34.1` `accesskit` als
**Default-Feature** mitbringt (`accesskit (default) → egui-winit/accesskit`),
ist die App **bereits accessibility-aktiv**: AT-SPI auf Linux, UIA auf
Windows, AX auf macOS — automatisch über `accesskit_winit`. Ab
egui 0.35+ wird das Feature laut Upstream-PR #7701 obligatorisch
(immer enabled). Der Modul-Header (`main.rs:7`) deklariert die App
explizit als „Accessibility target for `PlatynUI` functional tests via
`AccessKit`/AT-SPI".

**Aktion (Phase 0):** Verifikation, nicht Integration. **Durchgeführt
in Rev. 13** auf Linux/Wayland/GNOME — Ergebnis: AccessKit→AT-SPI-
Pipeline funktioniert, aber mit einer scharfen Bedingung (siehe unten).

**Verifizierte Smoke-Strecke (reproduzierbar):**

```bash
# 1. Build
cargo build -p platynui-test-app-egui -p platynui-cli

# 2. Precondition (Linux): AccessKit aktiviert sich nur bei aktivem
#    Screen Reader. Siehe „Lazy-Activation-Falle" unten.
gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
    --method=org.freedesktop.DBus.Properties.Set \
    org.a11y.Status ScreenReaderEnabled "<true>"

# 3. App starten (detached)
setsid nohup ./target/debug/platynui-test-app-egui \
    --app-id org.platynui.testapp \
    --title "PlatynUI Smoke Test" \
    --auto-close 0 > /tmp/testapp.log 2>&1 < /dev/null &
disown

# 4. Per XPath finden und Baum auslesen
./target/debug/platynui-cli-rs query \
    "//app:Application[contains(@Name, 'test-app-egui')]"
./target/debug/platynui-cli-rs snapshot \
    "//app:Application[@ProcessId=<PID>]"

# 5. Cleanup
kill <PID>
gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
    --method=org.freedesktop.DBus.Properties.Set \
    org.a11y.Status ScreenReaderEnabled "<false>"
```

**Verifizierte Fakten:**
- `control:Role="Application"`, `control:Name="platynui-test-app-egui"`,
  `control:ProcessId`, `control:Technology="AT-SPI2"` kommen korrekt durch.
- `control:Bounds`, `control:ActivationPoint`, `control:IsFocused`,
  `control:SupportedPatterns=["org.platynui.patterns.Focusable","org.platynui.patterns.WindowSurface"]` funktionieren.
- Widget-Hierarchie sichtbar: Frame → Panel (Menubar) → Button, Entry,
  CheckBox, SpinButton, ScrollBar etc. (43 Children im Frame).
- `native:Accessible.*` (Role, RoleName, State, Interfaces,
  RelationSet, ChildCount, IndexInParent) ist vollständig populiert.

**Lazy-Activation-Falle (Linux — kritisch!):**

`accesskit_unix 0.21` implementiert „lazy activation":
`src/context.rs:153–180` verbindet sich mit `org.a11y.Status` und
aktiviert den Adapter **nur wenn `ScreenReaderEnabled == true`**. Ohne
Screen-Reader-Flag ist der Adapter zwar konstruiert (`Adapter::new` →
`AdapterState::Inactive`), aber nicht am D-Bus-Registry gelistet — die
App erscheint für den AT-SPI-Provider unsichtbar.

- Kein App-Code kann das umgehen (Design von AccessKit).
- Betrifft alle winit/egui-Apps auf Linux, nicht nur unsere.
- Für Tests: Setup-Hook muss `ScreenReaderEnabled=true` setzen und
  beim Teardown zurückstellen. Siehe §11a.7.2 und §11a.7.6.

**Offene Beobachtungen (nicht Phase-0-blockierend):**

- **AT-SPI-Timeout:** Beim ersten Call gegen die cold-started App
  wirft `provider-atspi` einen `D-Bus call timed out elapsed_ms=1000`-
  Warn-Log. Der Call liefert trotzdem sein Ergebnis — Timeout scheint
  Warnschwelle, nicht Abbruch zu sein. TODO: Timeout-Strategie im
  AT-SPI-Provider prüfen (evtl. 3000ms oder adaptiv beim Cold-Start).
- **`native:Application.ToolkitName = null`:** AccessKit oder egui-
  winit setzt dieses AT-SPI-Property nicht. Für Tests unkritisch,
  aber eine Future-Improvement-Idee wäre, in `test-app-egui` o.Ä.
  einen Toolkit-Namen zu setzen, falls AccessKit-API das erlaubt.
- **Widget-Szenarien ausreichend:** Die bestehenden Widgets in
  `test-app-egui` bieten genug Test-Targets für Phase 1–4. Nachschärfen
  (stabile IDs pro Widget, zusätzliche Szenarien wie Liste, Dialog,
  geschachtelte Fenster) erst wenn konkrete RF-Tests das brauchen.

#### 11a.7.2 Linux — AT-SPI über Wayland-Compositor

- **Compositor:** `apps/wayland-compositor` (vorhanden, ~9889 LOC,
  Smithay-basiert). Läuft headless im CI, kontrollierter Focus/Input,
  Highlight-Support eingebaut.
- **Accessibility:** AT-SPI via D-Bus. `test-app-egui` mit AccessKit
  publiziert seinen Baum automatisch; `atspi`-Provider greift zu.
- **Precondition (zwingend, siehe §11a.7.1):** Vor dem App-Start muss
  `org.a11y.Status.ScreenReaderEnabled = true` gesetzt sein. Ohne das
  Flag aktiviert `accesskit_unix` den Adapter nicht, und die App
  bleibt unsichtbar am AT-SPI-Bus. Die Fixture muss das Flag setzen
  **bevor** die App startet (Environment wird beim App-Start gelesen)
  und nach den Tests zurückstellen.
- **CI-Setup:** D-Bus Session-Bus + at-spi-bus starten →
  ScreenReaderEnabled=true setzen → Compositor starten → test-app-egui
  als Child starten → RF-Suite laufen lassen → Teardown in umgekehrter
  Reihenfolge.
- **Runner:** GitHub-hosted `ubuntu-latest` reicht (Compositor ist
  headless). Kein echter GPU nötig (`wayland-compositor` nutzt
  Software-Rendering-Backend). AT-SPI-Bus muss als System-Dependency
  installiert sein (`at-spi2-core` Paket).

#### 11a.7.3 Windows — UIA

- **App:** `test-app-egui` mit AccessKit-UIA-Backend (AccessKit
  unterstützt UIA nativ auf Windows).
- **Accessibility:** Windows UI Automation API; Provider
  `provider-windows-uia` greift zu.
- **Runner:** GitHub `windows-latest`. Kein Display-Server-Trick
  nötig (Windows hat eingebauten Desktop-Session).
- **Besonderheit:** UIA-Trees können vom AT-SPI-Tree leicht
  abweichen — Tests brauchen plattform-spezifische Locator-
  Variationen oder einen Abstraktions-Layer.

#### 11a.7.4 macOS — AX

- **App:** `test-app-egui` mit AccessKit-AX-Backend.
- **Accessibility:** macOS Accessibility API; Provider
  `provider-macos-ax` greift zu. Benötigt u.U. Assistive-Access-
  Berechtigung — im CI über einen Init-Schritt.
- **Runner:** GitHub `macos-latest` (oder selbstgehosteter M-series-
  Runner für Cross-Architektur-Tests).

#### 11a.7.5 Dual-Mode-RF-Suites

Idealfall: **dieselbe** `.robot`-Datei läuft gegen Mock und gegen
Echt-Provider, gesteuert über eine Library-Variable.

```robotframework
*** Settings ***
# ${PROVIDER_MODE} wird vom CI-Runner gesetzt: mock|atspi|uia|ax
Library    PlatynUI    provider=${PROVIDER_MODE}
Suite Setup    Run Keyword If    '${PROVIDER_MODE}' == 'mock'    Load Mock Fixture    login-dialog.xml

*** Test Cases ***
Login With Valid Credentials
    Type Text       ${USERNAME}    alice
    Type Text       ${PASSWORD}    secret
    Activate        ${LOGIN_BUTTON}
    Wait For Window    name=Dashboard
```

- **Gleiche Semantik, gleiche Keywords.** Unterschiede leben im
  Locator (plattform-spezifische Rollen-/Property-Namen) bzw. in
  der Fixture-Vorbereitung (Mock lädt XML, echte Provider starten
  App-Prozess).
- **CI-Lauf:** Matrix über `PROVIDER_MODE={mock, atspi, uia, ax}` ×
  `OS={ubuntu, windows, macos}`, ungültige Kombinationen
  (z.B. `uia` auf Linux) werden ausgefiltert.

#### 11a.7.6 Fixture-/Setup-Konventionen

- **Mock-Fixtures:** XML-Dateien unter `tests/acceptance/fixtures/mock/`.
- **Echt-App-Szenarien:** Scripts unter `tests/acceptance/fixtures/egui/`,
  die `test-app-egui --scenario <name>` mit vorkonfiguriertem Layout
  starten.
- **Linux-a11y-Helper:** `scripts/linux-a11y-enable.sh` und
  `scripts/linux-a11y-restore.sh` (noch anzulegen) toggeln
  `org.a11y.Status.ScreenReaderEnabled` via `gdbus` (siehe §11a.7.1).
  Sie müssen idempotent sein und den vorherigen Wert restaurieren
  (Read-Modify-Write-Pattern). RF-`Suite Setup`/`Suite Teardown` und
  pytest-Session-Fixtures rufen sie auf Linux auf; No-Op auf Windows/
  macOS.
- **Suite-Layout:** `tests/acceptance/{smoke,core,devices,edge}/*.robot`.
  Jede Suite deklariert via Tags, in welchen Provider-Modi sie läuft
  (`atspi_only`, `all_providers`, `mock_only`).

## 12. Erfolgskriterien

1. Eine `.robot`-Datei mit Context-Klassen (Calculator-Beispiel) läuft
   gegen `apps/test-app-egui` in `wayland-compositor` und gegen einen
   nativen Calculator unter Windows UIA.
2. Alle Standard-Keywords aus §8 funktionieren.
3. Ein „Fake-Button" (Label mit ClickHandler) wird durch einen
   `@pattern_proxy_for(role="Label", attributes={...})`-Proxy korrekt als
   Button bedient — ohne dass die Test-Suite davon weiß.
4. Mindestens 30 Python-Unit-Tests gegen Mock-Adapter
   (`core/adapters/mock.py`).
5. Mindestens eine Robot-Acceptance-Suite läuft im CI.
6. Libdoc-Output ist im CI publiziert.

## 13. Offene Fragen

### 13.1 BareMetal vs. PlatynUI — Abgrenzung & Koexistenz

Beide Libraries sind **dauerhaft** Teil des Produkts und arbeiten in
derselben Robot-Suite zusammen.

- **`PlatynUI.BareMetal`** — Low-Level-Library, mappt die Rust-Runtime
  quasi 1:1 auf Robot-Keywords. XPath-Strings direkt, ohne Contexts
  oder Locator-Vererbung. Notwendig für Sonderfälle, die der
  Context-Layer nicht abdeckt (gezielte Pointer-/Keyboard-Operationen
  auf einem `UiNode`, ad-hoc-XPath-Diagnose, Tests gegen die Runtime
  selbst).
- **`PlatynUI`** — High-Level-Library mit Contexts,
  Locator-Vererbung, automatischer Adapter-/Pattern-Auswahl,
  semantischen Keywords mit Outcome-Vertrag.

**Geteilter Zustand:**

- **`Runtime`-Singleton** (`core.runtime.runtime`) — beide Libraries
  greifen dieselbe Instanz, sodass XPath-Queries und Context-
  Lookups dieselbe UI-Tree-Sicht haben.
- **Root-Element-Variable `${PLATYNUI_ROOT_ELEMENT}`** — Single Source
  of Truth für das aktive Root. `PlatynUI.Set Root Element` und das
  zukünftige `BareMetal.Set Root` schreiben/lesen dieselbe Variable
  über den Storage-Hook in `core/descriptor.py` (siehe §A.7).
- **Werte-Interop** — ein in BareMetal aufgelöster `UiNode`/`Adapter`
  kann an ein PlatynUI-Keyword übergeben werden und umgekehrt
  (Konverter akzeptieren beide Richtungen, sobald die BareMetal-
  Variante des Descriptors gelandet ist; siehe §13.2).

### 13.2 `UiNodeDescriptor`-Umzug — Übergangsplan

Aktueller Stand: `BareMetal/__init__.py:51` definiert einen eigenen
`UiNodeDescriptor`, der `UiNode` direkt wrappt, eine `BareMetal`-
Library-Referenz hält und gegen `library.runtime.evaluate_single`
auflöst. Er nutzt eine eigene Robot-Variable
`${PLATYNUI_ROOT_DESCRIPTOR}`. Das ist **Übergangscode**, entstanden
um die Runtime-Bindings früh testen zu können.

**Zielzustand:** beide Libraries nutzen denselben Descriptor-Mechanismus
aus `core/descriptor.py`. Da `BareMetal` aber zu `UiNode` auflösen
muss (kein `Adapter`/`ContextBase`-Wrapping), nicht zu `ContextBase`,
braucht es eine **BareMetal-Variante** — keine 1:1-Wiederverwendung
von `ElementDescriptor`. Skizze:

- gemeinsame Basis: das Generic + `convert`-Protokoll + der
  Storage-Hook (Root-Variable `${PLATYNUI_ROOT_ELEMENT}`),
- BareMetal-Variante (z. B. `BareMetalDescriptor`): `__call__` liefert
  `UiNode` statt `ContextBase`, `convert` baut eine Query aus dem
  String, das Caching liegt entweder am Descriptor selbst oder
  weiterhin in einem Library-seitigen Cache.

**Umsetzungsreihenfolge:**

1. (erledigt, Rev. 24) `core/descriptor.py` mit Storage-Hook —
   `core/` bleibt Robot-frei, der Hook ist die Vorbedingung dafür,
   dass beide Library-Inits später dieselbe Robot-Variable bedienen.
2. PlatynUI-Library-Init installiert den `EXECUTION_CONTEXTS`-Override
   (Phase 5 / §A.8).
3. BareMetal-Variante des Descriptors entwerfen, sobald die genaue
   Form des Werte-Interop (UiNode↔Adapter-Konvertierung,
   Library-Caching, Set-Root-Semantik über beide Libraries hinweg)
   geklärt ist.
4. `UiNodeDescriptor` durch die neue Variante ersetzen,
   `${PLATYNUI_ROOT_DESCRIPTOR}` durch `${PLATYNUI_ROOT_ELEMENT}`
   ablösen, Library-Cache anpassen.

Punkt 3+4 sind explizit *nicht* Teil von Phase 3. Bis dahin laufen
`UiNodeDescriptor` und `ElementDescriptor` parallel mit getrennten
Root-Variablen — der gemeinsame Mechanismus existiert, wird aber von
BareMetal noch nicht angesprochen.

### 13.3 `UiNode.supported_patterns` in Python

Bereits verfügbar: `UiNode.supported_patterns() -> list[str]` ist in
`packages/native/src/runtime.rs:157` implementiert und in
`_native.pyi:302` typisiert. `UiNodeAdapter` kann direkt darauf
aufsetzen — kein zusätzlicher Bindings-Bedarf.

### 13.4 Mehrere Runtimes / Pabot

`ContextFactory.registered_contexts` und
`PatternProxyFactory.registered_proxies` sind prozessweite Registries
(Class-Variables, gefüllt durch `@context` /
`@pattern_proxy_for`-Aufrufe beim Modul-Import — Implementierungs­
detail der jeweiligen Factory). Das ist bewusst und unabhängig von
der „keine globale Pattern-Registry"-Entscheidung in §5.4: dort geht
es um Pattern-Identifier, hier geht es um die Auflösungs-Tabellen
für Klassen-Auswahl.

Für `pabot` (parallele Robot-Runs im selben Prozess) ist das ok, weil
Registrierungen in Modul-Imports passieren und prozessweit gleich
aussehen. Bei Bedarf können wir später Runtime-Scoped Registries
einführen — dann würden auch die Klassen-Registries pro `Runtime`-
Instanz leben.

**Duplikat-Erkennung**: Beide Registries (`ContextFactory`,
`PatternProxyFactory`) emittieren beim Registrieren ein
`DuplicateRegistrationWarning` (Subklasse von `UserWarning`,
definiert in `core/exceptions.py`), wenn bereits ein Eintrag mit
**exakt denselben Kriterien** existiert (Vergleich der
Criteria-Dicts; `re.Pattern`-Werte werden über `(pattern, flags)`
verglichen). Re-Registrierung **derselben Klasse** ist kein
Duplikat — sie ersetzt den Vorgänger lautlos. CI kann
`-W error::PlatynUI.core.exceptions.DuplicateRegistrationWarning`
setzen, um Konflikte als Build-Fehler zu erzwingen. Tests, die
absichtlich einen real registrierten Rollennamen wie `Button`
oder `Window` verwenden, kollidieren sonst mit den Context-
Klassen aus `ui/`. Die Test-Konvention lautet daher: nutze
Test-spezifische Rollen (`'__test_button__'`, `'TestButton'`),
es sei denn der Test überprüft genau das Matching gegen die
Standard-Klassen.

### 13.5 Native Read-Escape-Hatch

`UiNode.attribute(name, "native")` deckt provider-spezifische Reads ab.
Ein Write-Escape-Hatch (z.B. direkter Aufruf einer UIA-Pattern-Methode,
die wir nicht abstrahiert haben) ist offen. Vorschlag: separate
`unsafe`-Helper-API auf dem Adapter, explizit als „nicht portabel"
markiert.

### 13.6 Rust-PatternId-Umstellung auf Reverse-DNS — RESOLVED (Rev. 14)

> **Hinweis (Rev. 19):** Der Newtype `PatternId` und das Konstanten-Modul
> `pattern_ids`, die in dieser Sektion erwähnt werden, heißen seit Rev. 19
> `PatternName` und `pattern_names`. Siehe §13.7 für die Umbenennung. Der
> ursprüngliche Wortlaut bleibt hier erhalten als historisches Record der
> Reverse-DNS-Umstellung.

Rust-`PatternId` verwendet jetzt durchgängig Reverse-DNS-Identifier
(`org.platynui.patterns.<Name>`). Damit sind Rust-`PatternId.as_str()`
und Python-`pattern_name` wörtlich identisch.

**Implementierung:**

- Neues Konstanten-Modul `core::ui::pattern_ids` mit `&'static str`-
  Konstanten (z.B. `pub const FOCUSABLE: &str = "org.platynui.patterns.Focusable";`)
- Alle `PatternId::from("BareName")`-Stellen in core/runtime/provider-atspi/
  provider-mock/platform-linux-wayland umgestellt
- `assets/mock_tree.xml` enthält voll qualifizierte Patterns (kein
  Loader-Expand-Shortcut)
- PyO3-Bindings: `Pattern.id()` liefert Reverse-DNS;
  `pattern_id_from_arg` liest `pattern_name`-ClassVar von Pattern-Klassen
  (kein Klassennamen-Fallback mehr)
- `match pattern.as_str() { … }` in Provider-Code auf `if`-Ketten
  umgestellt (`&str`-Konstanten in Patterns nicht erlaubt)

**Breaking Change:** PyO3 `pattern_object`/`Pattern.id()` liefert jetzt
Reverse-DNS-Strings statt bare names; Python-Aufrufer von
`get_pattern("Focusable")` müssen auf `get_pattern("org.platynui.patterns.Focusable")`
oder `get_pattern(Focusable)` (mit `pattern_name`-ClassVar) umstellen.

### 13.7 Rust-API-Symmetrie zu Python: PatternId → PatternName — RESOLVED (Rev. 19)

Rust-Code spricht ab Rev. 19 dieselbe Vokabel wie Python: `PatternName`
(statt `PatternId`), `pattern_names` (statt `pattern_ids`),
`UiPattern::pattern_name()` (statt `id()`),
`UiPattern::static_pattern_name()` (statt `static_id()`),
`UiNode::pattern_by_name()` (statt `pattern_by_id()`).

**Motivation:** Asymmetrie zwischen Rust (`PatternId` / `id()`) und
Python (`PatternName` / `pattern_name`) verursachte mentale
Übersetzungsarbeit beim Lesen. Der Wire-Inhalt (Reverse-DNS-String)
ist auf beiden Seiten identisch — die Bezeichner sind es jetzt auch.

**Implementierung:**

- `crates/core/src/ui/identifiers.rs`: `pub struct PatternName(Arc<str>)`,
  `pub mod pattern_names`
- `crates/core/src/ui/pattern.rs`: `UiPattern::pattern_name()`,
  `UiPattern::static_pattern_name()`
- `crates/core/src/ui/node.rs`: `UiNode::pattern_by_name()`
- Alle Implementierungen in `core`, `runtime`, `provider-*`, `platform-*`,
  `cli`, `inspector`, `packages/native` mechanisch nachgezogen
  (~27 Dateien, ~150 Bezeichner-Treffer)
- PyO3-Klasse `PatternName` wird intern in `platynui_native._native`
  registriert, aber bewusst NICHT aus `platynui_native.__init__`
  re-exportiert, weil sie sonst mit dem Python-TypeAlias
  `PlatynUI.core.types.PatternName: TypeAlias = str` kollidieren würde.
  Python-User-Code spricht den str-Alias.

**Verifikation:**

- `cargo nextest run --workspace`: 1980/1980 ✅
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `uv run pytest`: 265/265 ✅
- `uv run ruff check`, `mypy`, `pyright`: clean

**Breaking Change (Rust):** Direkte Rust-Konsumenten der `UiPattern`-/
`UiNode`-Traits müssen ihre Aufrufe von `id()` → `pattern_name()`,
`static_id()` → `static_pattern_name()`, `pattern_by_id()` →
`pattern_by_name()` umstellen, und `PatternId` / `pattern_ids` durch
`PatternName` / `pattern_names` ersetzen. Wire-Format unverändert.

**Breaking Change (Python):** PyO3 exportiert keine Klasse `PatternId`
mehr aus `platynui_native`. Wer den Wrapper braucht (selten —
typischerweise nur Bridging-Code), nutzt `platynui_native._native.PatternName`.
User-Code, der nur Strings vergleicht, ist nicht betroffen.

## 14. Zusammenfassung

- **Patterns leben in Python.** Der `WeightCalculator` plus
  `@context` und `@pattern_proxy_for` ist der Kern der Erweiterbarkeit
  und wird aus dem Altprojekt übernommen (dort hießen sie Strategies).
- **Batterien inklusive:** PlatynUI liefert für alle gängigen
  UI-Rollen fertige UI-Klassen und Default-Proxies mit (siehe §5a).
  Test-Projekte sind ohne eigenen Proxy-Code produktiv; App-Spezialfälle
  überschreiben gezielt über `framework_id`/`class_name`/`attributes`.
- **Rust ist eine konkrete Adapter-Implementierung**, nicht „die"
  Adapter-Schicht. Weitere Adapter (JSON-RPC, Mock, …) können
  gleichberechtigt daneben stehen. Patterns tragen einen stabilen
  Reverse-DNS-Identifier (`org.platynui.patterns.*`), über den externe
  Adapter Capabilities string-basiert melden und aufrufen.
- **Adapter dürfen Patterns mitbringen** (heute: `Focusable`,
  `WindowSurface`). Welches Pattern bei `adapter.get_pattern(X)`
  gewinnt, entscheidet pro UiNode die Kombination aus Proxy-Override
  und Adapter-eigenen Patterns.
- **User-Sicht ist führend:** UI-Elemente werden so beschrieben, wie sie
  sich präsentieren. Ein „Fake-Button" wird über `@pattern_proxy_for` mit
  passenden Kriterien zum Button — die Test-Klasse `Button` weiß nichts
  davon.
- **Robot-Keywords sind semantische Aktionen mit Outcome-Vertrag**
  (`Activate`, `Toggle`, `Set Value`, `Expand`, …), nicht
  Mechanismus-Verben (`Click`).
- **Pre-Conditions → Perform → Postcondition** ist der universelle
  Lebenszyklus jeder Aktion. Verifikation ist erstklassig.
- **~50 % Code-Reduktion** gegenüber dem Altprojekt (4.600 → ~2.000 LOC),
  weil die Adapter-Bridge und der XPath-Builder dank Rust schrumpfen
  — die Konzepte aber 1:1 erhalten bleiben.
- **Modernes Python 3.12+** ist verbindlich (siehe §2.6): ABC als
  Default für Pattern-Interfaces, Dataclasses
  (`frozen`/`slots`/`kw_only`) für Value-Objekte, `match`/`case` für
  Outcome-Dispatch, PEP 604 Union-Syntax, `Self`, `cached_property`,
  `@overload` für Locator-API, PEP 695 Generics-Syntax und
  `typing.override` in Proxy-Hierarchien.
  `__init_subclass__`-Hybrid mit
  Decorator für `@context`. Keine globale `PatternRegistry` —
  Convention plus Adapter-lokale Mapping-Tabellen reichen.
