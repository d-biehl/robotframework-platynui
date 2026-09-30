/// Optimizer pass for predicate pushdown and other IR transformations.
///
/// This module implements optimization passes that transform the IR after initial
/// compilation to improve execution performance while maintaining semantic correctness.
use crate::xdm::XdmAtomicValue;

#[cfg(test)]
use super::ir::{AxisIR, NodeTestIR};
use super::ir::{InstrSeq, OpCode, PredicateIR, PredicateKind};

/// Optimizes a compiled instruction sequence: folds constant arithmetic and moves predicates
/// into the axis step before them.
///
/// # Predicate pushdown
///
/// `(E)[p1][p2]…` applies its predicates to the whole sequence `E` produces. When `E` ends in an
/// axis step, the leading run of non-positional predicates moves into that step:
///
/// ```text
/// AxisStep(descendant, T, [])             AxisStep(descendant, T, [@a])
/// ApplyPredicates([@a, 1])          →     ApplyPredicates([1])
/// ```
///
/// A non-positional predicate filters each item on its own, so it selects the same items inside
/// the step as over the whole sequence, and inside the step it lets the step stream and merge its
/// context nodes. A predicate that may count positions stays where it was written: inside a step
/// it would count per context node, so `(//X)[1]`, the first X overall, would become the first X of
/// every parent. The predicates after it stay as well, because they filter what it selected. An
/// `EnsureDistinct` between the step and the predicates is no obstacle: a filter selects the same
/// items before and after duplicates are removed.
///
/// `//T[p]` needs no pushdown: the compiler already lowers it into one `descendant::T[p]` step
/// when `p` is non-positional, and pushdown then extends that step, as in `(//T)[@a]`.
#[must_use]
pub fn optimize(mut seq: InstrSeq) -> InstrSeq {
    fold_constants(&mut seq.0);
    push_down_predicates(&mut seq.0);
    seq
}

