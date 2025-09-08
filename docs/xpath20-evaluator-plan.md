# XPath 2.0 Evaluator – Architektur- und Umsetzungsplan

Status: Draft 3 — M1–M5 abgeschlossen, M6 in Arbeit
Autor: PlatynUI Team
Scope: `crates/platynui-xpath`

## Ziele
- Vollständige XPath 2.0-Unterstützung als Endziel (gemäß W3C Spezifikationen: XPath 2.0, XDM, F&O/XQuery Operators). Roadmap bildet die schrittweise Fertigstellung ab; noch offene Teilmengen sind in den Meilensteinen (z. B. FLWOR/Quantifizierer) benannt.
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
- Compiler: AST → IR (zwischensprachliche Repräsentation) + statische Analyse (Typen, Bindungen, Namensauflösung)
- Evaluator: IR-Interpreter auf der XDM-Laufzeit (Items, Sequenzen, Knotenmodell) mit dynamischem Kontext
- Runtimes & Services: Funktions-Registry, Kollations-Registry, Namespace-/QName-Resolver, Datentypen & Casting, Fehlercodes
- Caching: Kompilierte Executables per (Expression + statischer Kontext-Hash) zwischenspeichern

### Benennung (öffentliche Typen)
- Kompiliertes XPath: `XPathExecutable`
- IR-Sequenz: `InstrSeq`, einzelne Operation: `OpCode` (Terminologie fixiert; „Bytecode“ wird nicht weiter verwendet)
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

Hinweis (Umsetzungsstand):
- Parser-AST, Compiler-IR und Evaluator-VM sind implementiert und durch umfangreiche rstest-Suiten abgedeckt.
- Pfade, Achsen (vollständig), Prädikate mit Kontext (Position/Last), Vergleichsoperatoren (value/general), Mengen (union/intersect/except), Bereich `to`, Node-Vergleiche (`is`, `<<`, `>>`) sind implementiert.
- Typen: `cast`, `castable as`, `treat as`, `instance of` sind implementiert. `untypedAtomic`-Semantik (Atomisierung/Promotion) ist abgedeckt.
- Funktionsfamilien (erste Welle): boolean, string, numeric, sequence inkl. `sum/avg/min/max`, `string-join`, `normalize-space`, `translate` sind verfügbar.
- Collations: Codepoint-Collation ist als Default registriert und wird bei Vergleichen berücksichtigt. Regex/Date/Time/Duration folgen gemäß Roadmap (M6–M8).

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

    // Hinweis: Ab M6 erhält dieses Trait eine Default-Implementierung für
    // `compare_document_order`, die einen korrekten Fallback `compare_by_ancestry`
    // nutzt (korrekt innerhalb eines Baumes). Adapter in Multi-Root-Szenarien
    // MÜSSEN die Methode überschreiben und eine globale Ordnung herstellen.
}
```

Hinweis: Die Rückgaben als `Vec<Self>` halten das Trait einfach. Später kann optional eine Iterator-Variante als separates Trait ergänzt werden, ohne das bestehende zu brechen.

### Achsen- und Pfadsemantik (XDM-konform)
- Achsenordnung: Forward-Achsen (child, descendant, self, following-sibling, following, attribute, namespace) in Dokumentreihenfolge; Reverse-Achsen (parent, ancestor, ancestor-or-self, preceding-sibling, preceding) in umgekehrter Dokumentreihenfolge liefern.
- Pfadketten: Ergebnisse eines Schritts deduplizieren und in Dokumentreihenfolge sortieren, bevor der nächste Schritt angewandt wird. Details zur Definition der Dokumentreihenfolge s. Abschnitt „Dokumentreihenfolge & Identität (Adapter-Guidelines)“.
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
-- Regex: Umsetzung über einen Runtime-Provider (Default: Rust-`regex`) mit Ausrichtung auf XML-Schema-Regex-Semantik, wie von F&O gefordert. Mapping der F&O/XSD-Regex-Features/Flags auf die Engine wird dokumentiert; Abweichungen werden explizit markiert. Fehlercodes für ungültige Patterns gemäß `FORX0002`.
-- Collations: Trait-basierte API (compare/key), Default Codepoint Collation; Registry per URI; Fehler bei unbekannter Collation (`FOCH0002`).

Regex‑Kompatibilität (Hinweise):
- Flags: mindestens `i` (case‑insensitive), `m` (multiline), `s` (dotall), `x` (free‑spacing). Nicht unterstützte/unerlaubte Flags → `FORX0002`.
- Unicode‑Eigenschaften: `\\p{..}`/`\\P{..}` werden gemäß Engine‑Support gemappt; unbekannte Properties → `FORX0002`.
- XSD‑spezifische Klassen (z. B. `\\i`, `\\c`) werden initial nicht unterstützt; entweder Vorverarbeitung (Mapping auf Unicode‑Bereiche) oder Fehler `FORX0002` (Tests/Matrix definieren erwartetes Verhalten).
- Backreferences/Look‑around: nicht erforderlich für XSD‑Regex und nicht unterstützt.
- Semantik: `matches`/`replace`/`tokenize` folgen F&O 2.0; Anker/Quantoren verhalten sich gemäß XSD‑Regex‑Spezifikation, dokumentierte Abweichungen werden getestet.

### Standard-Collations & URIs
- Default Codepoint: `http://www.w3.org/2005/xpath-functions/collation/codepoint` (bereits vorhanden)
- Simple Case-Insensitive: `urn:platynui:collation:simple-case`
  - Vergleich: `to_lowercase()` auf Unicode‑Basis, dann Codepoint‑Vergleich
