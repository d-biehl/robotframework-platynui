use platynui_xpath::engine::runtime::{
    CallCtx, DynamicContextBuilder, Error, FunctionImplementations, StaticContextBuilder,
};
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::{
    ExpandedName, XdmSequence, compile_with_context, evaluate, evaluate_expr, xdm::XdmAtomicValue as A,
    xdm::XdmItem as I, xdm::XdmSequenceStream,
};
use rstest::rstest;
type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

const AXIS_SECTIONS: usize = 12;
const AXIS_ITEMS_PER_SECTION: usize = 32;

fn build_large_axis_document() -> N {
    let mut root_builder = elem("root");
    for section_idx in 0..AXIS_SECTIONS {
        let section_name = format!("section-{section_idx}");
        let mut section_builder = elem("section").attr(attr("name", &section_name));
        for item_idx in 0..AXIS_ITEMS_PER_SECTION {
            let item_id = format!("item-{section_idx}-{item_idx}");
            let text_content = format!("Section {section_idx} Item {item_idx}");
            let mut item_builder =
                elem("item").attr(attr("id", &item_id)).attr(attr("type", if item_idx % 2 == 0 { "a" } else { "b" }));
            if item_idx % 25 == 0 {
                item_builder = item_builder.attr(attr("featured", "true"));
            }
            item_builder = item_builder.child(text(&text_content));
            section_builder = section_builder.child(item_builder);
        }
        root_builder = root_builder.child(section_builder);
    }
    simple_doc().child(root_builder).build()
}

fn manual_predicate_heavy_metrics() -> (i64, i64) {
    let mut total_len: i64 = 0;
    let mut selected_count: i64 = 0;
    let mut position: usize = 0;
    for section_idx in 0..AXIS_SECTIONS {
        for item_idx in 0..AXIS_ITEMS_PER_SECTION {
            let has_following_b = ((item_idx + 1)..AXIS_ITEMS_PER_SECTION).any(|j| j % 2 == 1);
            if !has_following_b {
                continue;
            }
            position += 1;
            if position.is_multiple_of(7) {
                selected_count += 1;
                let text_content = format!("Section {section_idx} Item {item_idx}");
                total_len += text_content.len() as i64;
            }
        }
    }
    (total_len, selected_count)
}

#[rstest]
fn sequence_makeseq() {
    let out = evaluate_expr::<N>("(1,2,3)", &ctx()).unwrap();
    assert_eq!(out.len(), 3);
}

#[rstest]
fn comparisons_value_general() {
    let out = evaluate_expr::<N>("1 = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
    let out = evaluate_expr::<N>("(1,2) = (2,3)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn variables_and_functions() {
    // Add a custom function in default functions namespace
    let mut reg: FunctionImplementations<N> = FunctionImplementations::new();
    let ns = Some("http://www.w3.org/2005/xpath-functions".to_string());
    reg.register_stream(
        ExpandedName { ns_uri: ns.clone(), local: "twice".to_string() },
        1,
        std::sync::Arc::new(
            |_ctx: &CallCtx<N>, args: &[XdmSequenceStream<N>]| -> Result<XdmSequenceStream<N>, Error> {
                let first_arg: XdmSequence<N> = args[0].iter().collect::<Result<Vec<_>, _>>()?;
                let v = match &first_arg[0] {
                    I::Atomic(A::Integer(i)) => i,
                    _ => &0,
                };
                Ok(XdmSequenceStream::from_vec(vec![I::Atomic(A::Integer(v * 2))]))
            },
        ),
    );
    let dyn_ctx = DynamicContextBuilder::default()
        .with_functions(std::rc::Rc::new(reg))
        .with_variable(ExpandedName { ns_uri: None, local: "x".to_string() }, vec![I::Atomic(A::Integer(5))])
        .build();
    let static_ctx = StaticContextBuilder::new()
        .with_variable(ExpandedName::new(None, "x"))
        .with_function_signature(ExpandedName { ns_uri: ns, local: "twice".to_string() }, 1, Some(1))
        .build();
    let compiled = compile_with_context("twice($x)", &static_ctx).unwrap();
    let out = evaluate::<N>(&compiled, &dyn_ctx).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(10))]);
}

#[rstest]
fn predicates_filter() {
    let out = evaluate_expr::<N>("(1,2,3)[. gt 1]", &ctx());
    // Node axes not implemented, but predicate on sequence should work (atomization + EBV)
    assert!(out.is_ok());
    let out = out.unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(2)), I::Atomic(A::Integer(3))]);
}

#[rstest]
fn predicate_node_sequence_truthy() {
    let document = simple_doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c"))),
        )
        .build();
    let dyn_ctx = DynamicContextBuilder::default().with_context_item(I::Node(document)).build();
    let out = evaluate_expr::<N>("count(/root/item[following-sibling::item])", &dyn_ctx)
        .expect("predicate over node sequence should succeed");
    assert_eq!(out, vec![I::Atomic(A::Integer(2))]);
}

#[rstest]
fn predicate_heavy_sum_matches_manual() {
    let document = build_large_axis_document();
    let dyn_ctx = DynamicContextBuilder::default().with_context_item(I::Node(document)).build();
    let (expected_sum, expected_count) = manual_predicate_heavy_metrics();

    let count_global =
        evaluate_expr::<N>("count(//item[following-sibling::item[@type='b']][position() mod 7 = 0])", &dyn_ctx)
            .expect("count must succeed");
    assert_eq!(count_global, vec![I::Atomic(A::Integer(expected_count))]);

    let sum_global = evaluate_expr::<N>(
        "sum(for $i in //item[following-sibling::item[@type='b']][position() mod 7 = 0] return string-length($i))",
        &dyn_ctx,
    )
    .expect("global sum must succeed");
    assert_eq!(sum_global, vec![I::Atomic(A::Integer(expected_sum))]);

    let per_item_sum = evaluate_expr::<N>(
        "sum(for $i in //item return string-length($i[following-sibling::item[@type='b']][position() mod 7 = 0]))",
        &dyn_ctx,
    )
    .expect("per-item sum must succeed");
    assert_eq!(per_item_sum, vec![I::Atomic(A::Integer(0))]);
}
