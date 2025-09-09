use platynui_xpath::compiler::{compile_xpath, ir::*};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile_xpath(src).expect("compile ok").instrs
}

#[rstest]
fn path_steps_and_predicates() {
    let is = ir(".//a[@id]");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _))));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::AxisStep(_, NodeTestIR::Name(ExpandedName{ ns_uri: _, local: _ }), preds) if !preds.is_empty())));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::DocOrderDistinct)));
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
            assert!(matches!(p.last(), Some(OpCode::ToEBV)));
            found = true;
        }
    }
    assert!(found, "ApplyPredicates not found");
}

#[rstest]
fn filter_multiple_predicates() {
    let is = ir("(1,2,3)[. gt 1][. lt 3]");
    let mut count = 0usize;
    for op in &is.0 { if let OpCode::ApplyPredicates(preds) = op { count = preds.len(); break; } }
    assert_eq!(count, 2);
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
        ("self::document-node(element(*))", NodeTestIR::KindDocument(Some(Box::new(NodeTestIR::KindElement{ name: Some(NameOrWildcard::Any), ty: None, nillable: false })))),
        ("self::element(*)", NodeTestIR::KindElement{ name: Some(NameOrWildcard::Any), ty: None, nillable: false }),
        ("self::attribute(*)", NodeTestIR::KindAttribute{ name: Some(NameOrWildcard::Any), ty: None }),
    ] {
        let is = ir(src);
        assert!(is.0.iter().any(|op| match op { OpCode::AxisStep(_, t, _) => match (t, &expect) {
            (NodeTestIR::AnyKind, NodeTestIR::AnyKind) => true,
            (NodeTestIR::KindText, NodeTestIR::KindText) => true,
            (NodeTestIR::KindComment, NodeTestIR::KindComment) => true,
            (NodeTestIR::KindProcessingInstruction(a), NodeTestIR::KindProcessingInstruction(b)) => a==b,
            (NodeTestIR::KindDocument(a), NodeTestIR::KindDocument(b)) => {
                match (a, b) { (Some(ai), Some(bi)) => **ai==**bi, (None, None) => true, _ => false }
            },
            (NodeTestIR::KindElement{ name: an, ty: at, nillable: anil }, NodeTestIR::KindElement{ name: bn, ty: bt, nillable: bnil }) => an==bn && at==bt && anil==bnil,
            (NodeTestIR::KindAttribute{ name: an, ty: at }, NodeTestIR::KindAttribute{ name: bn, ty: bt }) => an==bn && at==bt,
            _ => false
        }, _ => false }));
    }
}

#[rstest]
fn name_tests_wildcards() {
    let any = ir(".//*");
    assert!(any.0.iter().any(|op| matches!(op, OpCode::AxisStep(_, NodeTestIR::WildcardAny, _))));
    let local_wc = ir(".//*:a");
    let mut found_local = false;
    for op in &local_wc.0 {
        if let OpCode::AxisStep(_, NodeTestIR::LocalWildcard(l), _) = op {
            if l == "a" { found_local = true; break; }
        }
    }
    assert!(found_local);
    let ns_wc = ir(".//ns:*");
    let mut found_ns = false;
    for op in &ns_wc.0 {
        if let OpCode::AxisStep(_, NodeTestIR::NsWildcard(p), _) = op {
            if p == "ns" { found_ns = true; break; }
        }
    }
    assert!(found_ns);
}

#[rstest]
fn path_ir_sequence_complex() {
    let is = ir("/descendant::a[@id]/@class");
    let ops = &is.0;
    assert!(matches!(ops.get(0), Some(OpCode::ToRoot)));
    match ops.get(1) {
        Some(OpCode::AxisStep(AxisIR::Descendant, NodeTestIR::Name(ExpandedName{ns_uri: _, local}), preds)) if local=="a" => {
            assert_eq!(preds.len(), 1);
        }
        other => panic!("unexpected first step: {:?}", other),
    }
    assert!(matches!(ops.get(2), Some(OpCode::DocOrderDistinct)));
    match ops.get(3) {
        Some(OpCode::AxisStep(AxisIR::Attribute, NodeTestIR::Name(ExpandedName{ns_uri: _, local}), preds)) if local=="class" => {
            assert!(preds.is_empty());
        }
        other => panic!("unexpected second step: {:?}", other),
    }
    assert!(matches!(ops.get(4), Some(OpCode::DocOrderDistinct)));
}

#[rstest]
fn path_ir_multiple_steps_with_predicates() {
    let is = ir(".//section[@role='main']/descendant::a[@href][position() lt 3]");
    let mut axis_steps = Vec::new();
    for op in &is.0 {
        if let OpCode::AxisStep(ax, test, preds) = op { axis_steps.push((ax.clone(), test.clone(), preds.clone())); }
    }
    assert_eq!(axis_steps.len(), 3);
    assert!(matches!(axis_steps[0], (AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, _)));
    match &axis_steps[1] { (ax, NodeTestIR::Name(ExpandedName{ns_uri: _, local}), preds) => {
        assert!(matches!(ax, AxisIR::Child | AxisIR::Descendant));
        assert_eq!(local, "section");
        assert_eq!(preds.len(), 1);
    } _ => panic!("unexpected step 2") }
    match &axis_steps[2] { (AxisIR::Descendant, NodeTestIR::Name(ExpandedName{ns_uri: _, local}), preds) => {
        assert_eq!(local, "a");
        assert_eq!(preds.len(), 2);
    } _ => panic!("unexpected step 3") }
}

#[rstest]
fn axis_multiple_predicates() {
    let is = ir(".//a[@id][@class]");
    let mut seen_two = false;
    for op in &is.0 {
        if let OpCode::AxisStep(_, NodeTestIR::Name(ExpandedName{ ns_uri: _, local }), preds) = op {
            if local=="a" { seen_two = preds.len() == 2; break; }
        }
    }
    assert!(seen_two);
}
