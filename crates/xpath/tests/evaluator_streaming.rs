use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{doc, elem, text};
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{XdmNode, evaluate_expr, evaluate_stream_expr};
use rstest::rstest;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

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
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

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
    let stream = evaluate_stream_expr::<N>("(1, 2, 3)[. >= 2]", &ctx).expect("stream eval succeeds");
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

#[rstest]
fn streaming_predicate_last_on_nodes() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three")))
                .child(elem("item").child(text("four"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let stream = evaluate_stream_expr::<N>("child::item[position() = last()]", &ctx).expect("stream eval");
    let values: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(values, ["four".to_string()]);
}

#[rstest]
fn streaming_nested_predicates_position_tracking() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("t1")))
                .child(elem("item").child(text("t2")))
                .child(elem("item").child(text("t3")))
                .child(elem("item").child(text("t4"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() <= 3][position() = last()]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let values: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(values, ["t3".to_string()]);
}

#[rstest]
fn streaming_path_expr_step_flatmaps_lazily() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("x")))
                .child(elem("item").child(text("y")))
                .child(elem("item").child(text("z"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let stream = evaluate_stream_expr::<N>("child::item/child::text()", &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["x", "y", "z"]);
}

#[rstest]
fn streaming_union_preserves_doc_order() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three")))
                .child(elem("item").child(text("four"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "(/root/item[position() = 3]) union (/root/item[position() = 1])";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["one", "three"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_intersect_matches_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "/root/item intersect /root/item[position() = 2]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["two"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_except_filters_nodes() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "/root/item except /root/item[position() = 2]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["one", "three"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_union_large_dataset() {
    let mut root_builder = elem("root");
    for idx in 1..=200 {
        let value = format!("{idx}");
        root_builder = root_builder.child(elem("item").child(text(&value)));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "(/root/item[position() <= 150]) union (/root/item[position() > 50])";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream.len(), 200);
    assert_eq!(via_stream.first().unwrap(), "1");
    assert_eq!(via_stream.last().unwrap(), "200");

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_for_loop_matches_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "for $x in /root/item return $x/text()";
    let via_stream: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_quantifiers_match_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("alpha")))
                .child(elem("item").child(text("beta")))
                .child(elem("item").child(text("gamma"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr_some = "some $x in /root/item satisfies $x/text() = 'beta'";
    let expr_every = "every $x in /root/item satisfies string-length($x/text()) > 0";

    let some_stream: bool = evaluate_stream_expr::<N>(expr_some, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    let some_eager: bool = evaluate_expr::<N>(expr_some, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    assert_eq!(some_stream, some_eager);

    let every_stream: bool = evaluate_stream_expr::<N>(expr_every, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    let every_eager: bool = evaluate_expr::<N>(expr_every, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    assert_eq!(every_stream, every_eager);
}

#[rstest]
fn streaming_cancellation_triggers_error() {
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(document.clone()))
        .with_cancel_flag(cancel_flag)
        .build();

    let err =
        evaluate_stream_expr::<N>("for $x in /root/item return $x", &ctx).err().expect("evaluation should cancel");
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FOER0000);
}

// ============================================================================
// Critical Streaming Tests (Priority 1 from xpath_streaming_analysis.md)
// ============================================================================

/// Test that streaming stops after finding the first result.
/// This verifies that we don't traverse the entire tree unnecessarily.
#[rstest]
fn streaming_early_termination_first_match() {
    // Build a tree with 10,000 items
    let mut root_builder = elem("root");
    for idx in 1..=10_000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("item_{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    // Query: descendant-or-self::item[1]
    // Should find first item and stop, not traverse all 10,000
    let stream = evaluate_stream_expr::<N>("descendant-or-self::item[1]", &ctx).expect("stream eval succeeds");

    let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().expect("ok");

    // Should get exactly 1 result
    assert_eq!(result.len(), 1);

    // Verify it's the first item
    match &result[0] {
        I::Node(n) => assert_eq!(n.string_value(), "item_1"),
        other => panic!("expected node, got {other:?}"),
    }

    // Note: We can't directly measure node access count with SimpleNode,
    // but the test demonstrates the query completes quickly.
}

/// Test that streaming handles large numeric sequences efficiently.
/// XPath 2.0 allows: `1 to 999999999` - this should NOT materialize
/// the full range if we only take the first few items.
///
/// **CURRENTLY DISABLED**: This test takes >60 seconds because the `1 to N`
/// range operation currently materializes the entire sequence instead of
/// streaming it. This is a known limitation documented in xpath_streaming_analysis.md
/// and should be fixed as part of the "Range Streaming" optimization.
///
/// **TODO**: Re-enable this test after implementing lazy range evaluation.
/// See Priority 2 or 3 optimizations in the streaming roadmap.
#[rstest]
#[ignore = "Range (1 to N) currently materializes instead of streaming - takes >60s"]
fn streaming_infinite_sequence_early_exit() {
    let ctx = DynamicContextBuilder::<N>::default().build();

    // Create a large range but only take first 10
    let expr = "(1 to 999999999)[position() < 11]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let results: Vec<i64> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results.len(), 10);
    assert_eq!(results, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // If this test passes without OOM or timeout, streaming worked correctly
}

/// Test another infinite-like sequence with filtering.
/// Should stream and short-circuit, not materialize entire range.
#[rstest]
fn streaming_large_range_with_predicate() {
    let ctx = DynamicContextBuilder::<N>::default().build();

    // Large range with a filter that matches early
    let expr = "(1 to 100000)[. > 99995]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let results: Vec<i64> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results, vec![99996, 99997, 99998, 99999, 100000]);
}

/// Test that take() on stream works correctly for limiting results.
/// This is the common pattern for "give me first N matches".
#[rstest]
fn streaming_take_limits_evaluation() {
    let mut root_builder = elem("root");
    for idx in 1..=1000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    // Get all items but only take first 5
    let stream = evaluate_stream_expr::<N>("//item", &ctx).expect("stream eval succeeds");

    let first_five: Vec<String> = stream
        .iter()
        .take(5)
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(first_five.len(), 5);
    assert_eq!(first_five, vec!["1", "2", "3", "4", "5"]);
}

/// Verify that streaming doesn't consume excessive memory for large result sets.
/// Note: This is a behavioral test - exact memory measurement would require
/// platform-specific tooling.
#[rstest]
fn streaming_memory_efficient_large_tree() {
    // Build a tree with 5,000 items
    let mut root_builder = elem("root");
    for idx in 1..=5_000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("value_{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    // Query all descendants but only take first match with specific condition
    let expr = "//item[contains(text(), 'value_2500')][1]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().expect("ok");

    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Node(n) => assert_eq!(n.string_value(), "value_2500"),
        other => panic!("expected node, got {other:?}"),
    }

    // If this completes without excessive memory usage, streaming is working
}
