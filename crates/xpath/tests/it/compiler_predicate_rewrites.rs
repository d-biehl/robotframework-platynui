//! What the compiler does with predicates: which predicates move into a step, and which paths
//! become one descendant step. Positional predicates count per context inside a step and over the
//! whole sequence on a parenthesized expression, so only non-positional ones may move or merge.

use PredicateKind::{NonPositional, PossiblyPositional};
use platynui_xpath::ExpandedName;
use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::compiler::ir::{AxisIR, NodeTestIR, OpCode, PredicateIR, PredicateKind};
use platynui_xpath::engine::runtime::StaticContextBuilder;
use rstest::rstest;

fn plan(xpath: &str) -> Vec<OpCode> {
    let n = ExpandedName { ns_uri: None, local: "n".to_string() };
    let static_ctx = StaticContextBuilder::new().with_variable(n).build();
    compile_with_context(xpath, &static_ctx).unwrap_or_else(|e| panic!("compile `{xpath}`: {e:?}")).instrs.0
}

fn kinds(predicates: &[PredicateIR]) -> Vec<PredicateKind> {
    predicates.iter().map(|p| p.kind).collect()
}

/// The axis steps at the top of a plan, with the kinds of their predicates.
fn steps(plan: &[OpCode]) -> Vec<(AxisIR, Vec<PredicateKind>)> {
    plan.iter()
        .filter_map(|op| match op {
            OpCode::AxisStep(axis, _, predicates) => Some((axis.clone(), kinds(predicates))),
            _ => None,
        })
        .collect()
}

/// The predicates the top of a plan applies to a whole sequence.
fn applied(plan: &[OpCode]) -> Vec<PredicateKind> {
    plan.iter()
        .filter_map(|op| match op {
            OpCode::ApplyPredicates(predicates) => Some(kinds(predicates)),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn only_the_leading_non_positional_predicates_move_into_the_step() {
    let moved = plan("(//T)[@a][1]");
    assert_eq!(steps(&moved), [(AxisIR::Descendant, vec![NonPositional])]);
    assert_eq!(applied(&moved), [PossiblyPositional]);

    let kept = plan("(//T)[1][@a]");
    assert_eq!(steps(&kept), [(AxisIR::Descendant, vec![])]);
    assert_eq!(applied(&kept), [PossiblyPositional, NonPositional]);
}

#[rstest]
#[case::number("(//T)[1]")]
#[case::variable("(//T)[$n]")]
#[case::function_that_is_not_boolean("(//T)[count(x)]")]
#[case::position_in_a_comparison("(//T)[position() < 3]")]
fn a_possibly_positional_predicate_never_moves_into_a_step(#[case] xpath: &str) {
    let plan = plan(xpath);
    assert_eq!(steps(&plan), [(AxisIR::Descendant, vec![])], "`{xpath}`");
    assert_eq!(applied(&plan), [PossiblyPositional], "`{xpath}`");
}

#[rstest]
#[case::attribute("(//T)[@a]")]
#[case::comparison("(//T)[@a='x']")]
#[case::boolean_function("(//T)[contains(@a,'x')]")]
fn a_non_positional_predicate_moves_into_the_step(#[case] xpath: &str) {
    let plan = plan(xpath);
    assert_eq!(steps(&plan), [(AxisIR::Descendant, vec![NonPositional])], "`{xpath}`");
    assert!(applied(&plan).is_empty(), "`{xpath}`");
}

#[rstest]
#[case::from_the_root("//T[@a]")]
#[case::from_the_context(".//T[@a]")]
fn a_descendant_search_with_non_positional_predicates_is_one_descendant_step(#[case] xpath: &str) {
    let plan = plan(xpath);
    assert_eq!(steps(&plan), [(AxisIR::Descendant, vec![NonPositional])], "`{xpath}`");
    assert!(
        plan.iter()
            .any(|op| matches!(op, OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::Name(q), _) if &*q.local == "T")),
        "`{xpath}` tests for T in the descendant step: {plan:?}"
    );
}

#[test]
fn a_child_step_after_a_descendant_step_with_predicates_stays() {
    let plan = plan("descendant::A[@q]/T[@p]");
    assert_eq!(steps(&plan), [(AxisIR::Descendant, vec![NonPositional]), (AxisIR::Child, vec![NonPositional])]);
}

fn is_self_step(op: &OpCode, name: &str) -> bool {
    matches!(op, OpCode::AxisStep(AxisIR::SelfAxis, NodeTestIR::Name(q), predicates) if &*q.local == name && predicates.is_empty())
}

#[rstest]
#[case::without_predicates("//(A|B)", vec![NonPositional])]
#[case::with_a_predicate(".//(A|B)[@a]", vec![NonPositional, NonPositional])]
#[case::with_a_predicate_moved_in("(//(A|B))[@a]", vec![NonPositional, NonPositional])]
fn a_union_step_after_a_descendant_search_is_one_descendant_step(
    #[case] xpath: &str,
    #[case] expected: Vec<PredicateKind>,
) {
    let plan = plan(xpath);
    assert!(!plan.iter().any(|op| matches!(op, OpCode::PathExprStep(_))), "`{xpath}`: {plan:?}");
    assert!(applied(&plan).is_empty(), "`{xpath}`");
    let axis_steps: Vec<&OpCode> = plan.iter().filter(|op| matches!(op, OpCode::AxisStep(..))).collect();
    let [OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::WildcardAny, predicates)] = axis_steps.as_slice() else {
        panic!("`{xpath}` must be one `descendant::*` step: {plan:?}");
    };
    assert_eq!(kinds(predicates), expected, "`{xpath}`");
    let names = &predicates[0].code.0;
    assert!(names.iter().any(|op| is_self_step(op, "A")), "`{xpath}`: the first predicate tests self::A: {names:?}");
    assert!(names.iter().any(|op| is_self_step(op, "B")), "`{xpath}`: the first predicate tests self::B: {names:?}");
}

#[rstest]
#[case::positional_predicate(".//(A|B)[1]")]
#[case::operand_that_is_not_one_child_step(".//(A|B/C)[@a]")]
fn other_union_steps_keep_their_filter_expression_step(#[case] xpath: &str) {
    let plan = plan(xpath);
    assert!(plan.iter().any(|op| matches!(op, OpCode::PathExprStep(_))), "`{xpath}`: {plan:?}");
    assert!(
        steps(&plan).iter().any(|(axis, _)| *axis == AxisIR::DescendantOrSelf),
        "`{xpath}` keeps its descendant-or-self step: {plan:?}"
    );
}