/// Moves the leading non-positional predicates of an `ApplyPredicates` into the `AxisStep`
/// before it, at this level and in every nested sequence.
fn push_down_predicates(instrs: &mut Vec<OpCode>) {
    // First, recursively optimize all nested sequences
    for instr in instrs.iter_mut() {
        match instr {
            OpCode::AxisStep(_, _, preds) | OpCode::ApplyPredicates(preds) => {
                for pred in preds {
                    push_down_predicates(&mut pred.code.0);
                }
            }
            OpCode::PathExprStep(inner) => {
                push_down_predicates(&mut inner.0);
            }
            OpCode::ForLoop { var: _, body } | OpCode::QuantLoop { kind: _, var: _, body } => {
                push_down_predicates(&mut body.0);
            }
            _ => {}
        }
    }

    // Then move predicates at this level: an AxisStep, maybe an EnsureDistinct, ApplyPredicates.
    let mut i = 0;
    while i < instrs.len() {
        let apply = if matches!(instrs.get(i + 1), Some(OpCode::EnsureDistinct)) { i + 2 } else { i + 1 };
        let movable = match (instrs.get(i), instrs.get(apply)) {
            (Some(OpCode::AxisStep(..)), Some(OpCode::ApplyPredicates(preds))) => {
                preds.iter().take_while(|p| p.kind == PredicateKind::NonPositional).count()
            }
            _ => 0,
        };
        if movable == 0 {
            i += 1;
            continue;
        }
        let OpCode::ApplyPredicates(preds) = &mut instrs[apply] else { unreachable!("matched above") };
        let moved: Vec<PredicateIR> = preds.drain(..movable).collect();
        if preds.is_empty() {
            instrs.remove(apply);
        }
        let OpCode::AxisStep(_, _, step_preds) = &mut instrs[i] else { unreachable!("matched above") };
        step_preds.extend(moved);
    }
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
// One folding table over operator and operand-type combinations; it keeps its shape.
#[allow(clippy::too_many_lines)]
fn fold_constants(instrs: &mut Vec<OpCode>) {
    // First, recursively fold constants in nested sequences
    for instr in instrs.iter_mut() {
        match instr {
            OpCode::AxisStep(_, _, preds) | OpCode::ApplyPredicates(preds) => {
                for pred in preds {
                    fold_constants(&mut pred.code.0);
                }
            }
            OpCode::PathExprStep(inner) => {
                fold_constants(&mut inner.0);
            }
            OpCode::ForLoop { var: _, body } | OpCode::QuantLoop { kind: _, var: _, body } => {
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
                    if y.is_zero() {
                        None
                    } else {
                        Some(XdmAtomicValue::Decimal(x / y))
                    }
                }

                // Mixed integer/decimal - promote to decimal
                (OpCode::Add, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(*x) + y))
                }
                (OpCode::Add, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x + rust_decimal::Decimal::from(*y)))
                }
                (OpCode::Sub, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(*x) - y))
                }
                (OpCode::Sub, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x - rust_decimal::Decimal::from(*y)))
                }
                (OpCode::Mul, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    Some(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(*x) * y))
                }
                (OpCode::Mul, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    Some(XdmAtomicValue::Decimal(x * rust_decimal::Decimal::from(*y)))
                }
                (OpCode::Div, XdmAtomicValue::Integer(x), XdmAtomicValue::Decimal(y)) => {
                    if y.is_zero() {
                        None
                    } else {
                        Some(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(*x) / y))
                    }
                }
                (OpCode::Div, XdmAtomicValue::Decimal(x), XdmAtomicValue::Integer(y)) => {
                    if *y == 0 {
                        None
                    } else {
                        Some(XdmAtomicValue::Decimal(x / rust_decimal::Decimal::from(*y)))
                    }
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

    fn predicate(value: XdmAtomicValue, kind: PredicateKind) -> PredicateIR {
        PredicateIR { code: InstrSeq(vec![OpCode::PushAtomic(value)]), kind }
    }

    fn boolean() -> PredicateIR {
        predicate(XdmAtomicValue::Boolean(true), PredicateKind::NonPositional)
    }

    fn number(n: i64) -> PredicateIR {
        predicate(XdmAtomicValue::Integer(n), PredicateKind::PossiblyPositional)
    }

    #[test]
    fn test_simple_predicate_pushdown() {
        let mut instrs = vec![
            OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::AnyKind, vec![]),
            OpCode::ApplyPredicates(vec![boolean()]),
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
    fn test_positional_literals_do_not_merge() {
        let mut instrs = vec![
            OpCode::AxisStep(AxisIR::Child, NodeTestIR::AnyKind, vec![boolean()]),
            OpCode::ApplyPredicates(vec![number(1), number(2)]),
        ];

        push_down_predicates(&mut instrs);

        // `(child::node()[true()])[1][2]` counts over the whole sequence, not per context node.
        assert_eq!(instrs.len(), 2);
        assert!(matches!(&instrs[0], OpCode::AxisStep(_, _, preds) if preds.len() == 1));
        assert!(matches!(&instrs[1], OpCode::ApplyPredicates(preds) if preds.len() == 2));
    }

    #[test]
    fn test_only_the_leading_non_positional_predicates_move() {
        let mut instrs = vec![
            OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::AnyKind, vec![]),
            OpCode::ApplyPredicates(vec![boolean(), number(1), boolean()]),
        ];

        push_down_predicates(&mut instrs);

        assert_eq!(instrs.len(), 2);
        assert!(matches!(&instrs[0], OpCode::AxisStep(_, _, preds) if preds.len() == 1));
        assert!(matches!(&instrs[1], OpCode::ApplyPredicates(preds)
            if preds.iter().map(|p| p.kind).collect::<Vec<_>>()
                == [PredicateKind::PossiblyPositional, PredicateKind::NonPositional]));
    }

    #[test]
    fn test_no_pushdown_without_axis_step() {
        let mut instrs = vec![OpCode::LoadContextItem, OpCode::ApplyPredicates(vec![boolean()])];

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
            vec![PredicateIR {
                code: InstrSeq(vec![
                    // Nested axis step with predicate to push
                    OpCode::AxisStep(AxisIR::Child, NodeTestIR::AnyKind, vec![]),
                    OpCode::ApplyPredicates(vec![boolean()]),
                ]),
                kind: PredicateKind::NonPositional,
            }],
        )];

        push_down_predicates(&mut instrs);

        // Check that nested predicate was pushed down
        if let OpCode::AxisStep(_, _, outer_preds) = &instrs[0] {
            assert_eq!(outer_preds.len(), 1, "Should have one outer predicate");
            // The inner sequence should have the child step with merged predicates
            if let OpCode::AxisStep(_, _, inner_preds) = &outer_preds[0].code.0[0] {
                assert_eq!(inner_preds.len(), 1, "Inner predicate should be merged");
                // After optimization, ApplyPredicates should be removed, so only 1 instruction
                assert_eq!(outer_preds[0].code.0.len(), 1, "ApplyPredicates should be removed");
            } else {
                panic!("Expected nested AxisStep, got {:?}", outer_preds[0].code.0[0]);
            }
        } else {
            panic!("Expected outer AxisStep");
        }
    }
}
