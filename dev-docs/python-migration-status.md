# Python-Migration — Status-Tracker

Lebendes Dokument, das den Fortschritt der Migration des Python-Teils
aus dem alten PlatynUI-Projekt (`/home/daniel/develop/tmp/robotframework-PlatynUI`)
in das neue Rust-basierte Projekt verfolgt.

Bezugsdokument: [`python-library-design.md`](./python-library-design.md)

**Stand:** 2026-06-16
**Aktuelle Revision:** Rev. 49 (**Phase 3 + Phase 4 abgeschlossen.**
Zwei lose Enden geschlossen:
**(1) `@locator` Method-Form** — `LocatorMethodDescriptor.__get__`
(plus `__call__` für die `@property`-Form) löst jetzt auf: der
Return-Type der gewrappten Funktion (eine `ContextBase`-Subklasse,
einmalig via `typing.get_type_hints` gelesen + gecacht) wird als
`instance.get(annotation, locator=self.__locator__)` aufgelöst.
Klare `TypeError` bei Nicht-`ContextBase`-Annotation bzw.
Nicht-`ContextBase`-Instanz. `core/locator.py` + `test_locator.py`
(4 neue Resolution/Fehler-Tests). **(2) `ApplicationReady`-Pattern
ersatzlos gestrichen** — der zuvor offene Punkt aus Rev. 40 wird
**nicht verdrahtet, sondern entfernt**: Eine kurze Verdrahtung
(Rust-Trait + Provider in mock/atspi/uia + PyO3 + Native-Wrapper)
zeigte, dass `ApplicationReady` auf allen realen Backends nur an
dieselbe Responsiveness-Probe delegiert wie `Responsive` — also ein
reines Duplikat ohne eigene Semantik. Konsequenz: die Waisen-ABC
`core/patterns/application_ready.py` ist gelöscht, der Export aus
`core/patterns/__init__.py` entfernt; `_application_is_ready` nutzt
weiterhin **`Responsive`** (Window-Responsiveness, nativ) **+
`Application.is_ready()`** (User-Hook) als Ready-Quellen. Kein
Rust-/PyO3-/Adapter-Code für `ApplicationReady` mehr. Eine echte
App-Load-Semantik (z.B. „Splash dismissed") bleibt künftigen
Providern vorbehalten und würde dann als eigenes Pattern eingeführt,
nicht als Responsive-Alias.
Gates: **113 nextest (core+mock) + 727 pytest grün**, clippy + ruff
+ mypy + fmt clean, pyright nur die 2 pre-existing Errors in
`test_ensure.py`. Uncommitted.)

**Vorherige Revision:** Rev. 48 (Adapter-bezogene Devices in eigenes
Modul gezogen, `_mixins.py` aufgelöst, `MouseProxy.ctrl_click()` als
echte Methode. **Schichtung:** `core/devices.py` enthält jetzt nur
noch die rein geometrische, adapter-freie Schicht
(`MouseProxy`/`KeyboardProxy`, `Anchor`/`VirtualPoint`, Action-Enums,
Settings-Bridge). Die Pattern-bewussten Wrapper `AdapterMouseProxy` /
`AdapterKeyboardProxy` leben in **`core/adapter_devices.py`**, weil
sie `Adapter` und `Element`/`ActivationTarget`-Patterns konsumieren.
**Neue Methode `MouseProxy.ctrl_click(*, button=LEFT, pos, x, y)`**
ersetzt den `ctrl_click_adapter`-Helper: hält `<Ctrl>` per
`runtime.current.keyboard_press` gedrückt, ruft `self.click(...)`,
gibt `<Ctrl>` im `try/finally` wieder frei. Modifier wird global
gesteuert (kein `KeyboardProxy`-Bezug nötig). **`ui/proxies/_mixins.py`
gelöscht.** Die Default-Proxies (`buttons.py`, `combobox.py`,
`item.py`, `text.py`) rufen direkt `AdapterMouseProxy(self.adapter)
.click(...)` / `.ctrl_click()` und `AdapterKeyboardProxy(self.adapter)
.type_keys(...)`. **Re-Export-Pfad unverändert:** `core/__init__.py`
importiert `AdapterMouseProxy`/`AdapterKeyboardProxy` aus dem neuen
Modul; `from PlatynUI.core import AdapterMouseProxy` funktioniert
weiter. Designdoc Rev. 48 — DONE: Header-Bullet voran, §A.9
Modul-Layout-Hinweis ergänzt, §A.9.3 mit `ctrl_click`-Codeblock,
§A.9.4 nach `core/adapter_devices.py` verschoben (Header-Annotation),
§A.9.5 `AdapterKeyboardProxy`-Docstring um Modul-Hinweis erweitert,
§A.14.5 ItemProxy-Codeblock auf direktes `AdapterMouseProxy(...)`
umgestellt, Verzeichnisbaum (§ vor §A.13) um `adapter_devices.py`
erweitert. **Settings→Overrides-Bridge aufgeräumt:** die beiden
`_*_overrides_from_settings()`-Helfer bauen das `TypedDict` jetzt
über direkte Literal-Key-Zuweisungen (Walrus-`:=`) statt über
`getattr` + Tuple-Tabelle; die zwei
`# type: ignore[literal-required]`-Kommentare entfallen dadurch.
Code-Implementierung — DONE. Tests angepasst (vier neue
`ctrl_click`-Tests in `test_devices.py`, `test_proxies.py` neu
geschrieben mit `patch_actions`-Fixture); 724 Tests grün, ruff
+ mypy clean, **Pyright-Baseline von 4 auf 2 Errors gesenkt**
(verbliebene 2 pre-existing in `test_ensure.py`). **Committet**
als `e16bfd6`.)

> **Vorherige Rev. 47:** `Deselectable` als eigenes Single-Select-
> Action-Pattern + saubere Trennung `deselect` vs
> `remove_from_selection` am `Item` + 4-Methoden-Selection-API am
> `ItemContainer[I]`. Pattern-Erweiterung: Neues Action-Pattern
> `Deselectable.deselect()` als Pendant zu `Selectable.select()` für
> Single-Select-Container, die „nichts selektiert" als gültigen
> Zustand kennen. Optional, ohne Konsistenz-Zwang — Default-
> `ItemProxy` registriert es nicht, weil sich Single-Select-Deselect
> nicht generisch über Maus/Tastatur synthetisieren lässt;
> `Item.deselect()` wirft sonst ehrlich `PatternNotSupportedError`.
> Item-API neu sortiert: `Item.select()` → `Selectable`,
> `Item.deselect()` → `Deselectable` (statt
> `MultiSelectable.remove_from_selection`),
> `Item.add_to_selection()` → `MultiSelectable`,
> `Item.remove_from_selection()` → `MultiSelectable` (NEU).
> Rückgabewerte für Chaining: `Item.<action>() -> Self`;
> `ItemContainer.<action>(*, locator) -> I`;
> `TreeItem.<action>(*, locator=None) -> 'TreeItem'` und
> `Row.<action>(*, locator=None) -> 'Row | Cell'` für die Dual-Role-
> Klassen; `ComboBox.select(*, locator=None) -> ListItem`. Alle vier
> Container-Wrapper symmetrisch. Dual-Role-Klassen mit
> `cast(...)` + `# type: ignore[redundant-cast]` und file-level
> `# pyright: reportUnnecessaryTypeIgnoreComment=false`. Konsistenztest
> erweitert: `IsSelectable` ⇒ `Selectable` (Pflicht); `Deselectable`
> NICHT Pflicht; `IsMultiSelectable` ⇒ `MultiSelectable`. Designdoc
> Rev. 47 — DONE; Code-Implementierung — DONE; 718 Tests grün.
> **Committet** als `3541d06`.

> **Vorherige Rev. 46:** Read/Action-Trennung der Capability-
> Patterns für Selection und Expand. Architektur-Prinzip:
> PlatynUI steuert Anwendungen ausschließlich über Maus und
> Tastatur, nie über plattformspezifische Steuer-APIs.
> Pattern-Spaltung: `IsSelectable` ↔ `Selectable`;
> `IsMultiSelectable` ↔ `MultiSelectable`;
> `IsExpandable` ↔ `Expandable`. Neues Container-Read-Pattern
> `Selection`. Konsistenz-Regel: Read-Pattern erfordert
> Action-Pattern (Suite-Test). API-Erweiterungen: `Item` mit
> `add_to_selection`/`deselect` (über `MultiSelectable`),
> `is_selected` aus `IsSelectable`; TreeItem mit
> `IsExpandable`/`Expandable`-Spaltung; ItemContainer[I] mit
> `can_select_multiple`/`is_selection_required`/
> `get_selected_items()`/`clear_selection()`. Action-Synthese
> via Maus/Tastatur (Ctrl+Click, Doppelklick, Pfeiltasten).
> Designdoc + Code + Tests komplett: 709 Tests grün, alle
> Quality-Gates clean. **Uncommittet (mit Rev. 44+45).**

> **Vorherige Rev. 45:** Item-Capabilities flach an `Item`,
> Pattern-checked. Mixins `SelectableItem`/`ExpandableItem`/
> `EditableItem` und `EditableCell` ersatzlos entfallen. Capability-
> Methoden (`select`, `set_text`, `clear`, `activate`) und Read-
> Properties (`text`, `is_selected`) liegen direkt auf `Item`; sie
> rufen das benötigte Pattern via `adapter.get_pattern(P)` und werfen
> `PatternNotSupportedError` wenn das Pattern fehlt. **Keine
> `supports_*`-Properties** — Capability-Verfügbarkeit prüft
> Aufrufer bei Bedarf direkt am Adapter via
> `adapter.get_pattern(P, raise_exception=False) is not None` oder
> fängt `PatternNotSupportedError`. **Expandable wandert ausschließlich
> auf `TreeItem`**: `expand`/`collapse`/`is_expanded`/`can_expand`
> liegen dort inline (nicht auf `Item`, `ListItem`, `TabItem`,
> `Cell`, `Row` — semantisch sinnlos).
> `ListItem`, `TabItem`, `Cell` bleiben als leere Marker-Subklassen
> für Default-Locator-Role und Generic-Bound. `TreeItem(Item,
> ItemContainer["TreeItem"])` ergänzt Expandable inline,
> `Row(Item, ItemContainer[Cell])` unverändert ohne Expand.
> `MenuItem` bleibt `Control`-basiert; `activate()` wurde **radikal
> vereinfacht**: kein Vorfahren-Walk mehr, nur `ensure_that(...)` +
> `Activatable.activate()`. Submenü-Pfade werden Robot-seitig durch
> sequentielle `activate()`-Aufrufe gegangen. `MenuItem` trägt
> **kein** Expandable. `EditableCell`-Klasse weg; Adapter mit
> `role="EditableCell"` fallen auf `Cell` zurück.
> `ui/item.py` umgebaut; `ui/tree.py` mit Expandable-Inline-
> Methoden; `ui/menus.py` MenuItem-Activate vereinfacht;
> `ui/lists.py`/`tabs.py`/`table.py`/`__init__.py` entrümpelt.
> Tests: `test_item.py` umgebaut (18 Tests gegen `Item` direkt,
> inkl. `activate()`-Pfade, ohne Expand- und ohne supports_*-Tests),
> `test_tree.py` um 9 Expandable-Tests für `TreeItem` erweitert
> (mit `_tree_item_with_parents` Helper, ohne supports_expand),
> `test_menus.py` um 5 Ancestor-Walk-Tests bereinigt,
> `test_table.py` (EditableCell-Test entfernt). Designdoc Rev. 45:
> Header (Rev. 45 oben, Rev. 44 unverändert), §A.14.2 Hierarchie-
> ASCII, §A.14.12 (Item ohne Expand, ohne supports_*),
> §A.14.14 (TreeItem mit Expand-Capability, ohne supports_expand),
> §A.14.24 (MenuItem ohne Ancestor-Walk),
> §A.14.13/15/19/21/23 auf flache Capabilities, §5a.2 Pattern-
> Tabelle (`EditableCell` raus).

> **Vorherige Rev. 44:** Generischer `ItemContainer[I: Item]`
> ersetzt Container-Default-Proxies. Neue Context-Basisklasse in
> `ui/control.py` (PEP 695 Syntax, Python 3.12+) trägt
> `get_items`/`iter_items`/`get_item` mit Item-Typ via Generic-Bound.
> Konkrete Container erben über
> `class List(ItemContainer[ListItem])`, `class Tree(ItemContainer[TreeItem])`,
> `class TabList(ItemContainer[TabItem])`, `class Row(Item, ItemContainer[Cell])`;
> `TreeItem` erbt zusätzlich `ItemContainer["TreeItem"]`.
> `Menu`/`MenuBar`/`Table` sind **keine** `ItemContainer[I]`
> (`MenuItem` erbt `Control`, nicht `Item`; `Table` trägt Row-Geometrie
> über das `Table`-Pattern) und behalten `get_items`/`get_rows` direkt
> am Context. Container-Default-Proxies aus Rev. 43
> (`ListProxy`/`TreeProxy`/`TabListProxy`/`MenuProxy`/`MenuBarProxy`/
> `TableProxy`) sind ersatzlos gelöscht; `ui/proxies/_mixins.py`
> reduziert sich auf die Action-Helper `click_adapter`/
> `type_keys_on_adapter`. Pattern-delegierende Default-Proxies
> (`ButtonProxy`, `CheckBoxProxy`, `EditProxy`, `TextProxy`,
> `ComboBoxProxy`, `ItemProxy` & Subklassen) bleiben unverändert.
> Neuer Test-Modul `test_item_container.py` (7 Tests) für die
> Generic-Mechanik; `test_proxies.py` schrumpft entsprechend. Designdoc
> Rev. 44: Header, §A.13.4 (Container-Block ersetzt, Hierarchie-ASCII
> ohne Container-Proxies, Tabelle reduziert), §A.14.13/14/15/16/23
> (Container-Beispiele auf Generic-Stil), neuer §A.14.20a
> (`ItemContainer[I: Item]`-Spec), §A.14.20 (Native-Wrapper-Hinweis
> um Generic-Hinweis ergänzt), Pattern-Tabelle (Default-Proxy-
> Spalte für `ItemContainer`/`Table` aktualisiert).

> **Vorherige Rev. 43:** Default-Proxy-Schicht
> `ui/proxies/*.py` gebaut. Module 1:1 zu `ui/*.py` aufgeteilt:
> `base.py` (`ElementProxy`/`ControlProxy`), `buttons.py`
> (`ButtonProxy`, `CheckBoxProxy`), `text.py` (`EditProxy`,
> `TextProxy`), `combobox.py` (`ComboBoxProxy`), `item.py`
> (`ItemProxy` + `ListItemProxy`/`TreeItemProxy`/`TabItemProxy`/
> `RowProxy`/`CellProxy`/`MenuItemProxy`), `lists.py` (`ListProxy`),
> `tree.py` (`TreeProxy`), `tabs.py` (`TabListProxy`), `menus.py`
> (`MenuProxy`, `MenuBarProxy`), `table.py` (`TableProxy`).
> Container-Proxies wurden in Rev. 44 wieder entfernt zugunsten der
> generischen Context-Basisklasse `ItemContainer[I: Item]`.

> **Vorherige Rev. 42:** Pattern-Refactor — `ItemContainer`
> reduziert auf `item_count`, `get_item(ctx, …)`, `get_items(ctx, …)`;
> `row_count`/`column_count` raus aus `ItemContainer`. Drei neue
> Patterns: `Table` (`row_count`, `column_count`, `get_row(s)(ctx, …)`,
> `pattern_name = 'org.platynui.patterns.Table'`), `HasRowHeaders`
> (`row_headers`, `get_row_headers(ctx, …)`) und `HasColumnHeaders`
> (`column_headers`, `get_column_headers(ctx, …)`) als getrennte ABCs in
> `core/patterns/`. `get_*`-Methoden geben Context-Instanzen zurück und
> teilen Locator-/Scope-Merge-Semantik mit `ContextBase.get`/`get_all`.
> `ItemContainer` ist explizit **kein** Native-Wrapper — Provider-Reads
> für `ItemCount` sind toolkit-übergreifend nicht zuverlässig genug
> (WPF `VirtualizingStackPanel`, HTML-Custom-Layouts), darum lebt die
> Default-Strategie komplett Python-seitig im (Phase-4e folgenden)
> `ItemContainerProxyMixin` mit "erste-Ebene"-Walk im descendant-Subtree.
> UI-Klassen `List`/`Tree`/`TreeItem`/`Table`/`Row`/`TabList` verlieren
> ihre direkten `item_count`/`row_count`/`column_count`-Properties und
> behalten nur ihre `get_*`/`iter_*`/`get_item`-API. Designdoc Rev. 42
> vollständig: Header, §A.13 Pattern-Inventar (vier neue Reihen), §A.13.3
> Exports (`ItemContainer`, `Table`, `HasRowHeaders`, `HasColumnHeaders`),
> §A.13.4 Default-Proxy-Schicht (Container-Proxies in der Hierarchie,
> Mixin-Tabelle erweitert, neuer Erklärungs-Block zu
> `ItemContainerProxyMixin`/`TableProxyMixin`/`HasRow|ColumnHeadersProxyMixin`
> und der "erste-Ebene"-Strategie), §A.14.13/14/15 (UI-Klassen
> entrümpelt), §A.14.20 (Pattern-Spec refaktoriert), §A.14.23 (TabList
> entrümpelt), neue §A.14.25/26/27 für die drei neuen Pattern-Specs.
> Code-Migration (Pattern-Refactor + Tests) folgt nach Doku-Sweep;
> Default-Proxy-Schicht (Mixins + Proxies) ist die nächste Phase
> darüber.

> **Vorherige Rev. 41:** Settings ↔ Native-Override-Bridge.
> Legacy-Sekunden-Felder `mouse_*`, `keyboard_after_press_*` und
> `input_after_input_delay` aus `Settings` entfernt; ersetzt durch
> Millisekunden-Felder, die 1:1 die Delay-Slots von
> `platynui_native.PointerOverridesDict` und `KeyboardOverridesDict`
> spiegeln (Default `None` = Profile-Wert beibehalten). `MouseProxy`/
> `KeyboardProxy` bauen pro Call aus `Settings.current()` ein
> Override-Dict via Helfer `_pointer_overrides_from_settings()` /
> `_keyboard_overrides_from_settings()` und reichen es als
> `overrides=`-Kwarg an alle `runtime.current.pointer_*` /
> `keyboard_*`-Calls durch. Profile-Tuning (Motion, Acceleration,
> Steps-pro-Pixel etc.) läuft direkt über
> `runtime.current.pointer_profile()` / `keyboard_profile()` und gehört
> nicht in `Settings`. Designdoc §A.1 und Header aktualisiert.
> Bestehende `test_devices.py`-Calls um `overrides=None` erweitert;
> sieben neue Bridge-Tests (`test_pointer_overrides_*`,
> `test_keyboard_overrides_*`).

> **Vorherige Rev. 40:** Drei attribut-only Patterns wandern
> vom geplanten `ElementProxy` zum `UiNodeAdapter`: `Element`
> (`Bounds`/`IsVisible`/`IsInView`/`IsEnabled`), `ActivationTarget`
> (`ActivationPoint`/`ActivationArea`/`ActivationHint`) und `Readable`
> (`IsReadOnly`). Native-Wrapper `_NativeElement`/`_NativeActivationTarget`/
> `_NativeReadable` analog zu `_NativeWindowState`. Generische
> `_ATTRIBUTE_ONLY_PATTERNS`-Tabelle ersetzt den WindowState-Spezialfall
> in `supports_pattern`/`supported_patterns`. Mock-Tree um
> `IsReadOnly=true` auf dem Status-Text-Knoten ergänzt. Designdoc §A.13.4
> Native-Wrapper-Tabelle und `ElementProxy`-Mixin-Spalte entsprechend
> aktualisiert. `ApplicationReady` bleibt offen — kein Rust-Trait,
> kein Adapter-Wrapper.

> **Vorherige Rev. 39:** Phase 4-rust-split eingeschoben — das
> Rust-Mega-Trait `WindowSurfacePattern` (8 Methoden) wird in 7
> orthogonale Sub-Traits zerlegt: `ActivatablePattern` (TopLevel-only,
> trägt `activate()` + Read `IsActive`), `MinimizablePattern`,
> `MaximizablePattern`, `RestorablePattern`, `CloseablePattern`,
> `MovablePattern`, `ResizablePattern`. Jedes Sub-Trait bekommt ein
> eigenes Attribut-Modul (`attributes::activatable`,
> `attributes::minimizable`, …); `attributes::window_surface`
> verschwindet komplett. Bestehende `SupportsMove`/`SupportsResize`
> werden zu `CanMove`/`CanResize` umbenannt; neue Read-Attribute
> `IsActive`, `CanMinimize`, `CanMaximize`, `CanClose`. Das `HasUserInput`-
> Pattern wird zu `Responsive` umbenannt (Methode `accepts_user_input()`
> bleibt unverändert); das alte `AcceptsUserInput`-Attribut entfällt.
> `Titled` entfällt komplett — `Window.title` liest direkt `control:Name`.
> `Focusable` wird an Windows/TopLevel nicht mehr implementiert
> (Aktiv-Status nur über `IsActive`, Tastatur-Fokus über
> `Focusable.is_focused` an Sub-Elementen). Provider-Migration in
> atspi/windows-uia/mock; `runtime/window.rs` stellt um.
> `core/patterns/defaults.py` (globale Default-Schicht) ist gestrichen —
> role-spezifische Defaults gehören in den Proxy. Diese Phase ist
> Voraussetzung für Phase 4e (Default-Proxy-Schicht inkl. Window-Proxy).
> Designdoc Rev. 37 vollständig (Header, §A.13.1, §5.1, §5a.3, §A.10
> gestrichen, §6.1, Verzeichnisbaum, §A.14.5, §_application_is_ready,
> Standard-Rollen-Tabelle, Phase-2-Plan, Schluss-Summary). Code-
> Migration (Rust + Python) folgt nach Doku-Sweep.

> **Vorherige Rev. 36:** Phase 4d abgeschlossen — `Tabs`
> (TabList/TabItem) und `Menus` (Menu/MenuBar/MenuItem) als
> UI-Klassen implementiert. `TabItem` erbt `SelectableItem` analog
> zu `ListItem`; `MenuItem` erbt bewusst `Control` statt `Item`, da
> ein Menü-Eintrag semantisch ein eigenständiges interaktives
> Control ist — kein Container-Inhalt — und in der Praxis selbst
> Sub-Hierarchien aufmacht. `MenuItem.activate()` walked die
> Vorfahren-Kette bis `Window`/`DesktopBase` hoch und expandiert
> sie außen → innen, bevor `Activatable.activate()` auf self läuft.
> Designdoc §A.14.23/§A.14.24 ergänzt. 19 neue Tests, 630/630 grün,
> ruff/mypy/pyright clean. Commit `07ca742`.

---

## Übersicht

| Phase | Status | Bemerkung |
|---|---|---|
| Phase 0 — Smoke-Verifikation | DONE | Commit `ceb3057` |
| §13.6 Rust-PatternId Reverse-DNS | DONE | Rev. 14, Commit `09fdc6a` (Newtype später in Rev. 19 zu `PatternName` umbenannt — siehe §13.7) |
| §13.7 Rust-API-Symmetrie PatternId → PatternName | DONE | Rev. 19, uncommitted (1980 nextest + 265 pytest grün) |
| Designdoku-Konsolidierung Rev. 15 | DONE | Commit `f64b441` (2026-04-23); Properties-Pattern entfernt, Attribute-Modell mit Namespaces |
| Designdoku Rev. 16 — Locator-Kwargs | DONE | Commit `f64b441` (2026-04-23); drei Eingangskanäle (Convenience-Felder, Kwargs, Dict) mit Konfliktregel |
| Designdoku Rev. 17 — Pattern-Konsolidierung | DONE | Commit `f64b441` (2026-04-23); Element/TextContent/TextEditable/Clearable/Toggleable/Activatable/Focusable; Rust IsOffscreen→IsInView |
| Rev. 18 — `@locator` Decorator-Form | DONE | Class-Decorator: Commit `f64b441` (2026-04-23). Method-/Property-Form: Rev. 49 (uncommitted) — `LocatorMethodDescriptor.__get__`/`__call__` lösen über die Return-Annotation via `instance.get(annotation, locator=...)` auf |
| Phase 1 — Fundament | DONE | Commit `f64b441` (2026-04-23); 10 Module incl. vorgezogenem `core/patterns/` (war Phase 2 #11); 128 pytest + 1980 nextest grün, ruff+mypy+pyright+clippy grün |
| Phase 2 — Adapter-Schicht | DONE | Adapter-ABC + AdapterProxy + UiNodeAdapter + Runtime-Singleton committed; Pipeline-Lücke in Phase 4-pre geschlossen |
| Phase 3 — Context-Schicht | DONE | `ContextBase`, `ContextFactory`, `@context`, `ElementDescriptor`, `@locator` Class-Decorator-Form; `@locator` Method-/Property-Form in Rev. 49 implementiert (uncommitted) |
| Phase 4 — UI-Klassen + Standard-Proxies | DONE | 4-pre/4a/4b/4c/4d/4-rust-split/4e alle DONE; Proxy-Schicht (4e) in Rev. 43–48 gebaut (`ui/proxies/{base,buttons,combobox,item,text}.py`), Container-Proxies durch `ItemContainer[I]` abgelöst (Rev. 44), `WindowProxy` entfällt (Rev. 38); zuletzt committed `e16bfd6`. `ApplicationReady`-Pattern in Rev. 49 ersatzlos gestrichen (Duplikat von `Responsive`); Ready-Quelle bleibt `Responsive` + `Application.is_ready()` (uncommitted) |
| Phase 4-rust-split — Rust-Trait-Splittung + Python-Anpassung | DONE | Designdoc Rev. 37 DONE; Rust-Split (`WindowSurfacePattern` → 7 Sub-Traits + `Responsive`-Polling) + Provider-Migration + Python-Pattern-Renames (`HasUserInput`→`Responsive`, `Titled` entfällt) abgeschlossen; alle Quality-Gates grün (1981 nextest + 629 pytest) |
| Phase 5 — Keywords + Robot-Library | PENDING | — |
| Phase 6 — Iterative Erweiterungen | PENDING | — |

---

## Abgeschlossen

### Phase 0 — Smoke-Verifikation (Commit `ceb3057`)

- Linux a11y / AT-SPI-Bridge Roundtrip via `accesskit_unix` verifiziert
  (`accesskit_unix-0.21.0/src/context.rs:153-180` Gate auf
  `org.a11y.Status.ScreenReaderEnabled == true` dokumentiert)
- Helper-Scripts angelegt:
  - `scripts/linux-a11y-enable.sh`
  - `scripts/linux-a11y-restore.sh`
- Designdoc Rev. 13 mit Smoke-Befunden aktualisiert

### §13.6 Rust-PatternId-Umstellung auf Reverse-DNS (Rev. 14, Commit `09fdc6a`)

> **Hinweis (Rev. 19):** Die hier erwähnten Bezeichner `PatternId`,
> `pattern_ids`, `UiPattern::id()` heißen seit Rev. 19 `PatternName`,
> `pattern_names`, `UiPattern::pattern_name()`. Siehe §13.7 weiter unten.
> Der historische Wortlaut bleibt erhalten.

Ziel: `PatternId.as_str()` (Rust) und `pattern_name` (Python ClassVar)
liefern wörtlich denselben String (`org.platynui.patterns.<Name>`).

**Implementiert:**

- [x] Konstanten-Modul `core::ui::pattern_ids` in
      `crates/core/src/ui/identifiers.rs` (12 `&'static str`-Konstanten)
- [x] `crates/core` umgestellt: `pattern.rs`, `node.rs`, `contract.rs`,
      `contract/testkit.rs`
- [x] `crates/runtime`: `runtime/desktop.rs`, `runtime/test_fixtures.rs`
- [x] `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`
- [x] `crates/provider-atspi/src/node.rs` — `match` durch `if`-Ketten
      ersetzt (`&str`-Konstanten in Patterns nicht erlaubt)
- [x] `crates/provider-mock/src/tree.rs` und `tests.rs` — analog
- [x] `crates/provider-mock/assets/mock_tree.xml` — alle 5 Pattern-
      Varianten voll qualifiziert (kein Loader-Expand-Shortcut)
- [x] PyO3 Bindings in `packages/native/src/runtime.rs`:
  - `PyFocusable::id()` / `PyWindowSurface::id()` → Reverse-DNS via
    `pattern_ids::FOCUSABLE` / `pattern_ids::WINDOW_SURFACE`
  - `pattern_object`-Mapper nutzt Konstanten in `match`
  - `pattern_id_from_arg` liest **`pattern_name`-ClassVar** von
    Pattern-Klassen (siehe Designdoc §5 Z. 624) statt Klassenname
  - Doc-Strings für `has_pattern`, `get_pattern`, `ancestor_pattern`
    aktualisiert
- [x] Pre-existing clippy-Lint in `crates/provider-atspi/src/process.rs:72`
      behoben (Toolchain-Update auf Rust 1.95.0)
- [x] Designdoc-Updates:
  - §13.6 als RESOLVED (Rev. 14) markiert
  - §5 Rev.-13-Hinweis auf aktuellen Stand gebracht
  - XPath-Beispiel in §11a.7 (Z. 2438) auf Reverse-DNS

**Verifikation:**

- `cargo build --workspace --all-features` ✓
- `cargo nextest run --workspace --features mock-provider`
  → **1980/1980 passed** ✓
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` ✓
- `cargo fmt --all` ✓

**Hinweis (PyO3):** `Pattern.id()` und `pattern_object` liefern jetzt
Reverse-DNS-Strings. Aufrufe wie `get_pattern("Focusable")` werden zu
`get_pattern("org.platynui.patterns.Focusable")` bzw.
`get_pattern(Focusable)` (mit `pattern_name`-ClassVar). Da das Projekt
noch nicht veröffentlicht ist, ist das kein Breaking Change im SemVer-Sinn.

---

## Offen

### Designdoku Rev. 15 — Attribute-Modell konsolidiert (uncommitted)

Aus den ursprünglichen `Properties` / `NativeProperties`-Patterns wird
**ein** namespacebasierter Attribut-Schlüsselraum, symmetrisch zur
Rust-Seite (`crates/core/src/ui/{node,namespace}.rs`).

**Doku-Pass (alle 16 Stellen in `dev-docs/python-library-design.md`):**

- Header auf Rev. 15 aktualisiert; Rev-14- und Rev-15-Notes
- §4.1 Tabelle: zwei Properties-Zeilen → eine `attributes[(ns, name)]`
- §4.2, §5a.3, §7.1, §8, §9, §11.4, §12, §14: durchgehend `attributes=`
- §5: Properties-Pattern aus Liste entfernt; expliziter Hinweis-Block
- §A.4 / §A.5: Adapter-Interface auf `attribute_names(namespace=None)`,
  `attribute_value(name, namespace='control')`, `attributes()` umgestellt
- §A.6 Locator: `attributes: dict[str | tuple[str, str], ...]`,
  XPath-Bau-Schritt 4 neu, Locator-Beispiele um `default_attribute_namespace`
  und Cross-Namespace-Tupel-Keys erweitert
- §10 Phase 1: Hinweis auf Tupel-Keys & `default_attribute_namespace`

**Code-Umbau (Phase 1, uncommitted):**

- [x] `core/patterns/properties.py` gelöscht (Properties-Pattern entfällt)
- [x] `core/patterns/__init__.py`: Re-Exports `Properties` /
      `NativeProperties` entfernt
- [x] `core/locator.py`: namespacebasiertes `attributes`-Modell;
      Bare-String → `default_attribute_namespace`,
      Tupel `(ns, name)` → expliziter Namespace; Render-Regel
      „control unprefixed, sonst `@ns:Name`"; neuer
      `default_attribute_namespace`-Parameter in `to_xpath`;
      `DEFAULT_ATTRIBUTE_NAMESPACE = "control"` Modul-Konstante
- [x] `core/weight_calculator.py`: `properties` /
      `native_properties` Criteria → ein `attributes`-Criterion;
      Cache umgestellt auf `dict[(ns, name), Any]`;
      `AdapterLike.attribute_value(name, namespace)` ersetzt die alte
      Properties-Pattern-Indirektion;
      optionaler `default_attribute_namespace`-Konstruktor-Parameter
- [x] Tests `test_patterns.py` / `test_locator.py` /
      `test_weight_calculator.py` angepasst und ausgebaut (jetzt 104
      Tests gesamt, alle grün)

**Verifikation:**

- `uv run ruff check src/PlatynUI tests/PlatynUI` ✓
- `uv run mypy src/PlatynUI tests/PlatynUI` ✓
- `uv run pyright src/PlatynUI tests/PlatynUI` ✓
- `uv run pytest tests/PlatynUI` → **104 passed** ✓

### Designdoku Rev. 16 — Locator-Kwargs (uncommitted)

`Locator` akzeptiert jetzt drei Eingangskanäle für
Attribut-Predicates: (1) sechs typisierte snake_case-Convenience-Felder,
(2) freies `attributes`-Dict, (3) freie `**kwargs` am Konstruktor (und
damit am `@locator`-Decorator). Doppelte Schlüssel über mehrere Kanäle
werfen `TypeError` mit konkreter Quellenangabe — kein stilles
Vorrang-Verhalten.

**Code-Umbau (uncommitted):**

- [x] `core/locator.py`: `@dataclass` durch handgeschriebene Klasse
      mit `__slots__` ersetzt, damit `**extra_attributes` möglich
      werden. Kwargs ohne `__` landen als Bare-String-Keys im
      `attributes`-Dict (Default-Namespace `control`); Kwargs mit
      `<ns>__<name>` werden zu Tupel-Keys (`native__HWND=0xABCD`
      → `attributes[("native","HWND")] = ...`). Mehrere `__` im
      Kwarg-Namen sind nicht erlaubt. Konfliktdetektion erfolgt im
      Konstruktor *nach* Namespace-Normalisierung — d.h.
      `Locator(name="A", Name="B")` und
      `Locator(name="A", attributes={("control","Name"): "B"})`
      werden beide erkannt.
- [x] `RESERVED_FIELDS` als ClassVar exponiert für Introspektion.
- [x] Tests `test_locator.py` um 13 neue Fälle erweitert: PascalCase-
      Kwargs, `ns__name`-Trenner, alle drei Konflikt-Konstellationen,
      Konflikt nach Namespace-Normalisierung, Koexistenz unterschied-
      licher Namespaces, „reservierte Felder nur exakt-snake_case"
      (`Locator(Path="x")` → `@Path="x"`).

**Doku-Pass (`dev-docs/python-library-design.md`):**

- §7.1 Attribut-Namenskonvention: Block von zwei auf drei
  Eingangskanäle erweitert; Empfehlungs-Reihenfolge ergänzt;
  Begründung der Asymmetrie umformuliert.
- §A.6 Locator: Skeleton-Kommentar um `**extra_attributes`-Block
  erweitert; XPath-Bau-Schritt 4 aufgeteilt in (a) Convenience-
  Felder, (b) freie Kwargs (mit `__`-Trenner), (c) `attributes`-
  Dict, (d) `custom_attributes`, plus Konflikt-Regel-Punkt;
  einleitender Absatz erklärt warum kein `@dataclass`.

**Verifikation:**

- `uv run ruff check src/PlatynUI tests/PlatynUI` ✓
- `uv run mypy src/PlatynUI tests/PlatynUI` ✓
- `uv run pyright src/PlatynUI tests/PlatynUI` ✓
- `uv run pytest tests/PlatynUI` → **117 passed** ✓ (104 + 13 neue)

### Designdoku Rev. 17 — Pattern-Konsolidierung + IsInView (uncommitted)

Python-Pattern-Hierarchie an die Rust-Capability-Gruppen
(`crates/core/src/ui/attributes.rs`, `pattern_ids` in
`crates/core/src/ui/identifiers.rs`) angeglichen. Parallel dazu
**Wire-Breaking Change** in Rust: `IsOffscreen`-Attribut zu `IsInView`
umbenannt und semantisch invertiert (Default-Fallback dreht von
`false` auf `true`). Da das Projekt unveröffentlicht ist, kein
SemVer-Breaking.

**Pattern-Mapping Rust ↔ Python (neu):**

| Rust-Modul | Pattern-Name | Python-Klasse | Methoden / Properties |
|---|---|---|---|
| `attributes::element` | `Element` | `Element` | `bounds`, `is_visible`, `is_in_view`, `is_enabled` |
| `attributes::text_content` | `TextContent` | `TextContent` | `text`, `locale`, `is_truncated` |
| `attributes::text_editable` | `TextEditable` | `TextEditable` | `set_text()`, `is_readonly`, `max_length`, `supports_password_mode`, `is_multi_line` |
| `attributes::clearable` (leer) | `Clearable` | `Clearable` | `clear()` |
| `attributes::toggleable` | `Toggleable` | `Toggleable` | `toggle()`, `state`, `supports_three_state` |
| `attributes::activatable` | `Activatable` | `Activatable` | `activate()`, `is_activation_enabled`, `default_accelerator` |
| `attributes::focusable` | `Focusable` | `Focusable` | `is_focused`, `focus()` |

Konsolidierungen gegenüber dem Altprojekt: `HasBounds + Visibility +
HasIsEnabled` → `Element`; `Toggleable + HasToggleState` →
`Toggleable`; `EditableText + HasIsReadonly` → `TextEditable` (+
`TextContent` separiert für read-only); `HasFocus` entfällt, geht in
`Focusable`. Rust-seitig ergänzt: `Clearable`, `Toggleable`,
`TextEditable` als neue PatternIds in `pattern_ids`.

**Rust-Code-Umbau (uncommitted):**

- [x] `crates/core/src/ui/attributes.rs`: `IS_OFFSCREEN` → `IS_IN_VIEW`
      (mit Doc-Cross-Reference); neues leeres Modul `clearable {}`
- [x] `crates/core/src/ui/identifiers.rs`: pattern_ids erweitert um
      `TEXT_EDITABLE`, `CLEARABLE`, `TOGGLEABLE`; Test
      `pattern_ids_are_reverse_dns` erweitert
- [x] `crates/core/src/ui/contract/testkit.rs`: `IS_OFFSCREEN` →
      `IS_IN_VIEW`
- [x] `crates/runtime/src/runtime/desktop.rs`: Desktop-Root liefert
      `IS_IN_VIEW=true` (war `IS_OFFSCREEN=false`)
- [x] `crates/provider-windows-uia/src/{map,node}.rs`:
      `get_is_offscreen()` → `get_is_in_view()` mit invertiertem
      Return; `IsOffscreenAttr` → `IsInViewAttr` (incl. unwrap_or-
      Default invertiert)
- [x] `crates/provider-atspi/src/node.rs`: enum-Variante
      `StdAttrKind::IsOffscreen` → `IsInView`, Wert-Berechnung
      invertiert (Negation entfernt)
- [x] Doku: `dev-docs/architecture.md` (4 Stellen),
      `dev-docs/platform-windows.md`, `dev-docs/platform-linux.md`
      durchgängig auf `IsInView` / `IsEnabled && IsInView`

**Python-Code-Umbau (uncommitted):**

- [x] `core/types.py`: `Point` und `Rect` als Re-Export aus
      `platynui_native` (kanonische pyo3-Bindings statt eigener
      Definition)
- [x] `core/patterns/element.py` (NEU): `Element` mit `bounds`,
      `is_visible`, `is_in_view`, `is_enabled` (Rev. 33:
      `default_click_position` entfernt — lebt jetzt nur noch im
      `MouseProxy`)
- [x] `core/patterns/focusable.py` (NEU): `Focusable` mit
      `is_focused` + `focus()`
- [x] `core/patterns/text.py` (umgeschrieben): `TextContent`,
      `TextEditable`, `Clearable`
- [x] `core/patterns/toggle.py` (umgeschrieben): konsolidiertes
      `Toggleable`; `HasToggleState` weg
- [x] `core/patterns/activation.py` (erweitert): `Activatable` um
      `is_activation_enabled` + `default_accelerator`
- [x] `core/patterns/state.py` und `core/patterns/geometry.py`
      gelöscht
- [x] `core/patterns/__init__.py`: Re-Exports neu (Activatable,
      Clearable, Element, Focusable, PatternBase, Point, Rect,
      TextContent, TextEditable, ToggleState, Toggleable)
- [x] `tests/PlatynUI/test_patterns.py`: komplett neu, 35 Tests inkl.
      `test_legacy_split_patterns_are_gone`,
      `test_pattern_names_match_rust_ids`,
      `test_point_and_rect_come_from_native_module`

**Doku-Pass (`dev-docs/python-library-design.md`):**

- Header auf Rev. 17; Rev-17-Note (Pattern-Konsolidierung +
  IsInView-Rename)
- §5 Pattern-Codeblock vollständig neu (Element, TextContent,
  TextEditable, Clearable, Toggleable, Activatable, Focusable);
  Folgeabsatz mit Konsolidierungs-Tabelle
- §5.3 CheckBox-Beispiel: `HasToggleState` → `Toggleable.state`
- §5.4 RUST_PATTERN_MAP-Beispiel: `HasBounds` → `Element`
- §5a.2 Standard-Rollen-Tabelle: Toggleable/TextContent/TextEditable/
  Clearable
- §8 Outcome-Tabelle: `Focus`/`Toggle`/`Set Value`/`Clear`-Zeilen
- §9 Datei-Layout: `toggle.py`/`text.py`/`element.py`/`focusable.py`-
  Kommentare
- §A.4 Adapter-Interface-Erläuterung
- §A.5 Element-Convenience-Properties auf `patterns.Element`
- §A.7 ElementDescriptor-Beispiel
- §A.9 AdapterMouseProxy auf `patterns.Element`
- §A.10 Pattern-Defaults: `DefaultTextEditable`,
  `Toggleable`-Default-Block

**Verifikation:**

- `cargo check --workspace` ✓
- `cargo nextest run --workspace` → **1980 passed** ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓
- `uv run ruff check src/PlatynUI tests/PlatynUI` ✓
- `uv run mypy src/PlatynUI tests/PlatynUI` ✓
- `uv run pyright src/PlatynUI tests/PlatynUI` ✓
- `uv run pytest tests/PlatynUI` → **119 passed** ✓ (117 + 2 Pattern-
  Identitäts-Tests; alte Pattern-Tests durch konsolidierte Tests
  ersetzt)

### Rev. 18 — `@locator` Decorator-Form (uncommitted)

Bis Rev. 17 war `locator = Locator` ein irreführendes Alias: die
Class-Decorator-Form `@locator(name="X")` lieferte zwar syntaktisch
einen Aufruf, ersetzte die Klasse aber durch eine Locator-Instanz —
*nicht* das, was Designdoc §7.1 / §A.6 spezifiziert.

Rev. 18 ersetzt das Alias durch eine echte Decorator-Funktion mit
zwei Verwendungsformen:

1. **Class-Decorator** (vollständig implementiert):
   ```python
   @locator(name='Calculator', role='Window')
   class CalculatorWindow: ...
   ```
   hängt einen `Locator` als Klassenattribut `__locator__` an und gibt
   die Klasse unverändert zurück.

2. **Method/Property-Decorator** (Phase-3-Stub):
   ```python
   class CalculatorWindow:
       @property
       @locator(AutomationId='num5Button')
       def n5(self) -> Button: ...
   ```
   Die Methode wird durch einen `LocatorMethodDescriptor` ersetzt, der
   den Locator und die Wrapped-Function speichert. Beim Zugriff auf
   einer Instanz wird derzeit `NotImplementedError("Phase 3")`
   geworfen. Die volle Resolution (Return-Type-Annotation lesen,
   `ContextBase.get(annotation, locator=...)` aufrufen) braucht
   `ContextBase` aus Phase 3.

**Begründung für die Stub-Variante:** Context-Code kann bereits
heute mit beiden Formen geschrieben werden; der Phase-3-Übergang
muss nur den `__get__`-Body austauschen, nicht die API. Das verhindert
spätere Quelltext-Änderungen an Contexts.

**Konkrete Änderungen:**

- [x] `src/PlatynUI/core/locator.py`: `locator = Locator`-Alias
      entfernt; `LocatorMethodDescriptor`-Klasse + `def locator(...)`
      mit identischer Kwargs-Signatur wie `Locator.__init__`
      hinzugefügt; `__all__` um `LocatorMethodDescriptor` erweitert.
- [x] `tests/PlatynUI/test_locator.py`: alter `test_locator_alias_is_class`
      entfernt; 9 neue Tests für Class-Decorator (Attribut-Anhang,
      Klasse-unverändert, free-form Kwargs, kwargs match constructor),
      Method-Decorator-Stub (Descriptor-Returntyp,
      `NotImplementedError` bei Instanzzugriff, Class-Access liefert
      Descriptor, `__set_name__` setzt `attr_name`), und
      Decorator-API-Identität (`locator is not Locator`, Reject von
      Nicht-Class/Nicht-Callable).
- [x] `dev-docs/python-migration-status.md` Phase 3 erweitert: explizite
      To-Do-Position für die Method-Decorator-Vervollständigung.

**Verifikation:**

- `cargo` unverändert.
- `uv run ruff check src/PlatynUI tests/PlatynUI` ✓
- `uv run mypy src/PlatynUI tests/PlatynUI` ✓
- `uv run pyright src/PlatynUI tests/PlatynUI` ✓
- `uv run pytest tests/PlatynUI` → **128 passed** ✓ (119 + 9 neue
  Decorator-Tests; ein alter Alias-Test entfernt).

### Phase 2 — Adapter-Schicht (Designdoc §10 Phase 2)

- [x] `core/adapter.py` — Adapter-ABC (§A.4) inkl. Template-Method-
      `_resolve_pattern`. 26 Tests.
- [x] `core/adapter_proxy.py` — `AdapterProxy` (Komposition),
      `PatternProxyFactory`, `@pattern_proxy_for`. 36 Tests.
- [x] `core/adapters/ui_node.py` — `UiNodeAdapter` über `platynui_native`
      (§A.4a) mit nativen `Focusable`-Wrappern. 32 Tests gegen
      `Runtime.new_with_mock()`. (Der ursprünglich vorgesehene
      `UiNodeTechnology`-Singleton ist mit Rev. 35 entfallen.)
- [x] `core/runtime.py` — Process-wide Runtime-Singleton (`runtime` /
      Klasse `Runtime`) mit `current` (lazy default), `set()`, `reset()`,
      `is_initialised()`. 12 Tests. Designdoc §A.5 (Rev. 20).
- [x] `core/devices.py` — `MouseProxy`/`KeyboardProxy` über
      `platynui_native.Runtime` (greifen auf `runtime.current` zu).

(`core/patterns/` Pattern-ABCs wurden in Phase 1 vorgezogen, siehe
Phase-1-Punkt 10 im Designdoc §10. Pattern-Default-Implementierungen
in `core/patterns/defaults.py` bleiben Phase 4. Ein dedizierter Python-
`MockAdapter` entfällt — Tests gegen den UI-Tree nutzen den Rust-Mock-
Provider; ABC-/Algorithmus-Tests nutzen Inline-Fakes. Siehe Designdoc
§A.11 und §11a.2.)

### Phase 3 — Context-Schicht (Designdoc §10 Phase 3)

- [ ] `core/context.py` — `ContextBase` (§A.5)
- [ ] `core/descriptor.py` — `ElementDescriptor[PatternT]` (§A.7)
- [ ] `@context`-Mechanik + `ContextFactory` (Klassenregistry pro
      Rolle, gewichtetes Match)
- [x] **`@locator` Method/Property-Form vervollständigt** (Rev. 49) —
      `LocatorMethodDescriptor.__get__` (bare Form) und `__call__`
      (`@property`-Form) lesen die Return-Type-Annotation der
      dekorierten Methode (eine `ContextBase`-Subklasse, via
      `typing.get_type_hints` einmalig gelesen + gecacht) und rufen
      `instance.get(annotation, locator=self.__locator__)` auf.
      `TypeError` bei Nicht-`ContextBase`-Annotation oder
      Nicht-`ContextBase`-Instanz. Class-Decorator-Form
      (`@locator(name="...")` auf Klasse → `__locator__`-Attribut) war
      bereits Phase-1-DONE.

(`core/locator.py` und `core/weight_calculator.py` wurden in Phase 1
abgeschlossen. Beide `@locator`-Formen (Class-Decorator und
Method/Property) sind jetzt vollständig — die Method-Form wurde in
Rev. 49 nachgereicht, ohne Quelltext-Änderungen an bereits
geschriebenen Contexts.)

### Phase 4 — UI-Klassen + Standard-Proxies (Designdoc §10 Phase 4)

Designdoc §10 listet die Items 16–21 (`ui/proxies/base.py`,
`ui/element.py`, `ui/control.py`, `ui/buttons.py` + `ui/proxies/standard.py`,
`ui/window.py` + `ui/proxies/window.py`, …). Wir gliedern Phase 4 in
Sub-Phasen, die jeweils einen kompletten Doku→Code→Tests→Commit-Zyklus
durchlaufen.

**Bereits committed (Auszug aus 4a/4c/4f-Items):**

- `core/patterns/` — Pattern-ABCs (Activation, ActivationTarget,
  Closeable, Element, Expandable, Focusable,
  HasEditor, ItemContainer, Maximizable, Minimizable, Movable,
  Readable, Resizable, Responsive, Restorable, Selectable, Text,
  Toggle). (`ApplicationReady` in Rev. 49 gestrichen — Duplikat von
  `Responsive`.)
- `ui/element.py`, `ui/control.py` — Basis-Contexts (Item 16
  UI-Teil).
- `ui/window.py` — `Window` und `Frame` (Item 18 UI-Teil).
- `ui/desktopbase.py`, `ui/desktop.py`, `ui/application.py` — Item 21
  UI-Teil.
- 466 pytest-Tests grün, davon 79 für die committeten UI-Klassen.

**Sub-Phasen:**

#### Phase 4-pre — Pipeline-Lücke schließen (DONE, uncommitted)

Designdoc §4.4 Schritt 3 ("`PatternProxyFactory.find_proxy_for`
innerhalb der Adapter-Auflösung") ist jetzt aktiv. Bis zum Abschluss
dieser Sub-Phase exposten `find_one`/`find_all` nur
Provider-Patterns; registrierte `@pattern_proxy_for`-Klassen wurden
ignoriert.

- [x] `AdapterProxy` als `Adapter`-Subklasse umgebaut
      (Designdoc §A.4 angepasst). `_resolve_pattern` delegiert an den
      gewrappten Adapter; `parent`/`children`-Signaturen auf
      `Adapter` zurückgeführt; `AdapterFacade`-Alias komplett
      entfernt. Dadurch kann
      `AdapterFactory.find_one/find_all` weiter `Adapter | None`
      zurückgeben, ohne Konsumenten (`ContextBase`,
      `ElementDescriptor`, `devices.py`) anpassen zu müssen.
- [x] `RuntimeAdapterFactory._wrap` ruft
      `PatternProxyFactory.find_proxy_for(adapter)` direkt nach
      `UiNodeAdapter.from_node(...)` auf. Designdoc §4.4 entsprechend
      präzisiert (kein „idealerweise" mehr).
- [x] Tests in `tests/PlatynUI/test_adapter_factory.py`:
      `test_find_one_returns_raw_adapter_without_matching_proxy`,
      `test_find_one_wraps_adapter_in_matching_proxy`,
      `test_find_all_wraps_each_adapter_in_matching_proxy`,
      `test_find_one_proxy_chooses_highest_score`. Bestehender
      `test_adapter_facade_is_runtime_usable_union` zu
      `test_adapter_facade_is_alias_for_adapter` umgeschrieben.
- [x] 470 pytest grün, ruff/mypy/pyright clean.

**Reihenfolge-Entscheidung (2026-04-27):** Die Default-Proxy-
Hierarchie (`ui/proxies/base.py` + alle Widget-Proxies) wird ans
Ende von Phase 4 verschoben. Begründung: `ElementProxy`/
`ControlProxy` ohne konkrete Widget-Subklassen sind reine
Aufhänger ohne Verhalten, und die Widget-Proxies leisten ihren
Mehrwert (Click-/Tastatur-Fallbacks) erst, wenn sie ein Provider-
Pattern *ersetzen* können. Wir bauen daher zuerst die UI-Klassen
gegen den reinen Provider-Pattern-Pfad (Mock-Adapter liefert
Pattern direkt) und führen die komplette Proxy-Schicht in einer
abschließenden Sub-Phase ein, sobald reale Fallbacks motiviert
sind.

#### Phase 4a — Buttons (Item 17, eingeschränkt: Button + CheckBox) — DONE (Commit `28898b2`, 2026-04-27)

- [x] Designdoc-Update §A.14.9 (Rev. 27): `AbstractButton` als
      abstrakte Zwischenklasse mit `text`-Convenience über
      `TextContent`; `Button` wrappt `Activatable`; `CheckBox`
      wrappt `Toggleable` mit `check`/`uncheck`/`toggle`/
      `set_state`/`is_checked`-Komfort. `CheckBox.activate()`
      ruft semantisch `check()`. Phase 4a verlangt den
      Provider-Pattern-Pfad; Click-Fallback ist Sache von
      Phase 4e.
- [x] `src/PlatynUI/ui/buttons.py`: `AbstractButton(Control,
      register=False)` mit abstract `activate()` und `text`
      Property, `Button(AbstractButton)` mit Pre/Perform/Post-
      Activate, `CheckBox(AbstractButton)` mit `state`/
      `is_checked`/`is_unchecked`/`check`/`uncheck`/`toggle`/
      `set_state`. `set_state` durch `len(ToggleState)` begrenzt.
- [x] `tests/PlatynUI/_ui_helpers.py` um `TextContentStub` und
      `ToggleableStub` (mit konfigurierbarem Cycle für Tri-State)
      erweitert.
- [x] `tests/PlatynUI/test_buttons.py`: 22 Tests (text via
      Pattern + Default, Activate-Pfad inkl. Predicate-Block bei
      `is_enabled=False`, raise wenn `Activatable` fehlt,
      CheckBox-State-Read-Back, `is_checked`/`is_unchecked`,
      raise wenn `Toggleable` fehlt, Toggle blockt bei
      `is_readonly`, `check`/`uncheck` no-op + toggle, Tri-State-
      Cycle erreicht `INDETERMINATE`, `set_state` terminiert auch
      ohne Treffer, Auto-Registrierung von Button/CheckBox).
- [x] `tests/PlatynUI/test_descriptor.py`: lokale Klassen
      `Button`/`Forced` zu `_TestButton`/`_TestForced`
      umbenannt, damit `__init_subclass__` nicht mit
      `ui.buttons.Button` kollidiert.
- [x] 492 pytest grün, ruff/mypy/pyright clean.
- [x] **Stop für Review** (jetzt).

#### Phase 4b — Text/Edit (Item 19 UI-Teil, ohne ComboBox)

ComboBox wandert nach Phase 4c, da sie auf `ListItem` angewiesen
ist und mehrere noch nicht existierende Patterns benötigt
(`Expandable`, `Selectable`, `Editable`). Phase 4b deckt nur
die reinen Text-Widgets ab.

- [x] Designdoc-Spec §A.14.10 (Text) und §A.14.11 (Edit) erstellt.
- [x] `core/patterns/text.py`: `TextEditable.is_multi_line` ergänzt
      (Rev. 31 — von `TextContent` auf `TextEditable` verschoben).
- [x] `ui/text.py`: `Text(Control)` (read-only via `TextContent`)
      und `Edit(Control)` (read+write via `TextContent` +
      `TextEditable` + `Clearable`). Keine Vererbungsbeziehung
      zwischen beiden.
- [x] Tests in `tests/PlatynUI/test_text.py` gegen den
      Provider-Pattern-Pfad: `text` lesen, `set_text`/`clear`
      mit Predicate-Block bei `is_enabled=False`,
      `is_readonly=True`, fehlendem Focus, fehlenden Patterns.
- [x] `_ui_helpers.py` um `TextContentStub`, `TextEditableStub`,
      `ClearableStub` erweitert.
- [x] pytest grün, ruff/mypy/pyright clean.

#### Phase 4c — ComboBox + Lists/Tree/Table (Item 19 ComboBox + Item 20 UI-Teil) — DONE (Commit `c0f7552`, 2026-04-27)

Komplett Python-seitig: Pattern-ABCs + Item-Hierarchie + Container-
Klassen + Tests gegen Stubs. Rust-`pattern_names`-Konstanten und
Native-Adapter-Anbindungen folgen in einer späteren Phase, sobald
Provider-Bindings konkret werden.

- [x] Designdoc-Spec §A.14.12 (Item-Hierarchie), §A.14.13 (List/
      ListItem), §A.14.14 (Tree/TreeItem), §A.14.15 (Table/Row/Cell/
      EditableCell), §A.14.16 (ComboBox), §A.14.17–§A.14.20
      (Pattern-Specs Selectable/Expandable/HasEditor/ItemContainer),
      §A.14.21 (Item-Lifecycle/Predicates).
- [x] `core/patterns/selectable.py`: `Selectable` (`is_selected`,
      `select()`).
- [x] `core/patterns/expandable.py`: `Expandable` (`can_expand`,
      `is_expanded`, `expand()`, `collapse()`).
- [x] `core/patterns/has_editor.py`: `HasEditor` (`open_editor()`,
      `accept()`, `cancel()`).
- [x] `core/patterns/item_container.py`: `ItemContainer`
      (`item_count`, `row_count`, `column_count`).
- [x] `core/patterns/__init__.py` Re-Exports ergänzt.
- [x] `ui/item.py`: `Item(Element, register=False)` Marker mit
      `text`-Property; `SelectableItem(Item, register=False)`;
      `ExpandableItem(Item, register=False)`;
      `EditableItem(Item, register=False)` mit
      `set_text`/`clear` über `HasEditor`-Lifecycle.
- [x] `ui/lists.py`: `List(Control)` mit `ItemContainer`-Wrapper +
      `get_item(s)`/`iter_items`/`select`; `ListItem(SelectableItem)`.
- [x] `ui/tree.py`: `Tree(Control)` analog; `TreeItem(SelectableItem,
      ExpandableItem)` mit eigenem `item_count`/Children-Lookup.
- [x] `ui/table.py`: `Table(Control)`/`Row(Item)`/`Cell(Item)`/
      `EditableCell(Cell, EditableItem)`.
- [x] `ui/combobox.py`: `ComboBox(Control)` mit
      `expand`/`collapse`/`get_item(s)`/`select`/`text`/`set_text`
      und `_expanded()`-Context-Manager. `iter_items` als
      Generator-Funktion (`yield from`), damit das Dropdown
      während der Iteration offen bleibt — Bugfix gegenüber dem
      Legacy-Code.
- [x] `ui/__init__.py` Re-Exports ergänzt + Hierarchie-Diagramm
      im Modul-Docstring aktualisiert.
- [x] `_ui_helpers.py` um `SelectableStub`, `ExpandableStub`,
      `HasEditorStub`, `ItemContainerStub` erweitert.
- [x] Tests pro UI-Klasse: `test_item.py` (19), `test_lists.py` (9),
      `test_tree.py` (11), `test_table.py` (12), `test_combobox.py`
      (14, inkl. Generator-Korrektheits-Test).
- [x] pytest grün (618/618), ruff/mypy/pyright clean.

#### Phase 4d — Menus/Tabs (Item 21 Rest, UI-Teil)

Letzte Standard-Container der UI-Schicht. Tabs folgen dem
List/ListItem-Muster aus Phase 4c. Menus sind eigenständig:
`MenuItem` erbt `Control` (kein `Item`), und `activate()` muss
die Vorgänger-Hierarchie öffnen, bevor der Blatt-Eintrag
ausgelöst werden kann.

- [x] Designdoc-Spec §A.14.23 (TabList/TabItem), §A.14.24
      (Menu/MenuBar/MenuItem), Korrektur Z. 4105 (`MenuItem` raus
      aus Item-Aufzählung), Rev-36-Note.
- [x] `ui/tabs.py`: `TabItem(SelectableItem)`;
      `TabList(Control)` mit `item_count`/`get_items`/
      `iter_items`/`get_item`/`select` (Form gespiegelt von
      `lists.py` mit `*, locator=None` und `scope='children'`).
- [x] `ui/menus.py`: `Menu(Control)`; `MenuBar(Control)`;
      `MenuItem(Control)` mit `activate()` (Vorfahren-Walk →
      `expand()` von außen nach innen →  `Activatable.activate()`
      auf self).
- [x] `ui/__init__.py` Re-Exports + Hierarchie-Diagramm im
      Modul-Docstring ergänzen.
- [x] Tests:
      - `test_tabs.py` (9 Tests): Registrierung, `item_count`,
        `get_items`/`iter_items`/`get_item`/`select` mit
        `scope='children'`, locator-Filter.
      - `test_menus.py` (10 Tests): Registrierung Menu/MenuBar/
        MenuItem; `activate()` ohne Vorfahren; fehlendes
        `Activatable`-Pattern wirft; Vorfahren-Kette wird
        außen → innen expandiert; bereits expandierter
        Vorfahre wird übersprungen; Vorfahre ohne
        `Expandable` wird still übergangen; Walk stoppt am
        `Window`-Boundary; Walk durch Menu-Popup-Container
        zwischen MenuItems.
- [x] pytest grün (630 Tests, +19); ruff/mypy/pyright clean.

#### Phase 4-rust-split — Rust-Trait-Splittung + Python-Pattern-Anpassung (Rev. 37, eingeschoben vor 4e)

Voraussetzung für die komplette Default-Proxy-Schicht (Phase 4e): das
Rust-Mega-Trait `WindowSurfacePattern` (8 Methoden) wird in 7
orthogonale Sub-Traits aufgesplittet, damit der Window-Proxy auf
Python-Seite dieselben granularen ABCs konsumieren kann, die in
`core/patterns/` schon existieren. Parallel zieht die Python-Seite
zwei Pattern-Renames nach (`HasUserInput` → `Responsive`, `Titled`
entfällt) und entfernt `Focusable` an Window-/TopLevel-Elementen.

**Designdoc (Rev. 37):**

- [x] Header + Rev-Notiz Rev. 37.
- [x] §A.13: Pattern-Suite-Tabelle (Spalte „Implementiert wo?",
      `Titled` raus, `HasUserInput` → `Responsive`); neue Absätze
      „Activatable mit zwei Implementations-Pfaden",
      „`is_focused` vs. `is_active` getrennt",
      „`Responsive.accepts_user_input()` ≠ `is_active`".
- [x] Neue §A.13.1 Rust-Trait-Splittung (Trait-Tabelle,
      Provider-Migration, Python-Skizze).
- [x] §A.13.2/§A.13.3: `Titled` raus, `Responsive` rein.
- [x] §5.1 dritte Quelle (`core/patterns/defaults.py`) gestrichen,
      Drei-Ebenen-Fallback wird Zwei-Ebenen-Fallback (§5a.3).
- [x] §A.10 (`patterns/defaults.py`) komplett gestrichen.
- [x] §6.1 Rust-Capabilities aktualisiert.
- [x] Verzeichnisbaum (`patterns/window.py`-Kommentar,
      `patterns/defaults.py` raus, `proxies/window.py`-Kommentar).
- [x] §A.14.5 (`Window`-Klasse) `Titled.title` → liest direkt
      `control:Name`.
- [x] `_application_is_ready` von `HasUserInput` auf `Responsive`
      umgestellt.
- [x] Standard-Rollen-Tabelle (Window-Zeile auf Sub-Patterns).
- [x] Phase-2-Plan-Punkt 10 (Strikethrough für `defaults.py`).
- [x] Schluss-Summary auf Sub-Patterns.

**Rust-Migration (DONE):**

- [x] `crates/core/src/ui/pattern.rs`: `WindowSurfacePattern`-Trait
      und `WindowSurfaceActions`-Builder durch 8 Sub-Trait-Definitionen
      ersetzt (`ActivatablePattern`, `MinimizablePattern`,
      `MaximizablePattern`, `RestorablePattern`, `CloseablePattern`,
      `MovablePattern`, `ResizablePattern`, `ResponsivePattern` —
      letzteres trägt `accepts_user_input() -> Result<Option<bool>>`
      als Methode); `declare_action_pattern!`-Makro für die fünf
      Pure-Action-Traits, `MovableAction`/`ResizableAction`/
      `ResponsiveAction` mit typisierten Payloads.
- [x] `crates/core/src/ui/attributes.rs`: aggressiv aufgeräumt auf
      12 aktiv genutzte Submodule (`activatable`, `minimizable`,
      `maximizable`, `closeable`, `movable`, `resizable`, `focusable`,
      `activation_target`, `application`, `common`, `desktop`,
      `element`); 16 ungenutzte Submodule (text/toggle/etc.) gelöscht.
      `IsTopmost` lebt unter `attributes::activatable`;
      `AcceptsUserInput`-Attribut ersatzlos entfernt.
- [x] `crates/core/src/ui/identifiers.rs`: 9 neue Pattern-Name-
      Konstanten ergänzt; `WINDOW_SURFACE` entfernt.
- [x] `crates/core/src/ui/mod.rs`: Re-Exports umgestellt.
- [x] Provider-Migration: `provider-atspi/src/node.rs` (Resolver-
      Struct + `make_window_pattern`-Factory),
      `provider-windows-uia/src/node.rs` (`pattern_by_name`-Dispatch
      mit `ElemSend`; `CanMove`/`CanResize` statt
      `SupportsMove`/`SupportsResize`),
      `provider-mock/src/{tests.rs,window.rs,tree.rs,input.rs}` +
      `assets/mock_tree.xml` (ACTIVATABLE als kanonischer Top-Level-
      Marker).
- [x] `crates/runtime/src/runtime/window.rs`: `pattern::<>()`-
      Aufrufe auf die einzelnen Sub-Traits umgestellt; `move_to` +
      `resize` werden inline komponiert (kein `move_and_resize` mehr).
- [x] `crates/cli/src/commands/window.rs`: probt ActivatableAction
      als Marker, dann jedes Sub-Pattern einzeln.
- [x] `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`:
      `WINDOW_SURFACE` → `ACTIVATABLE`-Marker.
- [x] PyO3 Bindings in `packages/native/src/runtime.rs`: 8 PyKlassen
      (`PyActivatable`, `PyMinimizable`, `PyMaximizable`, `PyRestorable`,
      `PyCloseable`, `PyMovable`, `PyResizable`, `PyResponsive`).
- [x] BareMetal-Python (`src/PlatynUI/BareMetal/__init__.py`): 13
      Keywords nutzen die Sub-Patterns; `move_and_resize_window`
      komponiert `Movable.move_to` + `Resizable.resize` inline.
- [x] cargo nextest grün (1981/1981), cargo clippy strict grün.

**Python-Migration (DONE):**

- [x] `core/patterns/has_user_input.py` → `responsive.py` umbenannt;
      `pattern_name = "org.platynui.patterns.Responsive"`; Methode
      `accepts_user_input()` unverändert.
- [x] `core/patterns/titled.py` gelöscht.
- [x] `core/patterns/__init__.py`: Re-Exports geupdated
      (`Responsive` rein, `HasUserInput`/`Titled` raus).
- [x] `src/PlatynUI/ui/element.py` `_application_is_ready`-Predicate
      von `HasUserInput` auf `Responsive` umgestellt.
- [x] `src/PlatynUI/ui/window.py` `Window.title` liest direkt
      `self.name` (kein `Titled`-Pattern mehr).
- [x] Test-Helpers / Window-Element-Stubs angepasst (`HasUserInputStub`
      → `ResponsiveStub`, `TitledStub` entfernt).
- [x] pytest grün (629 Tests); ruff/mypy/pyright clean.

**Quality Gate:** `cargo fmt --all`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo nextest run --workspace`,
`uv run ruff check .`, `uv run mypy .`, `uv run pyright`,
`uv run pytest`.

#### Phase 4e — Proxy-Schicht (Item 16 Proxy-Teil + Items 17/18/19/20/21 Proxy-Teile) — DONE (Rev. 43–48, zuletzt Commit `e16bfd6`)

Bündelt die komplette Default-Proxy-Hierarchie inklusive
Widget-Proxies mit Click-/Tastatur-Fallbacks. Wird erst nach den
UI-Klassen **und nach Phase 4-rust-split** gebaut, damit der
Window-Proxy die granularen Sub-Traits aus Rev. 37
(`Activatable`/`Minimizable`/…/`Responsive`) konsumieren kann.

- [x] Designdoc-Update: §A.13.4 ergänzt (Default-Proxy-Hierarchie,
      Pass-Through vs. Synthetic, Strategie-Tabelle pro Proxy).
- [x] **Native-Wrapper-Schicht im `UiNodeAdapter`** (Rev. 38):
      9 Native-Wrapper-Klassen (`_NativeActivatable`,
      `_NativeWindowState`, `_NativeMinimizable`, `_NativeMaximizable`,
      `_NativeRestorable`, `_NativeCloseable`, `_NativeMovable`,
      `_NativeResizable`, `_NativeResponsive`) plus
      `_NATIVE_PATTERN_BUILDERS`/`_NATIVE_PATTERN_TYPES`-Registries.
      Neues `WindowState`-Pattern-ABC; `Window.is_active`/
      `is_topmost`/`is_modal` lesen über `WindowState`.
- [x] **`WindowState.is_modal` + Rust-Modul-Split** (Rev. 39):
      Drittes Read-only Window-Statusbit `is_modal` ergänzt;
      `attributes::window_state` als neues Rust-Modul (IS_ACTIVE,
      IS_TOPMOST, IS_MODAL); atspi-Provider-Bug gefixt
      (`IsTopmost` → `IsActive` umbenannt, neuer `IsModal` via
      `State::Modal`); Windows-UIA-Provider exposiert jetzt
      `IsActive` (Foreground-Vergleich) und `IsModal`
      (`CurrentIsModal`).
- [x] `ui/proxies/__init__.py` mit Side-Effect-Imports (Rev. 43):
      registriert `base`/`buttons`/`combobox`/`item`/`text` via
      `@pattern_proxy_for`. Einziger erlaubter Import-Side-Effect.
- [x] `ui/proxies/base.py` (Rev. 43): `ElementProxy(AdapterProxy)`,
      `ControlProxy(ElementProxy)` als reine Marker — `Element`/
      `ActivationTarget`/`Readable`/`Focusable` liefert der
      `UiNodeAdapter` nativ.
- [x] `ui/proxies/buttons.py` (Rev. 43): `ButtonProxy` (synthetic
      `Activatable` via Click), `CheckBoxProxy` (synthetic
      `Toggleable` via Click, State aus `IsToggled`/`IsSelected`).
- [x] `ui/proxies/text.py` (Rev. 43): `EditProxy`/`TextProxy`
      (synthetic `TextContent`/`TextEditable`/`Clearable` via Click +
      Ctrl+A + type/Delete).
- [x] `ui/proxies/combobox.py` (Rev. 43): `ComboBoxProxy` (synthetic
      `Expandable`/`Selectable`/`TextContent`/`TextEditable`).
- [x] `ui/proxies/item.py` (Rev. 43–47): `ItemProxy` +
      `ListItemProxy`/`TreeItemProxy`/`TabItemProxy`/`RowProxy`/
      `CellProxy`/`MenuItemProxy`. Read/Action-Split (Rev. 46/47):
      `IsSelectable`/`IsExpandable` lesen Attribute, `Selectable`/
      `MultiSelectable`/`Expandable` synthetisieren Click/Ctrl+Click.
- [x] Action-Synthese-Schicht (Rev. 48): `AdapterMouseProxy`/
      `AdapterKeyboardProxy` in `core/adapter_devices.py`;
      `MouseProxy.ctrl_click()` als echte Methode; `_mixins.py`
      gelöscht (Commit `e16bfd6`).
- [x] Tests pro Proxy: `test_proxies.py` (Registry-Auflösung aller
      14 Rollen + Verhaltenstests je Proxy über `patch_actions`-
      Fixture).
- [x] **`ui/proxies/window.py` entfällt (Rev. 38):** der geplante
      `WindowProxy`/`FrameProxy` (Pure Pass-Through) wird nicht
      gebaut — er hätte das Native-Wrapping dupliziert. Die
      Window-Sub-Patterns (`Activatable`/`Minimizable`/…/`Responsive`)
      kommen nativ über `_Native*`-Wrapper im `UiNodeAdapter`;
      `Window`/`Frame`/`Dialog` greifen auf `ControlProxy` zurück.
- [x] **Container-Default-Proxies abgelöst (Rev. 44):** die in
      Rev. 43 noch gebauten `ListProxy`/`TreeProxy`/`TabListProxy`/
      `MenuProxy`/`MenuBarProxy`/`TableProxy` wurden ersatzlos
      entfernt — `List`/`Tree`/`TabList`/`Row` erben stattdessen die
      generische Context-Basis `ItemContainer[I: Item]`.

**Schon erledigte Item-Bestandteile** sind im Auszug oben gelistet;
sie zählen als Vorgriff auf den UI-Teil von Item 16 (Element/
Control), Item 18 (Window) und Item 21 (Desktop/Application).

**Phase 4e DONE** (Rev. 43–48; zuletzt Commit `e16bfd6`). Die
Proxy-Schicht ist vollständig: Default-Proxies für alle Standard-
Rollen synthetisieren ihre Action-Patterns über Maus/Tastatur, die
Tests decken Registry-Auflösung und Verhalten ab.

**`ApplicationReady` gestrichen (Rev. 49)** — der offene Punkt aus
Rev. 40 wird **nicht verdrahtet, sondern entfernt**. Ein
Verdrahtungs-Experiment (Rust-Trait `ApplicationReadyPattern` +
Provider mock/atspi/uia + PyO3 + Native-Wrapper) bestätigte, dass
`ApplicationReady` auf allen realen Backends nur an dieselbe
Responsiveness-Probe wie `Responsive` delegieren würde — ein reines
Duplikat ohne eigene Semantik. Daher: Waisen-ABC
`core/patterns/application_ready.py` gelöscht, Export aus
`core/patterns/__init__.py` entfernt, kein Rust-/PyO3-/Adapter-Code.
`_application_is_ready` bleibt auf **`Responsive`** (nativ,
Window-Responsiveness) **+ `Application.is_ready()`** (User-Hook).
Eine echte App-Load-Semantik (z.B. „Splash dismissed") würde, falls je
benötigt, als **eigenständiges** Pattern eingeführt — nicht als
Responsive-Alias.

### Phase 5 — Keywords + Robot-Library (Designdoc §10 Phase 5)

- [ ] Library-Init und Lifecycle (§A.8)
- [ ] Keywords (§8)
  - [ ] `Wait Until Ready` (`keywords/wait.py`) — neben `Wait Until
        Exists`/`Gone`. Outcome-orientiert (nicht `Wait Until
        Responsive`, §2.2). Setzt eine **neue öffentliche** Methode
        `Element.wait_until_ready()` voraus, die das bisher private
        Predicate `_application_is_ready` (`Responsive` +
        `Application.is_ready()`) über `ensure_that` (raises bei
        Timeout) kapselt. Argument-Typ: schlichtes
        `ElementDescriptor` **ohne** `[Responsive]`-Constraint —
        Readiness wird über den `top_level_parent` aufgelöst, nicht
        am Element selbst (sonst lehnt der Converter alles ab, das
        `Responsive` nicht trägt). Optionale window-genaue Variante
        `Wait Until Responsive` (typed `ElementDescriptor[Responsive]`)
        nur bei konkretem Bedarf — sonst Redundanz.
- [ ] Highlight + Diagnose (§A.12)

### Phase 6 — Iterative Erweiterungen (Designdoc §10 Phase 6)

- [ ] Application-Lifecycle Patterns
- [ ] Spezielle Container/Layout-Patterns
- [ ] Native-Read/Write-Escape-Hatches (§13.5)

---

## Offene Designfragen

Verweis auf Designdoc §13:

- §13.1 BareMetal vs. PlatynUI — Abgrenzung
- §13.3 `UiNode.supported_patterns` in Python
- §13.4 Mehrere Runtimes / Pabot
- §13.5 Native Read-Escape-Hatch (Write-Pfad)
- §13.6 ✓ RESOLVED in Rev. 14

---

## Konventionen für diesen Tracker

- Phasen entsprechen §10 des Designdokuments.
- Items werden mit `[x]` markiert, sobald Code + Tests + Lint grün.
- Bei größeren Umstellungen wird eine eigene §13.x-Sektion im Designdoc
  angelegt und hier als "RESOLVED in Rev. N" referenziert.
- Commit-Hashes nur für gemergte Arbeit; uncommittete Arbeit explizit
  als "uncommitted" markieren.
