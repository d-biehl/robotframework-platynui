# XPath 2.0 Evaluator – Architektur- und Umsetzungsplan

Status: Draft 1
Autor: PlatynUI Team
Scope: `crates/platynui-xpath`

## Ziele
- Vollständige XPath 2.0-Unterstützung (gemäß W3C Spezifikationen: XPath 2.0, XDM, F&O/XQuery Operators)
- Evaluator arbeitet auf beliebigen dynamischen Baumstrukturen (kein XML-Parsing), modelliert nach XDM
- Saubere Trennung: Parser → Compiler → Evaluator; vorkompilierbare, cachebare Expressions
- Erweiterbares Funktions-Ökosystem (keine festverdrahteten Funktionen), Namespace-fähig und mit Kollationen
- Stabiles, deterministisches Verhalten, umfassende Tests (Parser-Tests bleiben gültig, werden falls nötig angepasst)

Nichtziele (nur für Klarheit, spätere Erweiterbarkeit sicherstellen):
- Kein eigener XML-Schema-Validator. Built-in XSD-Typen unterstützen; Erweiterung für Schema-Import designen (später).

## Spezifikationen & Konformität
- XPath 2.0: https://www.w3.org/TR/xpath20/
  - MUSS vollständig implementiert werden (Syntax, Semantik, Achsen, Prädikate, Operatoren, Fehlerbedingungen/Fehlercodes).
- XDM 1.0 (XPath 2.0 Data Model), inklusive normativer Änderungen:
  - Diff/Errata-Dokument: https://www.w3.org/TR/2010/REC-xpath-datamodel-20101214/xpath-datamodel-diff-from-REC20070123.html
  - MUSS vollständig umgesetzt werden (Item-/Sequenzmodell, Knotentypen, Dokumentreihenfolge/Identität, Basetypen, `untypedAtomic`).
- XQuery and XPath Functions and Operators (F&O): https://www.w3.org/TR/xquery-operators/
  - MUSS vollständig implementiert werden (alle im XPath‑2.0‑Umfang relevanten Funktionen/Operatoren inkl. Collations, Regex, Numerik, Datum/Zeit, Edge‑Cases und Fehlerspezifikationen).

Hinweise:
- Keine Teilimplementierungen; Ziel ist vollständige Konformität mit den o. g. Spezifikationen einschließlich normativer Errata.
- Testfälle sollen, wo möglich, auf konkrete Spezifikationsabschnitte verweisen (Nachweis der Abdeckung/Kompatibilität).

## Architektur-Übersicht
- Parser (bestehend): Pest-Grammatik `xpath2.pest` → AST
- Compiler: AST → IR (zwischensprachliche Repräsentation/Bytecode) + statische Analyse (Typen, Bindungen, Namensauflösung)
- Evaluator: IR-Interpreter auf der XDM-Laufzeit (Items, Sequenzen, Knotenmodell) mit dynamischem Kontext
- Runtimes & Services: Funktions-Registry, Kollations-Registry, Namespace-/QName-Resolver, Datentypen & Casting, Fehlercodes
- Caching: Kompilierte Executables per (Expression + statischer Kontext-Hash) zwischenspeichern

### Benennung (öffentliche Typen)
- Kompiliertes XPath: `XPathExecutable`
- IR-Sequenz: `InstrSeq` (oder `Bytecode`), einzelne Operation: `OpCode`
- Parser-Ergebnis: `Ast` bzw. `XPathAst`
- Kontexte: `StaticContext`, `DynamicContext`

### Module/Crate-Aufteilung
- `crates/platynui-xpath`
  - `parser/`: Parser + vollständiger AST (Test-AST für Precedence-Tests bleibt als Test-Hilfsmodul verfügbar)
  - `compiler/`: AST → IR, statische Analyse, Capturing von statischem Kontext
  - `evaluator/`: IR-VM/Interpreter, dynamischer Kontext
  - `functions/`: Standard-Funktionsbibliothek (F&O/XQuery Operators), registriert via Registry
  - `runtime/`: Funktions-Registry, Kollations-Registry, Namespace-Resolver, Caches
  - `xdm/`: XDM-Datentypen, Werte, Fehlercodes, QName/SequenceType, Collation/Regex-Hilfen
  - `model/`: Datenmodell-Trait (siehe unten) für beliebige Baum-/Graphstrukturen
  - `tests/`: Parser bleibt; neue Evaluator-/Compiler-Tests nach Featuregruppen

