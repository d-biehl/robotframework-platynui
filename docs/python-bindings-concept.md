# PlatynUI Python Bindings – Design Concept

This document proposes a clean, future‑proof design for Python bindings to PlatynUI using PyO3 and maturin. It consolidates our discussion into a concrete plan suitable for review and staged implementation.

## Goals
- Provide ergonomic Python access to PlatynUI with a single native package.
- Expose only the essentials from `core` (types, UI identifiers/namespaces, attribute names) and all high‑level functionality from `runtime`.
- Keep Rust domain logic in existing crates (`crates/core`, `crates/runtime`); add thin PyO3 wrappers only.
- Deliver high‑quality typing: .pyi stubs and stable public imports for users and Robot Framework integration.
- Enable portable local development (mock provider) and platform builds (Windows/Linux/macOS) via feature flags.

## Non‑Goals
- Do not move business logic into the PyO3 layer.
- Do not expose platform/provider internals as Python API at this stage.

## Architecture Overview

### One Native Wheel, Two Submodules
- Create a single native package: `platynui_native` (maturin + PyO3), exporting two Python submodules implemented in Rust:
  - `platynui_native.core`: Types and UI definitions (from `crates/core`).
  - `platynui_native.runtime`: Runtime orchestration API (from `crates/runtime`).
- Rationale:
  - Shared PyClasses live in one binary → no cross‑binary class identity issues.
  - Simpler distribution and CI (one wheel per platform/arch).
  - Clear separation of concerns via submodule boundaries.

### Optional Pure‑Python Wrapper
- Keep/repurpose `packages/core` as a lightweight, pure‑Python wrapper `platynui_core` that re‑exports from `platynui_native.core` and can host extra Python ergonomics (dataclasses, helpers, Robot‑specific affordances) without requiring a native rebuild.
- Users import `platynui_core` for a stable product API; internal native structure can change without breaking public imports.

## Public API Surface

### `platynui_native.core`
- PyClasses mirroring key value types from `crates/core`:
  - `Point(x: float, y: float)` with properties `x`, `y`, `to_tuple()`.
  - `Size(width: float, height: float)`; `Rect(x, y, width, height)`.
  - IDs: `PatternId`, `RuntimeId`, `TechnologyId` with `as_str()` and standard `__str__/__repr__/__eq__/__hash__`.
  - `Namespace` as a Python enum, plus `all_namespaces() -> list[Namespace]`, `resolve_namespace(name: str) -> Namespace | None`.
- Functions:
  - `attribute_names() -> dict[str, dict[str, str]]` containing canonical attribute constants grouped by namespace (e.g., `common`, `element`, `desktop`, …), sourced from `crates/core/src/ui/attributes.rs`.
- Value Mapping:
  - `UiValue` is not a separate PyClass; it is converted to native Python types: `None | bool | int | float | str | list | dict | tuple` (for `Point/Rect/Size`).

### `platynui_native.runtime`
- `Runtime`
  - `Runtime()` (constructor), `shutdown()`
  - `evaluate(xpath: str, node: UiNode | None = None) -> list[UiNode | EvaluatedAttribute | UiValue]`
  - Pointer ops: `pointer_position`, `pointer_move_to`, `pointer_click`, `pointer_multi_click`, `pointer_drag`, `pointer_press`, `pointer_release`, `pointer_scroll`
  - Keyboard ops: `keyboard_type`, `keyboard_press`, `keyboard_release`
- `UiNode` (wraps `Arc<dyn UiNode>`)
  - Properties: `runtime_id`, `name`, `role`, `namespace`
  - Methods: `attribute(name, namespace=None) -> UiValue`, `parent() -> UiNode | None`, `children() -> list[UiNode]`, `attributes() -> list[UiAttribute]`, `supported_patterns() -> list[str]`, `doc_order_key() -> int | None`, `invalidate()`, `has_pattern(id) -> bool`, `pattern_by_id(id) -> object | None`
- Attribute classes
  - `UiAttribute`: `namespace`, `name`, `value` (no owner)
  - `EvaluatedAttribute`: `namespace`, `name`, `value`, `owner() -> UiNode | None`

## Type Conversion Strategy

### Flexible Inputs via `FromPyObject`
- Use `#[derive(FromPyObject)]` to accept ergonomic Python arguments, e.g. tuples or PyClasses:

