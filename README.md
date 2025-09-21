# robotframework-PlatynUI

## Disclaimer

This project is still under development and should not be used productively **yet**.

At the current state expect:

- bugs
- missing features
- missing documentation

Feel free to contribute, create issues, provide documentation or test the implementation.

## Project Description

PlatynUI is a library for Robot Framework, providing a cross-platform solution for UI test automation. Its main goal is to make it easier for testers and developers to identify, interact with, and verify various UI elements.

We aim to provide a Robot Framework-first library.

### Documentation

- Architektur- und Runtime-Konzept: `docs/architekturkonzept_runtime.md`
- Umsetzungsschritte: `docs/umsetzungsplan.md`
- Pattern-Katalog (Trait-basierte Fähigkeiten, Koordinatenrichtlinien, Mappings): `docs/patterns.md`

Alle Dokumente sind lebende Entwürfe – wir aktualisieren sie parallel zur Implementierung.

### Workspace Layout

- `crates/core`: Gemeinsame Datentypen (UiNode, Attribute-Keys, Pattern-Basistypen).
- `crates/xpath`: XPath-Evaluator und Parser-Hilfen für PlatynUI.
- `crates/runtime` (`platynui-runtime`): Orchestrierung von Providern, Geräten, Window Manager und XPath.
- `crates/server` (`platynui-server`): JSON-RPC-Server-Fassade über der Runtime.
- `crates/platform-*` (`platynui-platform-*`): Plattformnahe Gerätetreiber und Window-Management (Windows, Linux/X11, macOS, Mock).
- `crates/provider-*` (`platynui-provider-*`): UiTreeProvider-Implementierungen (UIAutomation, AT-SPI, macOS AX, JSON-RPC, Mock).
- `crates/cli` (`platynui-cli`): Kommandozeilenwerkzeug für XPath-Abfragen, Highlighting und Diagnosen.
- `apps/inspector` (`platynui-inspector`): Geplante GUI-Anwendung zum Erkunden des UI-Baums und Entwerfen von XPath-Ausdrücken.

### Why PlatynUI?

- Cross-platform capability with consistent API across Windows, Linux, and MacOS
- Direct access to native UI elements
- Simplified element identification
- Builtin ui spy tool

## Testable Frameworks

- **Linux**
  - X11
  - AT-SPI2
- **Windows**
  - Microsoft UI Automation (UIA)
- **MacOS**
  - Accessibility API

Extendable for any other ui technologies.