## XDM-Modellierung
- Items (generisch): `XdmItem<N> = Node(N) | Atomic(XdmAtomicValue)`
- Sequenzen: `XdmSequence<N> = Vec<XdmItem<N>>` (stabile Reihenfolge; Duplikate erlaubt)
- Atomare Typen (Subset zunächst vollständig für XPath 2.0):
  - Numerik: `xs:integer`, `xs:decimal`, `xs:double`, `xs:float` (Typ-Promotion-Regeln!), `xs:nonPositiveInteger` etc. via Facets strukturierbar, aber funktional wie Basistypen
  - `xs:string`, `xs:boolean`, `xs:anyURI`, `xs:QName`, `xs:NCName`
  - Datum/Zeit/Dauer: `xs:date`, `xs:time`, `xs:dateTime`, `xs:dayTimeDuration`, `xs:yearMonthDuration`
  - `xs:untypedAtomic` (wichtige Casting-/Vergleichsregeln)
- Knoten (XDM-Node-Arten): `document`, `element`, `attribute`, `text`, `comment`, `processing-instruction`, `namespace`
  - Benötigte Metadaten: Node-Kind, lokale Namen, Namespace-URI, String-Value, Typ (Typed/Untyped), Dokumentreihenfolge
  - Achsen-Unterstützung: `parent`, `child`, `attribute`, `self`, `descendant`, `descendant-or-self`, `ancestor`, `ancestor-or-self`, `following`, `following-sibling`, `preceding`, `preceding-sibling`, `namespace`

### XdmNode-Trait für beliebige Strukturen
- Ziel: Ein einziges, schlankes Trait, das direkt auf eine vorhandene Struct implementiert werden kann. Keine Abhängigkeit auf externe Crates.
- Attribute sind reguläre Knoten des Typs `Attribute` (keine separaten Typen nötig).
- Identität: `Eq` muss die Node-Identität (nicht Strukturgleichheit) widerspiegeln.
- Dokumentreihenfolge: Implementierer liefern eine Vergleichsfunktion für `<<`/`>>`.

Vorschlag (minimal):

```rust
use std::cmp::Ordering;

pub enum NodeKind { Document, Element, Attribute, Text, Comment, ProcessingInstruction, Namespace }

pub struct QName { pub prefix: Option<String>, pub local: String, pub ns_uri: Option<String> }

pub trait XdmNode: Clone + Eq + std::fmt::Debug + Send + Sync {
    // Basis
    fn kind(&self) -> NodeKind;
    fn name(&self) -> Option<QName>;           // für Element/Attribute/PI/Namespace
    fn string_value(&self) -> String;          // string-value
    fn base_uri(&self) -> Option<String> { None }

    // Navigation
    fn parent(&self) -> Option<Self>;
    fn children(&self) -> Vec<Self>;
    fn attributes(&self) -> Vec<Self>;         // Attribute als Knoten vom Kind Attribute
    fn namespaces(&self) -> Vec<Self> { Vec::new() } // optional, default leer

    // Ordnung/Identität
    // Eq definiert Identität (für 'is').
    fn compare_document_order(&self, other: &Self) -> Ordering;
}
```

Hinweis: Die Rückgaben als `Vec<Self>` halten das Trait einfach. Später kann optional eine Iterator-Variante als separates Trait ergänzt werden, ohne das bestehende zu brechen.

### Achsen- und Pfadsemantik (XDM-konform)
- Achsenordnung: Forward-Achsen (child, descendant, self, following-sibling, following, attribute, namespace) in Dokumentreihenfolge; Reverse-Achsen (parent, ancestor, ancestor-or-self, preceding-sibling, preceding) in umgekehrter Dokumentreihenfolge liefern.
- Pfadketten: Ergebnisse eines Schritts deduplizieren und in Dokumentreihenfolge sortieren, bevor der nächste Schritt angewandt wird.
- Kontextwechsel: Jeder Schritt setzt Kontext-Item/Position/Größe neu; Prädikate wirken im Schritt-Kontext und beeinflussen Position/Last.