- Simple Accent-Insensitive: `urn:platynui:collation:simple-accent`
  - Vergleich: Unicode‑Normalisierung (NFD) + Entfernen kombinierender Zeichen (General Category Mn), dann Codepoint‑Vergleich
- Simple Case+Accent-Insensitive: `urn:platynui:collation:simple-case-accent`
  - Kombination aus beiden Strategien

Fehler/Verhalten:
- Unbekannte Collation‑URI → `err:FOCH0002`
- Auflösung der Default-Collation (Priorität):
  1) explizite URI-Argumente in Funktionen,
  2) `DynamicContext.default_collation`,
  3) `StaticContext.default_collation`,
  4) Fallback: Codepoint-Collation.
- Dieselbe Auflösungslogik gilt für String-Vergleiche (value/general) und Collation-sensitive Funktionen.
- Optional später: ICU‑basierte Collation per Feature `icu-collation` (Locale‑aware UCA)

## Dokumentreihenfolge & Identität (Adapter-Guidelines)
- Zweck: Korrekte Semantik für Achsen (`preceding`, `following`, Geschwisterachsen), Mengenoperatoren (`union/intersect/except`), Prädikate (`position()`, `last()`), und Node‑Vergleiche (`is`, `<<`, `>>`).
- Vertrag (Adapter):
  - Identität: `Eq` vergleicht Node‑Identität (nicht Struktur).
  - Totalordnung: `compare_document_order(a,b)` liefert deterministisch `Less|Equal|Greater` für beliebige `a,b` (auch über Multi‑Root).
  - Konsistenz: Vorfahre < Nachfahre; Geschwister gemäß Kindreihenfolge; `Equal` nur bei Identität.
  - Stabilität: Ordnung ändert sich während der Evaluation nicht.

- Ideale Implementierung (empfohlen, O(1)):
  - Pro Knoten zwei Schlüsselwerte bereitstellen: `tree_id` (ID der Wurzel/dokumentweiten Struktur) und `preorder_index` (DFS‑Index).
  - Vergleich: lexikographisch über `(tree_id, preorder_index)`.
  - Multi‑Root: Eindeutige, stabile `tree_id` je Wurzel; Vergabe z. B. beim Einfügen in den Kontext oder im Adapter.

- Fallback‑Vergleich (ohne gespeicherte Indizes):
  - Algorithmus: Baue Ahnenketten `a→root`, `b→root`; wenn Roots verschieden → Adapter definiert globale Ordnung (z. B. nach Source‑Liste). Sonst LCA finden, divergierende Geschwister via `parent.children()` in Reihenfolge vergleichen.
  - Komplexität: O(h) für Ahnen + O(s) für Sibling‑Scan; für kleine/mittlere Bäume ok, aber langsamer als O(1).

- Attribute/Namensräume:
  - `preceding`/`following` schließen Attribute/Namensräume aus (bereits im Evaluator berücksichtigt).
  - Innerhalb eines Elements stabile Reihenfolge für Attribute/Namensräume sicherstellen (z. B. Einfügereihenfolge oder `(ns_uri,local)`), damit Gesamtordnung total bleibt.

- Utility‑Skizze (Fallback):
```rust
// Hilfsfunktion (Vorschlag für model.rs, optional)
fn compare_by_ancestry<N: XdmNode>(a: &N, b: &N) -> core::cmp::Ordering { /* LCA + siblings */ }
```

- Aufrufstelle/Integration:
  - Der Evaluator ruft niemals die Helper‑Funktion direkt auf, sondern ausschließlich `XdmNode::compare_document_order` (Vertrag mit dem Adapter).
  - Primär: Adapter implementiert `compare_document_order` und nutzt darin entweder
    - einen O(1)‑Vergleich mit `(tree_id, preorder_index)` oder
    - den Fallback‑Helper `compare_by_ancestry` (korrekt, aber langsamer).
  - Default im Trait: `XdmNode::compare_document_order` erhält eine Default‑Implementierung, die intern `compare_by_ancestry` nutzt (korrekt innerhalb eines Baumes). Adapter mit O(1)‑Keys oder Multi‑Root‑Szenarien überschreiben die Methode mit `(tree_id, preorder_index)`.
  - Fehler-/Diagnostik-Hinweis: Optional (Debug/Feature `strict-doc-order`) überwachen wir Multi-Root-Vergleiche in der Default-Implementierung und loggen/warnen, wenn ohne Adapter-Override über unterschiedliche Wurzeln verglichen wird.
  - Multi‑Root: Die Default‑Implementierung stellt keine globale Ordnung zwischen Wurzeln her. Für Multi‑Root MUSS der Adapter `compare_document_order` überschreiben und `tree_id` berücksichtigen; erst bei identischer `tree_id` den Fallback nutzen.

- Akzeptanzkriterien:
  - Sets/Steps werden dedupliziert und in Dokumentreihenfolge sortiert; `position()`/`last()` sind korrekt.
  - Node‑Vergleiche (`is`, `<<`, `>>`) liefern konsistente Ergebnisse.
  - Multi‑Root: Sequenzen aus mehreren Bäumen sind deterministisch global geordnet.
  - Evaluator verwendet ausschließlich `XdmNode::compare_document_order`; Helper und Default‑Implementierung sind Adapter/Trait‑Detail.

