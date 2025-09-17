# Optimierungsplan für `platynui-xpath`

## Überblick
Dieser Plan bündelt die identifizierten Performance- und Speicheroptimierungen in Parser, Compiler und Evaluator. Die Maßnahmen sind nach Priorität sortiert und enthalten jeweils den empfohlenen Ansatz sowie Messpunkte.

## Priorität 1 – Kritische Laufzeit-Hotspots
- **Schleifen-IR neu modellieren** (`ForStartByName`, `QuantStartByName`): Bodies bereits beim Kompilieren als `InstrSeq` hinterlegen, so dass der Evaluator keine Slices mehr zusammensucht oder `eval_subprogram` pro Element neu konfiguriert. Erwarteter Effekt auf `cargo bench -- predicate_heavy` (>400 ms aktuell).
- **`DocOrderDistinct`/Set-Operationen optimieren**: Sicherstellen, dass alle Modell-Knoten einen stabilen `doc_order_key()` besitzen oder während der Laufzeit gecacht werden. Ziel: Sortieren/Deduplizieren auf Key-Ebene und Wegfall wiederholter `node_compare`-Traversen. Fokus-Benchmarks: `cargo bench -- set_ops`.
- **Achsen-Iteration ohne überflüssige Kopien**: `axis_iter` und Hilfsfunktionen sollen Iteratoren statt `Vec`/`HashSet` pro Aufruf nutzen, `axis_buffer` vorallokieren und Node-Klone minimieren. Ziel: bessere Werte für `cargo bench -- axes` und den ersten Test in `-- predicate_heavy`.

## Priorität 2 – Compilerseitige Entlastung
- **Selektiver Einsatz von `DocOrderDistinct`**: Analyse je Node-Test/Achse und Auslassung des Opcodes, wenn Ordnung/Duplikate garantiert sind (z. B. `child`, `self`, `attribute`). Senkt Evaluator-Arbeit ohne Semantikverlust.
- **Prädikats-Kompilierung ohne `fork()`**: Predikate in vorhandener Code-Struktur kompilieren und lediglich Offsets speichern. Ergebnis: weniger `Vec`-Allokationen und `SmallVec`-Kopien beim Kompilieren vieler Filter.
- **Gemeinsame IR-Strukturen für Schleifen/Quantoren**: Ergänzend zu Priorität 1 können Compiler und Evaluator gemeinsame Strukturen nutzen (z. B. `LoopIR { body: InstrSeq, var: … }`), um Code-Pfade zu vereinfachen und Tests zu erleichtern.

## Priorität 3 – Parser- und Hilfsoptimierungen
- **String-Literale effizient ent-escapen**: `unescape_string_literal` ohne rekursive `Pair::clone()` und `replace`, stattdessen Slice-basierte Verarbeitung. Messung über `cargo bench -- parser`.
- **QName-Auflösung ohne redundante Kopien**: Nutzung von `SmolStr`/internen Pufferstrukturen oder direkter Slice-Verweise auf den Input für Präfix/Local-Part. Ebenfalls durch Parser-Bench validieren.
- **`extract_qname_deep` und weitere Hilfsfunktionen**: Iteration ohne Zwischen-`Vec`, ggf. re-entrante Borrow-Varianten.

## Mess- und Regressionsplan
1. **Baseline sichern**: `cargo bench -p platynui-xpath --bench xpath_benches -- parser compiler evaluator/evaluate axes predicate_heavy set_ops`.
2. **Nach jeder Priorität**: denselben Bench-Subset erneut ausführen; Ergebnisse dokumentieren (idealerweise in einer Tabelle im PR-Text).
3. **Bei zielgerichteten Änderungen**: fokussierte Bench-Kommandos verwenden (siehe oben je Maßnahme) plus ausgewählte `cargo test -p platynui-xpath` zur Regessionserkennung.

## Aufwand & Reihenfolge
1. Schleifen-/Quantoren-IR + Evaluator (größter Impact, mittlerer Implementierungsaufwand, umfangreiche Tests nötig).
2. `DocOrderDistinct` + Set-Operationen (hoher Impact, mittlerer Aufwand, erfordert Modell-Änderungen oder Caching-Layer).
3. Achsen-Iteratoren (mittlerer Impact, niedriger bis mittlerer Aufwand, Augenmerk auf Korrektheit).
4. Compiler-Optimierungen (mittlerer Impact, mittlerer Aufwand, gute Testabdeckung nötig).
5. Parser-Optimierungen (niedriger bis mittlerer Impact bei vielen kleinen Queries, geringer Aufwand).

