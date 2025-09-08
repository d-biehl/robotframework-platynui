# XPath 2.0 Evaluator – Detaillierte Umsetzungs‑Matrix nach W3C‑Dokumenten

Quellen geprüft:
- `docs/xpath/XML Path Language (XPath) 2.0 (Second Edition).html`
- `docs/xpath/XQuery 1.0 and XPath 2.0 Data Model (XDM) (Second Edition).html`
- `docs/xpath/XQuery 1.0 and XPath 2.0 Functions and Operators (Second Edition).html`
- Interner Plan: `docs/xpath20-evaluator-plan.md`

Hinweis: Der Plan nennt „Draft 4 — M1–M7 abgeschlossen, M8 als nächstes“. Im Code sind bereits Teile von M8 vorhanden (Date/Time `current-*`, `DynamicContextBuilder.with_now/with_timezone`). Diese Matrix bildet den tatsächlichen Stand ab und verweist auf Code/Tests.

## Dokument 1: XPath 2.0 (Syntax/Semantik)

- Parser/Grammatik: Fertig.
  - Grammatik: `crates/platynui-xpath/src/xpath2.pest:1`
  - AST‑Typen: `crates/platynui-xpath/src/parser/ast.rs:1`
  - Parser/AST‑Builder: `crates/platynui-xpath/src/parser.rs:1`
  - Abdeckung in Tests: `crates/platynui-xpath/tests/parser/*.rs:1`

- Ausdrücke – implementiert (✓):
  - Literale: String/Integer/Decimal/Double; leere Sequenz `()` (AST erzeugt). Parser: `parser.rs:~60`, `~95`, `~116`.
  - Kontext/Variablen: `.` (Context‑Item), `VarRef` mit ExpandedName; Unbekannte Variable → `err:XPST0008`. Evaluator: `evaluator.rs:71`, `~101`.
  - Pfade/Schritte/Prädikate: Absolute/Relative/`//`, alle Achsen inkl. Abkürzungen; Prädikate mit Positions‑Semantik (numerisch vs. boolsch). Compiler/Evaluator: `compiler.rs:300+`, `evaluator.rs:118+` Tests: `evaluator_paths.rs:1`, `evaluator_axes.rs:1`.
  - Vergleiche: Value (`eq/ne/lt/le/gt/ge`) und General (`=, !=, <, <=, >, >=`) inkl. Atomisierung/Promotion und Collation‑Einfluss. Evaluator: `evaluator.rs:185`, `~197`, `~582+`.
  - Knotenmengen/Set‑Ops: `union/intersect/except` (nur Nodes), Dedupl. + Sortierung in Dokumentreihenfolge. Evaluator: `evaluator.rs:204+`, `~870+`.
  - Knotenvergleiche: `is`, `<<`, `>>`; Multi‑Root Schutz mit `err:FOER0000`. Evaluator: `evaluator.rs:216+`, `~231+`; Tests: `evaluator_node_comp.rs:1`, `evaluator_multiroot_errors.rs:1`.
  - Arithmetik: `+ - * div idiv mod`, unäres Minus/Plus. Compiler: `compiler.rs:212+`; Evaluator: `evaluator.rs:28+`, `~646+`.
  - Bereich: `a to b` als aufsteigende Ganzzahlfolge, `start<=end` sonst leer. Evaluator: `evaluator.rs:240+`.
  - Sequenzen: Komma‑Operator, `MakeSeq`. Compiler/Evaluator: `compiler.rs:296+`, `evaluator.rs:171`.
  - Typen: `cast as`, `castable as`, `treat as`, `instance of` (Basistypen). Compiler: `compiler.rs:490+`; Evaluator: `evaluator.rs:261+`, `~708+`.

- Ausdrücke – teilweise/offen (~/✗):
  - If‑Ausdruck `if (..) then .. else ..`: Grammatik/AST vorhanden, Kompilierung/Eval fehlen (✗). Grammar: `xpath2.pest:41`, AST: `parser/ast.rs:42`; kein Compile‑Zweig in `compiler.rs`.
  - Quantifizierte Ausdrücke `some/every`: Grammatik/AST vorhanden, keine Kompilierung/Eval (✗). Grammar: `xpath2.pest:36`, AST: `parser/ast.rs:112`.
  - FLWOR‑Subset `for/let/return`: Grammatik/AST vorhanden, keine Kompilierung/Eval (✗). Grammar: `xpath2.pest:35`, AST: `parser/ast.rs:118`.

- Achsen/Semantik: Fertig.
  - Vollständige Achsen inkl. `namespace` (liefert nur bei Elementen Namespaces), `preceding/following` schließen Attribute/Namespaces aus. Evaluator: `evaluator.rs:969+`.
  - Knoten‑Tests (Kind/Name/Wildcards/PI‑target): Implementiert. Compiler/Evaluator: `compiler.rs:331+`, `evaluator.rs:1079+`; Parser‑Tests: `parser/kinds.rs:1`.