```rust
#[derive(FromPyObject)]
enum PointLike<'py> {
    Tuple((f64, f64)),
    Core(PyRef<'py, crate::core::Point>),
}

impl From<PointLike<'_>> for platynui_core::types::Point { /* map to core::Point */ }
```

- Apply the same pattern for rectangles, sizes, optional overrides, etc. Use `#[pyo3(from_py_with = "...")]` where custom parsing helps.

### Outbound Values
- Convert Rust `UiValue` to Python natively (`UiValue` in Python):
  - `Null → None`, `Bool/Integer/Number/String → bool/int/float/str`, `Array/Object → list/dict`, `Point/Size/Rect → core.Point/core.Size/core.Rect`.
- `Point/Size/Rect` sind echte Klassen mit Properties und `to_tuple()` bei Bedarf.

## Error Mapping
- Map domain errors to Python exceptions with helpful messages:
  - `EvaluateError` → `ValueError` (or custom `EvaluationError`).
  - `PlatformError` → `RuntimeError` (or custom `PlatformError`).
  - Keyboard/pointer domain errors → dedicated exceptions.
- Define these exception classes in the submodules and convert via `PyErr::new_err` in `From` impls.

## Threading & GIL
- Start with `#[pyclass(unsendable)]` for `Runtime` and `Node` to avoid accidental cross‑thread misuse.
- Where appropriate, use `Python::allow_threads` around blocking or OS calls; keep interior `Mutex`/`Arc` from Rust as is.
- Re‑evaluate `Send + Sync` safety once invariants are verified; potentially enable `#[pyclass]` without `unsendable` later.

## Platform Handling & Feature Flags
- Default dev builds can use a mock provider feature for portability:
  - Wrapper feature `mock-provider` → enables `platynui-runtime/mock-provider`.
- The native Python package links the real platform/provider crates per OS at build time (via `cfg(target_os)` in `packages/native/Cargo.toml` and `src/lib.rs`). The runtime itself does not auto‑link OS providers anymore; applications (CLI, Python extension) are responsible for pulling in the desired providers.

## Build, Dev, and Distribution

### Build Backend
- Use `maturin` as the build backend for the native package.
- PyO3 features: `extension-module` and `abi3-py310` (repository pins Python 3.10 via `.python-version`).

### Developer Workflow
- One‑time: `uv sync --dev`
- Local install (editable): `uv run maturin develop -m packages/native --release [--features mock-provider]`
- Rust workspace builds/tests remain (the native package is excluded from the Cargo workspace):
  - `cargo build --workspace`
  - `cargo test --workspace`
  - Platform crates are included indirectly by the CLI; unit tests link the mock providers in their test modules.
- Lint/Typecheck Python:
  - `uv run ruff check .`
  - `uv run mypy src/PlatynUI packages/core/src`

### Wheels & CI
- Build wheels: `uv run maturin build -m packages/native --release`.
- CI matrix per OS/arch; publish the single native wheel `platynui_native` plus pure‑Python wrapper sdist/wheel (`platynui_core`).

## Typing: .pyi Strategy

### Locations
- Place stubs alongside the Python sources shipped with the wheel (maturin’s `python-source`):
  - `packages/native/python/platynui_native/__init__.pyi` (simple `from . import core, runtime`).
  - `packages/native/python/platynui_native/core.pyi`.
  - `packages/native/python/platynui_native/runtime.pyi`.
- Optional wrapper stubs:
  - `packages/core/src/platynui_core/__init__.pyi` re‑exporting from `platynui_native.core`.
  - Include `py.typed` in wrapper for PEP 561.

### Authoring Process
- Generate coarse stubs as a starting point after a dev build:
  - `pyright --createstub platynui_native -o packages/native/tmp_stubs` or `python -m mypy.stubgen -m platynui_native -o packages/native/tmp_stubs`.
- Curate and move into `python/platynui_native/*.pyi`; add precise overloads mirroring `FromPyObject` flexibility, e.g.:
  - `@overload def pointer_move_to(self, p: core.Point, /) -> core.Point: ...`
  - `@overload def pointer_move_to(self, p: tuple[float, float], /) -> core.Point: ...`
- Maintain a small CI check to ensure symbol names in the module match the .pyi (to avoid drift).