## Typ-Registry (Atomic Types)
- Ziel: Offen für beliebige Baum-/Domain-Modelle und optional XML‑Schema‑abgeleitete Typen, ohne den Kern zu spezialisieren.
- Ansatz: Plugin‑fähige `TypeRegistry` im `StaticContext` für atomare Typen.
- Default‑Registry: Liefert alle XPath‑2.0/XDM‑Basistypen (z. B. `xs:string`, `xs:boolean`, `xs:integer/decimal/double/float`, `xs:anyURI`, `xs:QName`, `xs:untypedAtomic`). Optional (Feature‑Flag) `xs:date|time|dateTime|durations`.
- Erweiterbarkeit: Hosts können zusätzliche Typen registrieren (z. B. Schema‑abgeleitete Typen mit Facets). Der Kern bleibt unabhängig von XML.

API‑Skizze:
- Trait `TypeRegistry` (nur atomare Typen):
  - `resolve(q: &ExpandedName) -> Option<TypeId>`
  - `is_subtype(sub: TypeId, sup: TypeId) -> bool`
  - `castable(value: &XdmAtomicValue, target: TypeId) -> bool`
  - `cast(value: &XdmAtomicValue, target: TypeId) -> Result<XdmAtomicValue, Error>`
- Integration:
  - Compiler: Validierung bekannter Typ‑QNames optional (sonst Fehler zur Laufzeit). Empfehlung: Unbekannte Typen → `err:XPST0017` (statisch), wenn zur Compile‑Zeit eine Registry im `StaticContext` vorhanden ist; sonst dynamischer Fehler zur Laufzeit.
  - Evaluator: `cast/castable/instance of/treat as` delegieren für atomare Typen an Registry; Node‑Kinds/`item()` bleiben im Evaluator.
  - Zeitplan: Umsetzung in M9 (Roadmap). Zuerst Default‑Registry mit Basistypen, danach Delegation im Evaluator/Compiler‑Hooks aktivieren.

Stabilität/IDs:
- `TypeId` ist innerhalb einer Registry-Instanz stabil. Für Caching/Invalidierung versieht jede Registry (Type/Function/Collation) sich mit einem monotonen `version()`‑Zähler (oder Hash‑Fingerprint) für die Zusammensetzung; Änderungen invalidieren betroffene Caches.

Akzeptanzkriterien:
- Bestehende Typ‑Tests bleiben grün (Cast/Treat/Instance‑of/Castable für Basistypen).
- Unbekannte/abgeleitete Typ‑QNames schlagen statisch mit `XPST0017` fehl (wenn gewünscht), ansonsten dynamisch mit spezkonformem Fehler.
- Optional: Feature‑Flag für Date/Time/Duration aktiviert zusätzliche Typen + Funktionen.

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
  - Optimierungen (optional): Konstantfaltung, Dead-Code-Elimination (einfach), Predicate-Reordering (optional), Inline von einfachen Funktionen (optional)

## Evaluator (IR-Interpreter)
- Laufzeit-Stack für Items/Sequenzen, Call-Frames für Funktionsaufrufe/Bindings
- Iterator-basierte Ausführung für Pfade/Schritte (Streaming-freundlich)
- EBV-Regeln: booleans, numerics, strings, nodes, leere Sequenzen
- Vergleiche: general vs value comparisons, Node-Vergleich über Dokumentreihenfolge/ID
- Knotenachsen: Implementierung auf Basis des Node-Adapters; Prädikate beeinflussen Kontext (Position/Last)
- Datentypen/Casting: gemäß F&O, inkl. `untypedAtomic`-Semantik, Numerik-Promotion (decimal/double/float), Zeichenfolgen-Kollation (default codepoint)
- Fehlerbehandlung: Fehlercodes (QName `err:*`), statisch/dynamisch differenziert

Umsetzungsnotizen:
- Predicate-Auswertung respektiert XPath 2.0 EBV inkl. numerischer Indexprädikate.
- Achsen sind vollständig umgesetzt, inklusive `preceding`/`following` und Geschwisterachsen; Dokumentreihenfolge + Deduplizierung werden sichergestellt.
- Value/General-Comparisons berücksichtigen Atomisierung und Numeric-/String-Promotion, inkl. `untypedAtomic`.
 - String‑Vergleiche und string‑Funktionen verwenden dieselbe Collation‑Auflösung (Priorität s. oben).

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
  - Node/Tree: `name`, `local-name`, `namespace-uri`, `string()`, `root`
  - Datums/Zeitfunktionen: `current-date`, `current-time`, `current-dateTime`, Konstruktoren/Berechnungen für Duration
  - Aggregation: `count`, `sum`, `avg`, `min`, `max` (mit Collation/Typregeln)
  - Typen: `xs:type(...)`-Konstruktoren via `Cast`
- Kollationen:
  - Default: Codepoint Collation
  - Erweiterbar via Trait, Registry per URI

Umsetzungsnotizen:
- Funktions-Registry ist implementiert; derzeit erhalten Funktions-Closures nur die Argumente (kein direkter Context-Zugriff). Für Collation-/Resolver-/Regex-gestützte Funktionen wird ein Context-Objekt benötigt (Designentscheidung in M6; `CallCtx`). Der Default-Regex-Provider basiert auf Rust‑`regex` und ist über den Kontext erreichbar.

### API-Refactor: Kontextbewusste Funktionen (M6)
- Ziel: Funktionen sollen Zugriff auf Default-Collation, Resolver, Static/Dynamic Context, Regex‑Provider und (künftig) Now/Timezone haben.
- Änderung (Breaking, keine Abwärtskompatibilität erforderlich):
  - Neues Context-Objekt `CallCtx<'a, N>` mit Zugriff auf `&DynamicContext<N>`, `&StaticContext`, `default_collation` (implizit über Auflösungslogik), `resolver`, `regex_provider`.
  - Funktionssignatur wird auf kontextbewusste Form geändert: `type FunctionImpl<N> = Arc<dyn Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error> + Send + Sync>`.
  - `FunctionRegistry` speichert ausschließlich kontextbewusste Funktions-Impls; bestehende Funktionen werden migriert.
  - Evaluator erzeugt pro Aufruf ein `CallCtx` und ruft die Funktion mit `(ctx, args)` auf.

