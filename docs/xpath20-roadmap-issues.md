# XPath 2.0 Evaluator – Roadmap Issues (nach Milestones)

Quelle: docs/xpath20-evaluator-plan.md (Draft 5) und docs/xpath20-evaluator-matrix.md

Konventionen:
- Präfix [Mx] entspricht Milestone aus dem Plan.
- Jeder Issue‑Block enthält Zweck, Umfang, Schritte, Akzeptanzkriterien und Code-/Testverweise.

---

## [M8b] Date/Time/Duration – XDM‑Typen, Parsing/Format, Arithmetik

Title: Implement XDM date/time/duration types and full Date/Time F&O subset

- Zweck: Vervollständigen der Datum/Zeit/Dauer‑Unterstützung (XPath 2.0 / F&O), über current-* hinaus.
- Umfang:
  - XDM‑Typen ergänzen: `xs:date`, `xs:time`, `xs:dateTime`, `xs:dayTimeDuration`, `xs:yearMonthDuration`.
  - Parsing/Format (ISO‑8601): stabile Offset‑Formatierung; Fehlercodes für ungültige Literale.
  - Komponentenfunktionen (Auszug): `year-from-dateTime`, `month-from-dateTime`, `day-from-dateTime`, `hours-from-time`, `minutes-from-time`, `seconds-from-time`, `timezone-from-dateTime|date|time`.
  - Arithmetik: Addition/Subtraktion von Durations/DateTime gemäß F&O; Vergleichsoperatoren für Date/Time.
  - Kontext: `implicit-timezone()`; Nutzung von `with_now`/`with_timezone` bereits vorhanden.
- Schritte:
  1) XDM erweitern: neue Varianten in `XdmAtomicValue` + Stringdarstellung/Parsing‑Helper.
  2) Evaluator‑Vergleiche/Cast aktualisieren (numerisch vs. temporal; Fehlermappings).
  3) Functions: Implementierung der Komponenten‑ und Arithmetik‑Funktionen; Erweiterung `functions.rs`.
  4) Tests: Deterministische Fälle mit `with_now`/`with_timezone`; Negativfälle (ungültige Literale/Operationen).
- Akzeptanzkriterien: (erfüllt)
  - `current-dateTime|date|time` grün; Komponenten/Timezone/Arithmetik implementiert und getestet (inkl. Edges/Negativfälle).
  - Ungültige Eingaben → spezkonforme Fehler (z. B. `FORG0001`, `FOAR0001`).
  - Parser unverändert (Literals als Strings); Casts/Functions verarbeiten Datums-/Zeitstrings.
  - Bekannte Einschränkungen dokumentiert (tz‑lose `xs:dateTime`, Fraktionssekunden‑Ausgabe von `xs:time`, `deep-equal` string‑basiert).
- Status: abgeschlossen
- Code:
  - `crates/platynui-xpath/src/xdm.rs`, `evaluator.rs` (Vergleich/Cast/Arithmetik), `functions.rs` (Date/Time Familie)
- Tests:
  - `tests/functions_datetime.rs`, `tests/functions_datetime_components.rs`, `tests/functions_datetime_arith.rs`, `tests/functions_datetime_edges.rs`, `tests/evaluator_comparisons_temporal.rs`, `tests/evaluator_predicates_temporal.rs`

---

## [M9] Kontrollfluss & Bindungen – if / some / every / for / let

Title: Compile/Evaluate control-flow (if) and quantified/FLWOR subsets

- Zweck: XPath 2.0 Kontrollfluss und Bindungen vollständig unterstützen.
- Umfang:
  - `if (Expr) then Expr else Expr` (ohne neues OpCode, über Jumps oder mit `IfElse` + Evaluator‑Support).
  - Quantifizierte Ausdrücke: `some $x in S satisfies P`, `every $x in S satisfies P`.
  - FLWOR‑Subset: `for $x in S return E`, `let $x := E return F` (einfacher Scope ohne Order/where/group).