## Directory Layout (Target)

```
packages/
  native/
    pyproject.toml            # maturin backend, module-name = "platynui_native", python-source = "python"
    Cargo.toml                # pyo3 deps; depends on ../../crates/core and ../../crates/runtime
    src/
      lib.rs                  # #[pymodule] platynui_native; registers submodules
      core.rs                 # PyClasses + functions for types/UI
      runtime.rs              # Runtime/Node + ops
    python/
      platynui_native/
        __init__.py           # from . import core, runtime; light helpers
        __init__.pyi
        core.pyi
        runtime.pyi

  core/                       # optional pure-Python wrapper
    pyproject.toml            # pure Python; depends on platynui_native
    src/platynui_core/
      __init__.py             # re-exports from platynui_native.core
      __init__.pyi            # (optional) re-exports for typing
      py.typed                # optional PEP 561 marker
```

## Implementation Notes
- Use `#[pyo3(text_signature = "(...)")]` and `#[pyo3(signature = (...))]` to produce helpful Python introspection.
- Prefer returning Python‑native structures for flexible data exchange (especially for `UiValue`).
- Keep `#[derive(FromPyObject)]` enums small and focused; validate inputs early and produce clear error messages.
- Consider `pyo3-ffi`/`pyo3-serde` if serde bridging to/from Python dicts becomes substantial.

## Migration & Compatibility
- Public Python API should primarily be consumed through `platynui_core` (wrapper). This allows internal refactors of the native module without breaking imports.
- If the wrapper is deemed unnecessary long‑term, we can instruct direct imports from `platynui_native.core`/`runtime`; the design still works either way.

## Open Questions
1. Do we introduce custom Python exception classes (`EvaluationError`, `PlatformError`, …) now or later?
2. Exact shapes for screenshot/highlight requests and returns (dict vs. dedicated PyClasses)?
3. Event subscription: expose provider events to Python in v1, or defer?
4. Should `Node` be `Send + Sync` in the Python layer (once invariants are clear), or stay unsendable?

## Phased Implementation Plan
1. Scaffold `packages/native` (maturin, PyO3, submodules) and minimal `core` types (Point/Size/Rect, IDs, Namespace, `attribute_names()`).
2. Implement `Runtime.new()` and `evaluate()` in `platynui_native.runtime`, including conversions (`UiValue` → Python types) and a basic `Node` wrapper.
3. Add pointer/keyboard methods with `FromPyObject` helpers (`PointLike`, overrides), plus error mapping.
4. Author initial `.pyi` stubs for `core` and `runtime`; add wrapper stubs (optional).
5. Add `packages/core` as pure‑Python wrapper (if chosen), re‑exporting `core` symbols; document imports.
6. CI: maturin wheel builds (per OS/arch), Python type checks (mypy/pyright), linters (ruff), Rust fmt/lints/tests.
7. Documentation: quickstart, examples, and Robot Framework keyword mapping.

---

This concept aims to balance ergonomics, stability, and maintainability: one native wheel, clear submodules, flexible inputs via `FromPyObject`, strong typing with curated .pyi, and a thin optional wrapper layer for API stability and DX.

## Implementation Status (2025‑09‑29)

The first slice is implemented under `packages/native` and usable for local dev with the mock provider. Highlights below.

### Module Layout
- Native package: `platynui_native`
  - Rust cdylib module name: `platynui_native._native`
  - Python package stub: `packages/native/python/platynui_native/__init__.py` → re‑exports `_native.core` and `_native.runtime`

### `core` Submodule (Rust → Python)
- Types: `Point`, `Size`, `Rect` (`to_tuple()`, accessors, `__repr__`)
- IDs: `PatternId`, `RuntimeId`, `TechnologyId` (`as_str()`)
- Namespaces: `Namespace` (class attrs `Control`, `Item`, `App`, `Native`), `all_namespaces()`, `resolve_namespace()`
- Attributes: `attribute_names()` returns grouped canonical names (`common`, `element`, …)
- Conversion helpers for runtime: tuple conversions for `Point`/`Rect`