## Statischer und dynamischer Kontext
- Statischer Kontext (wird im Compile-Schritt fixiert und mit dem Executable verknüpft):
  - Namespace-Bindungen (Präfix→URI), Standard-Funktionsnamespace, Basiskollation, Base-URI, Verfügbarkeit von Schema-Typen
  - Verfügbare Variablen (Signaturen), Funktionssignaturen (Name, Arity, Typen)
- Dynamischer Kontext (Laufzeit):
  - Kontext-Item (generisch über Node-Typ), Position, Größe; Variablenwerte; aktuelle Kollation; aktuelle Datum/Zeit/Timezone
  - Funktionsimplementierungen (aus Registry)
  - Generisch über Node-Typ: `DynamicContext<N: XdmNode>`

## Parser → AST
- Parser bleibt Pest-basiert; extrahiert vollständige AST-Struktur (nicht nur Precedence-Testmodell):
  - Literale, Variablenreferenzen, Funktionsaufrufe (QName + Arity), Pfadausdrücke/Schritte (Axis, Node-Test, Prädikate), Operatoren (arithmetisch, Vergleich, logische, Mengen, Sequenzen), bedingte/quantifizierte Ausdrücke, `for`/`let`, Cast/Castable, Treat, Instance of, Type-Constructor (`xs:integer("…")`)
- Aktuelles vereinfachtes Test-AST in `tests` behalten; interne AST-Typen nach `parser/ast.rs` auslagern
- Namensauflösung erst im Compiler (Parser speichert QNames tokenisiert: Präfix, Local, ggf. URI placeholder)

## Operatoren & Ausdrücke (Vollständigkeit)
- Node-Vergleiche: `is`, `<<`, `>>` als eigene AST/IR-Operatoren; benötigen Totalordnung (TreeId + preorder_index) und stabile Identität.
- Bereichsoperator: `to` ergibt `xs:integer`-Sequenz; eigener IR-Opcode `RangeTo`.
- Ganzzahldivision: `idiv` mit definierter Rundung und Fehlerfällen (Division durch 0) gemäß F&O.
- Quantifizierte Ausdrücke: `some`/`every` mit Kurzschluss im Evaluator; Variablenbindung im dynamischen Kontext.
- FLWOR-Teilmengen: `for`/`let`-Binding, Scopes/Shadowing, deterministische Auswertungsreihenfolge.
- Mengenoperatoren: `union`, `intersect`, `except` nur für Node-Sequenzen; Ergebnisse in Dokumentreihenfolge und duplikatfrei.

## Typen, Atomisierung, Vergleiche
- SequenceType-Modell: `SequenceType = ItemType × OccurrenceIndicator (?, +, *, leer)`; unterstützt `treat as`, `instance of`, `cast`, `castable as`.
- Atomisierung: `data()`-Semantik für Knoten, spezielle Regeln für `xs:untypedAtomic` (implizite Casts in Vergleichen/Funktionen).
- EBV (Effective Boolean Value): leere Sequenz → false; ein bool → dessen Wert; numerisch/string/untypedAtomic → true, wenn nicht 0/NaN bzw. nicht leer; Nodes → true; >1 atomare Werte → Fehler.
- Vergleiche: Value vs. General comparisons – paarweise über Sequenzen, Atomisierung, Typ-Promotion (decimal/double/float), Collation für Strings; Fehlerfälle für inkompatible Typen/Arity.
- Default-Namespaces: Default Function Namespace, Default Element/Type Namespace; Attribute sind nie im Default-Element-NS.

## Namespaces, URIs & Ressourcen
- Base-URI: Statischer Base-URI im StaticContext und node-spezifischer Base-URI; Funktionen `base-uri()`, `resolve-uri()`.
- Ressourcen-Resolver: `fn:doc`, `doc-available`, `collection` via pluggable Resolver im Runtime-Layer; definierte Fehlercodes bei unbekannter Ressource/URI.

## Regex & Kollationen
- Regex: XSD-Regex-Kompatibilität (Unicode-Eigenschaften, Flags). Ggf. Übersetzer-Layer zur Rust-Regex-Engine; Fehlercodes für ungültige Patterns.
- Collations: Trait-basierte API (compare/key), Default Codepoint Collation; Registry per URI; Fehler bei unbekannter Collation.

