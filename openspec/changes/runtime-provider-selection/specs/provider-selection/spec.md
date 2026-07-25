## ADDED Requirements

### Requirement: Per-provider enablement
Every registered provider SHALL be individually enableable via `providers.<id>.enabled` (default true), resolved by the registry **before** the provider is constructed. A disabled provider SHALL perform no work whatsoever — no connection attempt, no client library or DLL probing, no enablement diagnostics — and SHALL contribute no nodes. Runtime construction SHALL NOT fail because a provider was disabled.

#### Scenario: Disabled provider is never constructed
- **WHEN** a runtime is built with `providers.<id>.enabled = false` for a provider that would otherwise be active
- **THEN** that provider contributes no nodes, performs no connection or library probing, and the runtime comes up normally

#### Scenario: Absent keys reproduce current behavior
- **WHEN** a runtime is built with no selection keys at all
- **THEN** every registered provider is active exactly as before this capability existed

### Requirement: Selection lists with explicit precedence
The `providers` bucket SHALL accept the reserved non-id keys `include` (allowlist) and `exclude` (blocklist). Resolution per provider id SHALL be: an explicit `providers.<id>.enabled` if present; otherwise membership in `include` when `include` is present (non-members off); otherwise `exclude` membership (members off); otherwise active. When both lists are present, `include` SHALL bound the set and `exclude` SHALL trim it.

#### Scenario: Allowlist restricts the session
- **WHEN** a runtime is built with `providers.include = ["java"]` on a host where other providers are also registered
- **THEN** only the Java provider is active and the others are neither constructed nor contribute nodes

#### Scenario: Explicit flag overrides a list
- **WHEN** `providers.include = ["java", "atspi"]` is combined with `providers.atspi.enabled = false`
- **THEN** only the Java provider is active — the explicit per-id flag wins over the list

### Requirement: Mis-selection is loud, and the decision is observable
When `include` is present but matches no registered provider, construction SHALL fail with an error naming both the requested ids and the registered ones. Unknown entries appearing alongside at least one matching entry SHALL be ignored with a warning, so one configuration stays portable across operating systems. At construction the runtime SHALL log which providers are active and, for each suppressed provider, which rule suppressed it (explicit flag, absent from `include`, present in `exclude`, or not registered on this platform).

#### Scenario: Allowlist matching nothing fails construction
- **WHEN** a runtime is built with `providers.include = ["nonexistent"]`
- **THEN** construction fails with an error naming the requested and the registered provider ids, rather than producing an empty desktop

#### Scenario: Portable list across operating systems
- **WHEN** `providers.include = ["windows-uia", "atspi"]` is resolved on Linux
- **THEN** the AT-SPI provider is active, the unmatched entry is ignored with a warning, and construction succeeds

### Requirement: Independent sessions per library instance
The Robot Framework library import SHALL accept the provider selection, and each library instance SHALL build its own runtime with its own selection. Two instances imported under different names in one suite SHALL behave as fully independent sessions — neither shares provider state with the other, and each one's tree reflects only its own selection.

#### Scenario: Two library instances with different provider sets
- **WHEN** a suite imports the library twice, once with a Java-only selection and once with an AT-SPI-only selection, under different names
- **THEN** each instance serves only its selected technology's nodes, and queries through one instance are unaffected by the other

### Requirement: Inspector toggles rebuild the session
The Inspector SHALL allow enabling and disabling providers interactively. Because session configuration is fixed at construction, a toggle SHALL rebuild the runtime with the new selection and re-root the displayed tree, rather than mutating a live runtime. The Inspector SHALL show which providers are active and indicate when a provider contributes nothing because it was suppressed by selection.

#### Scenario: Toggling a provider re-roots the tree
- **WHEN** a provider is toggled off in the Inspector while a tree is displayed
- **THEN** the runtime is rebuilt with the new selection, the tree re-roots without that provider's nodes, and the provider is shown as suppressed rather than as failed
