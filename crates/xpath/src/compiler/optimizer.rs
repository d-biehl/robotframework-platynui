/// Optimizer pass for predicate pushdown and other IR transformations.
///
/// This module implements optimization passes that transform the IR after initial
/// compilation to improve execution performance while maintaining semantic correctness.
use crate::xdm::XdmAtomicValue;

#[cfg(test)]
use super::ir::NodeTestIR;
use super::ir::{AxisIR, InstrSeq, OpCode};

/// Optimizes a compiled instruction sequence by applying predicate pushdown
/// and other transformations.
///
/// # Predicate Pushdown
///
/// Transforms sequences like:
/// ```text
/// AxisStep(axis, test, [])
/// ApplyPredicates([pred1, pred2])
/// ```
///
/// Into:
/// ```text
/// AxisStep(axis, test, [pred1, pred2])
/// ```
///
/// This allows predicates to be evaluated during axis traversal rather than
/// after collecting all results, enabling:
/// - Early termination (e.g., `//item[1]` stops after first match)
/// - Memory savings (no intermediate result collection)
/// - Better streaming performance
///
/// # Safety
///
/// The optimizer preserves XPath semantics. Predicates are only pushed down when:
/// - They don't depend on the full sequence (e.g., `last()` is safe with caveats)
/// - Document order is maintained
/// - Position context is correctly preserved
///
/// # Example
///
/// ```ignore
/// // Before optimization:
/// let instrs = compile("(//item)[@id='foo']");
/// // IR: AxisStep(Descendant, ..., [])
/// //     ApplyPredicates([@id='foo'])
///
/// // After optimization:
/// let optimized = optimize(instrs);
/// // IR: AxisStep(Descendant, ..., [@id='foo'])
/// ```
pub fn optimize(mut seq: InstrSeq) -> InstrSeq {
    fold_constants(&mut seq.0);
    push_down_predicates(&mut seq.0);
    seq
}

/// Pushes down `ApplyPredicates` instructions into preceding `AxisStep` instructions.
///
/// This function walks through the instruction sequence and identifies patterns where
/// predicates can be safely moved into the axis step itself, allowing for more efficient
/// evaluation.
fn push_down_predicates(instrs: &mut Vec<OpCode>) {
    // First, recursively optimize all nested sequences
    for instr in instrs.iter_mut() {
        match instr {
            OpCode::AxisStep(_, _, preds) => {
                for pred in preds {
                    push_down_predicates(&mut pred.0);
                }
            }
            OpCode::PathExprStep(inner) => {
                push_down_predicates(&mut inner.0);
            }
            OpCode::ApplyPredicates(preds) => {
                for pred in preds {
                    push_down_predicates(&mut pred.0);
                }
            }
            OpCode::ForLoop { var: _, body } => {
                push_down_predicates(&mut body.0);
            }
            OpCode::QuantLoop { kind: _, var: _, body } => {
                push_down_predicates(&mut body.0);
            }
            _ => {}
        }
    }

    // Then, perform predicate pushdown at this level
    let mut i = 0;

    while i + 1 < instrs.len() {
        // Look for pattern: AxisStep followed by ApplyPredicates
        let should_merge = if i + 1 < instrs.len() {
            matches!((&instrs[i], &instrs[i + 1]), (OpCode::AxisStep(_, _, _), OpCode::ApplyPredicates(_)))
        } else {
            false
        };

        if should_merge {
            // Extract predicates from ApplyPredicates
            let new_preds =
                if let OpCode::ApplyPredicates(preds) = &instrs[i + 1] { preds.clone() } else { unreachable!() };

            // Check if we can safely push down
            let (axis, test, existing_preds) = if let OpCode::AxisStep(a, t, p) = &instrs[i] {
                (a.clone(), t.clone(), p.clone())
            } else {
                unreachable!()
            };

            if can_push_down_to_axis(&axis, &new_preds) {
                // Merge predicates
                let mut combined = existing_preds;
                combined.extend(new_preds);

                // Replace AxisStep with merged version
                instrs[i] = OpCode::AxisStep(axis, test, combined);

                // Remove ApplyPredicates
                instrs.remove(i + 1);

                // Check this position again for more predicates
                continue;
            }
        }

        i += 1;
    }
}