- Statischer/Dynamischer Kontext: Fertig (Basis).
  - StaticContext: Base‑URI, Default‑Function‑NS, Default‑Collation, Namespace‑Bindings. `runtime.rs:286+`.
  - DynamicContext: Context‑Item, Variablen, Default‑Collation, Function/Collation/Regex‑Provider, (M8) now/timezone. `runtime.rs:318+`.
  - `position()`/`last()` per OpCodes. Evaluator: `evaluator.rs:59+`.

- Fehlercodes (Auszug verwendet):
  - Syntax: `err:XPST0003` (Parser/Compile), Unbekannte Funktion/Typ: `err:XPST0017`, Unbekannte Variable: `err:XPST0008`.
  - Typ/Kardinalität: `err:XPTY0004`, `err:FORG0006`.
  - Arithmetik: `err:FOAR0001` (Division durch 0).
  - Collation/Regex: `err:FOCH0002`, `err:FORX0002`.
  - Evaluator/Allg.: `err:FOER0000` (z. B. Multi‑Root‑Vergleich/Doc‑Order unter Fallback).

## Dokument 2: XDM 1.0 (Datenmodell)

- Items/Sequenzen: Fertig.
  - `XdmItem<N> = Node(N) | Atomic(XdmAtomicValue)`; `XdmSequence<N> = Vec<XdmItem<N>>`. `xdm.rs:1`.

- Atomare Typen: Teilweise.
  - Implementiert: `xs:boolean`, `xs:string`, `xs:integer`, `xs:decimal` (als f64), `xs:double`, `xs:float`, `xs:anyURI`, `xs:QName`, `xs:untypedAtomic`. `xdm.rs:16+`.
  - Nicht implementiert: `xs:date|time|dateTime|durations`, weitere XSD‑Typhierarchie/Faces (✗). Plan M8/M9.

- UntypedAtomic/Atomisierung/Promotion: Fertig (Basis‑Regeln).
  - Node→Atomisierung zu `untypedAtomic(string_value())`. `evaluator.rs:563+`.
  - Numerik/EBV‑Konversionen behandeln `untypedAtomic`. `evaluator.rs:527+`, `~600+`.

- Node‑Modell/Ordnungen: Fertig (mit Fallback).
  - Knotenarten: `Document, Element, Attribute, Text, Comment, ProcessingInstruction, Namespace`. `model.rs:4`.
  - Identität: `Eq`/Adapter‑Gleichheit. Ordnung: `XdmNode::compare_document_order() -> Result<Ordering, Error>` mit Default‑Fallback `try_compare_by_ancestry` (Attribut‑ vor Namespace‑ vor Kindreihenfolge). `model.rs:22+`.
  - Multi‑Root: Default gibt Fehler; Evaluator propagiert. Tests: `adapter_ordering.rs:1`, `evaluator_multiroot_errors.rs:1`.

- QName/Namespaces: Fertig (Basismodelle).
  - QName/ExpandedName, Default‑Funktions‑NS, Prefix‑Auflösung im Compiler. `compiler.rs:317+`, `runtime.rs:286+`.
  - Default‑Element‑NS wirkt nicht auf Attribute (korrekt). Tests: `evaluator_namespaces.rs:1`.

- SequenceType/Typing: Teilweise.
  - `instance of`/`treat as` prüfen Kern‑Typen/Node‑KindTests; Occurrence‑Indikatoren umgesetzt. `evaluator.rs:736+`, `compiler.rs:520+`.
  - Kein `TypeRegistry`/Schema‑aware Typ‑System; `schema-element/attribute` in SequenceType nicht unterstützt (→ `err:XPST0017`). `compiler.rs:548+`.

- Base‑URI: Vorhanden, aber nicht genutzt.
  - Feld in `StaticContext` und `XdmNode::base_uri()` (Default None). Keine `fn:base-uri/resolve-uri`. `runtime.rs:288`, `model.rs:63`.

## Dokument 3: XQuery/XPath Functions and Operators (F&O)

- Booleans (✓): `true()`, `false()`, `not()` — `functions.rs:28+`; EBV konsistent (Sequenzlänge/Typen). `functions.rs:9+`.

- String (✓): `string`, `string-length`, `concat` (2–5 Args), `substring` (2/3), `substring-before/after`, `lower-case`, `upper-case`, `normalize-space`, `translate`, `string-join` — `functions.rs:46+`, `~140+`, `~215+`, `~253+`.
  - Hinweis: Randenfälle (Rundungsregeln/NaN) sind für `substring` vereinfacht (floor‑basiert). Umfangreiche Konformanz‑Fälle noch offen.

- Numerik (✓): `abs`, `floor`, `ceiling`, `round`, `sum`, `avg` — `functions.rs:268+`.

