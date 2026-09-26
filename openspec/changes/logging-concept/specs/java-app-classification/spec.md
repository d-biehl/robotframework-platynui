# Spec Delta

## MODIFIED Requirements

### Requirement: Cross-platform enablement diagnostic
The Windows-only `SunAwt`-suspect warning SHALL be generalized into a single "JVM window absent from native accessibility" diagnostic, emitted at most once per window, naming the actionable enablement path for the detected toolkit/platform (and, once available, the agent provider). The JAB provider SHALL emit this shared diagnostic instead of its own. When the Access Bridge client DLL itself is missing, the Java provider SHALL instead warn once per process for all such windows, naming `providers.java.jab.dll_path`, `PLATYNUI_JAB_DLL` and a 64-bit JDK; the launch flag / `jabswitch` hint stays for a bridge that is installed but not enabled. A window that the in-JVM agent serves is not absent and SHALL NOT be reported.

#### Scenario: Bridge-less Swing window yields the actionable diagnostic once
- **WHEN** a JVM-backed Swing window is detected on Windows but is not reachable through native accessibility
- **THEN** the shared diagnostic fires once for that window, naming how to enable accessibility (the launch flag / `jabswitch`), and does not repeat on subsequent enumeration passes

#### Scenario: A missing Access Bridge DLL is reported once per process
- **GIVEN** Windows without a discoverable Access Bridge client DLL
- **WHEN** Swing windows are found that no Java backend serves
- **THEN** one warning per process SHALL name `providers.java.jab.dll_path`, `PLATYNUI_JAB_DLL` and a 64-bit JDK, instead of the `jabswitch` hint
- **AND** a Swing window that the in-JVM agent serves SHALL NOT be reported
- **NOTE** Windows only.