Beispiel (Skizze):
```rust
pub struct CallCtx<'a, N> {
    pub dyn_ctx: &'a DynamicContext<N>,
    pub static_ctx: &'a StaticContext,
    // Default-Collation wird gemäß Priorität (explizit → dyn → static → codepoint) aufgelöst
    pub default_collation: Option<std::sync::Arc<dyn Collation>>,
    pub resolver: Option<&'a dyn ResourceResolver>,
    pub regex_provider: &'a dyn RegexProvider,
}

pub type FunctionImpl<N> = std::sync::Arc<dyn Fn(&CallCtx<N>, &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error> + Send + Sync>;
```

Akzeptanzkriterien (API-Refactor):
- Alle existierenden Funktionen (boolean/string/numeric/sequence) sind auf die neue Signatur migriert.
- Alle bestehenden Tests bleiben grün; Default-Collation-Verhalten in Vergleichen bleibt unverändert korrekt.
 - Funktionen sind `Send + Sync` und rein oder nur kontext‑lesend (Nebenwirkungen sind vermieden oder intern synchronisiert).

## Namespaces & QNames
- Statischer Kontext: Präfix→URI Bindungen; Default-Funktionsnamespace
- `QName`-Utility (xdm): Auflösung, Vergleich, Serialisierung
- Node-Tests: `NameTest` (QName), `KindTest` (node(), text(), comment(), processing-instruction()), `Wildcard` (prefix:*, *:local, *)
 - Default-Element-Namespace gilt nicht für Attribute (Attribute sind nur mit expliziten Präfixen im Namespace).

## Caching & API
- Öffentliche API (Rust):
  - `parse_xpath(&str) -> Ast` (bestehend, neue AST-Struktur)
  - `compile_xpath(expr: &str, static_ctx: &StaticContext) -> XPathExecutable`
  - `XPathExecutable::evaluate::<N: XdmNode>(&self, dyn_ctx: &DynamicContext<N>) -> Result<XdmSequence<N>>`
- Cache: `XPathExecutableCache` (HashMap/LRU) keyed by `expr + static_ctx_fingerprint`. Der Fingerprint umfasst mind. Namespace-Bindungen, Default-Funktionsnamespace, Base‑URI, Default‑Collation (URI), Funktions‑Registry‑Version, Collation‑Registry‑Version, Type‑Registry‑Version. Dynamische Werte (Variablen, Now/Timezone) gehen nicht in den Key ein.

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

