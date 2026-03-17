# Error Handling

This document defines how error handling should work across the Rust workspace.

## Goals

- Keep public error surfaces small and predictable.
- Preserve structured machine-readable context instead of encoding everything into free-form messages.
- Keep user-visible text in English for now.
- Make later localization possible without redesigning the core error types.

## Boundary Errors

At crate boundaries, use one public error enum per domain.

- `PlatformError` for platform and device failures.
- `ProviderError` for UI tree provider failures.

Do not introduce companion `*Kind` enums for these boundary types.
Do not add helper constructors such as `not_supported(...)`, `simple(...)`, or `new(...)`.
Construct the enum variant directly at the call site.

## Current Shape

`PlatformError` and `ProviderError` use:

- a small, stable set of variants
- fixed English base text in `Display`
- named fields for structured context
- optional `details: Option<String>` for dynamic runtime information

Example:

```rust
return Err(PlatformError::CapabilityUnavailable {
    capability: "virtual pointer",
    details: None,
});
```

```rust
.map_err(|e| ProviderError::CommunicationFailure {
    channel: "windows uia",
    details: Some(e.to_string()),
})
```

## How To Choose A Variant

### PlatformError

Use `InitializationFailed` when constructing or wiring up a platform subsystem fails.
Examples: connecting to X11, creating a Wayland helper object, setting up a Windows helper.

Use `CapabilityUnavailable` when the current platform session or backend cannot provide a feature.
Examples: no virtual pointer, no compositor IPC backend, missing XTEST extension.

Use `UnsupportedPlatform` when the current operating system, session type, or environment is fundamentally unsupported.
Examples: session detection failed, required platform prerequisites are absent, the current target is outside supported scope.

Use `OperationFailed` when the capability exists in principle but the requested action failed at runtime.
Examples: flush failed, send_event failed, parsing a returned handle failed, window lookup failed.

### ProviderError

Use `InitializationFailed` when provider startup fails.
Examples: COM setup, AT-SPI bus setup, provider bootstrap.

Use `UnsupportedOperation` when the provider intentionally does not support an operation.

Use `CommunicationFailure` when a provider-specific channel or API call fails.
Examples: UIA call failure, AT-SPI communication failure, IPC/bridge failure.

Use `InvalidArgument` when a caller supplied an invalid provider-level argument.

Use `TreeUnavailable` when the provider cannot currently deliver a UI tree.

## Field Conventions

Named fields should be short, stable labels.

- `component`, `capability`, `platform`, `operation`, `provider`, `channel`, `argument` should usually be noun or verb-phrase identifiers, not full sentences.
- `details` may contain dynamic text from lower layers or formatted runtime context.
- Prefer `None` when the base variant text already explains the failure well enough.

Good:

```rust
PlatformError::OperationFailed {
    operation: "portal ConnectToEIS",
    details: Some(e.to_string()),
}
```

```rust
PlatformError::CapabilityUnavailable {
    capability: "virtual pointer",
    details: None,
}
```

Avoid putting the full final sentence into `details`.
This is bad because it duplicates the `Display` text and makes future localization harder:

```rust
PlatformError::OperationFailed {
    operation: "portal ConnectToEIS",
    details: Some("platform operation failed: portal ConnectToEIS".into()),
}
```

## Internal Errors vs Boundary Errors

Inside a crate, it is still fine to use richer internal error enums when they improve local clarity.
Examples: `UiaError`, protocol-specific errors, parser errors, transport errors.

Map those internal errors to `PlatformError` or `ProviderError` when crossing a crate boundary or trait boundary.

This keeps the external API small while allowing detailed internal modeling.

## Logging

Logging and returned errors serve different purposes.

- Use structured `tracing` fields for diagnostics and operational context.
- Return a `PlatformError` or `ProviderError` that describes the failure category cleanly.
- Do not rely on logs as the only place where failure context exists.
- Do not bloat the error variant itself with every diagnostic detail.

If both are useful, do both: emit a trace or warning and still return the typed boundary error.

## Testing Guidance

When testing boundary errors, prefer asserting on:

- the variant
- the structured field values
- whether `details` is present or absent when relevant

Avoid tests that depend on the full formatted `Display` string unless the display text itself is the behavior under test.

## Localization Readiness

The current `Display` output remains English.
That is intentional.

The important preparation for localization is not translated strings inside the core enums.
It is the structured shape:

- stable variant names
- stable semantic fields
- optional runtime details kept separate from the base message

If a UI layer later needs localized messages, it should map the error variant and fields to localized text there instead of changing the low-level error model.