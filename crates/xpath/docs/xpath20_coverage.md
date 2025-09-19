# XPath 2.0 Coverage in `platynui-xpath`

This note summarizes how the crate lines up with the W3C *XPath 2.0* recommendation (especially §2 and §3) and the accompanying 2010 *XQuery 1.0 and XPath 2.0 Functions and Operators* document across the parser, compiler, and evaluator. Each row references the relevant implementation spots or highlights gaps where the specification calls for behaviour that is not present yet.

## Static Context vs. Spec (§2.1.1, App. C.1)

| Spec component | Status in crate | Notes |
| --- | --- | --- |
| XPath 1.0 compatibility mode | ⚠️ flag unused | `StaticContext` exposes `xpath_compatibility_mode`, aber die Laufzeit ignoriert es – faktisch bleibt nur der XPath-2.0-Modus aktiv (`crates/platynui-xpath/src/engine/runtime.rs:1883-1917`). |
| Statically known namespaces | ✅ stored | `StaticContext.namespaces` hält das Prefix→URI-Mapping (`crates/platynui-xpath/src/engine/runtime.rs:1883-1917`). |
| Default element/type namespace | ✅ optional | `default_element_namespace` plus Builder-Hilfen fließen in Compiler und Tests ein (`crates/platynui-xpath/src/engine/runtime.rs:1887-1964`, `crates/platynui-xpath/src/compiler/mod.rs:525-631`). |
| Default function namespace | ✅ defaulted to `fn` | `default_function_namespace` initialises to the W3C function namespace (`crates/platynui-xpath/src/engine/runtime.rs:753`). |
| In-scope schema definitions (types/elements/attributes) | ❌ missing | There is no schema catalog in the static context, so schema-aware features are unsupported. |
| In-scope variables | ✅ tracked | `StaticContext.in_scope_variables` holds the global bindings (`crates/platynui-xpath/src/engine/runtime.rs:743`). |
| Context item static type | ❌ missing | Static typing metadata is not recorded; only dynamic checks exist. |
| Function signatures catalogue | ✅ implemented | `StaticContext.function_signatures` records known functions and their arity ranges, enabling compile-time validation (`crates/platynui-xpath/src/engine/runtime.rs`). |
| Statically known collations | ✅ implemented | `StaticContext.statically_known_collations` tracks available collations alongside the default (`crates/platynui-xpath/src/engine/runtime.rs`). |
| Default collation | ✅ stored | `default_collation` defaults to codepoint collation (`crates/platynui-xpath/src/engine/runtime.rs:754`). |
| Base URI | ✅ stored | `StaticContext.base_uri` is available and exposed to functions (`crates/platynui-xpath/src/engine/runtime.rs:738`). |
| Statically known documents / collections / default collection type | ❌ missing | These collections are not tracked; `fn:doc`/`fn:collection` rely on the runtime `NodeResolver` instead. |

## Dynamic Context vs. Spec (§2.1.2, App. C.2)

| Spec component | Status in crate | Notes |
| --- | --- | --- |
| Context item / focus triple | ✅ handled dynamically | `DynamicContext.context_item` stores the item, while the VM keeps position/size on the `Frame` stack (`crates/platynui-xpath/src/engine/runtime.rs:825`, `crates/platynui-xpath/src/engine/evaluator.rs:29-44`). |
| Variable values | ✅ supported | Local scopes push values on the VM stack; global values live in `DynamicContext.variables` (`crates/platynui-xpath/src/engine/runtime.rs:827`, `crates/platynui-xpath/src/engine/evaluator.rs:75-121`). |
| Current date/time & implicit timezone | ✅ configurable | `DynamicContext.now` and `timezone_override` feed the date/time functions and `fn:implicit-timezone` (`crates/platynui-xpath/src/engine/runtime.rs:833-834`, `crates/platynui-xpath/src/engine/functions/datetime.rs:247-260`). |
| Available documents / collections | ⚠️ host-provided | A `NodeResolver` trait must be supplied; otherwise `fn:doc`/`fn:collection` raise `FODC0005/0004` (`crates/platynui-xpath/src/engine/runtime.rs:307-314`, `crates/platynui-xpath/src/engine/functions/environment.rs:414-468`). |
| Default collection | ⚠️ host-provided | No internal default is stored—`collection()` without arguments errors unless the resolver handles it (`crates/platynui-xpath/src/engine/functions/environment.rs:438-468`). |
| Function & collation libraries | ✅ dynamic | `DynamicContext.functions` and `.collations` hold registries populated by `default_function_registry` (`crates/platynui-xpath/src/engine/runtime.rs:828-844`). |
| Regex support | ✅ pluggable | `DynamicContext.regex` defaults to none; `FancyRegexProvider` implements the spec-required flags (`crates/platynui-xpath/src/engine/runtime.rs:832`, `crates/platynui-xpath/src/engine/runtime.rs:316-360`). |

