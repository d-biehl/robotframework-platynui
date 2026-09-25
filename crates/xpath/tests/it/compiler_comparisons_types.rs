use platynui_xpath::compiler::{compile, ir::*};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
fn comparisons_all() {
    for (src, op) in [
        ("(1,2) = (2,3)", OpCode::CompareGeneral(ComparisonOp::Eq)),
        ("1 ne 2", OpCode::CompareValue(ComparisonOp::Ne)),
        ("1 lt 2", OpCode::CompareValue(ComparisonOp::Lt)),
        ("1 le 2", OpCode::CompareValue(ComparisonOp::Le)),
        ("2 gt 1", OpCode::CompareValue(ComparisonOp::Gt)),
        ("2 ge 1", OpCode::CompareValue(ComparisonOp::Ge)),
    ] {
        let is = ir(src);
        assert!(is.0.iter().any(|i| std::mem::discriminant(i) == std::mem::discriminant(&op)));
    }
}

#[rstest]
fn comparisons_general_value_node() {
    let gen_ir = ir("(1,2) = (2,3)");
    assert!(gen_ir.0.iter().any(|op| matches!(op, OpCode::CompareGeneral(ComparisonOp::Eq))));
    let val = ir("2 eq 2");
    assert!(val.0.iter().any(|op| matches!(op, OpCode::CompareValue(ComparisonOp::Eq))));
    let node = ir(". is .");
    assert!(node.0.iter().any(|op| matches!(op, OpCode::NodeIs)));
}

#[rstest]
fn node_comparisons() {
    assert!(ir(". is .").0.iter().any(|i| matches!(i, OpCode::NodeIs)));
    assert!(ir(". << .").0.iter().any(|i| matches!(i, OpCode::NodeBefore)));
    assert!(ir(". >> .").0.iter().any(|i| matches!(i, OpCode::NodeAfter)));
}

#[rstest]
fn comparisons_all_ops() {
    // general comparisons
    for (src, op) in [
        ("1 != 2", ComparisonOp::Ne),
        ("1 < 2", ComparisonOp::Lt),
        ("1 <= 2", ComparisonOp::Le),
        ("2 > 1", ComparisonOp::Gt),
        ("2 >= 1", ComparisonOp::Ge),
    ] {
        let is = ir(src);
        assert!(is.0.iter().any(|i| matches!(i, OpCode::CompareGeneral(o) if *o==op)));
    }
    // value comparisons
    for (src, op) in [
        ("1 ne 2", ComparisonOp::Ne),
        ("1 lt 2", ComparisonOp::Lt),
        ("1 le 2", ComparisonOp::Le),
        ("2 gt 1", ComparisonOp::Gt),
        ("2 ge 1", ComparisonOp::Ge),
    ] {
        let is = ir(src);
        assert!(is.0.iter().any(|i| matches!(i, OpCode::CompareValue(o) if *o==op)));
    }
}

#[rstest]
fn types_cast_treat_instance() {
    let inst = ir("1 instance of xs:integer");
    assert!(inst.0.iter().any(|op| matches!(op, OpCode::InstanceOf(SeqTypeIR::Typed{ item: ItemTypeIR::Atomic(ExpandedName{ ns_uri: _, local }), occ: _ }) if local=="integer")));
    let treat = ir("1 treat as xs:integer+");
    assert!(treat.0.iter().any(|op| matches!(op, OpCode::Treat(SeqTypeIR::Typed{ item: ItemTypeIR::Atomic(ExpandedName{ ns_uri: _, local }), occ: OccurrenceIR::OneOrMore }) if local=="integer")));
    let castable = ir("1 castable as xs:integer");
    assert!(castable.0.iter().any(|op| matches!(op, OpCode::Castable(SingleTypeIR{ atomic: ExpandedName{ ns_uri: _, local }, optional: false }) if local=="integer")));
    let cast_as = ir("1 cast as xs:integer");
    assert!(cast_as.0.iter().any(|op| matches!(op, OpCode::Cast(SingleTypeIR{ atomic: ExpandedName{ ns_uri: _, local }, optional: false }) if local=="integer")));
}

#[rstest]
fn types_occurrences() {
    let t_opt = ir("1 treat as xs:integer?");
    assert!(
        t_opt
            .0
            .iter()
            .any(|op| matches!(op, OpCode::Treat(SeqTypeIR::Typed { item: _, occ: OccurrenceIR::ZeroOrOne })))
    );
    let t_plus = ir("1 treat as xs:integer+");
    assert!(
        t_plus
            .0
            .iter()
            .any(|op| matches!(op, OpCode::Treat(SeqTypeIR::Typed { item: _, occ: OccurrenceIR::OneOrMore })))
    );
    let t_star = ir("1 treat as xs:integer*");
    assert!(
        t_star
            .0
            .iter()
            .any(|op| matches!(op, OpCode::Treat(SeqTypeIR::Typed { item: _, occ: OccurrenceIR::ZeroOrMore })))
    );
}