### `runtime` Submodule (Rust → Python)
- Exceptions: `EvaluationError`, `ProviderError`, `PointerError`, `KeyboardError`, `PatternError`
- Runtime lifecycle: `Runtime()` (constructor), `evaluate(xpath, node=None)`, `shutdown()`
- UiNode wrapper: properties + navigation and metadata
  - `runtime_id`, `name`, `role`, `namespace`
  - `attribute(name, namespace=None)` → Python native value
  - `parent() -> UiNode|None`, `children() -> list[UiNode]`
  - `attributes() -> list[UiAttribute]` (no owner)
  - `supported_patterns() -> list[str]`, `doc_order_key() -> int|None`, `invalidate()`
  - `has_pattern(id: str) -> bool`
  - `pattern_by_id(id: str) -> object|None` (see Pattern wrappers)

#### Pattern Wrappers (MVP)
- `Focusable` (id: "Focusable"): `focus()`
- `WindowSurface` (id: "WindowSurface"): `activate/minimize/maximize/restore/close`, `move_to(x,y)`, `resize(w,h)`, `move_and_resize(x,y,w,h)`, `accepts_user_input() -> bool|None`
- Returned via `Node.pattern_by_id("…")`; returns `None` if pattern unsupported

### Pointer/Keyboard APIs
- Pointer
  - Buttons: `ButtonLike = int | runtime.PointerButton`; ints `1/2/3` → `LEFT/MIDDLE/RIGHT`, sonst `Other(n)`
  - Overrides: `PointerOverrides` Klasse (nur Klasse); read‑only Properties für alle Felder
  - Origin: `'desktop' | core.Point | core.Rect`; Property `origin` liefert `'desktop'`, `core.Point` oder `core.Rect`
  - Methoden: wie oben
- Keyboard
  - Overrides: `KeyboardOverrides` Klasse (nur Klasse); read‑only Properties

### FromPyObject Ergonomics
- Points/Rects: `PointLike = core.Point` (keine Tuple‐Kurzform)
- Scroll delta: `ScrollLike = (float, float)`
- Buttons: `PointerButtonLike = str('left'|'middle'|'right') | int` (int maps to `Other(n)`)
- Origins: `OriginInput = 'desktop' | (x,y) | (x,y,w,h) | {'absolute':(x,y)} | {'bounds':(x,y,w,h)}`
- Pointer overrides: prefer concrete `runtime.PointerOverrides` class; dicts remain supported for convenience and are parsed via `FromPyObject`.
- Keyboard overrides: prefer concrete `runtime.KeyboardOverrides` class; dicts remain supported for convenience and are parsed via `FromPyObject`.

### Typing (.pyi)
- `core.pyi`: `Point/Size/Rect`, IDs, `Namespace`, helpers
- `runtime.pyi`: `UiNode`, `UiAttribute`, `EvaluatedAttribute`, `Runtime`, `PointerOverrides`, `KeyboardOverrides`, `PointerButton` enum; kompakte Signaturen mit `PointLike`, `ButtonLike` etc.

### Tests
- `packages/native/tests/test_runtime_basic.py`
  - `evaluate("/")` returns desktop node; basic pointer/keyboard smoke (skips on missing devices)
- `packages/native/tests/test_overrides_frompyobject.py`
  - Validates dict/tuple/str/int inputs convert via `FromPyObject` for pointer/keyboard overrides, origins, buttons

### Build/Run (local, mock)
- Install: `uv run maturin develop -m packages/native/Cargo.toml --release --features mock-provider`
- Tests: `uv run pytest -q packages/native/tests`
- Usage:
  - `from platynui_native import core, runtime`
  - `rt = runtime.Runtime()`
  - `items = rt.evaluate("/")` → list of `UiNode | EvaluatedAttribute | UiValue`

### Known Limitations / Next Steps
- Pattern coverage: only `Focusable` and `WindowSurface` are exposed. Extend with additional wrappers (Text*, Toggleable, Selection*, …) as traits stabilize.
- Generic pattern reflection is not implemented; wrappers bind to known traits for safety.
- Device availability: with the mock feature some pointer/keyboard calls may raise device errors; conversion paths are still validated.
- Warnings: non‑functional pyo3 warnings (richcmp omissions, non_snake_case classattrs for Namespace, unsafe_op_in_unsafe_fn from macro glue) are cosmetic and can be cleaned up later.
- Docs: expand README with tabular overrides reference (names, types, units, defaults) across platforms as they evolve.