## Compiler (AST → IR)
IR-OpCodes (Auszug):
- Daten/Variablen/Kontext: `PushConst`, `LoadVar/StoreVar`, `LoadContextItem`, `Position`, `Last`.
- Schritte/Filter: `AxisStep(axis, nodetest)`, `FilterStart/FilterEnd` oder `Filter(pred)`.
- Arithmetik/Logik: `Add/Sub/Mul/Div/IDiv/Mod`, `And/Or/Not`.
- Vergleiche: `Compare(Value|General, op)`; Node-Vergleiche: `NodeIs/NodeBefore/NodeAfter`.
- Sequenzen/Mengen: `MakeSeq(n)`, `ConcatSeq`, `Union/Intersect/Except`, Bereich: `RangeTo`.
- Kontrollfluss/Bindungen: `IfElse`, `Some/Every`, `ForStart/ForNext/ForEnd`, `LetBind`.
- Typen: `Cast(to)`, `Castable(to)`, `Treat(as)`, `InstanceOf(type)`.
- Funktionen: `Call(fn_id, argc)` mit Auflösung in der Registry (QName+Arity).
- Ziele: schnelle Ausführung, Caching, statische Überprüfung, Name/Variablenbindung, Typ-Promotion/Fehler früh erkennen
- IR-Design (stackbasierte VM, Beispiele):
  - Konstante/Variablen: `PushConst`, `LoadVar(slot)`, `StoreVar(slot)`
  - Kontext: `LoadContextItem`, `Position`, `Last`
  - Pfad/Schritte: `AxisStep(axis, nodetest)`, `PredicateStart`, `PredicateEnd` (oder als Filter-Opcode)
  - Operatoren: `Add/Sub/Mul/Div/IDiv/Mod`, `And/Or`, `Compare(=,!=,<,<=,>,>=; value/general/node)` mit Regeln aus F&O
  - Mengen/Sequenzen: `MakeSeq(n)`, `Union/Intersect/Except`, `ConcatSeq`
  - Kontrollfluss: `IfElse`, `ForStart/ForNext/ForEnd`, `Some/Every` mit Kurzschluss
  - Typen: `Cast(to)`, `Castable(to)`, `Treat(as)`, `InstanceOf(type)`
  - Funktionen: `Call(fn_id, argc)`; Compiler löst `fn_id` via Registry + Arity auf
- Statische Analyse:
  - Variablenbindung auf Slots; Sichtbarkeit/Schattierung
  - QName-Resolution via statischem Kontext; Fehler bei unbekannten Prefixen/Funktionen
  - Typinferenz (rudimentär zunächst), Promotion/Atomization-Regeln vorzubereiten
  - Prädikate: Effektive Boolesche Werte (EBV), Kontext-Iterationen
  - Optimierungen: Konstantfaltung, Dead-Code-Elimination (einfach), Predicate-Reordering (optional), Inline von einfachen Funktionen (optional)

## Evaluator (IR-Interpreter)
- Laufzeit-Stack für Items/Sequenzen, Call-Frames für Funktionsaufrufe/Bindings
- Iterator-basierte Ausführung für Pfade/Schritte (Streaming-freundlich)
- EBV-Regeln: booleans, numerics, strings, nodes, leere Sequenzen
- Vergleiche: general vs value comparisons, Node-Vergleich über Dokumentreihenfolge/ID
- Knotenachsen: Implementierung auf Basis des Node-Adapters; Prädikate beeinflussen Kontext (Position/Last)
- Datentypen/Casting: gemäß F&O, inkl. `untypedAtomic`-Semantik, Numerik-Promotion (decimal/double/float), Zeichenfolgen-Kollation (default codepoint)
- Fehlerbehandlung: Fehlercodes (QName `err:*`), statisch/dynamisch differenziert

## Funktionen & Operatoren (F&O/XQuery Operators)
- Funktions-Registry:
  - Schlüssel: Expanded QName + Arity → `FunctionImpl` mit Signatur (Param/Return XDM-Typen), Implementierung (Closure/Funktionszeiger)
  - Mehrere Overloads je QName (Typen/Arity)
  - Erweiterbarkeit: Benutzerdefinierte Funktionen registrierbar (auch zur Laufzeit)
