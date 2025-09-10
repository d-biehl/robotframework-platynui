# XPath 2.0 Evaluator Implementation Plan

Statusbasis (Ist): Pfadauswertung (Teilmenge Achsen), Prädikate ohne vollständige Positionslogik, grundlegende Operatoren (arith/vergleich bool), vereinfachtes `instance of` & `treat as`, Sequenzen & Ranges, Teilmenge F&O, Fehlercode-Grundgerüst.

Ziel: Funktionskomplette XPath 2.0 Evaluationsengine (ohne Schema Awareness & ohne XQuery-spezifische Erweiterungen), optional spätere Erweiterung.

---
## Phasenübersicht

1. Core Semantik: Position/last(), General vs Value Comparisons, Quantifizierer, Cast/Castable, Konstruktorfunktionen, Node Comparisons.
2. Bindings & Kontrolle: for/let Expressions, Variablen-Scope, generischer Funktionsaufruf.
3. Function Library Wave 1: Sequenzfunktionen, Numerik, Aggregation, Basis-String.
4. Erweiterte Funktionen: Regex, Date/Time Snapshot & Arithmetik, Collations Grundgerüst.
5. Typing & Promotions: untypedAtomic Regeln, numerische Promotions, erweiterte Node Tests.
6. Performance: Iterator-Pipeline, Materialisierungs-Reduktion, Optimierungen.
7. Library & Errors Komplettierung: deep-equal, distinct-values, compare, Rest-Fehlercodes.
8. Optionale Erweiterungen: Schema Awareness, Streaming, Optimizer.

---
## Vollständige Aufgabenliste (ToDo)

(Spiegel der gepflegten strukturierten Projekt-ToDo-Liste – IDs stabil für Tracking.)

| ID | Kategorie | Aufgabe | Kurzbeschreibung |
|----|-----------|---------|-----------------|
| 1 | Semantik | Positional predicates | Numerisches Prädikat & position() Semantik |
| 2 | Semantik | last() support | last() mit Längen-Caching / Zwei-Pass |
| 3 | Achsen | Remaining axes | following, preceding (namespace optional) |
| 4 | Semantik | Node comparison ops | is, <<, >> |
| 5 | Vergleiche | General comparisons | = != < > <= >= Sequenzlogik |
| 6 | Vergleiche | Value comparisons | eq ne lt le gt ge Einzelwerte |
| 7 | Quantifizierer | Quantifier binding | some/every echte Bindung & Kurzschluss |
| 8 | Bindings | for expression | for $v in Expr return Expr |
| 9 | Bindings | let expression | let $v := Expr return Expr |
| 10 | Casting | cast/castable expressions | Parser & Evaluator + Fehler |
| 11 | Casting | Constructor functions | xs:typ("lexical") Aufrufe |
| 12 | Typing | Treat completion | * und + Cardinalities, Fehlerdetail |
| 13 | Typing | Numeric & untyped promotions | Promotion & untypedAtomic Regeln |
| 14 | Typing | Full atomization | fn:data korrekte Atomisierung |
| 15 | Typing | Extended node tests | element(*,T?), attribute(*,T?) |
| 16 | Infra | Function call dispatch | QName+Arity Registry |
| 17 | Funktionen | Sequence funcs core | count, exists, empty, subsequence, reverse, insert-before, remove |
| 18 | Funktionen | Numeric funcs | abs, ceiling, floor, round, round-half-to-even |
| 19 | Funktionen | Aggregate funcs | sum, avg, min, max (Promotion + Collation arg) |
| 20 | Funktionen | Core string funcs | string-length, normalize-space, substring, contains, starts-with, ends-with |
| 21 | Funktionen | Advanced string funcs | substring-before/after, translate |
| 22 | Funktionen | Regex functions | matches, replace, tokenize |
| 23 | Collations | Collation interface | Abstraktion + Codepoint + FOCH0002 |
| 24 | Date/Time | Current date/time | Snapshot für current-date/time/dateTime |
| 25 | Date/Time | Date/time arithmetic | Komponenten & Arithmetik |
| 26 | Date/Time | Duration handling | yearMonthDuration, dayTimeDuration |
| 27 | Funktionen | deep-equal & distinct-values | Implementierung + Tests |
| 28 | Funktionen | compare & codepoint-equal | Vergleichsfunktionen |
| 29 | QName | QName functions | resolve-QName, QName, Namespace-Kontext |
| 30 | Fehler/Debug | Error & trace funcs | fn:error, fn:trace |
| 31 | IO | Document access | fn:doc, fn:doc-available + Loader |
| 32 | Infra | Node order index | Dokumentenordnungs-Indizes |
| 33 | IR | IR positional opcodes | position()/last() Filter Ops |
| 34 | IR | IR compare & cast opcodes | General/Value/Cast/Castable/Konstruktor |
| 35 | IR | IR node compare opcodes | is / << / >> |
| 36 | Errors | Error code completion | Fehlende Codes & Tests |
| 37 | Performance | Iterator pipeline | Lazy Evaluator Infrastruktur |
| 38 | Performance | Materialization minimization | Puffern reduzieren |
| 39 | Tests | Comparison test matrix | General/Value Typkombinationen |
| 40 | Tests | Casting test matrix | Erfolg & Fehlerfälle systematisch |
| 41 | Tests | Date/time tests | Zeitzonen & Grenzen |
| 42 | Tests | Regex tests | Flags, Unicode Kanten |
| 43 | Performance | Benchmark harness | Criterion Benchmarks |

