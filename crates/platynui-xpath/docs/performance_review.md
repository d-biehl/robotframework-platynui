# platynui-xpath Leistungsanalyse

## Umfang
- Code-Durchsicht von `crates/platynui-xpath` auf Basis des aktuellen Workspace-Standes.
- Fokus auf Evaluator, Verwaltung des Laufzeitkontexts, Funktionsregister und zugehoerige Datenstrukturen.
- Ziel: Performance- und Speicherrisiken sichtbar machen und pragmatische Verbesserungen skizzieren.

## Zusammenfassung
- Die Hot-Paths der Ausfuehrung klonen fuer jeden Praedikatdurchlauf, jeden Pfadschritt und jeden Iteratorkoerper den kompletten `DynamicContext`. Das fuehrt zu quadratischem Aufwand und vielen kurzlebigen Allokationen.
- Achsen-Iteratoren fuer `following`/`preceding` bauen pro Knoten eine komplette Dokumenttraversierung neu auf und erzeugen frische Vektoren; das resultiert in wiederholten O(n^2)-Durchlaeufen durch den Baum.
- Mengenoperatoren (`union`, `intersect`, `except`) werden quadratisch, weil jeder Mitgliedertest komplette Sequenzen linear durchsucht. Dadurch skalieren Ausdruecke fuer mittlere Dokumentgroessen kaum.
- Die Normalisierung der Dokumentordnung sortiert mit einem Komparator, der fuer jeden Vergleich komplette Ahnenketten rekonstruiert. Das verstaerkt die Kosten der ohnehin teuren Sortierungen und Mengenoperationen.
- Regex-Funktionen kompilieren Muster bei jedem Aufruf neu und vergeben damit deutliche Laufzeitgewinne in typischen XPath-Workloads.

## Detaillierte Erkenntnisse

### 1. DynamicContext-Kloning in Hot Paths des Evaluators
- **Schweregrad**: Hoch
- **Ort**: `crates/platynui-xpath/src/engine/evaluator.rs:149`, `crates/platynui-xpath/src/engine/evaluator.rs:201`, `crates/platynui-xpath/src/engine/evaluator.rs:220`, `crates/platynui-xpath/src/engine/evaluator.rs:1118`, `crates/platynui-xpath/src/engine/evaluator.rs:1187`, `crates/platynui-xpath/src/engine/runtime.rs:928`
- **Problem**: Jeder Praedikatdurchlauf, Pfadschritt, `for`-Loop und Quantor klont den kompletten `DynamicContext`. Das abgeleitete `Clone` kopiert `HashMap<ExpandedName, XdmSequence<N>>` und dupliziert damit Sequenzen und Variablenbindungen, obwohl meist nur `context_item` oder eine einzelne Variable angepasst wird.
- **Auswirkung**: Laufzeit und Speicherverbrauch wachsen mit der Sequenzlaenge quadratisch; umfangreiche Kontexte bezahlen O(n^2) Klon-Kosten und erzeugen viele kurzlebige Objekte. Variablenreiche Kontexte verlieren zusaetzlich Caching-Vorteile.
- **Empfehlung**: `Vm` so umbauen, dass ein geteilter `DynamicContext` nur geliehen wird und pro Ausfuehrungsrahmen ein leichtgewichtiger, mutierbarer Ueberlagerungs-Frame kontextbezogene Werte haelt. Optionen: `DynamicContext` in einen gemeinsamen `Arc`-Teil (`Arc<HashMap<...>>`) plus kleinen Frame aufspalten oder pro Frame Overlays mit Abweichungen gegen die Basismap speichern.