- Standard-Funktionsfamilien (vollständig abzudecken):
  - Numerik: arithmetisch, rounding, abs, floor/ceiling, `idiv`
  - Boolean/Logik: `not`, `true`, `false`
  - String/Regex/Collation: `string`, `concat`, `contains`, `starts-with`, `ends-with`, `substring`, `string-length`, `normalize-space`, `translate`, `matches`, `replace`, `tokenize`
  - Sequenzen: `empty`, `exists`, `distinct-values`, `index-of`, `insert-before`, `remove`, `reverse`, `subsequence`
  - Node/Tree: `name`, `local-name`, `namespace-uri`, `string()`, `root`, `doc-order`-Operationen
  - Datums/Zeitfunktionen: `current-date`, `current-time`, `current-dateTime`, Konstruktoren/Berechnungen für Duration
  - Aggregation: `count`, `sum`, `avg`, `min`, `max` (mit Collation/Typregeln)
  - Typen: `xs:type(...)`-Konstruktoren via `Cast`
- Kollationen:
  - Default: Codepoint Collation
  - Erweiterbar via Trait, Registry per URI

## Namespaces & QNames
- Statischer Kontext: Präfix→URI Bindungen; Default-Funktionsnamespace
- `QName`-Utility (xdm): Auflösung, Vergleich, Serialisierung
- Node-Tests: `NameTest` (QName), `KindTest` (node(), text(), comment(), processing-instruction()), `Wildcard` (prefix:*, *:local, *)

## Caching & API
- Öffentliche API (Rust):
  - `parse_xpath(&str) -> Ast` (bestehend, neue AST-Struktur)
  - `compile_xpath(expr: &str, static_ctx: &StaticContext) -> XPathExecutable`
  - `XPathExecutable::evaluate::<N: XdmNode>(&self, dyn_ctx: &DynamicContext<N>) -> Result<XdmSequence<N>>`
- Cache: `XPathExecutableCache` (HashMap/LRU) keyed by `expr + static_ctx_hash`

### Ergonomie: Builder & Convenience-Methoden
- Ziel: Einfache Nutzung ohne manuelles Befüllen des gesamten `DynamicContext`.
- `DynamicContextBuilder<N: XdmNode>` (fluent API):
  - `new()`, `with_context_item(item: impl Into<XdmItem<N>>)`
  - `with_variable(name: ExpandedName, value: impl Into<XdmSequence<N>>)`
  - `with_now(dt: DateTime<FixedOffset>)`, `with_timezone(tz: FixedOffset)`
  - `with_default_collation(uri: impl Into<Uri>)`
  - `with_functions(reg: Arc<FunctionRegistry<N>>)`
  - `with_collations(reg: Arc<CollationRegistry>)`
  - `with_resolver(res: Arc<ResourceResolver>)`
  - `with_regex(provider: Arc<RegexProvider>)`
  - `build() -> DynamicContext<N>`

- Convenience an `XPathExecutable`:
  - `evaluate_on<N: XdmNode>(&self, context_item: impl Into<Option<N>>) -> Result<XdmSequence<N>>`
  - `evaluate_with_vars<N: XdmNode>(&self, context_item: impl Into<Option<N>>, vars: impl IntoIterator<Item=(ExpandedName, XdmSequence<N>)>) -> Result<XdmSequence<N>>`
  - Intern wird jeweils der `DynamicContextBuilder` verwendet.

- Beispiel (kurz):
```rust
// compile
let exec = compile_xpath("//book[@price < 10]", &StaticContext::default())?;

// evaluate with only a context item
let result = exec.evaluate_on(Some(root_node))?; // root_node: impl XdmNode

// evaluate with variables via builder
let ctx = DynamicContextBuilder::new()
    .with_context_item(root_node)
    .with_variable(expanded!("", "threshold"), vec![XdmItem::Atomic(xs::integer(10))])
    .build();
let result = exec.evaluate(&ctx)?;
```