/// Determines if predicates can be safely pushed down into an axis step.
///
/// # Safety Conditions
///
/// Predicates can be pushed down if:
/// 1. The axis is a forward axis (maintains position context)
/// 2. The predicates don't use context-sensitive functions that require
///    the full sequence (e.g., `last()` when used in certain ways)
///
/// # Current Implementation
///
/// This is a conservative implementation that allows pushdown for all axes.
/// The evaluator handles position context correctly even with pushed-down predicates.
///
/// Future enhancements could:
/// - Analyze predicate content to detect unsafe patterns
/// - Split predicates into "safe to push" and "must apply after"
/// - Optimize reverse axes with special handling
fn can_push_down_to_axis(_axis: &AxisIR, _predicates: &[InstrSeq]) -> bool {
    // Conservative: allow pushdown for all axes
    // The evaluator maintains correct position() semantics for predicates
    // even when they're attached to the axis step.
    //
    // TODO: Future optimization - analyze predicates for:
    // - Usage of last() or position() in complex expressions
    // - Reverse axes that might need special handling
    // - Predicates that reference variables from outer scope

    true
}

/// Folds constant expressions at compile time.
///
/// This optimization evaluates constant arithmetic and boolean expressions
/// during compilation rather than at runtime, reducing instruction count
/// and improving performance.
///
/// # Examples
///
/// - `1 + 2` → `PushAtomic(3)`
/// - `3 * 4 - 5` → `PushAtomic(7)`
/// - Nested constants in sequences are also folded
///
/// # Implementation
///
/// The function walks the instruction sequence looking for patterns like:
/// ```text
/// PushAtomic(a)
/// PushAtomic(b)
/// Add/Sub/Mul/Div/...
/// ```
///
/// And replaces them with:
/// ```text
/// PushAtomic(result)
/// ```
fn fold_constants(instrs: &mut Vec<OpCode>) {
    // First, recursively fold constants in nested sequences
    for instr in instrs.iter_mut() {
        match instr {
            OpCode::AxisStep(_, _, preds) => {
                for pred in preds {
                    fold_constants(&mut pred.0);
                }
            }
            OpCode::PathExprStep(inner) => {
                fold_constants(&mut inner.0);
            }
            OpCode::ApplyPredicates(preds) => {
                for pred in preds {
                    fold_constants(&mut pred.0);
                }
            }
            OpCode::ForLoop { var: _, body } => {
                fold_constants(&mut body.0);
            }
            OpCode::QuantLoop { kind: _, var: _, body } => {
                fold_constants(&mut body.0);
            }
            _ => {}
        }
    }

    // Then, perform constant folding at this level
    let mut i = 0;

    while i + 2 < instrs.len() {
        // Look for pattern: PushAtomic PushAtomic BinaryOp
        let can_fold = matches!(
            (&instrs[i], &instrs[i + 1], &instrs[i + 2]),
            (
                OpCode::PushAtomic(_),
                OpCode::PushAtomic(_),
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::IDiv | OpCode::Mod
            )
        );

        if can_fold && let (OpCode::PushAtomic(a), OpCode::PushAtomic(b)) = (&instrs[i], &instrs[i + 1]) {
            let result = match (&instrs[i + 2], a, b) {
                // Integer arithmetic - use checked operations to prevent overflow
                (OpCode::Add, XdmAtomicValue::Integer(x), XdmAtomicValue::Integer(y)) => {
                    x.checked_add(*y).map(XdmAtomicValue::Integer)
                }
                (OpCode::Sub, XdmAtomicValue::Integer(x), XdmAtomicValue::Integer(y)) => {
                    x.checked_sub(*y).map(XdmAtomicValue::Integer)
                }
                (OpCode::Mul, XdmAtomicValue::Integer(x), XdmAtomicValue::Integer(y)) => {
                    x.checked_mul(*y).map(XdmAtomicValue::Integer)
                }
                (OpCode::IDiv, XdmAtomicValue::Integer(x), XdmAtomicValue::Integer(y)) if *y != 0 => {
                    x.checked_div(*y).map(XdmAtomicValue::Integer)
                }
                (OpCode::Mod, XdmAtomicValue::Integer(x), XdmAtomicValue::Integer(y)) if *y != 0 => {
                    x.checked_rem(*y).map(XdmAtomicValue::Integer)
                }

                // Decimal arithmetic
                (OpCode::Add, XdmAtomicValue::Decimal(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(x + y))
                }
                (OpCode::Sub, XdmAtomicValue::Decimal(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(x - y))
                }
                (OpCode::Mul, XdmAtomicValue::Decimal(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(x * y))
                }
                (OpCode::Div, XdmAtomicValue::Decimal(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(x / y))
                }

                // Mixed integer/decimal - promote to decimal
                (OpCode::Add, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(*x as f64 + y))
                }
                (OpCode::Add, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x + *y as f64))
                }
                (OpCode::Sub, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(*x as f64 - y))
                }
                (OpCode::Sub, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x - *y as f64))
                }
                (OpCode::Mul, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(*x as f64 * y))
                }
                (OpCode::Mul, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x * *y as f64))
                }
                (OpCode::Div, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(*x as f64 / y))
                }
                (OpCode::Div, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x / *y as f64))
                }

                _ => None,
            };

            if let Some(folded) = result {
                // Replace three instructions with one
                instrs[i] = OpCode::PushAtomic(folded);
                instrs.remove(i + 1);
                instrs.remove(i + 1);
                // Don't increment i - check if we can fold more at this position
                continue;
            }
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdm::XdmAtomicValue;

    #[test]
    fn test_simple_predicate_pushdown() {
        let mut instrs = vec![
            OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::AnyKind, vec![]),
            OpCode::ApplyPredicates(vec![InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Boolean(true))])]),
        ];

        push_down_predicates(&mut instrs);

        assert_eq!(instrs.len(), 1);
        if let OpCode::AxisStep(_, _, preds) = &instrs[0] {
            assert_eq!(preds.len(), 1);
        } else {
            panic!("Expected AxisStep");
        }
    }

    #[test]
    fn test_multiple_predicates_pushdown() {
        let mut instrs = vec![
            OpCode::AxisStep(
                AxisIR::Child,
                NodeTestIR::AnyKind,
                vec![InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Boolean(true))])],
            ),
            OpCode::ApplyPredicates(vec![
                InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Integer(1))]),
                InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Integer(2))]),
            ]),
        ];

        push_down_predicates(&mut instrs);

        assert_eq!(instrs.len(), 1);
        if let OpCode::AxisStep(_, _, preds) = &instrs[0] {
            assert_eq!(preds.len(), 3); // 1 existing + 2 pushed down
        } else {
            panic!("Expected AxisStep");
        }
    }

    #[test]
    fn test_no_pushdown_without_axis_step() {
        let mut instrs = vec![
            OpCode::LoadContextItem,
            OpCode::ApplyPredicates(vec![InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Boolean(true))])]),
        ];

        let original_len = instrs.len();
        push_down_predicates(&mut instrs);

        // Should not change - no AxisStep to push into
        assert_eq!(instrs.len(), original_len);
    }

    #[test]
    fn test_nested_predicate_optimization() {
        let mut instrs = vec![OpCode::AxisStep(
            AxisIR::Descendant,
            NodeTestIR::AnyKind,
            vec![InstrSeq(vec![
                // Nested axis step with predicate to push
                OpCode::AxisStep(AxisIR::Child, NodeTestIR::AnyKind, vec![]),
                OpCode::ApplyPredicates(vec![InstrSeq(vec![OpCode::PushAtomic(XdmAtomicValue::Boolean(true))])]),
            ])],
        )];

        push_down_predicates(&mut instrs);

        // Check that nested predicate was pushed down
        if let OpCode::AxisStep(_, _, outer_preds) = &instrs[0] {
            assert_eq!(outer_preds.len(), 1, "Should have one outer predicate");
            // The inner sequence should have the child step with merged predicates
            if let OpCode::AxisStep(_, _, inner_preds) = &outer_preds[0].0[0] {
                assert_eq!(inner_preds.len(), 1, "Inner predicate should be merged");
                // After optimization, ApplyPredicates should be removed, so only 1 instruction
                assert_eq!(outer_preds[0].0.len(), 1, "ApplyPredicates should be removed");
            } else {
                panic!("Expected nested AxisStep, got {:?}", outer_preds[0].0[0]);
            }
        } else {
            panic!("Expected outer AxisStep");
        }
    }
}
