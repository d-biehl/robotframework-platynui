# Streaming Evaluator Architecture

## Cursor Primitives
The evaluator now centres around the `SequenceCursor` trait. Every value that can be
placed on the virtual machine stack implements a cursor that can be cloned,
provides a `size_hint`, and yields items on demand. This allows opcodes to
compose cheap streaming pipelines rather than materialising `Vec<T>` eagerly.

The `XdmSequenceStream` helper is the thin wrapper that owns a cursor. Calling
`.cursor()` returns a boxed cursor ready for composition while `.iter()` keeps
existing iterator-based call sites working.

## VM Handles & State Reuse
`VmHandle` keeps a cached `Vm` instance initialised from a `VmSnapshot`. Cursor
implementations call `with_vm` to execute sub-programs without paying the cost
of repeatedly rebuilding the VM. The helper now honours an optional cancellation
flag (propagated from the `DynamicContext`) and returns a `Result`, so callers
can propagate cancellation or execution errors naturally.

## Streaming Control Flow
`ForLoop` and `QuantLoop` opcodes are backed by dedicated cursors:

- `ForLoopCursor` walks the input cursor lazily, binds the loop variable as a
  one-item stream, and evaluates the body via `eval_subprogram_stream`, yielding
  body results as soon as they are available.
- `QuantLoopCursor` reuses the same streaming machinery but short-circuits as
  soon as the XPath quantifier semantics are satisfied, finally emitting a
  single boolean.

`Let` bindings now store the incoming `XdmSequenceStream` directly, avoiding the
previous `Vec` round-trip.

## Set Operations & Doc Order
`DocOrderDistinctCursor` provides lazy document-order sorting/deduplication and
is reused by the set operation cursors. `SetOperationCursor` defers work until a
consumer requests the first item, at which point the operands are materialised
only once before deduplicated.

## Cancellation & Error Propagation
The dynamic context accepts an optional `Arc<AtomicBool>` via
`with_cancel_flag`. Cursor code and the VM check this flag cooperatively and
return the standard `err:FOER0000` cancellation error. All helpers now forward
errors via `Result`, so cancellation or sub-expression failures remain lazy and
transparent to the caller.