## Migrationsplan (Parser-Tests erhalten)
1) AST aus `parser.rs` extrahieren und vollständige AST-Strukturen ergänzen; Tests, die das vereinfachte AST prüfen, auf Hilfs-API umleiten
2) Parser so anpassen, dass er den vollständigen AST erzeugt; bestehende Parser-Tests unverändert grün halten (nur Precedence-Strukturtests nutzen das Test-AST)
3) Static/Dynamic Context-Typen hinzufügen (zunächst minimal)
4) IR entwerfen und minimalen Compiler bauen (Literals, Variablen, einfache arithmetische/logische Ops, Funktionsaufruf-Stub)
5) Evaluator-VM implementieren (Stack/Frames, EBV, Sequenzen); Smoke-Tests
6) Achsen/Schritte implementieren (child/attribute/self, dann alle übrigen) über `XdmNode`
7) Vergleichsoperatoren (value/general), Mengenoperatoren, Sequenzen
8) Prädikate, Pfad-Pipeline (Kontextposition/-größe korrekt)
9) Cast/Castable/Treat/Instance-of und Typ-Promotion
10) Funktionen sukzessive vollständig; Collations; Datum/Zeit/Duration korrekt
11) Fehlercodes/Edge-Cases; Performanceprofiling; Caching

## Teststrategie (rstest)
- Parser (bestehend): weiterführen; ggf. Anpassungen der AST-Hilfen
- Compiler-Unit-Tests: AST→IR für gezielte Snippets (Operatorpräzedenz, Bindung, QNames)
- Evaluator-Unit-Tests: IR-Ausführung für jede Operator-/Funktionengruppe
- End-to-End-Tests: Ausdruck → Ergebnis auf einer Mock-Baumstruktur (Elemente/Attribute/Text)
- Achsentests: für alle XPath-Achsen; Namens-/Namespace-Tests; Prädikate mit Kontext
- Typen/Casting: umfangreiche Positiv/Negativ-Cases inkl. `untypedAtomic`
- Funktionen: pro Familie (string, numeric, sequence, node, regex, date/time)
- Property-Tests (optional): z. B. `reverse(reverse(S)) == S`, `count(insert-before(S,i,x)) == count(S)+1`
- Performance-/Speichertests: große Sequenzen, tiefe Pfade, viele Prädikate

## Konformitäts-Matrix (erste Iteration)
Ziel: Jede relevante Spezifikationsfläche ist vollständig abgedeckt (Implementierung + Tests). Status-Felder dienen der Nachverfolgung während der Entwicklung.

- Spez: XPath 2.0 – Ausdrücke, Schritte, Operatoren
  - Implementierung: Parser/Compiler/Evaluator (Pfadausdrücke, Schritte & Achsen, Prädikate, Sequenzkonstruktoren, Arithmetik/Logik, Wert‑ vs. Allgemein‑Vergleiche, Knotenvergleiche `is`/`<<`/`>>`, quantifizierte Ausdrücke `some/every`, `for/let` (FLWOR‑Subset), `if‑then‑else`, Bereichsoperator `to`, `idiv`).
  - Tests: `crates/platynui-xpath/tests/{parser_*.rs, compiler_*.rs, evaluator_{path,axes,predicates,operators}.rs, e2e_xpath_*.rs}`.
  - Status: [geplant]

- Spez: XDM (XPath Data Model) – Items/Sequenzen, Knotentypen, Ordnung
  - Implementierung: `xdm`-Typen (Item/Sequence/Atomic), `XdmNode`-Trait, Dokumentreihenfolge/Identität, Atomisierung, EBV‑Regeln, `xs:untypedAtomic`, Zahl‑Promotion.
  - Tests: `crates/platynui-xpath/tests/{xdm_model.rs, xdm_ordering.rs, ebv_rules.rs, promotion_untyped.rs}`.
  - Status: [geplant]

- Spez: XQuery and XPath Functions and Operators (F&O)
  - Implementierung: Standardfunktionsbibliothek (string/regex/collation, numeric, sequence, node, QName/Namespace, date/time/duration, aggregate), Operator‑Semantik, Fehlerfälle.
  - Tests: `crates/platynui-xpath/tests/functions_{string,regex,numeric,sequence,node,qname,datetime,aggregate}.rs`.
  - Status: [geplant]