---
## Abhängigkeiten & Reihenfolge (Kurzform)

1 → 2 → 33 (Positionsfeatures)  
5 & 6 brauchen 13 (Promotion Regeln) teilweise; können rudimentär vorher.  
7 (Quantifier) unabhängig von 5/6, aber nutzt Atomisierung (14).  
10,11 benötigen 13 & Fehlercodes (36).  
24–26 brauchen Basis-Datums-/Durationtypen im Typenum (großteils vorhanden).  
23 vor 19/27/28 falls Collation-Param implementiert werden soll.  
37/38 nach stabiler Semantik (≥ Phase 5).  

---
## Design Notizen (Auszug)

- Position/last(): Iterator-Wrap mit (index, optional length). Länge nur bei Bedarf (erste Nachfrage nach last()).
- General vs Value: Typisierungs-Hilfsfunktion (normalize_for_comparison) + Sequenzpaar-Schleife mit Kurzschluss.
- Cast/Castable: Tabelle (Quelltyp → Zieltyp) + lexical parse; Fehlercodes FORG0001/0006, FOCA0001.
- Collations: Trait `Collation { fn compare(a,b)->Ordering; fn contains(hay,needle)->bool; }` Start: Codepoint.
- Date/Time Snapshot: DynContext hält eine `DateTimeSnapshot` einmalig pro Evaluation.
- Node Order: Preorder-Indexierung beim Dokument-Laden oder lazily on-demand (memo Map<NodeId, u64>). 
- Iterator Pipeline: Trait `XdmIter` -> adaptierende Kombinatoren (map, filter_position, flat_map). IR Ops erzeugen verkettete Iteratorgraphen.

---
## Teststrategie Kernpunkte

- Vergleichsmatrix: (numeric, string, untyped, boolean, date, duration) × Operatoren.
- Cast Matrix: Jede zulässige + gezielt unzulässige paarige Kombination.
- Quantifizierer: some/every (leer, 1 true, 1 false, gemischt, Kurzschluss). 
- Position/last(): Pfade mit [1], [last()], [position() < 3], kombinierte Filter.
- Regex: Verschiedene Flags, Unicode Klassen, Ersatz-Gruppen.
- Date/Time: Zeitzonen-Normalisierung, Tagesgrenzen, Dauerarithmetik.

---
## Definition "Fertig" (Non-Optional Kern)

Alle Tasks 1–36 + 39–42 erfüllt, Benchmarks (43) optional für 1.0.  
Keine NYI Fehler für abgedeckte Features.  
Fehlercodes vollständig dokumentiert & getestet.  
Performance: Grundbenchmark ohne Regression (Baseline nach Phase 3 etabliert).

---
## Nächste Schritte (Vorschlag)

Beginne mit 33 (IR positional opcodes) zusammen mit 1 & 2, danach 5/6 (Vergleiche) und 7 (Quantifier). Anschließend 10/11 (Cast/Castable/Konstruktor). Danach Funktionswelle (17–21).

(Die fortlaufende Bearbeitung erfolgt synchron mit der ToDo-Liste im Projekt – IDs konsistent halten.)