- Schritte:
  1) Compiler: `if_expr`, `quantified_expr`, `for_expr` in `compiler.rs` kompilieren (Lowering auf vorhandene Jumps/Stack‑Konvention oder Implementierung `IfElse`/`Some`/`Every`/`For*`/`LetBind`).
  2) Evaluator: fehlende OpCodes implementieren oder die kompilierten Jump‑Sequenzen interpretieren.
  3) Scoping/Variablen: vereinfachte Slot/Name‑Bindungen via `ExpandedName`; keine Shadowing‑Optimierung nötig.
  4) Tests: Parser‑→IR‑Unit‑Tests; E2E‑Fälle (z. B. some/every mit leeren/nichtleeren Sequenzen; for‑Mapping über Sequenz; let‑Binding).
- Akzeptanzkriterien:
  - `if` short‑circuit gemäß EBV; korrektes Then/Else.
  - `some` true, wenn mindestens ein Item EBV‑true; `every` entsprechend.
  - `for` bildet Sequenz über `return` korrekt ab; `let` bindet einmalig.
- Code:
  - `crates/platynui-xpath/src/compiler.rs`, `evaluator.rs`
- Tests:
  - Neue Suiten: `tests/evaluator_if.rs`, `tests/evaluator_quantified.rs`, `tests/evaluator_flwor.rs`

---

## [M10] Node/QName/Namespace & URI – F&O Funktionen

Title: Implement node/qname/namespace and base/resolve-uri functions

- Zweck: F&O‑API für Node- und Namenfunktionen sowie URI‑Auflösung ergänzen.
- Umfang (Auszug):
  - Node/QName: `name()`, `local-name()`, `namespace-uri()`, `node-name()`, `namespace-uri-for-prefix()`, `in-scope-prefixes()` (sofern anwendbar), `QName()`, `resolve-QName()`, `prefix-from-QName()`, `local-name-from-QName()`, `namespace-uri-from-QName()`.
  - URI: `base-uri()`, `resolve-uri($relative as xs:string?, $base as xs:string?)` mit Fallback auf `StaticContext.base_uri`/`XdmNode.base_uri()`.
- Schritte:
  1) `functions.rs`: Implementierungen; Zugriff auf `CallCtx` (Static/Dynamic Context, Node‑Name/NS via `XdmNode`).
  2) Hilfen in `model.rs` (falls nötig) für `base_uri()`; Standard‑Default bleibt None.
  3) Tests: Namespaces/Prefixes in `evaluator_namespaces.rs` erweitern; neue Datei `tests/functions_node_qname.rs`, `tests/functions_uri.rs`.
- Akzeptanzkriterien:
  - Ergebnisse entsprechen F&O; Default‑NS/Attribut‑NS‑Spezialfälle abgedeckt.
  - `resolve-uri` deckt leere/absolute/relative Fälle ab.
- Code: `functions.rs`, `model.rs`, `runtime.rs` (Base‑URI vorhanden)

---

## [M11] Ressourcen – doc / doc-available / collection

Title: Implement resource functions over ResourceResolver

- Zweck: Dokument‑/Sammlungszugriff abstrahiert über `ResourceResolver`.
- Umfang:
  - `fn:doc($uri as xs:string) as document-node()?`
  - `fn:doc-available($uri as xs:string) as xs:boolean`
  - `fn:collection(($uri as xs:string)?) as node()*`
  - Fehlerverhalten gemäß Spezifikation (unbekannte Ressource/URI → definierte Codes, z. B. `FODC0005`/`FODC0002` nach F&O 2.0; konsistent zur internen `Error`).
- Schritte:
  1) `functions.rs`: thin wrappers, die `CallCtx.resolver` nutzen; Fehlercodes mappen.
  2) Beispiel‑Resolver (Tests) implementieren; realer Resolver bleibt Plug‑in.
  3) Tests: Positive/Negative Fälle; leere Sammlung; ungültige URIs.
- Akzeptanzkriterien:
  - Resolver‑freie Konfiguration führt zu definiertem Fehler (oder leer je Funktion, s. Spez); bekannte URIs liefern Daten.