- Spez: Kollationen & Regex (F&O)
  - Implementierung: Default Codepoint Collation, Registry/URI‑basierte Collations, Regex‑Kompatibilität (XSD‑Semantik, Flags), Fehlercodes bei ungültigen Patterns/Collations.
  - Tests: `crates/platynui-xpath/tests/{collations.rs, regex_compat.rs}`.
  - Status: [geplant]

- Spez: Namespaces/QNames
  - Implementierung: Statischer Kontext (Präfixbindungen, Default‑Funktionsnamespace), QName‑Auflösung, Node‑Tests (Name/Kind/Wildcards), Attribut‑NS‑Regeln.
  - Tests: `crates/platynui-xpath/tests/{namespaces.rs, qname_resolution.rs, nodetest_wildcards.rs}`.
  - Status: [geplant]

- Spez: Ressourcen/Resolver (F&O: `doc`, `doc-available`, `collection`)
  - Implementierung: Resolver‑Trait + Fehlerfälle, Base‑URI‑Verhalten.
  - Tests: `crates/platynui-xpath/tests/{resolver_doc.rs, resolver_collection.rs}`.
  - Status: [geplant]

- Spez: Fehlercodes und Diagnostik (err:*)
  - Implementierung: Einheitlicher Error‑Typ mit Spez‑konformen Codes; statisch vs. dynamisch.
  - Tests: `crates/platynui-xpath/tests/errors_{static,dynamic}.rs` mit gezielten Negativfällen.
  - Status: [geplant]

## Fehler- und Diagnostik-Design
- Einheitlicher `Error`-Typ mit Codes `err:*` (Spezifikationskonform), Quelle (statisch/dynamisch), Positionsinfos
- Gute Debug-/Trace-Optionen: IR-Dump, Ausführungs-Trace (optional feature)

## Runtime, Thread-Safety & Caching
- Thread-Safety: `XPathExecutable: Send + Sync`. Funktions-/Collation-Registry thread-safe; `DynamicContext` nicht zwingend `Sync`.
- Dokumentreihenfolge/Identität über Multi-Root: `TreeId + preorder_index` Totalordnung; stabil über Lebensdauer der Bäume.
- Cache: LRU mit Kapazität, Key = `expr + static_ctx_hash`; Invalidierung bei Änderung der Registry/StaticContext; Hash-Kollisionsschutz.

## Roadmap & Meilensteine
- M1: AST-Refactor + Static/Dynamic Context Draft + IR-Skizze
- M2: Minimal-Compiler + Minimal-VM (arithmetik, bool, function-call stub) + Sequenzen
- M3: Achsen (child/attribute/self), Prädikate, Pfade, EBV
- M4: Vergleiche, Mengenoperatoren, restliche Achsen, Node-Vergleich
- M5: Typen/Casting/Treat/Instance-of + Funktionsfamilien (string/numeric/sequence)
- M6: Regex/Collations, Datum/Zeit/Duration, Aggregatfunktionen
- M7: Vollständigkeit, Fehlercodes, Caching, Performance, Doku

## Offene Designpunkte (bewusst festgehalten)
- Vollständige XML Schema 1.0-Facets vs. funktionale Basistypen: Implementieren wir zunächst funktionsäquivalente Basistypen, strukturierte Facets optional.
- Dokumentreihenfolge über generische Bäume: globaler, stabiler Ordnungs-Indexer pro Baumquelle; Adapter definiert Totalordnung.
- Regex-Implementierung: Rust `regex` reicht für XPath 2.0? Notwendig: Flags, Unicode-Eigenschaften, `fn:matches/replace/tokenize`-Semantik – sorgfältig abgleichen.
- Collations: Start mit Codepoint-Collation; Erweiterungspunkt für ICU-basiert (optional Feature).

---

## TODO-Liste (umsetzungsorientiert)

### Parser/AST
- [ ] Neues Modul `parser/ast.rs` mit vollständigen AST-Knoten (XPath 2.0)
- [ ] Bestehende vereinfachte AST-Hilfsfunktionen in Test-Utils verschieben/duplizieren
- [ ] Parser-Regeln anpassen, damit vollständiger AST entsteht (QNames tokenisieren)
- [ ] Parser-Tests weiter grün; ggf. Anpassungen an AST-Extraktion in Tests

