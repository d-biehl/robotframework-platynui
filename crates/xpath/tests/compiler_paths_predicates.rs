use platynui_xpath::compiler::{compile, ir::*};
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
fn path_steps_and_predicates() {
    let is = ir(".//a[@id]");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _))));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(_, NodeTestIR::Name(_), preds) if !preds.is_empty())));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::EnsureDistinct | OpCode::EnsureOrder)));
}

#[rstest]
fn context_item() {
    let is = ir(".");
    assert!(matches!(is.0.last(), Some(OpCode::LoadContextItem)));
}

#[rstest]
fn filter_apply_predicates() {
    let is = ir("(1,2,3)[. gt 1]");
    let mut found = false;
    for op in &is.0 {
        if let OpCode::ApplyPredicates(preds) = op {
            assert_eq!(preds.len(), 1);
            let p = &preds[0].0;
            // Predicate body should no longer be auto-wrapped with ToEBV; last op is comparison
            assert!(matches!(p.last(), Some(OpCode::CompareGeneral(_) | OpCode::CompareValue(_))));
            found = true;
        }
    }
    assert!(found, "ApplyPredicates not found");
}

#[rstest]
fn filter_multiple_predicates() {
    let is = ir("(1,2,3)[. gt 1][. lt 3]");
    let mut count = 0usize;
    for op in &is.0 {
        if let OpCode::ApplyPredicates(preds) = op {
            count = preds.len();
            break;
        }
    }
    assert_eq!(count, 2);
}

#[rstest]
fn filter_step_in_path_emits_map_opcode() {
    let is = ir("child::a/xs:string(.)");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::PathExprStep(_))));
}

#[rstest]
fn path_from() {
    let is = ir("(.)/self::node()");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(AxisIR::SelfAxis, NodeTestIR::AnyKind, _))));
}

#[rstest]
fn root_descendant() {
    let is = ir("//a");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::ToRoot)));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _))));
}

#[rstest]
fn axes_all() {
    let src = "/child::node()/descendant::node()/attribute::*/self::node()/descendant-or-self::node()/following-sibling::node()/following::node()/namespace::node()/parent::node()/ancestor::node()/preceding-sibling::node()/preceding::node()/ancestor-or-self::node()";
    let is = ir(src);
    let has = |ax| is.0.iter().any(|op| matches!(op, OpCode::AxisStep(a, _, _) if *a==ax));
    assert!(has(AxisIR::Child));
    assert!(has(AxisIR::Descendant));
    assert!(has(AxisIR::Attribute));
    assert!(has(AxisIR::SelfAxis));
    assert!(has(AxisIR::DescendantOrSelf));
    assert!(has(AxisIR::FollowingSibling));
    assert!(has(AxisIR::Following));
    assert!(has(AxisIR::Namespace));
    assert!(has(AxisIR::Parent));
    assert!(has(AxisIR::Ancestor));
    assert!(has(AxisIR::PrecedingSibling));
    assert!(has(AxisIR::Preceding));
    assert!(has(AxisIR::AncestorOrSelf));
}

#[rstest]
fn kind_tests() {
    for (src, expect) in [
        ("self::node()", NodeTestIR::AnyKind),
        ("self::text()", NodeTestIR::KindText),
        ("self::comment()", NodeTestIR::KindComment),
        ("self::processing-instruction()", NodeTestIR::KindProcessingInstruction(None)),
        ("self::processing-instruction('t')", NodeTestIR::KindProcessingInstruction(Some("t".into()))),
        (
            "self::document-node(element(*))",
            NodeTestIR::KindDocument(Some(Box::new(NodeTestIR::KindElement {
                name: Some(NameOrWildcard::Any),
                ty: None,
                nillable: false,
            }))),
        ),
        ("self::element(*)", NodeTestIR::KindElement { name: Some(NameOrWildcard::Any), ty: None, nillable: false }),
        ("self::attribute(*)", NodeTestIR::KindAttribute { name: Some(NameOrWildcard::Any), ty: None }),
    ] {
        let is = ir(src);
        assert!(is.0.iter().any(|op| match op {
            OpCode::AxisStep(_, t, _) => match (t, &expect) {
                (NodeTestIR::AnyKind, NodeTestIR::AnyKind) => true,
                (NodeTestIR::KindText, NodeTestIR::KindText) => true,
                (NodeTestIR::KindComment, NodeTestIR::KindComment) => true,
                (NodeTestIR::KindProcessingInstruction(a), NodeTestIR::KindProcessingInstruction(b)) => a == b,
                (NodeTestIR::KindDocument(a), NodeTestIR::KindDocument(b)) => {
                    match (a, b) {
                        (Some(ai), Some(bi)) => **ai == **bi,
                        (None, None) => true,
                        _ => false,
                    }
                }
                (
                    NodeTestIR::KindElement { name: an, ty: at, nillable: anil },
                    NodeTestIR::KindElement { name: bn, ty: bt, nillable: bnil },
                ) => an == bn && at == bt && anil == bnil,
                (NodeTestIR::KindAttribute { name: an, ty: at }, NodeTestIR::KindAttribute { name: bn, ty: bt }) =>
                    an == bn && at == bt,
                _ => false,
            },
            _ => false,
        }));
    }
}