## Teststrategie (rstest)
Kurze Einführung
- Ziel: Spezifikationskonforme, deterministische und wartbare Tests mit klarer Trennung zwischen Parser, Compiler und Evaluator.
- Ansatz: Dünne, schnelle Unit-Tests pro Schicht, ergänzt durch zielgerichtete End‑to‑End‑Fälle auf kleinen Mock‑Bäumen. Fehlerfälle prüfen wir über spezkonforme Error‑Codes.
- Werkzeug: rstest für lesbare Parametrisierung (#[case]/#[values]/#[matrix]) und wiederverwendbare Fixtures; fokussierte Utilities vereinheitlichen die Assertions und Ergebnis‑Extraktion.
- Prinzipien: kleine, präzise Testfälle; keine Sleeps oder Nondeterminismus; nachvollziehbare Namen/Struktur; Tests bleiben unabhängig und schnell lauffähig.

Ein-Thema-Tests (Single-Responsibility)
- Grundsatz: Ein Test prüft genau EINEN Aspekt und hat nur einen legitimen Grund zu scheitern.
- Struktur: Arrange/Act/Assert klar trennen; idealerweise eine primäre Assertion pro Test (weitere Asserts nur zur Präzisierung desselben Aspekts, nicht für neue Aspekte).
- Eingaben minimieren: Kleinste DOMs/Sequenzen wählen, die den Aspekt sichtbar machen (z. B. für `preceding-sibling` genau zwei Geschwister; keine zusätzlichen Attribute/Namespaces, wenn sie nicht getestet werden).
- Parametrisierung: `#[case]`/`#[values]`/`#[matrix]` verwenden, um denselben Aspekt über mehrere Werte zu prüfen, statt mehrere Aspekte in einem Test zu kombinieren.
- Negative Tests: Pro Test genau ein erwarteter Fehlercode/Pfad (z. B. nur `XPST0017`), keine gemischten Fehlerfälle.
- E2E-Tests: Ebenfalls nur einen Aspekt der End-to-End-Kette validieren (z. B. “Prädikat-Indexierung über Pfadkette”), keine “Küchenspülen”-Szenarien.
- Benennung: Test- und Case-Namen spiegeln den einen Aspekt wider (z. B. `preceding_excludes_ancestors__returns_only_non_ancestor_nodes`).
- Anti-Pattern: Kombinationen wie “Achsen + Collation + Regex” im selben Test vermeiden; stattdessen aufteilen in getrennte, fokussierte Tests.

- Parser (bestehend): weiterführen; ggf. Anpassungen der AST-Hilfen
- Compiler-Unit-Tests: AST→IR für gezielte Snippets (Operatorpräzedenz, Bindung, QNames)
- Evaluator-Unit-Tests: IR-Ausführung für jede Operator-/Funktionengruppe
- End-to-End-Tests: Ausdruck → Ergebnis auf einer Mock-Baumstruktur (Elemente/Attribute/Text)
- Achsentests: für alle XPath-Achsen; Namens-/Namespace-Tests; Prädikate mit Kontext
- Typen/Casting: umfangreiche Positiv/Negativ-Cases inkl. `untypedAtomic`
- Funktionen: pro Familie (string, numeric, sequence, node, regex, date/time)
- Property-ähnliche Checks via Parameter-Matrizen: z. B. `reverse(reverse(S)) == S`, `count(insert-before(S,i,x)) == count(S)+1`
- Performance-/Speichertests: große Sequenzen, tiefe Pfade, viele Prädikate

rstest-Best Practices & Patterns
- Gemeinsame Fixtures:
  - `#[fixture] fn sc() -> StaticContext` (mit xs/ns-Prefixen), `#[fixture] fn ctx<N: XdmNode>() -> DynamicContext<N>`
  - Reusable DOM-Bäume: `dom_small()`, `dom_ns()`, `dom_siblings()`, `dom_deep()`; Rückgabe von Root-Knoten (Arc)
- Zentrale Test-Helfer in `tests/support/mod.rs`:
  - Konverter: `as_bool`, `as_string`, `as_num`, `atoms`, `names`
  - Shortcuts: `eval(expr)`, `compile(expr)`, `assert_err_code(expr, code)`
- Parametrisierung:
  - `#[rstest]` mit `#[case]` für Szenarien
  - `#[values]` für kleine Wertmengen (z. B. Operatoren, Literale)
  - `#[matrix]` für kartesische Kombinationen (z. B. Vergleichsoperator × Wertepaare)
  - `#[trace]` auf selektiven Tests, um Parameter bei Fehlschlägen zu loggen
- Negative Tests:
  - Keine `should_panic`; stattdessen `Result` prüfen und `Error{kind, code}` verifizieren (z. B. `XPST0017`, `FOCH0002`)
- Lange/teure Tests (Feature‑gated):
  - Über Cargo‑Feature (z. B. `perf`) kapseln: `#[cfg(feature = "perf")]` an Testmodul/-funktionen.
  - Ausführen nur gezielt: `cargo test --features perf` (optional zusätzlich mit `-- --nocapture`).
  - Optional separate Dateien (z. B. `tests/perf_large_paths.rs`), klar dokumentiert.
  - Für echte Benchmarks Criterion in einem eigenen Benchmark‑Target verwenden (kein Teil des Standard‑Testlaufs).
- Struktur & Benennung:
  - Tests nach Feature/Milestone gruppieren; Dateinamen sind Empfehlungen, keine Pflicht.
    - Namensmuster: `evaluator_*`, `functions_*`, `parser/*`, `perf_*`.
    - Beispiele: `functions_string.rs`, `functions_collations_*.rs` (Aufteilung in basic/edge/perf), `functions/regex.rs` (Unterordner erlaubt).
  - Funktionsnamen “szenario__erwartung” oder sprechende Beschreibungen mit `#[case::label]`
  - Pro tests/*-Datei logische Abschnitte mit Kommentar-Headern (Given/When/Then) zur Orientierung
- Debugbarkeit:
  - Bei Bedarf `exec.debug_dump_ir()` in Fehlerinformationen einbinden (nicht als Snapshot, nur diagnostisch)

Geplante Ergänzungen (M6–M9)
- Hinweis: Die folgenden Dateinamen sind Vorschläge. Wenn ein Test nicht in diese Dateien passt, neue Dateien/Submodule nach obigem Muster anlegen (z. B. `functions_collations_basic.rs`, `functions_collations_edge.rs`, oder `tests/functions/collations.rs`).
- M6 (Context & Ordnung):
  - (z. B.) `function_context.rs`:
    - Verifiziert, dass Funktionen via CallCtx Default‑Collation sehen (z. B. `contains('Ä','ä')` mit simple‑case‑accent Collation vs. Default).
    - Negativ: unbekannte Collation‑URI in Funktionsaufruf → `FOCH0002`.
  - (z. B.) `adapter_ordering.rs`:
    - Pfadketten mit gemischten Achsen, Prüfung von `position()`/`last()` gegen erwartete Reihenfolge.
    - Node‑Vergleiche: `a[1] << a[2]`, `//c >> //a[1]` (Single‑Root); Deduplizierung nach Steps/Set‑Ops.
    - Hinweis: nutzt Default‑Dokumentreihenfolge (Fallback) in Single‑Root‑Adapter.
- M7 (Collations & Regex):
  - (z. B.) `functions_collations.rs` oder `functions_collations_*.rs`:
    - Overloads (2/3‑Arg) für `contains/starts-with/ends-with` mit `[case]` Parametern (Codepoint, simple‑case, simple‑accent, simple‑case‑accent).
    - `compare`, `codepoint-equal`, `deep-equal` (Stringpfad) unter verschiedenen Collations.
    - Fehler: unbekannte URI → `FOCH0002`; Leerstring/Unicode‑Kombinatoren.
    - Nutzung von `#[matrix]` für (Collation × Eingabepaare) und `#[trace]` zur Diagnose.
  - (z. B.) `functions_regex.rs` oder `functions/regex.rs` (Unterordner):
    - `matches/replace/tokenize` mit Flag‑Kombinationen (z. B. `i`, `m`) via `#[matrix]`.
    - Ungültige Patterns → spezkonforme Fehler; Grenzfälle mit Unicode.
- M8 (Date/Time):
  - (z. B.) `functions_datetime.rs`:
    - `current-date|time|dateTime` vs. gesetztes `with_now/with_timezone`; Offsets prüfen.
    - Dauer‑Arithmetik (falls implementiert), Format‑Grenzfälle, Negativfälle (ungültige Literale).
-- M9 (Resolver & Types & Cache):
  - (z. B.) `resolver_doc.rs` / `resolver_collection.rs`:
    - Erfolgs-/Fehlerfälle (nicht vorhandene URI), Base‑URI‑Auflösung; ggf. `doc-available`.
  - (z. B.) `type_registry.rs`:
    - Basistypen, Delegation in `cast/…`, Negativfälle (`XPST0017` für unbekannte Typnamen).
  - (z.B.) `cache_lru.rs`:
    - Gleiche Expression + StaticContext → Cache‑Hit; unterschiedliche Collation/Namespace → Cache‑Miss.



## Konformitäts-Matrix (erste Iteration)
Ziel: Jede relevante Spezifikationsfläche ist vollständig abgedeckt (Implementierung + Tests). Status-Felder dienen der Nachverfolgung während der Entwicklung.

- Spez: XPath 2.0 – Ausdrücke, Schritte, Operatoren
  - Implementierung: Parser/Compiler/Evaluator (Pfadausdrücke, Schritte & Achsen, Prädikate, Sequenzkonstruktoren, Arithmetik/Logik, Wert‑ vs. Allgemein‑Vergleiche, Knotenvergleiche `is`/`<<`/`>>`, Bereichsoperator `to`, `idiv`). FLWOR/Quantifizierer/`if-then-else` stehen noch aus.
  - Tests: `crates/platynui-xpath/tests/{parser/*, evaluator_{paths,axes,predicates,comparisons,sets,range,types,functions*}.rs}`.
  - Status: [umgesetzt (M1–M5), FLWOR/Quantifizierer: geplant]

- Spez: XDM (XPath Data Model) – Items/Sequenzen, Knotentypen, Ordnung
  - Implementierung: `xdm`-Typen (Item/Sequence/Atomic), `XdmNode`-Trait, Dokumentreihenfolge/Identität, Atomisierung, EBV‑Regeln, `xs:untypedAtomic`, Zahl‑Promotion. Date/Time/Duration-Typen fehlen noch.
  - Tests: über Evaluator-/Vergleichs-/Achsentests abgedeckt; zusätzliche XDM-Unit-Tests optional.
  - Status: [weitgehend umgesetzt], Date/Time/Duration: [geplant]

- Spez: XQuery and XPath Functions and Operators (F&O)
  - Implementierung: Standardfunktionsbibliothek (string/numeric/sequence) vorhanden; regex/collation-spezifische und date/time-Funktionen fehlen.
  - Tests: `evaluator_functions*.rs` vorhanden; regex/datetime/collation: [geplant].
  - Status: [teilweise umgesetzt]

- Spez: Kollationen & Regex (F&O)
  - Implementierung: Default Codepoint Collation und Registry vorhanden; Regex über den Runtime‑Provider (Default: Rust‑`regex`). Collation‑Handling in Vergleichen vorhanden; funktionsseitige Collation‑Parameter werden ergänzt.
  - Tests: `crates/platynui-xpath/tests/{collations.rs, regex_compat.rs}` [geplant].
  - Status: [in Arbeit]

- Spez: Namespaces/QNames
  - Implementierung: Statischer Kontext (Präfixbindungen, Default‑Funktionsnamespace), QName‑Auflösung, Node‑Tests (Name/Kind/Wildcards); Default-Elementns wird berücksichtigt, Attribute ohne Default-NS.
  - Tests: `crates/platynui-xpath/tests/evaluator_namespaces.rs` u. a.
  - Status: [umgesetzt]

- Spez: Ressourcen/Resolver (F&O: `doc`, `doc-available`, `collection`)
  - Implementierung: Resolver‑Trait + Fehlerfälle, Base‑URI‑Verhalten.
  - Tests: `crates/platynui-xpath/tests/{resolver_doc.rs, resolver_collection.rs}`.
  - Status: [geplant]

- Spez: Fehlercodes und Diagnostik (err:*)
  - Implementierung: Einheitlicher Error‑Typ mit Spez‑konformen Codes; statisch vs. dynamisch.
  - Tests: `crates/platynui-xpath/tests/errors_{static,dynamic}.rs` mit gezielten Negativfällen. (geplant)
  - Status: [teilweise umgesetzt]

## Fehler- und Diagnostik-Design
- Einheitlicher `Error`-Typ mit Codes `err:*` (Spezifikationskonform), Quelle (statisch/dynamisch), Positionsinfos
- Gute Debug-/Trace-Optionen: IR-Dump, Ausführungs-Trace (optional feature)

Fehlercodes – häufige Mappings (Auszug):
- `FOAR0001`: Division durch 0 (`div`, `idiv`).
- `FORG0001`: Ungültige numerische Konversion (z. B. `string`/`untypedAtomic` → Zahl).
- `FORG0006`: Falsche Arity/Kardinalität (z. B. Wertvergleich erwartet genau ein atomares Item je Operand; `to` erwartet genau ein Integer je Seite).
- `XPTY0004`: Typverletzung zur Laufzeit (z. B. `treat as` schlägt fehl; nicht-numerischer Operand in Arithmetik).
- `XPST0017`: Unbekannte Funktion/Typname (statisch, falls zur Compile‑Zeit auflösbar).
- `XPDY0002`: Fehlendes Kontext‑Item zur Laufzeit.
- `FOCH0002`: Unbekannte Collation‑URI.
- `FORX0002`: Ungültiges Regex‑Pattern/Flags.

## Runtime & Thread-Safety
- Thread-Safety: `XPathExecutable: Send + Sync`. Funktions-/Collation-/Type‑Registry thread-safe; `DynamicContext` muss nicht `Sync` sein. Parallele Auswertung ist möglich, solange pro Auswertung ein eigener `DynamicContext` verwendet wird. Funktions‑Implementierungen sind reentrant (`Send + Sync`) und greifen nur lesend über `CallCtx` zu.
- Dokumentreihenfolge/Identität über Multi-Root: `tree_id + preorder_index` liefert eine Totalordnung; stabil über Lebensdauer der Bäume.

Umsetzungsstand:
- Caching noch nicht implementiert. Dokumentreihenfolge via `XdmNode::compare_document_order` hergestellt; Deduplizierung und Sortierung sind im Evaluator enthalten.

## Roadmap & Meilensteine
- M1: Parser/AST & Grammatikbasis — AST‑Modul, Tokenisierung (QNames), Parser‑Tests [abgeschlossen]
- M2: IR & Minimal‑VM — InstrSeq/OpCodes, Arithmetik/Bool/Sequenzen, Funktionsaufruf‑Stub [abgeschlossen]
- M3: Pfade & Basisachsen — child/attribute/self, Prädikate, EBV, relative/absolute Pfade [abgeschlossen]
- M4: Vergleiche & Mengen & restliche Achsen — value/general, `is/<< >>`, `union/intersect/except`, `to`, `idiv` [abgeschlossen]
- M5: Typen — `cast/castable/treat/instance of`, Promotion/Atomisierung, `untypedAtomic`‑Regeln [abgeschlossen]
- M6: Runtime/Registry‑Refactor & Ordnung — CallCtx (kontextbewusste Funktionen), Default‑Collation in Funktionen, Adapter‑Guidelines + Default‑Dokumentreihenfolge (`compare_by_ancestry`) [in Arbeit]
- M7: Funktionen (String/Sequence/Node) komplettieren — Collation‑Overloads (`contains/starts-with/ends-with`, `compare`, `codepoint-equal`), Regex‑Familie (`matches/replace/tokenize`) via Rust‑`regex`, Aggregat‑Feinkanten [geplant]
- M8: Datum/Zeit/Dauer — XDM‑Typen (Feature), `current-*`, Timezone/Now im Kontext, Basis‑Funktionen [geplant]
- M9: Typ‑Registry & Resolver & Caching — pluggable `TypeRegistry` (atomic), Delegation in `cast/…`, Ressourcen‑Resolver (`doc/collection`), `base-uri/resolve-uri`, LRU‑Cache (s. „Caching & API“), Fehlercodes/Conformance/Performance/Doku [geplant]

## Offene Designpunkte (bewusst festgehalten)
 - Collations: Start mit Codepoint-Collation; Erweiterungspunkt für ICU-basiert (optional Feature).


---

## TODO-Liste (umsetzungsorientiert)

### Parser/AST
- [x] (M1) Neues Modul `parser/ast.rs` mit vollständigen AST-Knoten (XPath 2.0)
- [x] (M1) Bestehende vereinfachte AST-Hilfsfunktionen in Test-Utils verschieben/duplizieren
- [x] (M1) Parser-Regeln anpassen, damit vollständiger AST entsteht (QNames tokenisieren)
- [x] (M1) Parser-Tests weiter grün; ggf. Anpassungen an AST-Extraktion in Tests

### Grammatik-/Parser-Checkliste
- [x] (M1) Operatoren/Tokens: `idiv`, `is`, `<<`, `>>`, `to`, `treat as`, `instance of`, `cast`, `castable as`
- [x] (M1) Knoten-Tests: `processing-instruction('target')`, `node()`, `text()`, `comment()`
- [x] (M1) Wildcards: `*`, `ns:*`, `*:local`, `@*`

### Compiler/IR
- [x] (M2) IR-Design dokumentieren und Types/OpCodes implementieren
- [x] (M2) Statischer Kontext: Namespaces, Funktionen, Variablen, Base-URI, Kollation
- [x] (M2) Binder: Variablen/Slots, Funktionsauflösung (QName+Arity)
- [ ] (M9) Optimierungen (Konstantfaltung, einfache DCE) (optional)
- [x] (M4) Node-Compare-OpCodes (`is`, `<<`, `>>`), `RangeTo`, `IDiv`
- [ ] (M9) `Some/Every`, `For/Let`
- [ ] (M9) Typ‑Registry‑Hook: Optionale statische Validierung bekannter Typ‑QNames (`XPST0017` für unbekannte)

### Evaluator
- [x] (M2) VM/Interpreter (Stack, Frames, Sequenzen, EBV)
- [x] (M3) Pfad-Auswertung mit Prädikaten, Kontextposition/-größe
- [x] (M4) Vergleiche (value/general), Mengen, Sequenz-Operatoren
- [x] (M5) Casting/Promotion/`untypedAtomic`-Handling, Treat/Instance-of
- [x] (M4) Achsen-Ordnung & Deduplizierung, Node-Gesamtordnung (Multi-Root)
- [ ] (M9) Quantifizierte Ausdrücke, FLWOR-Bindungen
- [ ] (M9) `cast/castable/treat/instance of` auf `TypeRegistry` delegieren (atomare Typen)
- [x] (M3) Knotenmodell: `XdmNode`-Integration, Beispiel-Adapter + Tests
- [ ] (M6) Dokumentation: Adapter-Guidelines zu Identität & Dokumentreihenfolge (tree_id + preorder_index, Fallback‑Algorithmus)
- [ ] (M6) Utility `compare_by_ancestry` im `model`-Modul bereitstellen + einfache Tests
- [ ] (M6) Default‑Implementierung von `XdmNode::compare_document_order` im Trait, die `compare_by_ancestry` nutzt (Single‑Root korrekt); Hinweis/Doc, dass Multi‑Root Adapter überschreiben müssen; optionales Feature `strict-doc-order` für Warnungen bei Multi‑Root mit Default

### Funktionen & Kollationen
- [x] (M2) Funktions-Registry (erweiterbar), Signaturen, Overloads
- [x] (M5) Standardfunktionen (erste Welle): string, numeric, sequence (inkl. `distinct-values`, `index-of`, `insert-before`, `remove`, `reverse`, `subsequence`)
- [x] (M2) Collation-Registry; Default-Codepoint; Erweiterungspunkt
- [ ] (M6) API-Refactor: Kontextbewusste Funktionssignatur (`CallCtx`) in Registry/Evaluator (inkl. Regex‑Provider)
- [ ] (M6) Migration: Alle bestehenden Funktionen (boolean/string/numeric/sequence) auf neue Signatur umstellen
- [ ] (M7) Collations in Funktionen: 2-/3-Arg-Varianten (`contains`, `starts-with`, `ends-with`) + `compare`, `codepoint-equal`; Default-Collation nutzen
- [ ] (M7) Built-in Simple Collations registrieren (URIs: `simple-case`, `simple-accent`, `simple-case-accent`), FOCH0002 für unbekannte URIs
- [ ] (M7) Regex-Familie (`matches`, `replace`, `tokenize`) inkl. Flags/Fehlercodes
- [ ] (M7) Gleichheit/Ähnlichkeit: `compare`, `codepoint-equal`, `deep-equal` (Collation-aware)
- [ ] (M8) Date/Time/Duration-Funktionen (inkl. `current-*`)
- [ ] (M9) Node-/QName-/Namespace-Funktionen
- [ ] (M9) Ressourcen-Resolver: `doc`, `doc-available`, `collection`
- [ ] (M9) Base-URI/URI: `base-uri`, `resolve-uri`

### API & Caching
- [x] (M2) Öffentliche API: `compile_xpath`, `evaluate`
- [ ] (M9) `XPathExecutableCache` (LRU/HashMap), Fingerprint über `expr + static_ctx_fingerprint`
- [x] (M2) Doku/Beispiele (grundlegend)
- [x] (M2) DynamicContextBuilder und Convenience-Methoden (`evaluate_on`, `evaluate_with_vars`)
- [ ] (M8) DynamicContextBuilder: `with_now`, `with_timezone`
- [ ] (M9) `StaticContext` enthält `Arc<TypeRegistry>`; Default‑Registry mit XPath‑Basistypen bereitstellen
- [ ] (M6) `CallCtx`-Struktur definieren und Evaluator-Callsite anpassen

### Tests
- [x] (M1–M5) Unit-Tests je Modul (rstest)
- [x] (M3–M5) End-to-End-Tests auf Mock-Baum (Evaluator-Funktionsfamilien, Achsen/Pfade)
- [ ] (M7–M9) Umfangreiche F&O-Conformance-Cases (Regex/Date/Time/Collations) inkl. Kompatibilitätsmatrix (unterstützte Flags/Features) und Negativfälle (z. B. `FORX0002`, `FOCH0002`).
- [ ] (M9) Performance-/Speicher-Tests
- [x] (M3–M4) Achsen/Pfade: Reihenfolge & Deduplizierung; Pfadketten
- [x] (M4) Node-Vergleiche: `is`, `<<`, `>>`; Bereich `to`; `idiv`
- [x] (M3) EBV/Prädikate: numerisch vs. boolsch; Fehler bei >1 atomaren Items
- [x] (M4–M5) Vergleiche/Promotion/`untypedAtomic`-Fälle
- [x] (M4) Mengenoperatoren nur für Nodes (doc order + dedup)
- [ ] (M7) Collations: Overloads (2/3‑Arg), Default vs. explizite URI, FOCH0002 bei unbekannter URI; Case/Diakritika‑Fälle; `compare`/`codepoint-equal`/`deep-equal`
- [ ] (M7–M8) Regex/Date/Time/Timezone Grenzfälle
- [ ] (M9) Ressourcen/Resolver Erfolg/Fehler

### Qualität & Tooling
- [ ] Fehlercodes/Diagnostik konsolidieren

---

## Nächste konkrete Schritte (M6)
1) Funktions-API-Refactor: `CallCtx` einführen (inkl. Regex‑Provider), Registry/Evaluator auf kontextbewusste Signaturen umstellen; bestehende Funktionen migrieren.
2) Collations in Funktionen: 2-/3-Arg-Varianten und `compare`/`codepoint-equal` implementieren; Default-Collation respektieren (einheitliche Auflösung).
3) Regex: Default‑Provider auf Rust-`regex` aufsetzen; `matches`/`replace`/`tokenize` inkl. Flags/Fehlercodes (`FORX0002`) und dokumentierter Feature‑Abdeckung.
4) Date/Time/Duration: XDM-Typen ergänzen; `current-date`/`current-time`/`current-dateTime` und Timezone-Handling; `DynamicContextBuilder.with_now/with_timezone`.
5) Tests: neue rstest-Suiten für Collation/Regex/DateTime inkl. Negativ-/Fehlerfälle und Kompatibilitätsmatrix; bestehende Tests grün halten.