## Expression Grammar Coverage (§3)

| Spec area | Implemented elements | Notes |
| --- | --- | --- |
| Primary expressions (§3.1) | Literals, variable refs, function calls, parenthesized expressions, context item | Enumerated in `parser::ast::Expr` and built in `build_primary_expr` (`crates/platynui-xpath/src/parser/ast.rs:65-151`, `crates/platynui-xpath/src/parser/mod.rs:1113-1153`). Boolean literals are not emitted; `fn:true()`/`fn:false()` cover booleans per spec. |
| Path expressions (§3.2) | Root/relative paths, all forward/backward axes, namespace axis, filter steps, predicates | AST variants and lowering cover every axis (`crates/platynui-xpath/src/parser/ast.rs:191-228`, `crates/platynui-xpath/src/compiler/mod.rs:374-419`). Predicates maintain focus using VM frames (`crates/platynui-xpath/src/engine/evaluator.rs:140-189`). |
| Node tests & kind tests (§3.2.1.2) | `node()`, `text()`, `comment()`, `processing-instruction`, `element()`, `attribute()` | Parsed in `ast::KindTest`; the compiler maps them to IR (`crates/platynui-xpath/src/parser/ast.rs:243-303`, `crates/platynui-xpath/src/compiler/mod.rs:468-520`). Schema-aware forms (typed `element()`, `attribute()`, `schema-element()`, `schema-attribute()`) raise `XPST0003` because schema support is absent (`crates/platynui-xpath/src/compiler/mod.rs:468-495`). Runtime matching currently treats schema kind tests as wildcards (`crates/platynui-xpath/src/engine/evaluator.rs:1989-2016`). |
| Sequence constructors (§3.3) | Comma sequences, empty sequence `()`, filter expressions, set operations (`union`, `intersect`, `except`) | Represented by `Expr::Sequence`, `Expr::Filter`, and `Expr::SetOp` (`crates/platynui-xpath/src/parser/ast.rs:73-158`); compiler emits the corresponding opcodes (`crates/platynui-xpath/src/compiler/mod.rs:335-344`). |
| Arithmetic & range (§3.4) | `+`, `-`, `*`, `div`, `idiv`, `mod`, unary `+/-`, `to` | Lowered to arithmetic opcodes (`crates/platynui-xpath/src/compiler/mod.rs:192-237`), evaluated with numeric promotion per §3.4 (`crates/platynui-xpath/src/engine/evaluator.rs:1095-1259`). |
| Comparisons (§3.5) | Value, general, and node comparisons | Code maps AST operators to IR and evaluates with EBV and numeric promotion rules (`crates/platynui-xpath/src/compiler/mod.rs:197-235`, `crates/platynui-xpath/src/engine/evaluator.rs:1033-1188`). |
| Logical expressions (§3.6) | `and`, `or` | Binary operations emitted as `OpCode::And`/`Or` (`crates/platynui-xpath/src/compiler/mod.rs:192-215`). |
| FLWOR subset (§3.7) | `for … return …` and `let … return …` | Grammar matches the XPath subset (no `where`/`order by` clauses) and the compiler generates nested loops/scopes (`crates/platynui-xpath/src/parser/xpath2.pest:166-171`, `crates/platynui-xpath/src/compiler/mod.rs:272-334`). |
| Conditional expressions (§3.8) | `if (…) then … else …` | Lowered with explicit jumps in IR (`crates/platynui-xpath/src/compiler/mod.rs:213-236`). |
| Quantified expressions (§3.9) | `some/every` | Compiler nests quantifiers and evaluator short-circuits per semantics (`crates/platynui-xpath/src/compiler/mod.rs:247-271`, `crates/platynui-xpath/src/engine/evaluator.rs:1232-1289`). |
| Type expressions (§3.10) | `instance of`, `treat`, `castable`, `cast`, sequence type grammar | Sequence types and single types are parsed (`crates/platynui-xpath/src/parser/ast.rs:276-303`), lowered to IR (`crates/platynui-xpath/src/compiler/mod.rs:312-318`), and enforced during evaluation (`crates/platynui-xpath/src/engine/evaluator.rs:1259-1454`). Unsupported atomic targets fall back to `err:NYI0000` (`crates/platynui-xpath/src/engine/evaluator.rs:2140-2402`). |

## Compiler & Evaluator Behaviour

