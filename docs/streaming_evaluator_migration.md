# Streaming Evaluator Migration Notes

Downstream integrations that previously relied on eager `Vec` semantics should
be aware of the following behavioural tweaks:

- `ForLoop` and `QuantLoop` now surface streaming cursors. If you were assuming
  an intermediate vector (e.g. by calling `materialize()` immediately), prefer
  iterating the returned `XdmSequenceStream` or call `.materialize()` explicitly
  at the boundary you control.
- `DynamicContextBuilder::with_cancel_flag` accepts an `Arc<AtomicBool>`. Flip
  the flag from another thread to cooperatively cancel an evaluation. Callers
  should be prepared for `err:FOER0000` to bubble up.
- `doc-order-distinct`, `union`, `intersect`, and `except` no longer allocate
  intermediate vectors unless the consumer does. Semantics are unchanged, but
  any custom instrumentation that inspected the temporary vectors should be
  updated to consume `XdmSequenceStream` instead.
- The legacy `eval_subprogram` helper remains (and still returns a `Vec`), but
  new code should prefer the streaming variant `eval_subprogram_stream` to avoid
  materialisation when composing additional cursors.

No public API has been removed; existing eager callers continue to function,
although they may now observe fewer allocations when they opt into the streaming
path.
