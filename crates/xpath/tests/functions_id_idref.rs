use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode};
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

fn build_sample_doc() -> N {
    use platynui_xpath::model::simple::{attr, doc, elem};
    // <root>
    //   <item xml:id="A"/>
    //   <item xml:id="B"/>
    //   <ref1 idref="A"/>
    //   <ref2 ref="A B"/>
    //   <group><child id="C"/></group>
    // </root>
    doc()
        .child(
            elem("root")
                .child(elem("item").attr(attr("xml:id", "A")))
                .child(elem("item").attr(attr("xml:id", "B")))
                .child(elem("ref1").attr(attr("idref", "A")))
                .child(elem("ref2").attr(attr("ref", "A B")))
                .child(elem("group").child(elem("child").attr(attr("id", "C")))),
        )
        .build()
}

#[fixture]
fn doc() -> N {
    return build_sample_doc();
}

#[rstest]
fn fn_id_returns_element_by_xml_id(doc: N) {
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(doc.clone()).build();
    let a = evaluate_expr::<N>("id('A', /)", &ctx).unwrap();
    assert_eq!(a.len(), 1);
    match &a[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "item"),
        _ => panic!("expected node"),
    }
}

#[rstest]
fn fn_id_returns_element_by_plain_id(doc: N) {
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(doc.clone()).build();
    let c = evaluate_expr::<N>("id('C', /)", &ctx).unwrap();
    assert_eq!(c.len(), 1);
    match &c[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "child"),
        _ => panic!("expected node"),
    }
}

#[rstest]
fn fn_element_with_id_behaves_like_id_for_attributes(doc: N) {
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(doc.clone()).build();
    let b = evaluate_expr::<N>("element-with-id('B', /)", &ctx).unwrap();
    assert_eq!(b.len(), 1);
    match &b[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "item"),
        _ => panic!("expected node"),
    }
}

#[rstest]
fn fn_idref_returns_attributes_referencing_ids(doc: N) {
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(doc.clone()).build();
    let r = evaluate_expr::<N>("idref('A', /)", &ctx).unwrap();
    // Expect two attributes: @idref on <ref1> and @ref on <ref2>
    assert_eq!(r.len(), 2);
    let kinds: Vec<_> = r
        .iter()
        .map(|it| match it {
            XdmItem::Node(n) => n.kind(),
            _ => panic!("expected node"),
        })
        .collect();
    for k in kinds {
        assert!(matches!(k, platynui_xpath::model::NodeKind::Attribute));
    }
}

#[rstest]
fn fn_id_ignores_non_ncname_tokens(doc: N) {
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(doc.clone()).build();
    let out = evaluate_expr::<N>("id('1bad', /)", &ctx).unwrap();
    assert!(out.is_empty());
}

#[rstest]
fn fn_id_requires_context_when_second_arg_missing() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("id('A')", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPDY0002);
}

#[rstest]
fn fn_element_with_id_requires_context_when_second_arg_missing() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("element-with-id('A')", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPDY0002);
}

#[rstest]
fn fn_idref_requires_context_when_second_arg_missing() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("idref('A')", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPDY0002);
}
