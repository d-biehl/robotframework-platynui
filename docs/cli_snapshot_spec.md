# CLI `snapshot` – XML Tree Export

Ziel: Teilbäume des UI‑Modells als XML exportieren, so dass externe XPath‑Parser mit denselben Präfixen/Namensräumen identische Abfragen ausführen können. Der Export ist streaming‑basiert, unterstützt Attribut‑Filter, Alias‑Attribute und Tiefenbegrenzung sowie Mehrfach‑Wurzeln.

## Kurzüberblick
- Befehl: `platynui-cli snapshot`
- Eingabe: XPath‑Ausdruck bestimmt 1..N Wurzel‑Knoten.
- Ausgabe (Standard): Lesbarer Text‑Baum auf stdout (kann umgeleitet oder via `--output` in Datei geschrieben werden).
- Ausgabe (XML): Nur mit `--format xml`; siehe XML‑Modell unten.
- Elementname = Rolle (local name), Präfix = Knoten‑Namespace.
- Attribute als XML‑Attribute mit Namespace‑Präfix; komplexe Werte (Rect/Point/Size/Array/Object) als JSON‑String.
- Kinder werden in Dokumentreihenfolge als Kindelemente geschrieben.

## Aufruf

```
platynui-cli snapshot <XPATH>
  [--output FILE | --split PREFIX]
  [--max-depth N]
  [--attrs default|all|list]
  [--include NAME...]
  [--exclude NAME...]
  [--exclude-derived]
  [--include-runtime-id]
  [--pretty]
  [--format text|xml]
  [--no-attrs]
  [--no-color]
```

### Optionen
- `<XPATH>`: XPath‑Ausdruck, der die Snapshot‑Wurzeln selektiert.
- `--output FILE`: schreibt die Ausgabe in eine Datei (Text oder XML je nach `--format`).
- `--split PREFIX`: erzeugt pro Wurzel eine Datei `PREFIX-001.txt` (Text) oder `PREFIX-001.xml` (XML). Kein Wrapper.
- `--max-depth N`: begrenzt die Tiefe (0 = nur Wurzel; 1 = +Kinder; …). Standard: unbeschränkt.
- `--attrs`:
  - `default`: nur `control:Name` und – falls vorhanden – `control:Id`.
  - `all`: alle Attribute des Knotens.
  - `list`: nur die via `--include` angegebenen Attribute (optional durch `--exclude` einschränkbar).
- `--include NAME`: zusätzliche Attribute aufnehmen; mehrfach möglich. Muster `ns:Attr` mit Wildcards, z. B. `control:Bounds*`, `app:*`.
- `--exclude NAME`: Attribute mit Muster ausschließen; mehrfach möglich.
- `--exclude-derived`: unterdrückt abgeleitete Alias‑Attribute (z. B. `control:Bounds.X/Y/Width/Height`, `control:ActivationPoint.X/Y`). Standard: Alias‑Attribute werden erzeugt, wenn das zugrundeliegende Basis‑Attribut im Set ist.
- `--include-runtime-id`: fügt `control:RuntimeId` zum Attributsatz hinzu (praktisch für spätere Re‑Resolution).
- `--pretty`: formatiert lesbar (Einrückung/Zeilenumbrüche). Gilt für Text und XML.
- `--format`: `text` (Default) oder `xml`. Ohne `--format xml` wird niemals XML auf stdout ausgegeben.
- `--no-attrs`: unterdrückt Attributzeilen im Text‑Baum (nur Struktur). Hat keinen Einfluss auf XML.
- `--no-color`: deaktiviert ANSI‑Farben in der Textausgabe.

## XML‑Modell (nur `--format xml`)
- Namespaces (fix):
  - `xmlns:control = "urn:platynui:control"`
  - `xmlns:item    = "urn:platynui:item"`
  - `xmlns:app     = "urn:platynui:app"`
  - `xmlns:native  = "urn:platynui:native"`
- Elementname = Rolle, Präfix = Knoten‑Namespace.
- Attribute als XML‑Attribute, Präfix = Attribut‑Namespace. Werte:
  - Primitive (`Null/Bool/Integer/Number/String`) → XML‑Attributwert.
  - Komplex (Rect/Point/Size/Array/Object) → JSON‑String (identisch zur Evaluator‑Konvertierung).
- Kinder: UI‑Kinder in Dokumentreihenfolge als Kindelemente.
- Mehrere Wurzeln:
  - `--output`: Wrapper `<snapshot>` mit Namespace‑Deklarationen und Metadaten (optional), darunter Root‑Elemente.
  - `--split`: je Root ein vollständiges Dokument ohne Wrapper.

## Attribut‑Selektion
1) Basismenge über `--attrs`:
   - `default`: `control:Name` (+ optional `control:Id`).
   - `all`: alle.
   - `list`: leer, bis `--include` greift.
2) `--include`/`--exclude` verfeinern mit Namespace‑qualifizierten Mustern (Wildcards `*`).
3) Alias‑Attribute:
   - Standard: erzeugen (`Bounds.*`, `ActivationPoint.*`), wenn das Basisattribut enthalten ist.
   - `--exclude-derived`: Alias‑Attribute weglassen.
4) `--include-runtime-id`: `control:RuntimeId` erzwingen.

## Verhalten & Kantenfälle
- Leere Query → Exit mit klarer Meldung (kein XML).
- Dokumentreihenfolge bleibt erhalten (keine zusätzliche Sortierung).
- Sehr große Bäume: Streaming‑Ausgabe (keine komplette Materialisierung im Speicher).

## Beispiele
- Alle Fenster als lesbarer Text‑Baum (Standardformat):
  ```
  platynui-cli snapshot "//control:Window" --max-depth 2 --pretty
  ```
- Alle Fenster mit vollem Baum und allen Attributen als XML (inkl. Alias), formatiert:
  ```
  platynui-cli snapshot "//control:Window" --attrs all --pretty --format xml --output windows.xml
  ```
- Nur Bounds und Enabled, ohne Alias‑Attribute:
  ```
  platynui-cli snapshot "//control:Button" --attrs list \
      --include control:Bounds --include control:IsEnabled --exclude-derived
  ```
- Begrenzte Tiefe und Einzeldokumente pro Root:
  ```
  platynui-cli snapshot "//control:Window[@Name='Calc']" --max-depth 2 --split calc
  ```

## Nicht‑Ziele (v1)
- YAML‑Ausgabe (kann später folgen).
- Deduplizierung bei überlappenden Treffern (jeder Root → eigener Teilbaum).

## Implementierungsnotizen
- Writer: `quick-xml` (Streaming). Kein globales Pre‑Sortieren.
- Typ‑Konvertierung: `UiValue` → String; komplexe Werte via `serde_json`.
- Alias‑Ableitung aus bekannten Strukturen: `Rect` → `.X/.Y/.Width/.Height`, `Point` → `.X/.Y`.

## Test‑Skizze
- Mock‑Tree: Default‑Snapshot (nur Name/Id), `--attrs all`, `--include/--exclude`, `--exclude-derived`, `--max-depth`, Multi‑Root (Wrapper vs. Split), `--include-runtime-id`, Pretty/Compact.
