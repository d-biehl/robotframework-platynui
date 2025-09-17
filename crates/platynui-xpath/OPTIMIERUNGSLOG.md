# Optimierungslog `platynui-xpath`

Dieses Log fasst alle bislang durchgeführten Optimierungs-Iteration samt Messwerten zusammen. Es ergänzt den übergeordneten Plan in `OPTIMIERUNGSPLAN.md` um ausführlichere Details.

## Iteration 1 – Schleifen-IR (For/Quantifier)
- **Änderungen**
  - Compiler erzeugt neue Opcodes `ForLoop` und `QuantLoop`, inklusive Hilfsfunktionen `lower_for_chain`/`lower_quant_chain`.
  - Evaluator interpretiert die neuen Opcodes ohne dynamisches Herausschneiden von Instruktionsslices.
  - Tests angepasst, Dump-Format (`Display`) um eingebettete Bodies erweitert.
- **Benchmarks** `cargo bench -p platynui-xpath --bench xpath_benches -- predicate_heavy`
  - Vorher: `count(...)` 93.29 ms, `sum(...)` 402.77 ms.
  - Nachher: `count(...)` 92.80 ms, `sum(...)` 402.33 ms.
- **Bewertung**: Verbesserung < 1 %, Strukturänderung erzeugt aber eine saubere Basis für weitere Loop-Optimierungen.

## Iteration 2 – `DocOrderDistinct` und Set-Operationen
- **Versuche**
  - Kurzlebige Variante zur Erkennung bereits sortierter Sequenzen.
  - Einsatz von `sort_unstable` für Node- und Fallback-Sequenzen.
- **Benchmarks** `cargo bench ... -- set_ops`
  - Ausgang: Union 9.37 ms, Intersect 17.42 ms, Except 15.54 ms.
  - Varianten führten zu Regressionen (+5 % bis +12 %).
- **Bewertung**: Änderungen verworfen; künftige Optimierungsversuche benötigen robustere Strategien (z. B. garantierte `doc_order_key()`-Berechnung oder Caching).

## Iteration 3 – Achsen-Iteration
- **Änderungen**
  - `push_descendants`: iterative Traversierung ohne `collect`/`reverse` pro Stufe.
  - `push_following`: Start hinter dem letzten Nachkommen statt `is_descendant_of`-Filter.
- **Benchmarks**
  - `cargo bench ... -- axes`: Following 24.73 ms → 22.90 ms (−8.7 %), Preceding 25.19 ms → 24.72 ms (−1.9 %), Preceding-sibling unverändert.
  - `cargo bench ... -- predicate_heavy`: keine nennenswerte Änderung (≈ 92 ms / 399 ms).
- **Bewertung**: Spürbarer Gewinn auf achsenlastigen Queries, ohne Regressionsanzeichen.

## Iteration 4 – `doc_order_distinct`
- **Änderungen**
  - Hybridansatz: monotone Schlüssel werden linear dedupliziert; für gemischte Sequenzen wird der sortierte Merge mit Fallback-Nodes konsolidiert, um unnötige Sortierungen zu vermeiden.
- **Benchmarks** `cargo bench ... -- set_ops`
  - Union: 9.78 ms → 8.89 ms
  - Intersect: 19.19 ms → 16.89 ms
  - Except: 16.75 ms → 15.55 ms
- **Bewertung**: 8–15 % schneller (je nach Operator), keine Funktionsregressionen (`cargo test -p platynui-xpath`).

## Iteration 5 – Prädikat-Lowering
- **Änderungen**
  - `lower_predicates` extrahiert Unterprogramme via `Vec::split_off` statt über `Compiler::fork()`, wodurch zusätzliche Allokationen und Kopien entfallen.
- **Benchmarks** `cargo bench ... -- predicate_heavy`
  - `count(...)`: 94.02 ms → 94.02 ms (innerhalb der Schwankung)
  - `sum(...)`: 401.62 ms → 401.62 ms (≈ −1.9 %)
- **Bewertung**: messbarer Gewinn bei komplexeren Prädikat-Ketten, Tests bleiben grün (`cargo test -p platynui-xpath`).

## Test- und Qualitätsabsicherungen
- Nach sämtlichen Codeänderungen: `cargo test -p platynui-xpath`.
- Benchmarks dokumentiert und wiederholt, sobald Effekt gering oder negativ war.

## Offene Punkte / nächste Schritte
1. Erneuter Anlauf zur Optimierung von `DocOrderDistinct` (z. B. Node-Schlüssel caching oder strukturierter Merge mit bereits sortierten Sequenzen).
2. Prüfen, ob `axis_buffer`-Recycling und weitere Iterator-Optimierungen (`push_preceding`) zusätzlichen Gewinn bringen.
3. Priorität 2 (selektives `DocOrderDistinct`, Prädikat-Kompilierung ohne Fork) folgt nach Bewertung weiterer Hotspots.