#[rstest]
fn name_tests_wildcards() {
    let any = ir(".//*");
    assert!(any.0.iter().any(|op| matches!(op, OpCode::AxisStep(_, NodeTestIR::WildcardAny, _))));
    let local_wc = ir(".//*:a");
    let mut found_local = false;
    for op in &local_wc.0 {
        if let OpCode::AxisStep(_, NodeTestIR::LocalWildcard(l), _) = op
            && l.as_ref() == "a"
        {
            found_local = true;
            break;
        }
    }
    assert!(found_local);
    let ns_wc = ir(".//ns:*");
    let mut found_ns = false;
    for op in &ns_wc.0 {
        if let OpCode::AxisStep(_, NodeTestIR::NsWildcard(p), _) = op
            && p.as_ref() == "ns"
        {
            found_ns = true;
            break;
        }
    }
    assert!(found_ns);
}

#[rstest]
fn path_ir_sequence_complex() {
    let is = ir("/descendant::a[@id]/@class");
    let ops = &is.0;
    let mut idx = 0;
    assert!(matches!(ops.get(idx), Some(OpCode::LoadContextItem)));
    idx += 1;
    if matches!(ops.get(idx), Some(OpCode::Treat(_))) {
        idx += 1;
    }
    assert!(matches!(ops.get(idx), Some(OpCode::Pop)));
    idx += 1;
    assert!(matches!(ops.get(idx), Some(OpCode::ToRoot)));
    idx += 1;
    match ops.get(idx) {
        Some(OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::Name(name), preds)) if name.original.local == "a" => {
            assert_eq!(preds.len(), 1);
        }
        other => panic!("unexpected first step: {:?}", other),
    }
    // Descendant step requires at least distinctness
    assert!(matches!(ops.get(idx + 1), Some(OpCode::EnsureDistinct)));
    match ops.get(idx + 2) {
        Some(OpCode::AxisStep(AxisIR::Attribute, NodeTestIR::Name(name), preds)) if name.original.local == "class" => {
            assert!(preds.is_empty());
        }
        other => panic!("unexpected second step: {:?}", other),
    }
    // Attribute axis does not need normalization (each attribute belongs to exactly one element)
    assert!(!matches!(ops.get(idx + 3), Some(OpCode::EnsureOrder | OpCode::EnsureDistinct)));
}

#[rstest]
fn path_ir_multiple_steps_with_predicates() {
    let is = ir(".//section[@role='main']/descendant::a[@href][position() lt 3]");
    let mut axis_steps = Vec::new();
    for op in &is.0 {
        if let OpCode::AxisStep(ax, test, preds) = op {
            axis_steps.push((ax.clone(), test.clone(), preds.clone()));
        }
    }
    assert_eq!(axis_steps.len(), 3);
    assert!(matches!(axis_steps[0], (AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _)));
    match &axis_steps[1] {
        (ax, NodeTestIR::Name(name), preds) => {
            assert!(matches!(ax, AxisIR::Child | AxisIR::Descendant));
            assert_eq!(name.original.local, "section");
            assert_eq!(preds.len(), 1);
        }
        _ => panic!("unexpected step 2"),
    }
    match &axis_steps[2] {
        (AxisIR::Descendant, NodeTestIR::Name(name), preds) => {
            assert_eq!(name.original.local, "a");
            assert_eq!(preds.len(), 2);
        }
        _ => panic!("unexpected step 3"),
    }
}

#[rstest]
fn union_compiles_distinct_operands() {
    let is = ir("//control:Window | //control:Button");
    use OpCode::*;
    // Count name tests for Window and Button separately
    let mut has_window = false;
    let mut has_button = false;
    for op in &is.0 {
        if let AxisStep(_, NodeTestIR::Name(q), _) = op {
            if q.original.local == "Window" {
                has_window = true;
            }
            if q.original.local == "Button" {
                has_button = true;
            }
        }
    }
    assert!(has_window, "union must contain Window operand");
    assert!(has_button, "union must contain Button operand");
}

#[rstest]
fn axis_multiple_predicates() {
    let is = ir(".//a[@id][@class]");
    let mut seen_two = false;
    for op in &is.0 {
        if let OpCode::AxisStep(_, NodeTestIR::Name(name), preds) = op
            && name.original.local == "a"
        {
            seen_two = preds.len() == 2;
            break;
        }
    }
    assert!(seen_two);
}
#[rstest]
fn filter_step_preserves_doc_order_for_descendant_insertion() {
    let is = ir(".//section/xs:string(value)");
    let mut saw_descendant = false;
    let mut saw_filter = false;
    for op in &is.0 {
        match op {
            OpCode::AxisStep(AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _) => saw_descendant = true,
            OpCode::PathExprStep(_) => saw_filter = true,
            _ => {}
        }
    }
    assert!(saw_descendant && saw_filter);
}
