# XPath 2.0 Feature Coverage (platynui-xpath)

This document tracks the major XPath 2.0 features currently implemented in
`platynui-xpath`. The checklist reflects functional coverage as of the latest
remediation pass (May 2026).

## Data Model & Static Context

- [x] Static context defaults (function namespace, element namespace, compatibility mode)
- [x] Namespace handling for axes (`namespace`, prefixed/unprefixed steps)
- [x] QName interning through compiler IR and evaluator fast paths
- [ ] Schema-aware constructs (`schema-element`, `schema-attribute`, type annotations)

## Type System & Casting

- [x] Core numeric tower (`xs:integer`, `xs:decimal`, `xs:float`, `xs:double`)
- [x] Numeric subtypes (`xs:long`, `xs:int`, `xs:short`, `xs:byte`, unsigned/non-(positive|negative) variants)
- [x] String-derived types (`xs:normalizedString`, `xs:token`, `xs:language`, `xs:Name`, `xs:NCName`, `xs:NMTOKEN`, `xs:ID`, `xs:IDREF`, `xs:ENTITY`)
- [x] Binary types (`xs:base64Binary`, `xs:hexBinary`)
- [x] QName/NOTATION casts with static-context validation
- [x] Temporal and duration types (`xs:date`, `xs:time`, `xs:dateTime`, `xs:gYear`, `xs:gYearMonth`, `xs:gMonth`, `xs:gMonthDay`, `xs:gDay`, `xs:yearMonthDuration`, `xs:dayTimeDuration`)
- [ ] Schema validation for user-defined simple types

## Runtime & Evaluation

- [x] Shared scratch buffers for axis traversal and set operations (avoids per-step allocations)
- [x] Streaming sequence cursors for axes, predicates, and set operators
- [x] Name-test optimisations via interned atoms and cached namespace lookups
- [ ] Streaming, stateless evaluation for user-defined functions (still materialises sequences)

## Testing & Tooling

- [x] Regression suites covering casts and castability (positive/negative)
- [x] Namespace axis and static context tests
- [x] Advanced Criterion benches for axis/set operations (`advanced_benchmarks`, `parser_smallvec`)
- [ ] Benchmarks covering extended casts (to be added after stabilising evaluator micro-optimisations)

## Pending Follow-up

- Update Criterion benches with scenarios targeting extended cast operations and fast paths.
- Document benchmark results in the project README or dedicated performance notes.
- Expand coverage once schema-aware features or additional F&O functions are prioritised.