- Code: `runtime.rs` (Trait vorhanden), `functions.rs`

---

## [M12] TypeRegistry – Basistypen & Delegation

Title: Add pluggable TypeRegistry and delegate atomic typing

- Zweck: Atomare Typ‑Delegation/Validierung; Schema‑Erweiterbarkeit ohne XML‑Kopplung.
- Umfang:
  - Trait `TypeRegistry` + Default‑Registry (XPath 2.0 Basistypen inkl. QName, anyURI, untypedAtomic; optional Date/Time/Duration via Feature).
  - `StaticContext` trägt `Arc<TypeRegistry>`; Compiler validiert bekannte Typ‑QNames optional (`XPST0017`).
  - Evaluator delegiert `cast/castable/instance of/treat as` für atomare Typen.
- Schritte:
  1) Trait/Default‑Registry implementieren; Versionierung/Fingerprint aufnehmen.
  2) `StaticContext` erweitern; Fingerprint in späterem Cache nutzen.
  3) Compiler/Evaluator‑Hooks aktivieren; Tests anpassen.
- Akzeptanzkriterien:
  - Bestehende Typ‑Tests bleiben grün; Unbekannte Typen schlagen wahlweise statisch (`XPST0017`) fehl, sonst dynamisch.
- Code: `runtime.rs` (StaticContext), `evaluator.rs` (Delegation), `compiler.rs` (statische Checks)

---

## [M13] Caching & Optimierungen

Title: Implement XPathExecutableCache and light IR optimizations

- Zweck: Performance und Wiederverwendung kompilierter Ausdrücke.
- Umfang:
  - `XPathExecutableCache` (LRU/HashMap) key = `expr + static_ctx_fingerprint`.
  - Fingerprint umfasst: Namespaces, Default‑Funktions‑NS, Base‑URI, Default‑Collation (URI), Function‑Registry‑Version, Collation‑Registry‑Version, Type‑Registry‑Version.
  - Einfache Optimierungen: Konstantfaltung, DCE, toposort‑freie Peepholes.
- Schritte:
  1) Cache‑Struktur + API; optional Feature‑Flag.
  2) Fingerprint‑Berechnung implementieren.
  3) Einfache Optimierungs‑Pass in Compiler (optional feature‑gated).
  4) Performance‑Tests/Samples.
- Akzeptanzkriterien:
  - Cache‑Hit reduziert Kompilierzeit; Semantik unverändert.
  - Optimierungen ändern Ergebnisse nicht; Tests bleiben grün.
- Code: `runtime.rs` (Fingerprints), `compiler.rs` (Opt‑Pass), neuer Cache‑Modul
- Tests: `tests/perf_*` (feature‑gated), Vergleich Läufe mit/ohne Cache

---

## [M14] Conformance & Qualität

Title: Build conformance matrices and consolidate diagnostics

- Zweck: Spez‑Konformität nachweisen und Kantenfälle dokumentieren.
- Umfang:
  - F&O‑Conformance‑Matrix: Regex (Flags/Unicode‑Properties; Abweichungen), Date/Time (Komponenten/Arithmetik), Collations (URIs, Case/Accent, Gleichheit/Ordnung).
  - Fehlercode‑Konsolidierung: Mapping auf W3C‑Codes konsistent, interne `Error` harmonisieren.
  - Doku: Adapter‑Guidelines/Beispiele; README/Docs ergänzen.
- Schritte:
  1) Testsuiten je Familie strukturieren (basic/edge/negative) und dokumentieren.
  2) Fehlercodes sichten; Vereinheitlichung in `runtime::Error`.
  3) Developer‑Docs erweitern (Regex‑Abweichungen, Collation‑Strategien, Multi‑Root‑Hinweise).
- Akzeptanzkriterien:
  - Matrizen vorhanden; alle geplanten Fälle abgedeckt; Negativfälle prüfen `FORX0002`, `FOCH0002`, etc.
  - Dokumentation aktuell; Beispiele kompilieren/laufen.
- Code/Tests: `tests/functions_*`, `tests/evaluator_*`, `docs/`