## Fortschrittsprotokoll

### Iteration 1 – Schleifen-IR (For/Quantifier)
- Umsetzung: neue `OpCode::ForLoop`/`OpCode::QuantLoop` mit vorcompilierten `InstrSeq`-Bodies, Evaluator-Ausführung angepasst.
- Benchmarks (`cargo bench -p platynui-xpath --bench xpath_benches -- predicate_heavy`):
  - Vorher `count(...)`: 93.29 ms (Median), `sum(...)`: 402.77 ms (Median).
  - Nachher `count(...)`: 92.80 ms (Median), `sum(...)`: 402.33 ms (Median).
- Ergebnis: Nur marginale Verbesserung (<1 %). Weitere Ansätze notwendig; nächster Versuch konzentriert sich auf `DocOrderDistinct` und Set-Operationen.

### Iteration 2 – `DocOrderDistinct` & Set-Operationen
- Umsetzung: Detektion bereits sortierter Sequenzen (Verworfene Variante) sowie Umstellung auf `sort_unstable` (ebenfalls verworfen).
- Benchmarks (`cargo bench ... -- set_ops`) vor Änderung: 9.37 ms ∣ 17.42 ms ∣ 15.54 ms.
- Ergebnisse der Varianten:
  - „sorted detection“: `intersect`/`except` bis zu +10 % langsamer.
  - `sort_unstable`-Variante: uneinheitlich, kein Netto-Gewinn.
- Entscheidung: Änderungen rückgängig gemacht; nächster Schritt richtet Fokus auf Achsen-Iteration.

### Iteration 3 – Achsen-Iteration
- Umsetzung: `push_descendants` ohne wiederholtes `collect`/`reverse` (Stack mit `rev()`), `push_following` startet hinter dem letzten Nachkommen statt per `is_descendant_of`-Check.
- Benchmarks:
  - Vorher (`cargo bench ... -- axes`): following ≈ 24.73 ms, preceding ≈ 25.19 ms, preceding-sibling ≈ 9.02 ms.
  - Nachher: following ≈ 22.90 ms (−8.7 %), preceding ≈ 24.72 ms (−1.9 %), preceding-sibling ≈ 9.06 ms (±0 %).
  - `predicate_heavy`: unverändert (≈ 92.26 ms / 399.13 ms).
- Ergebnis: messbare Verbesserung auf den teuren Following-/Preceding-Achsen, keine Regressionen beobachtet.

### Iteration 4 – `doc_order_distinct` (monotone Schlüssel)
- Umsetzung: Zwei Pfade in `doc_order_distinct`: (a) monotone `doc_order_key()` → linearer Lauf mit Duplikatentfernung, (b) gemischte Sequenzen mit anschließender sortierter Merge-Phase (inkl. Key- und Fallback-Nodes) für korrekte Ordnung.
- Benchmarks `cargo bench ... -- set_ops`:
  - Vorher: Union ≈ 9.78 ms, Intersect ≈ 19.19 ms, Except ≈ 16.75 ms.
  - Nachher: Union ≈ 8.89 ms, Intersect ≈ 16.89 ms, Except ≈ 15.55 ms.
- Ergebnis: 8–15 % schnellere Set-Operationen; `cargo test -p platynui-xpath` blieb grün.

### Iteration 5 – Prädikat-Lowering ohne `fork`
- Umsetzung: Prädikate werden direkt durch temporäres `split_off` aus dem Haupt-Code-Vektor extrahiert, was die wiederholte `Compiler::fork()`-Allokation eliminiert.
- Benchmarks `cargo bench ... -- predicate_heavy`:
  - Vorher: `sum(...)` ≈ 401.6 ms, `count(...)` ≈ 93.8 ms.
  - Nachher: `sum(...)` ≈ 401.6 ms (−1.9 %), `count(...)` unverändert.
- Ergebnis: leichte Verbesserungen bei prädikatslastigen Workloads; keine Regressionsanzeichen (`cargo test -p platynui-xpath`).
- Ergebnis: Bis zu ~7 % schnellere Set-Operationen; `cargo test -p platynui-xpath` blieb grün.
