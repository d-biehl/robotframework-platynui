# XPath Function Atomization Plan

## Overview
The XPath engine currently fails the `numeric/large_sum` benchmark because `fn:sum` receives attribute nodes instead of atomized values. The XPath 2.0 specification (§3.1.5) and the XQuery Functions & Operators spec require implicit atomization before function execution. We will extend the compiler and runtime metadata so that functions expecting atomic inputs automatically trigger atomization, eliminating per-function workarounds and aligning the engine with the standard.

**Reference Specs**
- [XPath 2.0 Specification – §3.1.5 Function Calls](https://www.w3.org/TR/2010/REC-xpath20-20101214/?utm_source=openai)
- [XQuery/XPath Functions & Operators (F&O)](https://www.w3.org/TR/xquery-operators/?utm_source=openai)

## Implementation Objectives
- Track parameter expectations (atomic vs. node vs. any) in the static function metadata.
- Emit `OpCode::Atomize` in the compiler when lowering arguments that must be atomized.
- Keep runtime helpers (`sum_default`, `avg_fn`, …) simple by assuming they receive atomized sequences.
- Add regression coverage to ensure node arguments are atomized for all numeric aggregators and other affected functions.

## Execution Order
1. **Augment Function Signatures**
   - Introduce a parameter kind enum and store it alongside each registered function signature.
   - Update the default function registration to declare parameter kinds, focusing first on numeric aggregators (`sum`, `avg`, `min`, `max`) and other operators requiring atomics.
2. **Expose Metadata to the Compiler**
   - Extend `StaticContext.function_signatures` APIs to surface parameter kinds for a given function/arity.
   - Adjust `ensure_function_available` (or the surrounding call path) so compiled function calls can read the metadata.
3. **Emit Atomize Opcode During Lowering**
   - After lowering each argument expression, consult the parameter metadata and insert `OpCode::Atomize` when the target parameter expects atomic values.
   - Handle optional or variadic arguments conservatively (only atomize those declared as atomic).
4. **Tighten Runtime Helpers**
   - Review numeric reducers to ensure they assume inputs are already atomized; remove redundant node checks where safe.
   - Verify other helpers (e.g., comparison utilities) remain compatible with the new compiler behavior.
5. **Regression Tests**
   - Add unit tests that evaluate expressions like `sum(//number/@value)` without explicit `data()` calls.
   - Re-run existing benchmarks/tests to confirm no performance or behavioral regressions.
6. **Documentation & Cleanup**
   - Document the new metadata model for contributors.
   - Update any internal design notes referencing the old behavior.

## Status Tracker
| Step | Description | Status |
|------|-------------|--------|
| 1 | Augment function signatures with parameter kinds | Completed |
| 2 | Surface metadata to compiler | Completed |
| 3 | Emit `OpCode::Atomize` when compiling calls | Completed |
| 4 | Simplify runtime helpers (assume atomized input) | Completed |
| 5 | Add regression tests & run suite | Completed |
| 6 | Update docs / clean up | Completed |

## Notes
- Start by modelling only the parameters that must be atomic; we can extend the enum later for node-only functions if needed.
- Keep the metadata change backwards-compatible by defaulting unspecified parameters to `Any`.
- After the compiler change lands, re-run `cargo bench -p platynui-xpath numeric/large_sum --bench performance_analysis` to verify the panic is gone.

## Implementation Details for Contributors
- Parameter requirements are stored via `FunctionSignatures::set_param_types`. Every entry is keyed by the function's expanded name plus the concrete arity and should list one `ParamTypeSpec` per parameter position.
- `ParamTypeSpec::requires_atomization()` drives compiler-side emission of `OpCode::Atomize` immediately after lowering the corresponding argument expression. Use atomic specs for functions that, per spec, demand atomized operands (e.g., `fn:sum`, `fn:avg`, `fn:min`, `fn:max`).
- Functions that accept nodes or heterogeneous inputs can continue to omit explicit parameter specs; they default to accepting any item sequence and bypass automatic atomization.
- When adding new built-ins, register both the signature range and, if applicable, the parameter kinds in `register_default_functions` so code generation stays consistent with the spec.