- Sequenzen (✓): `empty`, `exists`, `count`, `reverse`, `subsequence` (2/3), `distinct-values`, `index-of`, `insert-before`, `remove`, `min`, `max` (mit optionaler Collation) — `functions.rs:300+`.

- Collations (✓):
  - Registry mit Default Codepoint + einfachen Built‑ins: `simple-case`, `simple-accent`, `simple-case-accent`. `runtime.rs:129+`.
  - Collation‑sensitive Funktionen: `contains/starts-with/ends-with` (2/3‑Arg), `compare` (2/3), `codepoint-equal`, `deep-equal` — `functions.rs:88+`, `~424+`.
  - Auflösung Default‑Collation: dyn→static→codepoint. Evaluator `resolve_default_collation`: `evaluator.rs:1148+`.
  - Fehler bei unbekannter URI: `err:FOCH0002`. `functions.rs:660+`.
  - Hinweis: `deep-equal` für Nodes vergleicht aktuell String‑Werte, nicht Baumstruktur/ID (vereinfachte Semantik).

- Regex (✓): `matches` (2/3), `replace` (3/4), `tokenize` (2/3). `functions.rs:486+`.
  - Flags: `i, m, s, x` unterstützt; andere → `err:FORX0002`. `runtime.rs:182+`.
  - Provider austauschbar (Default: Rust `regex`). `runtime.rs:167+`.
  - XSD‑Regex‑Spezifika (z. B. \i/\c Klassen) nicht implementiert; dokumentierte Abweichungen im Plan vorgesehen.

- Date/Time (~/Teilmenge):
  - Implementiert: `current-dateTime`, `current-date`, `current-time` — Formatierung mit Offset, steuerbar via `with_now`/`with_timezone`. `functions.rs:540+`, Tests: `tests/functions_datetime.rs:1`.
  - Offen: Typen/Arithmetik/Parsing weiterer Date/Time/Duration‑Funktionen.

- Node/QName/Namespace (✗): `name`, `local-name`, `namespace-uri`, Node‑Navigations‑Funktionen etc. fehlen.

- Ressourcen/URI (✗): `doc`, `doc-available`, `collection`, `base-uri`, `resolve-uri` fehlen. Resolver‑Trait vorhanden: `runtime.rs:205+`.

## Runtimes/Services & API

- FunctionRegistry/CallCtx (✓): `runtime.rs:1`, `functions.rs:1`.
- CollationRegistry (✓): mit Built‑ins — `runtime.rs:129+`.
- RegexProvider (✓): `runtime.rs:167+`.
- ResourceResolver (Teilweise): Trait definiert; keine F&O‑Fns registriert — `runtime.rs:205+`.
- Öffentliche API (✓): `compile_xpath`, `XPathExecutable::evaluate`, `evaluate_on`, `evaluate_with_vars` — `lib.rs:1`, `evaluator.rs:318+`, `~336+`.
- DynamicContextBuilder (✓): inkl. `with_now`, `with_timezone` — `runtime.rs:346+`, `~388+`.
- Caching (✗): Kein `XPathExecutableCache`.
- TypeRegistry (✗): Nicht vorhanden; keine Delegation von Cast/Typing.

## Tests/Conformance

- Parser: Umfangreich, inklusive Fehlerfälle. `tests/parser/*.rs:1`.
- Evaluator E2E/Unit: Achsen/Pfade/Prädikate, Vergleiche, Mengen, Typen, Funktionen. `tests/evaluator_*.rs:1`, `tests/functions_*.rs:1`.
- Collations/Regex: Dedizierte Suiten. `tests/functions_collations.rs:1`, `tests/functions_regex.rs:1`.
- Date/Time: `current-*` deterministisch via Builder. `tests/functions_datetime.rs:1`.
- Multi‑Root Fehlerpfade: `tests/evaluator_multiroot_errors.rs:1`.
- Offene Conformance‑Matrix (F&O) und Performance‑Suiten (✗) — laut Plan M7–M9.

## Lücken und empfohlene Schritte

- Compiler/Evaluator für `if`, `some/every`, `for/let` ergänzen (AST/Grammatik vorhanden).
- F&O Date/Time/Duration komplettieren (XDM‑Typen, Arithmetik, Parsing, `implicit-timezone`).
- Node/QName/Namespace‑Funktionen hinzufügen.
- Ressourcen/URI‑Funktionen (`doc`, `doc-available`, `collection`, `base-uri`, `resolve-uri`) mit Resolver nutzen.
- TypeRegistry + Delegation von `cast/castable/treat/instance of` implementieren; optionale statische Typprüfung (`XPST0017`).
- `XPathExecutableCache` (LRU/HashMap) nach Plan aufbauen.
- Regex/XSD‑Kompatibilität (Klassen \i/\c, Properties) dokumentieren/testen; Collation Edge‑Cases.
- Konsolidierung Fehlercodes/Diagnostik; Conformance-/Performance‑Matrix aufbauen.

