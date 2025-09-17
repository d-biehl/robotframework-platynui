use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{doc, elem, text};
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{XdmNode, evaluate_expr, evaluate_stream_expr};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

#[rstest]
fn streaming_child_axis_iterates_in_order() {
    // <root><item>a</item><item>b</item><item>c</item></root>
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(root.clone()))
        .build();

    let stream = evaluate_stream_expr::<N>("child::item", &ctx).expect("stream eval succeeds");
    let mut iter = stream.iter();

    let first = iter.next().expect("first node exists").expect("ok");
    let second = iter.next().expect("second node exists").expect("ok");
    let third = iter.next().expect("third node exists").expect("ok");
    assert!(iter.next().is_none());

    let names = [first, second, third]
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["item", "item", "item"]);
}

#[rstest]
fn streaming_iter_is_repeatable_and_matches_eager() {
    // <root><item>a</item><item>b</item><item>c</item></root>
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("alpha")))
                .child(elem("item").child(text("beta")))
                .child(elem("item").child(text("gamma"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(document.clone()))
        .build();

    let stream = evaluate_stream_expr::<N>("//item", &ctx).expect("stream eval succeeds");
    // First pass
    let first_pass: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    // Second pass should yield identical results
    let second_pass: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(first_pass, second_pass);

    // Compare against eager evaluation for safety.
    let eager = evaluate_expr::<N>("//item", &ctx).expect("eager eval succeeds");
    let eager_values: Vec<_> = eager
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(first_pass, eager_values);
}

#[rstest]
fn streaming_atomic_sequence_behaves_like_eager() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let stream =
        evaluate_stream_expr::<N>("(1, 2, 3)[. >= 2]", &ctx).expect("stream eval succeeds");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager = evaluate_expr::<N>("(1, 2, 3)[. >= 2]", &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(via_stream, via_eager);
}