### 2. Neuaufbau des Standard-Funktionsregisters bei jeder Ausfuehrung
- **Schweregrad**: Mittel-Hoch
- **Ort**: `crates/platynui-xpath/src/engine/evaluator.rs:60`, `crates/platynui-xpath/src/engine/runtime.rs:1032`, `crates/platynui-xpath/src/engine/functions/mod.rs:902-921`
- **Problem**: Wenn ein `DynamicContext` ohne eigenes Funktionsregister erstellt wird, ruft `provide_functions()` bei jedem Start einer VM `default_function_registry()` auf. Diese Routine legt ein neues `FunctionImplementations` an und fuehrt `register_default_functions()` aus, das ueber 100 Funktionen einzeln registriert. Jeder Aufruf erzeugt frische `ExpandedName`-Strings, `Arc`-Wrapper fuer Closures und sortiert die Ueberladungslisten (`FunctionImplementations::register_range`) erneut. Weil `Vm::new()` in Hot Paths (Praedikate, geschachtelte Pfade) aufgerufen wird, werden diese Kosten pro Iteration multipliziert.
- **Auswirkung**: Schon bei moderaten Ausdruecken gehen mehrere hundert Allokationen und HashMap-Operationen verloren, bevor die eigentliche XPath-Auswertung startet. Kombiniert mit dem Klonen des `DynamicContext` (Erkenntnis #1) entstehen identische Registerkopien, die sofort wieder verworfen werden. Das fuehrt zu messbarer Startlatenz pro VM und hohem Speicher-Traffic.
- **Empfehlung**: Das Standardregister einmalig in einem `OnceLock<Arc<FunctionImplementations<_>>>` initialisieren (analog zu `ensure_default_signatures()`) und in `provide_functions()` nur noch das `Arc` klonen. Wer ein individuelles Register benoetigt, kann es weiterhin explizit in den Kontext injizieren. Optional laesst sich der Cache erweitern, um pro Knotentyp (`N`) getrennte Instanzen vorzuhalten, solange die Registrierung rein generisch bleibt.

- **Status**: Umsetzung erfolgt – das Funktionsregister wird jetzt pro Knotentyp einmalig erzeugt, in einer `OnceLock`-gestuetzten Map zwischengespeichert und bei Bedarf als `Arc` geteilt.

### 3. Achsen-Auswertung traversiert Dokumente wiederholt komplett
- **Schweregrad**: Hoch
- **Ort**: `crates/platynui-xpath/src/engine/evaluator.rs:1830-1930`, `crates/platynui-xpath/src/engine/evaluator.rs:1934-1946`
- **Problem**: Die Achsen `following` und `preceding` steigen bis zur Wurzel auf und rufen `collect_descendants` auf, um fuer jeden Eingangsknoten alle Knoten des Dokuments zu materialisieren. `collect_descendants` rekursiert und klont jeden Kindvektor erneut.
- **Auswirkung**: Achsenschritte arbeiten pro Eingangsknoten mit O(T) (T = Baumgroesse). Praedikate oder geschachtelte Pfade treiben das Gesamtergebnis in Richtung O(T^2), inklusive grosser temporaerer Vektoren und tiefer Rekursion (Stack-Overflow-Risiko auf tiefen Baeumen).
- **Empfehlung**: Streaming-Iteratoren implementieren, die Geschwister- und Ahnenketten ohne vollstaendigen Neuaufbau durchlaufen. Dokumentordnungs-Indizes pro Knoten (z. B. beim Baumaufbau) cachen und damit arithmetisches Navigieren ermoeglichen. Minimal sollte eine einmal erzeugte Preorder-Traversierung pro Dokument wiederverwendet werden.

- **Status**: Teilloesung umgesetzt – following/preceding-Achsen laufen nun ueber Dokument-Nachbarn statt komplette Baeume aufzubauen; spaetere Arbeiten fuer globale Dokumentordnungs-Indizes bleiben offen.

### 4. Mengenoperationen arbeiten quadratisch
- **Schweregrad**: Hoch
- **Ort**: `crates/platynui-xpath/src/engine/evaluator.rs:2093-2123`
- **Problem**: `set_intersect` und `set_except` rufen `contains`, welches die komplette Gegensequenz mit `item_equal` linear durchsucht. `set_union` konkateniert und verlässt sich danach auf Sortieren (siehe Erkenntnis #5).
- **Auswirkung**: `union`/`intersect`/`except` auf Knotensequenzen verursachen O(n^2), zusaetzlich verstaerkt durch teure Dokumentordnungs-Vergleiche. Groessere Ergebnislisten werden praktisch unhandhabbar.
- **Empfehlung**: Temporare `HashSet`s aufbauen, die Knotensequenzen ueber Identitaet (Pointer/Handle) und atomare Werte ueber bestehende Equal-Keys abbilden. Das senkt Mitgliedstests auf O(1). Kombiniert mit vor-sortierten Dokumentordnungsmetadaten entfallen wiederholte Sortierungen.

### 5. Normalisierung der Dokumentordnung nutzt teuren Komparator
- **Schweregrad**: Mittel-Hoch
- **Ort**: `crates/platynui-xpath/src/engine/evaluator.rs:1777-1798`, `crates/platynui-xpath/src/model/mod.rs:19-98`, `crates/platynui-xpath/src/model/simple.rs:645-665`
- **Problem**: `doc_order_distinct` sortiert Knoten mit `compare_document_order`. Die Standardimplementierung baut fuer jeden Vergleich komplette Ahnenpfade (`Vec`-Allokationen und Klone) neu auf. Sortieren oder Deduplizieren von N Knoten verursacht O(N log N) Vergleiche, jeder davon mit O(Baumhoehe).
- **Auswirkung**: Zusammen mit Erkenntnissen #3 und #4 dominieren die Kosten der Dokumentordnungs-Pflege die Laufzeit bei mittleren Ergebnismengen.
- **Empfehlung**: Dokumentordnungs- bzw. Preorder-Indizes pro Knoten cachen (`SimpleNode` fuehrt bereits `doc_id`). Knoten um einen praekalkulierten Ordinalwert erweitern (z. B. `(doc_id, preorder, sibling)`), der einmal beim Baumaufbau entsteht und fuer Vergleiche, Deduplikation und Sortierung verwendet wird.

- **Status**: SimpleNode weist Dokumentknoten nun beim Build praekalkulierte Preorder-Indizes zu; `compare_document_order` nutzt diese Werte statt tiefer Ahnen-Scans.

### 6. Regex-Funktionen kompilieren Muster bei jedem Aufruf
- **Schweregrad**: Mittel
- **Ort**: `crates/platynui-xpath/src/engine/runtime.rs:325-374`
- **Problem**: `FancyRegexProvider` erzeugt fuer jeden Aufruf von `matches`, `replace` oder `tokenize` ein neues `Regex`, selbst bei identischem Muster/Flag-Kombination.
- **Auswirkung**: Regex-intensive XPath-Ausdruecke investieren den Grossteil der Zeit in die Kompilation statt ins Matching. Der Backtracking-Engine-Overhead verstaerkt den Effekt.
- **Empfehlung**: Kompilierte Regexe im Provider zwischenspeichern (z. B. `HashMap<(String, String), Arc<Regex>>` unter `RwLock` oder `dashmap`) und bei Bedarf mit Cache-Limits versehen.

## Weitere Beobachtungen
- `collect_descendants` rekursiert ohne Tiefenkontrolle (`crates/platynui-xpath/src/engine/evaluator.rs:1934-1946`); eine Iteration mit explizitem Stack verhindert Stack-Overflows auf tiefen Dokumenten.
- `SimpleNode::children`/`attributes` klonen jeweils den kompletten unterliegenden Vektor (`crates/platynui-xpath/src/model/simple.rs:592-604`) und verstaerken damit den Allokationsdruck der oben genannten Hotspots. Geteilte `Arc<Vec<_>>`-Slices oder Iteratoren wuerden helfen, sobald das API angepasst werden kann.

## Empfohlene naechste Schritte
## Umsetzungsreihenfolge
- **1 – DynamicContext refaktorieren** (Finding 1, Zeile 16): Klonen eliminieren, bevor andere Optimierungen greifen.
- **2 – Funktionsregister cachen** (Finding 2, Zeile 26): `OnceLock<Arc<_>>` fuer Standardfunktionen einbauen, senkt Latenz nach Refactor 1.
- **3 – Achsen streamingfaehig machen** (Finding 3, Zeile 35): Traversierung nach Kontext-/Cache-Bereinigung modernisieren.
- **4 – Dokumentordnungs-Indizes einfuehren** (Finding 5, Zeile 46): Stabiler Vergleich fuer Achsen und Mengen.
- **5 – Mengenoperationen hashen** (Finding 4, Zeile 41): Mit Indizes effizient, verringert quadratische Vergleiche.
- **6 – Regex-Caching implementieren** (Finding 6, Zeile 52): Feinschliff nach strukturellen Verbesserungen.
## Teststrategie
- Tests stets ueber `timeout --foreground 300` aufrufen (maximal 5 Minuten Laufzeit), z. B. `timeout --foreground 300 cargo test -p platynui-xpath`.
- Bei neuen Benchmarks oder langlaufenden Suites fruehzeitig Metriken sammeln, um Deadlocks/Endlosschleifen zu erkennen und den Timeout anzupassen.
- Hangenbleibende Jobs abbrechen und mit Logging/Profiling reproduzieren, bevor weitere Aenderungen gemerged werden.


1. Einen leichtgewichtigen Ausfuehrungsframe prototypisieren, der `context_item` und Variablen ohne Klonen des Basis-`DynamicContext` mutiert.
2. Dokumentordnungs-Metadaten (z. B. Preorder-Index) beim Baumaufbau einfuehren und Achsen- sowie Mengenoperationen darauf aufsetzen.
3. Ein `OnceLock<Arc<FunctionImplementations<_>>>`-Cache hinzufuegen und `DynamicContext::provide_functions` darauf verweisen lassen.
4. Memoisierung im `FancyRegexProvider` implementieren und die Wirkung auf vorhandene XPath-Tests messen.
5. Nach den strukturellen Aenderungen Criterion-Benchmarks fuer repraesentative XPath-Programme aufbauen, um Regressionen abzusichern.