| Concern | Implementation | Notes |
| --- | --- | --- |
| Document order & distinct nodes | `DocOrderDistinct` normalises node sequences after each step (`crates/platynui-xpath/src/compiler/mod.rs:374-414`, `crates/platynui-xpath/src/engine/evaluator.rs:1768-1801`). |
| Predicate focus | Predicates are compiled as nested instruction sequences; the VM installs a `Frame` so `position()`/`last()` behave per spec (`crates/platynui-xpath/src/compiler/mod.rs:364-373`, `crates/platynui-xpath/src/engine/evaluator.rs:140-210`). |
| Axis evaluation | `axis_iter` covers every axis defined in XPath 2.0, including namespace handling and exclusions for `preceding`/`following` (`crates/platynui-xpath/src/engine/evaluator.rs:1802-1976`). |
| Namespace wildcards | Compiler resolves namespace prefixes via the static context when lowering wildcard node tests (`crates/platynui-xpath/src/compiler/mod.rs:440-458`). |
| Type matching | `assert_treat`, `instance_of`, and `is_castable` implement cardinality and basic type checks, but schema-derived type tests are effectively pass-throughs because schema metadata is absent (`crates/platynui-xpath/src/engine/evaluator.rs:2300-2461`). |
| Casting support | Built-in support covers strings, numerics, date/time, durations, QName, anyURI; other targets raise `err:NYI0000` (`crates/platynui-xpath/src/engine/evaluator.rs:2140-2402`). |
| Set operations | Unions, intersections, and except are implemented on node sequences with document-order semantics (`crates/platynui-xpath/src/engine/evaluator.rs:2044-2142`). |
| Function resolution | Calls are resolved by name and arity through `FunctionRegistry::resolve`; unknown or wrong-arity invocations map to `XPST0017` (`crates/platynui-xpath/src/engine/evaluator.rs:1291-1343`, `crates/platynui-xpath/src/engine/runtime.rs:220-302`). |
| External resources | `NodeResolver` and `RegexProvider` abstractions surface host-provided document access and regex engines, aligning with spec-defined host interfaces (`crates/platynui-xpath/src/engine/runtime.rs:307-360`). |

## Function & Operator Library (Spec references [F&O 2010])

The default registry exposes the core function families required by XPath 2.0:

- Boolean, numeric, and EBV helpers (`crates/platynui-xpath/src/engine/functions/mod.rs:24-45`).
- String and codepoint processing (`crates/platynui-xpath/src/engine/functions/mod.rs:46-150`).
- QName, namespace, and node-name accessors (`crates/platynui-xpath/src/engine/functions/mod.rs:152-214`).
- Numeric aggregates and sequence combinators, including `sum`, `avg`, `distinct-values`, `index-of`, `insert-before`, `remove`, `reverse`, `unordered` (`crates/platynui-xpath/src/engine/functions/mod.rs:214-298`).
- Collation-aware comparison and deep equality (`crates/platynui-xpath/src/engine/functions/mod.rs:298-336`).
- Regular-expression functions (`crates/platynui-xpath/src/engine/functions/mod.rs:336-368`).
- Diagnostics (`fn:error`, `fn:trace`) (`crates/platynui-xpath/src/engine/functions/mod.rs:368-388`).
- Environment/document helpers (`fn:base-uri`, `fn:doc`, `fn:collection`, URI encoding, language checks) (`crates/platynui-xpath/src/engine/functions/mod.rs:388-456`).
- ID/IDREF family, `fn:nilled`, `fn:root` (`crates/platynui-xpath/src/engine/functions/mod.rs:456-496`).
- `xs:*` constructors for the primitive type lattice (`crates/platynui-xpath/src/engine/functions/mod.rs:496-836`).
- Date, time, and duration accessors (`crates/platynui-xpath/src/engine/functions/mod.rs:456-656`).

Functions that depend on host-defined I/O or formatting rules from the 2010 specification—for example the `fn:unparsed-text*` family, which would need a text loader akin to the `NodeResolver` used by `fn:doc`—are not registered yet. Later additions introduced in XPath 3.x such as `fn:analyze-string` are intentionally out of scope for this crate.

## Highlighted Gaps

- **Schema awareness**: typed `element()`/`attribute()`, `schema-element()`, and `schema-attribute()` are rejected at compile time (`crates/platynui-xpath/src/compiler/mod.rs:468-495`). Sequence type checks therefore cannot validate schema-derived types.
- **Static typing feature**: the static context lacks type information, so only dynamic checks enforce sequence types.
- **Casting coverage**: `cast as` supports the common primitives but falls back to `err:NYI0000` for the rest of the XML Schema hierarchy (`crates/platynui-xpath/src/engine/evaluator.rs:2400-2402`).
- **Function library completeness**: parts of the F&O catalogue that rely on host I/O or formatting (for example the `fn:unparsed-text*` family) are not implemented in `default_function_registry`.
- **Host dependencies**: document and collection access rely on an external `NodeResolver`; without one the respective functions raise `FODC0005/0004` (`crates/platynui-xpath/src/engine/functions/environment.rs:414-468`).

Overall, the crate implements the full core expression grammar and a substantial subset of XPath 2.0 semantics. Completing schema awareness, static typing metadata, and the remaining F&O functions would be the largest steps toward full spec conformance.
