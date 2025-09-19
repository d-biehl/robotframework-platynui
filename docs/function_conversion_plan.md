# Function Conversion Rules Plan

## Overview
XPath 2.0 requires a full set of function conversion rules before invoking any user- or system-defined function. Beyond the atomization support we already added, we still need to implement the remaining steps mandated by the specification:

1. Atomization of each argument value.
2. Conversion of `xs:untypedAtomic` arguments to the expected atomic type of the parameter.
3. Numeric and URI type promotion when required by the target sequence type.
4. Verification that value types and cardinalities conform to the declared parameter sequence type, raising the mandated dynamic errors otherwise.

This plan describes the work needed to bring the PlatynUI XPath engine into compliance with those rules and keep our implementation aligned with the official specifications throughout the changes.

**Reference Specs**
- [XPath 2.0 Specification – §3.1.5 Function Calls](https://www.w3.org/TR/2010/REC-xpath20-20101214/?utm_source=openai)
- [XQuery/XPath Functions & Operators (F&O) – Function Conversion Rules](https://www.w3.org/TR/xquery-operators/?utm_source=openai)

## Implementation Objectives
- Extend static function metadata so each parameter records its expected sequence type (item type + cardinality).
- Apply the full function conversion pipeline inside the evaluator prior to dispatching a call implementation.
- Ensure compiler-emitted bytecode still cooperates with the new runtime conversions.
- Add regression tests covering successful conversions, promotions, and error scenarios according to the spec.

## Execution Order
1. **Model Parameter Sequence Types**
   - Define a lightweight representation (e.g., `ParamTypeSpec`) that captures item type (`ItemTypeSpec`) and occurrence constraints (min/max or `Occ::ZeroOrOne`, etc.).
   - Provide a single registration helper/macro, e.g. `reg_fn!`, that takes `(reg, sigs, ns, local, arity_range, func, param_specs)` and records both implementation and metadata in one call for the core XPath `fn:` namespace.
   - `ParamTypeSpec` should support at least: `AnyAtomic`, `Numeric`, `String`, `UntypedPromotable`, `AnyItem`, plus an occurrence marker.
2. **Expose Metadata Accessors**
   - Add APIs on `FunctionSignatures` to retrieve sequence-type specifications for a name+arity pair, respecting default function namespaces.
3. **Implement Conversion Pipeline**
   - In the evaluator's `CallByName` branch, after atomization, invoke a helper (e.g. `apply_function_conversions(&mut args, param_specs)`) that:
     1. Casts each `xs:untypedAtomic` to the target atomic type as defined by the spec (numeric → `xs:double`, string → `xs:string`, etc.).
     2. Performs numeric promotion (`xs:float` → `xs:double`, integer family → `xs:decimal`/`xs:double`) and URI promotion (`xs:anyURI` when allowed).
     3. Verifies cardinality (raising `err:XPTY0004` for violations) and type conformance (`err:XPTY0004` / `err:FOTY0012` as appropriate).
     4. Returns the converted sequences or propagates errors like `FORG0006` when conversion fails.
4. **Adjust Function Implementations**
   - Simplify built-in functions that previously performed ad-hoc conversions so they rely on the centralized pipeline.
   - Ensure variadic or optional arguments remain compatible.
5. **Compiler Validation**
   - Confirm compiler-lowered code paths (including inline Atomize opcodes) align with the new runtime expectations; adjust the registration macro or emitted metadata if additional compile-time hints become necessary.
6. **Regression & Error Tests**
   - Add unit tests covering at minimum:
     - `sum(//@value)` where the attributes are `xs:untypedAtomic` → result as `xs:double`.
     - Passing `xs:float` and `xs:integer` to numeric functions to confirm promotion to `xs:double`.
     - URI promotion scenarios (e.g., functions expecting `xs:string?` receiving `xs:anyURI`).
     - Cardinality errors (`fn:substring-before` with two items) producing `err:XPTY0004`.
     - Conversion failures (`xs:untypedAtomic` that cannot cast to numeric) producing `err:FORG0001`/`FORG0006` as mandated.
7. **Documentation & Cleanup**
   - Update developer docs to describe the parameter-type metadata and conversion pipeline.
   - Remove redundant conversion logic from existing code once the centralized path is verified.

## Status Tracker
| Step | Description | Status |
|------|-------------|--------|
| 1 | Model parameter sequence types | Completed |
| 2 | Expose metadata accessors | Completed |
| 3 | Implement conversion pipeline in evaluator | Completed |
| 4 | Adjust built-in function implementations | Completed |
| 5 | Validate compiler integration | Completed |
| 6 | Add regression and error tests | Completed |
| 7 | Update docs / cleanup | Completed |

## Notes
- Focus initial coverage on core `fn:` functions; extension modules can follow once the pattern is proven.
- Treat unspecified parameter metadata as accepting any sequence to remain backwards compatible until all functions are annotated.
- Plan to re-run the full `cargo test -p platynui-xpath` suite plus relevant benchmarks after landing the conversion logic to confirm no regressions.
- Runtime conversions now coerce `xs:untypedAtomic` inputs into strings, booleans, numeric families (`xs:double`, `xs:decimal`, `xs:float`), `xs:anyURI`, `xs:duration`, and `xs:QName` before dispatch, raising `err:XPTY0004` for cardinality violations and `err:FONS0004` when namespace prefixes are unknown.
- Regression coverage includes string/numeric/URI/duration/QName promotion scenarios (e.g., `substring`, `round`, `encode-for-uri`, `resolve-uri`, `years-from-duration`, `namespace-uri-from-QName`); the compiler and runtime now share a single source of truth for these conversions, so new built-ins only need to register their `ParamTypeSpec`s.

## Developer Guidance
- Register every built-in through `register_default_functions` with both the arity range and the concrete `ParamTypeSpec` list. This keeps the compiler (`ParamTypeSpec::requires_atomization`) and runtime (`ParamTypeSpec::apply_to_sequence`) in sync.
- Use `ParamTypeSpec::qname`, `::duration`, `::year_month_duration`, and `::day_time_duration` to opt-in to the relevant conversion helpers. Avoid per-function parsing; the runtime will supply already-typed atomics or raise the mandated errors.
- When introducing new namespaces or prefixes in tests, prefer the statically declared namespaces (e.g., `xml`) or extend the test context via the compiler; the runtime now enforces prefix resolution rules strictly.
- Legacy helpers that previously accepted strings/untyped values (e.g., duration accessors) should no longer perform inline parsing. If a helper still must accept multiple atomic flavours, document the expectation and rely on the centralized conversions for lexical handling.