### Grammatik-/Parser-Checkliste
- [ ] Operatoren/Tokens: `idiv`, `is`, `<<`, `>>`, `to`, `treat as`, `instance of`, `cast`, `castable as`
- [ ] Knoten-Tests: `processing-instruction('target')`, `node()`, `text()`, `comment()`
- [ ] Wildcards: `*`, `ns:*`, `*:local`, `@*`

### Compiler/IR
- [ ] IR-Design dokumentieren und Types/OpCodes implementieren
- [ ] Statischer Kontext: Namespaces, Funktionen, Variablen, Base-URI, Kollation
- [ ] Binder: Variablen/Slots, Funktionsauflösung (QName+Arity)
- [ ] Optimierungen (Konstantfaltung, einfache DCE)
 - [ ] Node-Compare-OpCodes (`is`, `<<`, `>>`), `RangeTo`, `IDiv`, `Some/Every`, `For/Let`

### Evaluator
- [ ] VM/Interpreter (Stack, Frames, Sequenzen, EBV)
- [ ] Pfad-Auswertung mit Prädikaten, Kontextposition/-größe
- [ ] Vergleiche (value/general), Mengen, Sequenz-Operatoren
- [ ] Casting/Promotion/`untypedAtomic`-Handling, Treat/Instance-of
- [ ] Achsen-Ordnung & Deduplizierung, Node-Gesamtordnung (Multi-Root)
- [ ] Quantifizierte Ausdrücke, FLWOR-Bindungen
 - [ ] Knotenmodell: `XdmNode`-Integration, Beispiel-Adapter + Tests

### Funktionen & Kollationen
- [ ] Funktions-Registry (erweiterbar), Signaturen, Overloads
- [ ] Standardfunktionen nach Familien implementieren (string, numeric, sequence, node, regex, date/time, aggregate)
- [ ] Collation-Registry; Default-Codepoint; Erweiterungspunkt
 - [ ] Ressourcen-Resolver: `doc`, `doc-available`, `collection`
 - [ ] QName/Namespace-Funktionen: `resolve-QName`, `QName`, `namespace-uri-for-prefix`
 - [ ] Gleichheit/Ähnlichkeit: `compare`, `codepoint-equal`, `deep-equal`, `distinct-values`
 - [ ] Base-URI/URI: `base-uri`, `resolve-uri`

### API & Caching
- [ ] Öffentliche API: `compile_xpath`, `evaluate`
- [ ] `XPathExecutableCache` (LRU/HashMap), Hash über `expr + static_ctx`
- [ ] Doku/Beispiele
 - [ ] DynamicContextBuilder und Convenience-Methoden (`evaluate_on`, `evaluate_with_vars`)

### Tests
- [ ] Unit-Tests je Modul (rstest)
- [ ] End-to-End-Tests auf Mock-Baum
- [ ] Umfangreiche F&O-Conformance-Cases
- [ ] Performance-/Speicher-Tests
 - [ ] Achsen/Pfade: Reihenfolge & Deduplizierung; Pfadketten
 - [ ] Node-Vergleiche: `is`, `<<`, `>>`; Bereich `to`; `idiv`
 - [ ] EBV/Prädikate: numerisch vs. boolsch; Fehler bei >1 atomaren Items
 - [ ] Vergleiche/Promotion/Collation/`untypedAtomic`-Fälle
 - [ ] Mengenoperatoren nur für Nodes (doc order + dedup)
 - [ ] Regex/Date/Time/Timezone Grenzfälle
 - [ ] Ressourcen/Resolver Erfolg/Fehler

### Qualität & Tooling
- [ ] Fehlercodes/Diagnostik konsolidieren

---

## Nächste konkrete Schritte (M1)
1) AST in eigenes Modul extrahieren und vervollständigen (ohne Logik-Änderungen an Tests)
2) Static/Dynamic Context-Typen definieren (stubs) und öffentliche API-Signaturen für `compile_xpath`/`evaluate` entwerfen
3) IR-Entwurf (OpCodes) mit serieller Ausgabelogik (Debug-Dump) dokumentieren
